use super::{enforced_project_command_args, validate_project_command_policy};

fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

#[test]
fn project_command_policy_accepts_supported_script_opt_out_forms() {
    for executable in ["npm", "pnpm"] {
        for flags in [
            vec!["--ignore-scripts"],
            vec!["--ignore-scripts=true"],
            vec!["--ignore-scripts=1"],
            vec!["--ignore-scripts", "true"],
            vec!["--ignore-scripts", "--ignore-scripts=true"],
        ] {
            let mut command = vec!["install", "some-package"];
            command.extend(flags);
            assert!(
                validate_project_command_policy(executable, &args(&command)).is_ok(),
                "{executable} {command:?}"
            );
        }
    }
    for executable in ["yarn", "bun"] {
        assert!(validate_project_command_policy(
            executable,
            &args(&["add", "some-package", "--ignore-scripts"])
        )
        .is_ok());
    }
    assert!(validate_project_command_policy(
        "composer",
        &args(&[
            "require",
            "vendor/package",
            "--no-scripts",
            "--no-plugins",
            "--no-scripts"
        ])
    )
    .is_ok());
}

#[test]
fn project_command_policy_rejects_conflicting_script_flags_in_either_order() {
    for executable in ["npm", "pnpm", "yarn", "bun"] {
        for conflicting in [
            vec!["--ignore-scripts=false"],
            vec!["--ignore-scripts=0"],
            vec!["--ignore-scripts=FALSE"],
            vec!["--ignore-scripts", "false"],
            vec!["--ignore-scripts", "0"],
            vec!["--no-ignore-scripts"],
            vec!["--no-ignore-scripts=false"],
        ] {
            for opt_out_first in [true, false] {
                let mut command = vec!["install"];
                if opt_out_first {
                    command.push("--ignore-scripts");
                }
                command.extend(&conflicting);
                if !opt_out_first {
                    command.push("--ignore-scripts");
                }
                assert!(
                    validate_project_command_policy(executable, &args(&command)).is_err(),
                    "{executable} {command:?} must be rejected"
                );
            }
        }
    }
}

#[test]
fn project_command_policy_rejects_option_aliases_that_override_script_flags() {
    for (executable, alias) in [
        ("npm", "--ignore-script=false"),
        ("npm", "--ignore=false"),
        ("npm", "--no-ignore-script"),
        ("npm", "-ignore-scripts=false"),
        ("npm", "--no-no-ignore-scripts"),
        ("pnpm", "--config.ignore-scripts=false"),
        ("pnpm", "--config.ignoreScripts=false"),
        ("pnpm", "--config.no-ignore-scripts"),
        ("pnpm", "--no-config.ignore-scripts"),
    ] {
        let command = args(&["install", "--ignore-scripts", alias]);
        assert!(
            validate_project_command_policy(executable, &command).is_err(),
            "{executable} {command:?} must be rejected"
        );
    }
}

#[test]
fn project_command_policy_rejects_installer_option_terminators() {
    for executable in ["npm", "pnpm", "yarn", "bun"] {
        for command in [
            vec!["install", "--", "--ignore-scripts"],
            vec!["install", "--ignore-scripts", "--", "some-package"],
        ] {
            assert!(validate_project_command_policy(executable, &args(&command)).is_err());
        }
    }
    for command in [
        vec!["install", "--", "--no-scripts", "--no-plugins"],
        vec!["install", "--no-scripts", "--", "--no-plugins"],
        vec!["install", "--no-scripts", "--no-plugins", "--"],
    ] {
        assert!(validate_project_command_policy("composer", &args(&command)).is_err());
    }
}

#[test]
fn project_command_policy_rejects_yarn_mode_only_installs() {
    for flags in [
        vec!["--mode=skip-build"],
        vec!["--mode=skip-builds"],
        vec!["--mode", "skip-build"],
        vec!["--mode=skip-build", "--mode=update-lockfile"],
    ] {
        let mut command = vec!["install"];
        command.extend(flags);
        assert!(validate_project_command_policy("yarn", &args(&command)).is_err());
    }
}

#[test]
fn project_command_policy_requires_bare_flags_for_non_boolean_cli_options() {
    for executable in ["yarn", "bun"] {
        assert!(validate_project_command_policy(
            executable,
            &args(&["install", "--ignore-scripts=true"])
        )
        .is_err());
    }
    for flag in [
        "--no-scripts=false",
        "--no-scripts=true",
        "--scripts",
        "--no-script",
        "--no-no-scripts",
        "--no-plugins=false",
        "--plugins",
    ] {
        let command = args(&["install", "--no-scripts", "--no-plugins", flag]);
        assert!(validate_project_command_policy("composer", &command).is_err());
    }
}

#[test]
fn project_command_policy_rejects_implicit_default_installs() {
    for executable in ["npm", "pnpm", "yarn", "bun", "composer"] {
        assert!(validate_project_command_policy(executable, &[]).is_err());
        assert!(validate_project_command_policy(executable, &args(&[""])).is_err());
    }
}

#[test]
fn project_command_policy_rejects_composer_aliases_and_custom_scripts() {
    for command in [
        "i",
        "ins",
        "u",
        "up",
        "req",
        "rm",
        "run",
        "run-script",
        "exec",
        "test",
        "custom-script",
    ] {
        let error = validate_project_command_policy(
            "composer",
            &args(&[command, "--no-scripts", "--no-plugins"]),
        )
        .expect_err("only canonical dependency commands can be approved");
        assert!(error.contains("not allowed"), "{command}: {error}");
    }
    for command in ["install", "update", "require", "remove"] {
        assert!(validate_project_command_policy(
            "composer",
            &args(&[command, "--no-scripts", "--no-plugins"]),
        )
        .is_ok());
    }
}

#[test]
fn project_command_policy_enforces_canonical_flags_around_user_options() {
    for executable in ["npm", "pnpm", "yarn", "bun"] {
        let prepared = enforced_project_command_args(
            executable,
            args(&["install", "--ignore-scripts", "--prefix"]),
        )
        .expect("valid opt-out");
        assert_eq!(prepared[0], "install");
        assert_eq!(prepared[1], "--ignore-scripts");
        assert_eq!(
            prepared.last().map(String::as_str),
            Some("--ignore-scripts")
        );
    }
    let prepared = enforced_project_command_args(
        "composer",
        args(&["install", "--no-scripts", "--no-plugins", "--working-dir"]),
    )
    .expect("valid Composer opt-outs");
    assert_eq!(&prepared[1..3], &["--no-scripts", "--no-plugins"]);
    assert_eq!(
        &prepared[prepared.len() - 2..],
        &["--no-scripts", "--no-plugins"]
    );
}

#[test]
fn project_command_policy_revalidates_before_constructing_process_arguments() {
    assert!(enforced_project_command_args(
        "npm",
        args(&["install", "--ignore-scripts", "--ignore-scripts=false"])
    )
    .is_err());
    let command = args(&["check", "--all-targets"]);
    assert_eq!(
        enforced_project_command_args("cargo", command.clone()).expect("safe Cargo command"),
        command
    );
}
