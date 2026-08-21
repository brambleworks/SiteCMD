//! sitecmd fix command
//!
//! Reads the last scan result from `.sitecmd/last-scan.json` and outputs
//! AI-ready fix prompts for failing issues, ranked by score impact.

use crate::cli::impact::{self, Applicability};
use crate::core::scanner::ScanResult;

pub struct FixArgs {
    pub all: bool,
    pub id: Option<String>,
    pub type_filter: Option<String>,
    pub category: Option<String>,
}

/// Run the fix subcommand.
///
/// Reads `.sitecmd/last-scan.json`, ranks issues by impact, applies any
/// filters, and prints AI-ready fix prompts to stdout.
pub async fn run(args: FixArgs) -> Result<(), String> {
    let sitecmd_dir = crate::cli::find_config_dir()
        .ok_or_else(|| "No .sitecmd/ directory found. Run `sitecmd init` first.".to_string())?;

    let last_scan_path = sitecmd_dir.join("last-scan.json");
    let contents = std::fs::read_to_string(&last_scan_path)
        .map_err(|_| "No scan data. Run `sitecmd scan` first.".to_string())?;

    let scan: ScanResult = serde_json::from_str(&contents)
        .map_err(|e| format!("Failed to parse last-scan.json: {}", e))?;

    let ranked = impact::rank_issues(&scan.issues, scan.detected_stack.as_ref());

    if ranked.is_empty() {
        println!("No issues found. Score: {}/100.", scan.overall_score);
        return Ok(());
    }

    let url = &scan.url;
    let detected_stack = scan.detected_stack.as_ref();

    if let Some(ref check_id) = args.id {
        let found = ranked.iter().find(|r| r.issue.check_id == *check_id);

        let ri = found.ok_or_else(|| {
            format!(
                "Issue '{}' not found in failing checks. Run `sitecmd scan` to see current issues.",
                check_id
            )
        })?;

        let prompt = crate::ai::build_fix_prompt(ri.issue, url, detected_stack);
        println!("{}", prompt);
        println!("\n---\nFix applied? Run `sitecmd scan --diff` to verify.");
        return Ok(());
    }

    let type_filter: Option<Applicability> = match args.type_filter.as_deref() {
        Some("code") => Some(Applicability::Code),
        Some("config") => Some(Applicability::Config),
        Some("content") => Some(Applicability::Content),
        Some(other) => {
            return Err(format!(
                "Unknown --type '{}'. Valid values: code, config, content.",
                other
            ));
        }
        None => None,
    };

    let category_filter: Option<String> = args.category.as_deref().map(|s| s.to_lowercase());

    let filtered: Vec<_> = ranked
        .iter()
        .filter(|r| {
            // type filter
            if let Some(ref tf) = type_filter {
                if &r.applicability != tf {
                    return false;
                }
            }
            // category filter (case-insensitive match on category debug name)
            if let Some(ref cf) = category_filter {
                let cat_name = format!("{:?}", r.issue.category).to_lowercase();
                if !cat_name.contains(cf.as_str()) {
                    return false;
                }
            }
            true
        })
        .collect();

    if filtered.is_empty() {
        println!(
            "No issues match the given filters. Score: {}/100.",
            scan.overall_score
        );
        return Ok(());
    }

    let divider = "=".repeat(72);

    if args.all {
        // Print ALL matching prompts separated by dividers
        for (i, ri) in filtered.iter().enumerate() {
            if i > 0 {
                println!("\n{}\n", divider);
            }
            let prompt = crate::ai::build_fix_prompt(ri.issue, url, detected_stack);
            println!("{}", prompt);
        }
    } else {
        let top = &filtered[0];
        let prompt = crate::ai::build_fix_prompt(top.issue, url, detected_stack);
        println!("{}", prompt);

        let remaining = filtered.len() - 1;
        if remaining > 0 {
            eprintln!(
                "{} more issue{} available. Use `sitecmd fix --all` to see all.",
                remaining,
                if remaining == 1 { "" } else { "s" }
            );
        }
    }

    println!("\n---\nFix applied? Run `sitecmd scan --diff` to verify.");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{run, FixArgs};

    #[tokio::test]
    async fn fix_command_is_not_tier_gated() {
        let original = std::env::var_os("SITECMD_DB_PATH");
        let temp = tempfile::tempdir().expect("temp dir");
        let db_path = temp.path().join("missing-sitecmd.db");
        std::env::set_var("SITECMD_DB_PATH", &db_path);

        let result = run(FixArgs {
            all: false,
            id: None,
            type_filter: None,
            category: None,
        })
        .await;

        if let Some(value) = original {
            std::env::set_var("SITECMD_DB_PATH", value);
        } else {
            std::env::remove_var("SITECMD_DB_PATH");
        }

        let error = result.expect_err("no scan artifacts exist in the temp dir");
        assert!(
            !error.contains("paid plan"),
            "the fix command must not be tier-gated: {error}"
        );
    }
}
