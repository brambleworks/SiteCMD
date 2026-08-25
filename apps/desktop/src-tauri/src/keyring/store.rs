#[cfg(debug_assertions)]
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::{LazyLock, Mutex};
use tauri::AppHandle;
use tauri_plugin_keyring::KeyringExt;

const SERVICE_NAME: &str = crate::app_identity::KEYRING_SERVICE_NAME;

/// Debug-only plaintext secret store persisted to app-data `dev-secrets.json`.
/// Release builds use the OS keychain and do not compile this path.
#[cfg(debug_assertions)]
pub(super) static DEBUG_SECRET_STORE: LazyLock<Mutex<HashMap<String, String>>> =
    LazyLock::new(|| {
        #[cfg(not(test))]
        if let Some(path) = dev_secret_store_path() {
            return Mutex::new(load_dev_secrets(&path));
        }
        Mutex::new(HashMap::new())
    });

/// Serialize tests that mutate the process-global debug secret store.
#[cfg(test)]
pub(crate) static SECRET_TEST_GUARD: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

#[cfg(all(debug_assertions, not(test)))]
fn dev_secret_store_path() -> Option<std::path::PathBuf> {
    crate::app_identity::default_storage_dir().map(|dir| dir.join("dev-secrets.json"))
}

#[cfg(debug_assertions)]
pub(super) fn load_dev_secrets(path: &std::path::Path) -> HashMap<String, String> {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return HashMap::new();
    };
    // A corrupt file degrades to "no stored secrets" (the pre-persistence
    // behavior) rather than blocking startup.
    serde_json::from_str(&raw).unwrap_or_default()
}

#[cfg(debug_assertions)]
pub(super) fn persist_dev_secrets(
    path: &std::path::Path,
    secrets: &HashMap<String, String>,
) -> Result<(), String> {
    let json = serde_json::to_string_pretty(secrets)
        .map_err(|e| format!("Failed to serialize dev secrets: {}", e))?;
    crate::app_identity::write_private_file(path, json.as_bytes())
        .map_err(|e| format!("Failed to write dev secret store: {}", e))
}

/// Best-effort write-back after every debug-store mutation. Tests stay
/// memory-only so `cargo test` never touches the real app data dir.
#[cfg(debug_assertions)]
fn persist_debug_store_best_effort(secrets: &HashMap<String, String>) {
    #[cfg(not(test))]
    if let Some(path) = dev_secret_store_path() {
        if let Err(e) = persist_dev_secrets(&path, secrets) {
            tracing::warn!(
                "Dev secret store write failed; secrets will not survive an app restart: {}",
                e
            );
        }
    }
    #[cfg(test)]
    let _ = secrets;
}

/// Cache per-session keychain read failures to avoid repeated system prompts.
#[derive(Default)]
pub(super) struct ReadFailureCache {
    keys: Mutex<HashSet<String>>,
}

impl ReadFailureCache {
    fn contains(&self, user: &str) -> bool {
        self.keys
            .lock()
            .map(|keys| keys.contains(user))
            .unwrap_or(false)
    }

    fn record(&self, user: &str) {
        if let Ok(mut keys) = self.keys.lock() {
            keys.insert(user.to_string());
        }
    }

    fn clear(&self, user: &str) {
        if let Ok(mut keys) = self.keys.lock() {
            keys.remove(user);
        }
    }
}

static READ_FAILURES: LazyLock<ReadFailureCache> = LazyLock::new(ReadFailureCache::default);

/// Debug builds opt into the plaintext `dev-secrets.json` store by setting
/// this variable to `1` before launch. Without it a debug build uses the OS
/// keychain exactly like a release build. Tests always use the in-memory
/// debug store and never touch the keychain. Release builds have no consumer,
/// so the constant is debug-gated to keep them warning-free.
#[cfg(debug_assertions)]
pub(crate) const DEV_PLAINTEXT_SECRETS_ENV: &str = "SITECMD_DEV_PLAINTEXT_SECRETS";

#[cfg(debug_assertions)]
fn dev_store_opted_in(value: Option<&str>, is_test: bool) -> bool {
    is_test || value == Some("1")
}

