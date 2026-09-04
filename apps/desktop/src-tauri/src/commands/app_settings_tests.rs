use super::*;
use serde_json::json;
use std::path::Path;
use tauri::Manager;

fn settings_app(directory: &Path) -> tauri::App<tauri::test::MockRuntime> {
    let mut context = tauri::test::mock_context(tauri::test::noop_assets());
    // An absolute test identifier confines the app-data resolver to the temporary directory.
    context.config_mut().identifier = directory.to_string_lossy().into_owned();
    let app = tauri::test::mock_builder()
        .plugin(tauri_plugin_store::Builder::new().build())
        .build(context)
        .expect("build settings test app");
    assert_eq!(app.path().app_data_dir().unwrap(), directory);
    app.store_builder(APP_SETTINGS_FILE)
        .disable_auto_save()
        .build()
        .expect("load isolated settings store");
    app
}

#[tokio::test]
async fn existing_settings_survive_the_restricted_commands() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join(APP_SETTINGS_FILE);
    std::fs::write(&path, r#"{"desktop-prefs":{"refreshOnFocus":false}}"#).unwrap();
    let app = settings_app(directory.path());

    assert_eq!(
        get_app_setting(app.handle().clone(), "desktop-prefs".into())
            .await
            .unwrap(),
        Some(json!({ "refreshOnFocus": false }))
    );
    assert_eq!(
        get_app_setting(app.handle().clone(), "missing".into())
            .await
            .unwrap(),
        None
    );
    set_app_setting(
        app.handle().clone(),
        "scan-prefs".into(),
        json!({ "retentionLimit": 30 }),
    )
    .await
    .unwrap();
    app.store(APP_SETTINGS_FILE).unwrap().save().unwrap();
    let persisted: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(
        persisted["desktop-prefs"],
        json!({ "refreshOnFocus": false })
    );
    assert_eq!(persisted["scan-prefs"], json!({ "retentionLimit": 30 }));
}

#[tokio::test]
async fn path_shaped_keys_cannot_redirect_settings_reads_or_writes() {
    let directory = tempfile::tempdir().unwrap();
    let app_data = directory.path().join("app-data");
    std::fs::create_dir(&app_data).unwrap();
    let outside = directory.path().join("outside.json");
    let original = r#"{"private":"untouched"}"#;
    std::fs::write(&outside, original).unwrap();
    let app = settings_app(&app_data);

    for key in [
        "../outside.json".to_string(),
        outside.to_string_lossy().into_owned(),
    ] {
        assert_eq!(
            get_app_setting(app.handle().clone(), key.clone())
                .await
                .unwrap(),
            None
        );
        set_app_setting(app.handle().clone(), key.clone(), json!({ "test": true }))
            .await
            .unwrap();
        assert_eq!(
            get_app_setting(app.handle().clone(), key).await.unwrap(),
            Some(json!({ "test": true }))
        );
    }
    app.store(APP_SETTINGS_FILE).unwrap().save().unwrap();
    assert!(app_data.join(APP_SETTINGS_FILE).is_file());
    assert_eq!(std::fs::read_to_string(outside).unwrap(), original);
}
