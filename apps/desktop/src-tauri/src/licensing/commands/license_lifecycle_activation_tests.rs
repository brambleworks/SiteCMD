use std::collections::VecDeque;
use std::sync::Mutex;

use super::*;

struct FakeActivationPorts {
    state: Mutex<Option<LicenseState>>,
    key: Mutex<Option<String>>,
    confirmed_replacement: Mutex<Option<(LicenseState, String)>>,
    confirmation: Mutex<Option<Result<(), ActivationConfirmationError>>>,
    persistence_race: Mutex<Option<(LicenseState, String)>>,
    minted: Mutex<VecDeque<Result<api::LicenseResult, String>>>,
    released_credentials: Mutex<Vec<(String, String, String)>>,
    released_instances: Mutex<Vec<(String, String)>>,
    audits: Mutex<Vec<&'static str>>,
    revalidation: Mutex<Option<Result<LicenseInfo, String>>>,
    catalog_delete_error: Mutex<Option<String>>,
    store_error: Mutex<Option<String>>,
    save_error: Mutex<Option<String>>,
    tier: Tier,
}

impl FakeActivationPorts {
    fn fresh(result: api::LicenseResult, tier: Tier) -> Self {
        Self {
            state: Mutex::new(None),
            key: Mutex::new(None),
            confirmed_replacement: Mutex::new(None),
            confirmation: Mutex::new(None),
            persistence_race: Mutex::new(None),
            minted: Mutex::new(VecDeque::from([Ok(result)])),
            released_credentials: Mutex::new(Vec::new()),
            released_instances: Mutex::new(Vec::new()),
            audits: Mutex::new(Vec::new()),
            revalidation: Mutex::new(None),
            catalog_delete_error: Mutex::new(None),
            store_error: Mutex::new(None),
            save_error: Mutex::new(None),
            tier,
        }
    }

    fn with_installed_license(self, state: LicenseState, key: &str) -> Self {
        *self.state.lock().expect("state lock") = Some(state);
        *self.key.lock().expect("key lock") = Some(key.to_string());
        self
    }

    fn with_revalidation(self, info: LicenseInfo) -> Self {
        *self.revalidation.lock().expect("revalidation lock") = Some(Ok(info));
        self
    }

    fn with_confirmation_race(self, state: LicenseState, key: &str) -> Self {
        *self
            .confirmed_replacement
            .lock()
            .expect("confirmation race lock") = Some((state, key.to_string()));
        *self.confirmation.lock().expect("confirmation lock") = Some(Ok(()));
        self
    }

    fn with_confirmed_replacement(self) -> Self {
        *self.confirmation.lock().expect("confirmation lock") = Some(Ok(()));
        self
    }

    fn with_confirmation(self, result: Result<(), ActivationConfirmationError>) -> Self {
        *self.confirmation.lock().expect("confirmation lock") = Some(result);
        self
    }

    fn with_persistence_race(self, state: LicenseState, key: &str) -> Self {
        *self.persistence_race.lock().expect("persistence race lock") =
            Some((state, key.to_string()));
        self
    }

    fn with_save_error(self, error: &str) -> Self {
        *self.save_error.lock().expect("save error lock") = Some(error.to_string());
        self
    }

    fn with_store_error(self, error: &str) -> Self {
        *self.store_error.lock().expect("store error lock") = Some(error.to_string());
        self
    }

    fn with_catalog_delete_error(self, error: &str) -> Self {
        *self
            .catalog_delete_error
            .lock()
            .expect("catalog delete error lock") = Some(error.to_string());
        self
    }

    fn with_mint_results(
        self,
        results: impl IntoIterator<Item = Result<api::LicenseResult, String>>,
    ) -> Self {
        *self.minted.lock().expect("mint lock") = results.into_iter().collect();
        self
    }
}

#[async_trait::async_trait]
impl ActivationPorts for FakeActivationPorts {
    async fn read_state(&self) -> Result<Option<LicenseState>, String> {
        Ok(self.state.lock().expect("state lock").clone())
    }

    fn read_key(&self) -> Result<Option<String>, String> {
        Ok(self.key.lock().expect("key lock").clone())
    }

    async fn revalidate(&self) -> Result<LicenseInfo, String> {
        self.revalidation
            .lock()
            .expect("revalidation lock")
            .take()
            .expect("configured revalidation")
    }

