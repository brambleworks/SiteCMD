use super::super::*;

#[test]
fn is_vendored_path_skips_drupal_core_and_contrib() {
    let root = Path::new("/projects/drupal-site");

    // Drupal (web-root layout) - all vendored
    assert!(is_vendored_path(root, &root.join("web/core")));
    assert!(is_vendored_path(
        root,
        &root.join("web/core/lib/Drupal.php")
    ));
    assert!(is_vendored_path(
        root,
        &root.join("web/modules/contrib/views/views.module")
    ));
    assert!(is_vendored_path(
        root,
        &root.join("web/themes/contrib/bootstrap5/bootstrap5.theme")
    ));
    assert!(is_vendored_path(
        root,
        &root.join("web/profiles/contrib/standard/standard.profile")
    ));
    assert!(is_vendored_path(
        root,
        &root.join("web/libraries/jquery/jquery.min.js")
    ));
    assert!(is_vendored_path(
        root,
        &root.join("web/sites/default/files/uploaded.pdf")
    ));

    // Drupal (docroot layout)
    assert!(is_vendored_path(
        root,
        &root.join("docroot/core/lib/Drupal.php")
    ));
    assert!(is_vendored_path(
        root,
        &root.join("docroot/modules/contrib/views/views.module")
    ));

    // WordPress
    assert!(is_vendored_path(root, &root.join("wp-admin/admin.php")));
    assert!(is_vendored_path(
        root,
        &root.join("wp-includes/functions.php")
    ));
    assert!(is_vendored_path(
        root,
        &root.join("wp-content/uploads/2026/launch-hero.png")
    ));
}

#[test]
fn is_vendored_path_keeps_user_code() {
    let root = Path::new("/projects/drupal-site");

    // Drupal - user's custom modules, themes, profiles, and settings
    assert!(!is_vendored_path(
        root,
        &root.join("web/modules/custom/my_module/my_module.module")
    ));
    assert!(!is_vendored_path(
        root,
        &root.join("web/themes/custom/my_theme/my_theme.theme")
    ));
    assert!(!is_vendored_path(
        root,
        &root.join("web/profiles/custom/my_profile/my_profile.profile")
    ));
    assert!(!is_vendored_path(
        root,
        &root.join("web/sites/default/settings.php")
    ));

    assert!(!is_vendored_path(
        root,
        &root.join("wp-content/plugins/acme-core/acme-core.php")
    ));
    assert!(!is_vendored_path(
        root,
        &root.join("wp-content/themes/acme-theme/functions.php")
    ));

    // Prefix lookalikes must not match - exact segment boundary only
    assert!(!is_vendored_path(
        root,
        &root.join("web/core_custom/app.php")
    ));
    assert!(!is_vendored_path(
        root,
        &root.join("web/modules/contrib_custom/x.module")
    ));

    // Top-level user code
    assert!(!is_vendored_path(root, &root.join("src/app.ts")));
    assert!(!is_vendored_path(root, &root.join("scripts/deploy.sh")));
}

#[test]
fn should_skip_walker_dir_respects_disabled_suffix() {
    let root = Path::new("/projects/drupal-site");

    assert!(should_skip_walker_dir(
        root,
        &root.join("web/modules/custom/geo_optimizer.disabled"),
        "geo_optimizer.disabled",
    ));
    assert!(should_skip_walker_dir(
        root,
        &root.join("web/themes/custom/legacy_theme.disabled"),
        "legacy_theme.disabled",
    ));

    // Active modules are still scanned
    assert!(!should_skip_walker_dir(
        root,
        &root.join("web/modules/custom/geo_optimizer"),
        "geo_optimizer",
    ));
}

#[test]
fn should_skip_walker_dir_combines_all_ignore_rules() {
    let root = Path::new("/projects/app");

    // IGNORED_DIRS rule
    assert!(should_skip_walker_dir(
        root,
        &root.join("node_modules"),
        "node_modules",
    ));
    assert!(should_skip_walker_dir(
        root,
        &root.join(".open-next"),
        ".open-next",
    ));
    // is_vendored_path rule
    assert!(should_skip_walker_dir(root, &root.join("web/core"), "core",));
    // Regular user code passes through
    assert!(!should_skip_walker_dir(
        root,
        &root.join("src/components"),
        "components",
    ));
}

#[test]
fn audit_project_rejects_folder_without_app_source_or_project_markers() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        ".astro/content.d.ts",
        "declare module 'astro:content';",
    );

    let error = audit_project(temp.path()).expect_err("folder should not scan as a clean project");

    assert!(
        error.contains("could not find app source files"),
        "unexpected error: {error}"
    );
}

#[test]
fn drupal_scaffold_files_are_recognized_by_site_location_and_name() {
    assert!(is_drupal_scaffold_file(
        "web/sites/default/default.settings.php"
    ));
    assert!(is_drupal_scaffold_file(
        "web/sites/default/default.services.yml"
    ));
    assert!(is_drupal_scaffold_file(
        "web/sites/example.settings.local.php"
    ));
    assert!(is_drupal_scaffold_file("web/sites/example.sites.php"));
    assert!(is_drupal_scaffold_file(
        "docroot/sites/default/default.settings.php"
    ));

    // The project's own settings, and a same-named file outside sites/, stay in scope.
    assert!(!is_drupal_scaffold_file("web/sites/default/settings.php"));
    assert!(!is_drupal_scaffold_file(
        "web/sites/default/settings.ddev.php"
    ));
    assert!(!is_drupal_scaffold_file("web/sites/default/services.yml"));
    assert!(!is_drupal_scaffold_file("config/default.settings.php"));
}

#[test]
fn drupal_scaffold_settings_file_is_not_analyzed_as_first_party_source() {
    let temp = TempDir::new().unwrap();
    let mut big = String::from("<?php\n// Drupal-style settings documentation.\n");
    for i in 0..920 {
        big.push_str(&format!("// option {}\n", i));
    }
    big.push_str("$databases['default']['default'] = ['driver' => 'mysql'];\n");
    big.push_str("$pdo = new PDO('mysql:host=localhost');\n");
    write_file(
        temp.path(),
        "composer.json",
        r#"{ "name": "acme/site", "require": { "drupal/core-recommended": "^11.0" } }"#,
    );
    write_file(temp.path(), "web/sites/default/default.settings.php", &big);
    // Negative control: the same content in the project's own module is still graded.
    write_file(
        temp.path(),
        "web/modules/custom/acme/src/Service/Big.php",
        &big,
    );

    let report = audit_project(temp.path()).unwrap();
    let scaffold_findings: Vec<&str> = report
        .issues
        .iter()
        .filter(|issue| issue.relative_path == "web/sites/default/default.settings.php")
        .map(|issue| issue.id.as_str())
        .collect();
    assert!(
        scaffold_findings.is_empty(),
        "drupal/core scaffolds default.settings.php into every site, got {:?}",
        scaffold_findings
    );
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.id == "oversized-module:web/modules/custom/acme/src/Service/Big.php"),
        "negative control: first-party module code keeps its size finding, got {:?}",
        report.issues.iter().map(|i| &i.id).collect::<Vec<_>>()
    );
}
