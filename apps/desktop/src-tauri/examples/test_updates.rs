// Quick test: run detect_updates against the SiteCMD project directory
#[tokio::main]
async fn main() {
    let project_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    println!("Scanning: {}", project_dir.display());

    let detection = app_lib::updates::detect_dependencies(project_dir);
    if detection.partial {
        println!("(partial: at least one dependency file was present but unreadable)");
    }
    let packages = detection.packages;
    let ecosystems = app_lib::updates::detected_ecosystems(&packages);

    println!(
        "\nDetected {} packages across {} ecosystems:",
        packages.len(),
        ecosystems.len()
    );
    for eco in &ecosystems {
        let count = packages.iter().filter(|p| p.ecosystem == *eco).count();
        println!("  {} - {} packages", eco.label(), count);
    }

    println!("\nChecking registries + OSV for updates...\n");
    let start = std::time::Instant::now();
    let scan = app_lib::updates::registry::check_for_updates(&packages).await;
    let elapsed = start.elapsed();

    if !scan.install_script_packages.is_empty() {
        println!(
            "{} npm packages run install scripts",
            scan.install_script_packages.len()
        );
    }
    println!(
        "{} npm packages report a license posture\n",
        scan.licenses.len()
    );
    let updates = scan.updates;

    let security_count = updates.iter().filter(|u| u.is_security).count();
    let regular_count = updates.len() - security_count;

    if updates.is_empty() {
        println!("All packages are up to date. No known vulnerabilities.");
    } else {
        println!(
            "{} updates available ({:.1}s):\n",
            updates.len(),
            elapsed.as_secs_f64()
        );

        if security_count > 0 {
            println!("Security ({}):", security_count);
            for u in updates.iter().filter(|u| u.is_security) {
                let sev = u.advisory_severity.as_deref().unwrap_or("unknown");
                println!(
                    "  {:<8} {} {} → {} ({})",
                    sev,
                    u.name,
                    u.current_version,
                    u.latest_version,
                    u.advisory_url.as_deref().unwrap_or("no url"),
                );
            }
            println!();
        }

        if regular_count > 0 {
            println!("Updates ({}):", regular_count);
            for u in updates.iter().filter(|u| !u.is_security) {
                let kind = match u.update_type {
                    app_lib::updates::types::UpdateType::Major => "major",
                    app_lib::updates::types::UpdateType::Minor => "minor",
                    app_lib::updates::types::UpdateType::Patch => "patch",
                    app_lib::updates::types::UpdateType::Unknown => "unknown",
                };
                println!(
                    "  {:<8} {} {} → {}{}",
                    kind,
                    u.name,
                    u.current_version,
                    u.latest_version,
                    if u.is_dev { " (dev)" } else { "" }
                );
            }
        }
    }
}