    async fn confirm_replacement(
        &self,
        _current_tier: &str,
    ) -> Result<(), ActivationConfirmationError> {
        if let Some((state, key)) = self
            .confirmed_replacement
            .lock()
            .expect("confirmation race lock")
            .take()
        {
            *self.state.lock().expect("state lock") = Some(state);
            *self.key.lock().expect("key lock") = Some(key);
        }
        self.confirmation
            .lock()
            .expect("confirmation lock")
            .take()
            .expect("configured confirmation")
    }

    fn instance_name(&self) -> String {
        "test-machine".to_string()
    }

    async fn activate(
        &self,
        _key: &str,
        _instance_name: &str,
    ) -> Result<api::LicenseResult, String> {
        self.minted
            .lock()
            .expect("mint lock")
            .pop_front()
            .expect("configured mint result")
    }

    async fn release_instance(&self, key: &str, instance_id: &str) {
        self.released_instances
            .lock()
            .expect("release lock")
            .push((key.to_string(), instance_id.to_string()));
    }

    async fn release_credential(&self, key: &str, instance_id: &str, phase: &str) {
        self.released_credentials
            .lock()
            .expect("credential release lock")
            .push((key.to_string(), instance_id.to_string(), phase.to_string()));
    }

    fn delete_catalog_token(&self) -> Result<(), String> {
        match self
            .catalog_delete_error
            .lock()
            .expect("catalog delete error lock")
            .clone()
        {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    async fn persist_generation(
        &self,
        expected_state: Option<&LicenseState>,
        expected_key: Option<&str>,
        state: &LicenseState,
    ) -> Result<(), ActivationPersistenceError> {
        if let Some((state, winner_key)) = self
            .persistence_race
            .lock()
            .expect("persistence race lock")
            .take()
        {
            *self.state.lock().expect("state lock") = Some(state);
            *self.key.lock().expect("key lock") = Some(winner_key);
        }
        let row_now = self.state.lock().expect("state lock").clone();
        let key_now = usable_key(self.key.lock().expect("key lock").clone());
        if !snapshot_unchanged(
            expected_state.map(|row| row.instance_id.as_str()),
            row_now.as_ref(),
            expected_key,
            key_now.as_deref(),
        ) {
            return Err(ActivationPersistenceError::Changed);
        }
        if self.store_error.lock().expect("store error lock").is_some() {
            return Err(ActivationPersistenceError::Storage);
        }
        *self.key.lock().expect("key lock") = Some(state.license_key.clone());
        if self.save_error.lock().expect("save error lock").is_some() {
            *self.key.lock().expect("key lock") = expected_key.map(str::to_string);
            return Err(ActivationPersistenceError::Storage);
        }
        *self.state.lock().expect("state lock") = Some(state.clone());
        Ok(())
    }

    fn tier_for_variant(&self, _variant_id: u64) -> Tier {
        self.tier
    }

    fn now_iso(&self) -> String {
        "2026-08-21T12:00:00Z".to_string()
    }

    async fn ensure_credential(&self, _key: &str, _instance_id: &str) -> Result<(), String> {
        Ok(())
    }

    fn request_catalog_refresh(&self) {}

    fn audit(&self, _detail: serde_json::Value, outcome: &'static str) {
        self.audits.lock().expect("audit lock").push(outcome);
    }
}

#[tokio::test]
async fn fresh_activation_persists_the_license_and_returns_its_tier() {
    let ports = FakeActivationPorts::fresh(
        api::LicenseResult {
            valid: true,
            status: "active".to_string(),
            variant_id: 42,
            instance_id: Some("instance-1".to_string()),
            expires_at: None,
            error: None,
        },
        Tier::Core,
    );

    let info = activate_license_with_ports(&ports, "  LICENSE-KEY  ".to_string())
        .await
        .expect("activation succeeds");

    assert_eq!(info.tier, Tier::Core);
    assert!(info.is_active);
    assert_eq!(
        ports.key.lock().expect("key lock").as_deref(),
        Some("LICENSE-KEY")
    );
    let state = ports
        .state
        .lock()
        .expect("state lock")
        .clone()
        .expect("saved state");
    assert_eq!(state.instance_id, "instance-1");
    assert_eq!(state.license_key, "LICENSE-KEY");
    assert!(ports
        .released_instances
        .lock()
        .expect("release lock")
        .is_empty());
}

#[tokio::test]
async fn failed_state_persistence_restores_the_key_and_releases_the_new_instance() {
    let ports = FakeActivationPorts::fresh(
        api::LicenseResult {
            valid: true,
            status: "active".to_string(),
            variant_id: 42,
            instance_id: Some("instance-1".to_string()),
            expires_at: None,
            error: None,
        },
        Tier::Core,
    )
    .with_save_error("disk full");

    let error = activate_license_with_ports(&ports, "LICENSE-KEY".to_string())
        .await
        .expect_err("persistence failure refuses activation");

    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&error).expect("typed error")["code"],
        "incomplete"
    );
    assert!(ports.key.lock().expect("key lock").is_none());
    assert!(ports.state.lock().expect("state lock").is_none());
    assert_eq!(
        *ports.released_instances.lock().expect("release lock"),
        vec![("LICENSE-KEY".to_string(), "instance-1".to_string())]
    );
}

