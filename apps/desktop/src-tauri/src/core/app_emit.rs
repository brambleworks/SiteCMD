//! Shared Tauri event-emission helpers without command-layer semantics.

/// Emit a Tauri event and log failures.
#[tracing::instrument(skip(app, payload), fields(event = %event))]
pub(crate) fn emit_event<R: tauri::Runtime, S: serde::Serialize + Clone>(
    app: &tauri::AppHandle<R>,
    event: &str,
    payload: S,
) {
    if let Err(e) = tauri::Emitter::emit(app, event, payload) {
        tracing::warn!("Failed to emit '{}' event: {}", event, e);
    }
}

pub(crate) fn emit_site_score_changed<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    project_id: i64,
) {
    emit_event(
        app,
        "site-score-changed",
        serde_json::json!({ "projectId": project_id }),
    );
}