#[cfg(debug_assertions)]
fn plaintext_dev_store_enabled() -> bool {
    static ENABLED: LazyLock<bool> = LazyLock::new(|| {
        let value = std::env::var(DEV_PLAINTEXT_SECRETS_ENV).ok();
        dev_store_opted_in(value.as_deref(), cfg!(test))
    });
    *ENABLED
}

/// Use the OS keychain unless a debug build explicitly opted into the
/// plaintext dev store.
fn keyring_enabled() -> bool {
    #[cfg(debug_assertions)]
    {
        !plaintext_dev_store_enabled()
    }
    #[cfg(not(debug_assertions))]
    {
        true
    }
}

/// Both stores survive a restart (the keychain, or `dev-secrets.json` at
/// mode 0600), so SQLite never needs to hold a plaintext credential.
pub(super) fn durable_secret_store_enabled() -> bool {
    true
}

pub(super) fn secure_store_available_for_migration() -> bool {
    durable_secret_store_enabled()
}

pub(super) fn set_secret<R: tauri::Runtime>(
    app: &AppHandle<R>,
    user: &str,
    value: &str,
) -> Result<(), String> {
    if keyring_enabled() {
        app.keyring()
            .set_password(SERVICE_NAME, user, value)
            .map_err(|e| format!("Failed to store secret in keychain: {}", e))?;
        // The freshly written item is owned by this build, so clear any prior
        // read-failure: a reconnect must restore normal reads immediately.
        READ_FAILURES.clear(user);
        return Ok(());
    }

    #[cfg(debug_assertions)]
    {
        let mut store = DEBUG_SECRET_STORE
            .lock()
            .map_err(|_| "Failed to access debug secret store".to_string())?;
        store.insert(user.to_string(), value.to_string());
        persist_debug_store_best_effort(&store);
        return Ok(());
    }

    #[allow(unreachable_code)]
    Err("Secure secret storage is unavailable in this build".into())
}

pub(super) fn get_secret<R: tauri::Runtime>(
    app: &AppHandle<R>,
    user: &str,
) -> Result<Option<String>, String> {
    if keyring_enabled() {
        // A secret that already failed to read this session is treated as
        // unavailable without touching the OS keychain again - otherwise every
        // scan re-prompts for the same inaccessible item.
        if READ_FAILURES.contains(user) {
            return Ok(None);
        }
        return match app.keyring().get_password(SERVICE_NAME, user) {
            Ok(found) => Ok(found),
            Err(e) => {
                // Treat an unreadable key as unavailable so scans continue, and
                // suppress repeat prompts until the secret is rewritten.
                READ_FAILURES.record(user);
                tracing::warn!(
                    "Keychain read failed for '{}'; treating the secret as unavailable for this session (reconnect to restore): {}",
                    user,
                    e
                );
                Ok(None)
            }
        };
    }

    #[cfg(debug_assertions)]
    {
        let store = DEBUG_SECRET_STORE
            .lock()
            .map_err(|_| "Failed to access debug secret store".to_string())?;
        return Ok(store.get(user).cloned());
    }

    #[allow(unreachable_code)]
    Ok(None)
}

/// Read a lifecycle secret without treating keychain failure as absence.
/// A false absence could mint a replacement while the unreadable credential
/// still consumes its server-side slot.
pub(super) fn get_secret_strict<R: tauri::Runtime>(
    app: &AppHandle<R>,
    user: &str,
) -> Result<Option<String>, String> {
    if keyring_enabled() {
        return app
            .keyring()
            .get_password(SERVICE_NAME, user)
            .map_err(|error| format!("keychain read failed for '{user}': {error}"));
    }

    #[cfg(debug_assertions)]
    {
        let store = DEBUG_SECRET_STORE
            .lock()
            .map_err(|_| "Failed to access debug secret store".to_string())?;
        return Ok(store.get(user).cloned());
    }

    #[allow(unreachable_code)]
    Ok(None)
}