#[tokio::test]
async fn failed_key_persistence_releases_the_new_instance() {
    let ports = FakeActivationPorts::fresh(
        api::LicenseResult {
            valid: true,
            status: "active".to_string(),
            variant_id: 42,
            instance_id: Some("instance-1".to_string()),
            expires_at: None,
            error: None,
        },
        Tier::Core,
    )
    .with_store_error("keychain unavailable");

    let error = activate_license_with_ports(&ports, "LICENSE-KEY".to_string())
        .await
        .expect_err("key persistence failure refuses activation");

    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&error).expect("typed error")["code"],
        "incomplete"
    );
    assert!(ports.key.lock().expect("key lock").is_none());
    assert!(ports.state.lock().expect("state lock").is_none());
    assert_eq!(
        *ports.released_instances.lock().expect("release lock"),
        vec![("LICENSE-KEY".to_string(), "instance-1".to_string())]
    );
}

#[tokio::test]
async fn reentering_the_active_key_revalidates_without_minting() {
    let installed = LicenseState {
        license_key: "LICENSE-KEY".to_string(),
        instance_id: "installed-instance".to_string(),
        variant_id: 42,
        tier: Tier::Core,
        status: "active".to_string(),
        last_validated_at: "2026-08-21T12:00:00Z".to_string(),
        activated_at: "2026-08-20T12:00:00Z".to_string(),
        expires_at: None,
    };
    let expected = info_from_state(&installed);
    let ports = FakeActivationPorts::fresh(
        api::LicenseResult {
            valid: true,
            status: "active".to_string(),
            variant_id: 42,
            instance_id: Some("must-not-mint".to_string()),
            expires_at: None,
            error: None,
        },
        Tier::Core,
    )
    .with_installed_license(installed, "LICENSE-KEY")
    .with_revalidation(expected);

    let info = activate_license_with_ports(&ports, "LICENSE-KEY".to_string())
        .await
        .expect("same key revalidates");

    assert_eq!(info.tier, Tier::Core);
    assert_eq!(info.status, "active");
    assert_eq!(ports.minted.lock().expect("mint lock").len(), 1);
    assert!(ports
        .released_instances
        .lock()
        .expect("release lock")
        .is_empty());
}

#[tokio::test]
async fn replacement_refuses_when_the_installed_generation_changes_during_confirmation() {
    let installed = LicenseState {
        license_key: "OLD-KEY".to_string(),
        instance_id: "old-instance".to_string(),
        variant_id: 42,
        tier: Tier::Core,
        status: "active".to_string(),
        last_validated_at: "2026-08-21T12:00:00Z".to_string(),
        activated_at: "2026-08-20T12:00:00Z".to_string(),
        expires_at: None,
    };
    let winner = LicenseState {
        license_key: "WINNER-KEY".to_string(),
        instance_id: "winner-instance".to_string(),
        ..installed.clone()
    };
    let ports = FakeActivationPorts::fresh(
        api::LicenseResult {
            valid: true,
            status: "active".to_string(),
            variant_id: 42,
            instance_id: Some("must-not-mint".to_string()),
            expires_at: None,
            error: None,
        },
        Tier::Core,
    )
    .with_installed_license(installed, "OLD-KEY")
    .with_confirmation_race(winner, "WINNER-KEY");

    let error = activate_license_with_ports(&ports, "NEW-KEY".to_string())
        .await
        .expect_err("raced replacement is refused");

    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&error).expect("typed error")["code"],
        "changed_during_activation"
    );
    assert_eq!(ports.minted.lock().expect("mint lock").len(), 1);
    assert_eq!(
        ports.key.lock().expect("key lock").as_deref(),
        Some("WINNER-KEY")
    );
    assert!(ports
        .released_instances
        .lock()
        .expect("release lock")
        .is_empty());
}

