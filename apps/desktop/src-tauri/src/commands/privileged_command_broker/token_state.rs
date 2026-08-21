use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::time::{Duration, Instant};

use super::broker_allowed_commands;

// allow-inline-duration: privileged-command token TTL belongs with the
// token store, not in shared constants - it is part of the brokered
// security contract documented in this module.
pub(super) const PRIVILEGED_COMMAND_TOKEN_TTL: Duration = Duration::from_secs(15);

#[derive(Debug)]
pub(super) struct IssuedToken {
    pub(super) broker_command: String,
    pub(super) command: String,
    pub(super) args_signature: String,
    pub(super) issued_at: Instant,
}

/// Store of expiring, single-use privileged-command tokens. Each token is
/// bound to its broker command, target command, and arguments; consumption
/// rejects reuse and any scope, argument, or TTL mismatch.
pub struct TokenStore {
    pub(super) tokens: std::sync::Mutex<HashMap<String, IssuedToken>>,
    ttl: Duration,
}

impl Default for TokenStore {
    fn default() -> Self {
        Self::new(PRIVILEGED_COMMAND_TOKEN_TTL)
    }
}

impl TokenStore {
    pub fn new(ttl: Duration) -> Self {
        Self {
            tokens: std::sync::Mutex::new(HashMap::new()),
            ttl,
        }
    }

    pub fn issue(
        &self,
        broker_command: &str,
        command: &str,
        args: &Value,
    ) -> Result<String, String> {
        let Some(allowed_commands) = broker_allowed_commands(broker_command) else {
            return Err("Unsupported privileged command broker.".to_string());
        };
        if !allowed_commands.contains(&command) {
            return Err("Unsupported privileged command token request.".to_string());
        }

        let args_signature = args_signature(args)?;
        let token = random_privileged_command_token()?;
        let mut tokens = self
            .tokens
            .lock()
            .map_err(|_| "Privileged command token store is unavailable.".to_string())?;
        prune_expired_tokens(&mut tokens, self.ttl);
        tokens.insert(
            token.clone(),
            IssuedToken {
                broker_command: broker_command.to_string(),
                command: command.to_string(),
                args_signature,
                issued_at: Instant::now(),
            },
        );
        Ok(token)
    }

    pub fn consume(
        &self,
        token: Option<&str>,
        broker_command: &str,
        command: &str,
        args: &Value,
    ) -> Result<(), String> {
        let Some(token) = token.filter(|value| !value.trim().is_empty()) else {
            return Err("Missing privileged command token.".to_string());
        };
        let args_signature = args_signature(args)?;
        let mut tokens = self
            .tokens
            .lock()
            .map_err(|_| "Privileged command token store is unavailable.".to_string())?;
        prune_expired_tokens(&mut tokens, self.ttl);
        let Some(record) = tokens.remove(token) else {
            return Err("Privileged command token is invalid or expired.".to_string());
        };
        if record.broker_command != broker_command || record.command != command {
            return Err("Privileged command token does not match this request.".to_string());
        }
        if record.args_signature != args_signature {
            return Err("Privileged command token does not match these arguments.".to_string());
        }
        Ok(())
    }
}

fn random_privileged_command_token() -> Result<String, String> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes)
        .map_err(|error| format!("Could not issue privileged command token: {error}"))?;
    Ok(hex::encode(bytes))
}

fn prune_expired_tokens(tokens: &mut HashMap<String, IssuedToken>, ttl: Duration) {
    tokens.retain(|_, token| token.issued_at.elapsed() <= ttl);
}

fn canonical_json_value(value: &Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.iter().map(canonical_json_value).collect()),
        Value::Object(map) => {
            let mut entries = map.iter().collect::<Vec<_>>();
            entries.sort_by_key(|(left_key, _)| *left_key);
            let mut canonical = serde_json::Map::new();
            for (key, value) in entries {
                canonical.insert(key.clone(), canonical_json_value(value));
            }
            Value::Object(canonical)
        }
        _ => value.clone(),
    }
}

fn args_signature(args: &Value) -> Result<String, String> {
    let canonical = serde_json::to_vec(&canonical_json_value(args))
        .map_err(|error| format!("Could not sign privileged command arguments: {error}"))?;
    let digest = Sha256::digest(&canonical);
    Ok(hex::encode(digest))
}
