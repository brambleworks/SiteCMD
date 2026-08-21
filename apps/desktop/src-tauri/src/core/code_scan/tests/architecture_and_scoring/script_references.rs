use super::super::*;

#[test]
fn script_referenced_packages_finds_cli_tools_in_package_json_scripts() {
    let manifest = r#"{
            "name": "my-app",
            "scripts": {
                "dev": "nodemon src/index.js",
                "lint": "biome check .",
                "build": "webpack --config webpack.config.js",
                "css": "sass --watch src/styles",
                "fmt": "stylelint '**/*.scss'",
                "combo": "cross-env NODE_ENV=production webpack build"
            },
            "devDependencies": {
                "nodemon": "^3.0.0"
            }
        }"#;
    let declared = vec![
        "nodemon".to_string(),
        "biome".to_string(),
        "webpack".to_string(),
        "sass".to_string(),
        "stylelint".to_string(),
        "cross-env".to_string(),
        "react".to_string(), // declared but NOT in scripts
    ];
    let found = collect_script_referenced_packages(manifest, &declared);
    assert!(found.contains("nodemon"));
    assert!(found.contains("biome"));
    assert!(found.contains("webpack"));
    assert!(found.contains("sass"));
    assert!(found.contains("stylelint"));
    assert!(found.contains("cross-env"));
    assert!(!found.contains("react"));
}

#[test]
fn script_referenced_packages_handles_missing_or_invalid_json() {
    let declared = vec!["nodemon".to_string()];
    assert!(collect_script_referenced_packages("{not json}", &declared).is_empty());
    assert!(collect_script_referenced_packages(r#"{"name":"x"}"#, &declared).is_empty());
    assert!(collect_script_referenced_packages(r#"{"scripts": {}}"#, &declared).is_empty());
}

#[test]
fn appears_as_script_token_requires_word_boundary() {
    // "nodemon" should not match inside "nodemon-like"
    assert!(appears_as_script_token("nodemon src/index.js", "nodemon"));
    assert!(appears_as_script_token("pnpm run nodemon", "nodemon"));
    assert!(!appears_as_script_token("run nodemonlike", "nodemon"));
    // Scoped package tokens
    assert!(appears_as_script_token(
        "pnpm run @biomejs/biome check",
        "@biomejs/biome"
    ));
    // Plain substring inside another word must NOT match
    assert!(!appears_as_script_token("awebpackage build", "webpack"));
}