#[tokio::test]
async fn same_key_limit_releases_this_machine_and_retries_once() {
    let installed = LicenseState {
        license_key: "LICENSE-KEY".to_string(),
        instance_id: "old-instance".to_string(),
        variant_id: 42,
        tier: Tier::Core,
        status: "expired".to_string(),
        last_validated_at: "2026-08-21T12:00:00Z".to_string(),
        activated_at: "2026-08-20T12:00:00Z".to_string(),
        expires_at: None,
    };
    let issued = api::LicenseResult {
        valid: true,
        status: "active".to_string(),
        variant_id: 42,
        instance_id: Some("new-instance".to_string()),
        expires_at: None,
        error: None,
    };
    let ports = FakeActivationPorts::fresh(issued.clone(), Tier::Core)
        .with_installed_license(installed, "LICENSE-KEY")
        .with_mint_results([
            Err("activation limit reached for this key".to_string()),
            Ok(issued),
        ]);

    let info = activate_license_with_ports(&ports, "LICENSE-KEY".to_string())
        .await
        .expect("own seat is reclaimed and retried");

    assert_eq!(info.tier, Tier::Core);
    assert_eq!(ports.minted.lock().expect("mint lock").len(), 0);
    assert_eq!(
        *ports.released_instances.lock().expect("release lock"),
        vec![("LICENSE-KEY".to_string(), "old-instance".to_string())]
    );
    assert_eq!(
        ports
            .state
            .lock()
            .expect("state lock")
            .as_ref()
            .map(|state| state.instance_id.as_str()),
        Some("new-instance")
    );
}

#[tokio::test]
async fn confirmed_replacement_releases_the_predecessor_before_installing_the_new_key() {
    let installed = LicenseState {
        license_key: "OLD-KEY".to_string(),
        instance_id: "old-instance".to_string(),
        variant_id: 42,
        tier: Tier::Core,
        status: "active".to_string(),
        last_validated_at: "2026-08-21T12:00:00Z".to_string(),
        activated_at: "2026-08-20T12:00:00Z".to_string(),
        expires_at: None,
    };
    let ports = FakeActivationPorts::fresh(
        api::LicenseResult {
            valid: true,
            status: "active".to_string(),
            variant_id: 84,
            instance_id: Some("new-instance".to_string()),
            expires_at: None,
            error: None,
        },
        Tier::Pro,
    )
    .with_installed_license(installed, "OLD-KEY")
    .with_confirmed_replacement();

    let info = activate_license_with_ports(&ports, "NEW-KEY".to_string())
        .await
        .expect("confirmed replacement succeeds");

    assert_eq!(info.tier, Tier::Pro);
    assert_eq!(
        *ports.released_instances.lock().expect("release lock"),
        vec![("OLD-KEY".to_string(), "old-instance".to_string())]
    );
    assert!(ports
        .released_credentials
        .lock()
        .expect("credential release lock")
        .iter()
        .any(|(key, instance, _)| key == "OLD-KEY" && instance == "old-instance"));
    assert_eq!(
        ports.key.lock().expect("key lock").as_deref(),
        Some("NEW-KEY")
    );
    assert_eq!(
        ports
            .state
            .lock()
            .expect("state lock")
            .as_ref()
            .map(|state| state.instance_id.as_str()),
        Some("new-instance")
    );
}

#[tokio::test]
async fn activation_preserves_a_generation_installed_before_persistence() {
    let winner = LicenseState {
        license_key: "WINNER-KEY".to_string(),
        instance_id: "winner-instance".to_string(),
        variant_id: 84,
        tier: Tier::Pro,
        status: "active".to_string(),
        last_validated_at: "2026-08-21T12:00:00Z".to_string(),
        activated_at: "2026-08-21T12:00:00Z".to_string(),
        expires_at: None,
    };
    let ports = FakeActivationPorts::fresh(
        api::LicenseResult {
            valid: true,
            status: "active".to_string(),
            variant_id: 42,
            instance_id: Some("attempt-instance".to_string()),
            expires_at: None,
            error: None,
        },
        Tier::Core,
    )
    .with_persistence_race(winner, "WINNER-KEY");

    let error = activate_license_with_ports(&ports, "ATTEMPT-KEY".to_string())
        .await
        .expect_err("the winning generation is preserved");

    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&error).expect("typed error")["code"],
        "changed_during_activation"
    );
    assert_eq!(
        ports.key.lock().expect("key lock").as_deref(),
        Some("WINNER-KEY")
    );
    assert_eq!(
        ports
            .state
            .lock()
            .expect("state lock")
            .as_ref()
            .map(|state| state.instance_id.as_str()),
        Some("winner-instance")
    );
    assert_eq!(
        *ports.released_instances.lock().expect("release lock"),
        vec![("ATTEMPT-KEY".to_string(), "attempt-instance".to_string())]
    );
}