pub(super) fn delete_secret<R: tauri::Runtime>(
    app: &AppHandle<R>,
    user: &str,
) -> Result<(), String> {
    if keyring_enabled() {
        // Only confirmed absence may skip deletion; a failed read is not absence.
        match app.keyring().get_password(SERVICE_NAME, user) {
            Ok(None) => {}
            Ok(Some(_)) => {
                app.keyring()
                    .delete_password(SERVICE_NAME, user)
                    .map_err(|e| format!("Failed to delete from keychain: {}", e))?;
            }
            Err(error) => {
                return Err(format!(
                    "Keychain entry could not be read, so it was not deleted: {error}"
                ));
            }
        }
        // Drop any cached failure so a later re-create reads the new item fresh.
        READ_FAILURES.clear(user);
        return Ok(());
    }

    #[cfg(debug_assertions)]
    {
        let mut store = DEBUG_SECRET_STORE
            .lock()
            .map_err(|_| "Failed to access debug secret store".to_string())?;
        store.remove(user);
        persist_debug_store_best_effort(&store);
        return Ok(());
    }

    #[allow(unreachable_code)]
    Ok(())
}

#[cfg(test)]
mod dev_secret_persistence_tests {
    use super::{load_dev_secrets, persist_dev_secrets};
    use std::collections::HashMap;

    #[test]
    fn persist_and_load_round_trip_preserves_secrets() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("dev-secrets.json");

        let mut secrets = HashMap::new();
        secrets.insert("app:license_key".to_string(), "lc-test-key".to_string());
        secrets.insert("shk:abc:cloudflare".to_string(), "cf-token".to_string());

        persist_dev_secrets(&path, &secrets).unwrap();
        assert_eq!(load_dev_secrets(&path), secrets);
    }

    #[test]
    fn load_degrades_to_empty_for_missing_or_corrupt_files() {
        let temp = tempfile::tempdir().unwrap();
        let missing = temp.path().join("nope.json");
        assert!(load_dev_secrets(&missing).is_empty());

        let corrupt = temp.path().join("corrupt.json");
        std::fs::write(&corrupt, "{not json").unwrap();
        assert!(load_dev_secrets(&corrupt).is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn persisted_file_is_owner_readable_only() {
        use std::os::unix::fs::PermissionsExt;
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("dev-secrets.json");
        persist_dev_secrets(&path, &HashMap::new()).unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "dev secrets must be 0600");
    }

    #[test]
    fn debug_store_mutations_write_back_to_disk() {
        let source = include_str!("store.rs");
        // concat! keeps this test's own source from matching the needle.
        let needle = concat!("persist_debug_store_best_effort", "(&store);");
        let persist_calls = source.matches(needle).count();
        assert_eq!(
            persist_calls, 2,
            "set_secret and delete_secret must each persist the debug store after mutating it"
        );
    }
}

#[cfg(test)]
mod read_failure_cache_tests {
    use super::ReadFailureCache;

    #[test]
    fn records_failures_per_key_and_clears_on_rewrite() {
        let cache = ReadFailureCache::default();
        let cloudflare = "shk:abc:cloudflare";
        let license = "app:license_key";

        // A key is readable until it fails.
        assert!(!cache.contains(cloudflare));

        // After a failed read it is skipped, but unrelated keys are untouched
        // so one inaccessible integration can't disable the others.
        cache.record(cloudflare);
        assert!(cache.contains(cloudflare));
        assert!(!cache.contains(license));

        // Rewriting (reconnecting) the secret clears the failure so the next
        // read hits the OS keychain again instead of staying stuck unavailable.
        cache.clear(cloudflare);
        assert!(!cache.contains(cloudflare));
    }
}

#[cfg(all(test, debug_assertions))]
mod dev_store_gate_tests {
    use super::dev_store_opted_in;

    #[test]
    fn plaintext_store_requires_an_explicit_opt_in_outside_tests() {
        assert!(!dev_store_opted_in(None, false));
        assert!(!dev_store_opted_in(Some("0"), false));
        assert!(!dev_store_opted_in(Some("true"), false));
        assert!(dev_store_opted_in(Some("1"), false));
        assert!(dev_store_opted_in(None, true), "tests stay memory-only");
    }
}
