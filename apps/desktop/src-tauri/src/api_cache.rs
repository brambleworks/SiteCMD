//! Process-local integration response cache.

use crate::constants::{CACHE_MAX_ENTRIES, CACHE_TTL_SECS};
use moka::sync::Cache;
use std::sync::LazyLock;
use std::time::Duration;

static API_CACHE: LazyLock<Cache<String, serde_json::Value>> = LazyLock::new(|| {
    Cache::builder()
        .max_capacity(CACHE_MAX_ENTRIES)
        .time_to_live(Duration::from_secs(CACHE_TTL_SECS))
        .support_invalidation_closures()
        .build()
});

pub fn cache_key(project_id: i64, service: &str, period: &str) -> String {
    format!("{}:{}:{}", project_id, service, period)
}

pub fn get(key: &str) -> Option<serde_json::Value> {
    API_CACHE.get(key)
}

pub fn set(key: &str, value: serde_json::Value) {
    API_CACHE.insert(key.to_string(), value);
}

pub fn clear_all() {
    API_CACHE.invalidate_all();
    API_CACHE.run_pending_tasks();
}

pub fn invalidate_project(project_id: i64) {
    let prefix = format!("{}:", project_id);
    let _ = API_CACHE
        .invalidate_entries_if(move |k: &String, _v: &serde_json::Value| k.starts_with(&prefix));
    API_CACHE.run_pending_tasks();
}

#[cfg(test)]
mod tests {
    use super::{cache_key, clear_all, get, set};

    #[test]
    fn clear_all_removes_cached_entries() {
        let key = cache_key(42, "plausible", "30d");
        clear_all();
        set(&key, serde_json::json!({ "visitors": 123 }));

        assert_eq!(get(&key), Some(serde_json::json!({ "visitors": 123 })));

        clear_all();

        assert!(get(&key).is_none());
    }
}