#[tokio::test]
async fn replacement_decline_cancels_without_minting() {
    let installed = LicenseState {
        license_key: "OLD-KEY".to_string(),
        instance_id: "old-instance".to_string(),
        variant_id: 42,
        tier: Tier::Core,
        status: "active".to_string(),
        last_validated_at: "2026-08-21T12:00:00Z".to_string(),
        activated_at: "2026-08-20T12:00:00Z".to_string(),
        expires_at: None,
    };
    let ports = FakeActivationPorts::fresh(
        api::LicenseResult {
            valid: true,
            status: "active".to_string(),
            variant_id: 84,
            instance_id: Some("must-not-mint".to_string()),
            expires_at: None,
            error: None,
        },
        Tier::Pro,
    )
    .with_installed_license(installed, "OLD-KEY")
    .with_confirmation(Err(ActivationConfirmationError::Declined));

    let error = activate_license_with_ports(&ports, "NEW-KEY".to_string())
        .await
        .expect_err("declined replacement is cancelled");

    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&error).expect("typed error")["code"],
        "cancelled"
    );
    assert_eq!(ports.minted.lock().expect("mint lock").len(), 1);
    assert_eq!(
        ports.key.lock().expect("key lock").as_deref(),
        Some("OLD-KEY")
    );
}

#[tokio::test]
async fn replacement_dialog_failure_is_not_reported_as_a_cancellation() {
    let installed = LicenseState {
        license_key: "OLD-KEY".to_string(),
        instance_id: "old-instance".to_string(),
        variant_id: 42,
        tier: Tier::Core,
        status: "active".to_string(),
        last_validated_at: "2026-08-21T12:00:00Z".to_string(),
        activated_at: "2026-08-20T12:00:00Z".to_string(),
        expires_at: None,
    };
    let ports = FakeActivationPorts::fresh(
        api::LicenseResult {
            valid: true,
            status: "active".to_string(),
            variant_id: 84,
            instance_id: Some("must-not-mint".to_string()),
            expires_at: None,
            error: None,
        },
        Tier::Pro,
    )
    .with_installed_license(installed, "OLD-KEY")
    .with_confirmation(Err(ActivationConfirmationError::Failed));

    let error = activate_license_with_ports(&ports, "NEW-KEY".to_string())
        .await
        .expect_err("dialog failure refuses replacement");

    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&error).expect("typed error")["code"],
        "incomplete"
    );
    assert_eq!(ports.minted.lock().expect("mint lock").len(), 1);
}

#[tokio::test]
async fn provider_refusal_releases_any_instance_returned_with_it() {
    let ports = FakeActivationPorts::fresh(
        api::LicenseResult {
            valid: false,
            status: "inactive".to_string(),
            variant_id: 42,
            instance_id: Some("refused-instance".to_string()),
            expires_at: None,
            error: Some("license expired".to_string()),
        },
        Tier::Core,
    );

    let error = activate_license_with_ports(&ports, "LICENSE-KEY".to_string())
        .await
        .expect_err("provider refusal is returned");

    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&error).expect("typed error")["code"],
        "expired"
    );
    assert_eq!(
        *ports.released_instances.lock().expect("release lock"),
        vec![("LICENSE-KEY".to_string(), "refused-instance".to_string())]
    );
    assert_eq!(*ports.audits.lock().expect("audit lock"), vec!["fail"]);
}

#[tokio::test]
async fn uncleared_catalog_token_refuses_activation_and_releases_the_new_instance() {
    let ports = FakeActivationPorts::fresh(
        api::LicenseResult {
            valid: true,
            status: "active".to_string(),
            variant_id: 42,
            instance_id: Some("instance-1".to_string()),
            expires_at: None,
            error: None,
        },
        Tier::Core,
    )
    .with_catalog_delete_error("keychain unavailable");

    let error = activate_license_with_ports(&ports, "LICENSE-KEY".to_string())
        .await
        .expect_err("stray token must be cleared");

    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&error).expect("typed error")["code"],
        "incomplete"
    );
    assert!(ports.key.lock().expect("key lock").is_none());
    assert_eq!(
        *ports.released_instances.lock().expect("release lock"),
        vec![("LICENSE-KEY".to_string(), "instance-1".to_string())]
    );
}
