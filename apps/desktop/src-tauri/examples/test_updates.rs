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
        "\n📦 Detected {} packages across {} ecosystems:",
        packages.len(),
        ecosystems.len()
    );
    for eco in &ecosystems {
        let count = packages.iter().filter(|p| p.ecosystem == *eco).count();
        println!("  {} - {} packages", eco.label(), count);
    }

    println!("\n🔍 Checking registries + OSV for updates...\n");
    let start = std::time::Instant::now();
    let scan = app_lib::updates::registry::check_for_updates(&packages).await;
    let elapsed = start.elapsed();

    if !scan.install_script_packages.is_empty() {
        println!(
            "⚙️  {} npm packages run install scripts",
            scan.install_script_packages.len()
        );
    }
    println!(
        "📜 {} npm packages report a license posture\n",
        scan.licenses.len()
    );
    let updates = scan.updates;

    let security_count = updates.iter().filter(|u| u.is_security).count();
    let regular_count = updates.len() - security_count;

    if updates.is_empty() {
        println!("✅ All packages are up to date! No known vulnerabilities.");
    } else {
        println!(
            "📋 {} updates available ({:.1}s):\n",
            updates.len(),
            elapsed.as_secs_f64()
        );

        if security_count > 0 {
            println!("🛡️  SECURITY ({}):", security_count);
            for u in updates.iter().filter(|u| u.is_security) {
                let sev = u.advisory_severity.as_deref().unwrap_or("?");
                println!(
                    "  🔴 [{}] {} {} → {} ({})",
                    sev.to_uppercase(),
                    u.name,
                    u.current_version,
                    u.latest_version,
                    u.advisory_url.as_deref().unwrap_or("no url"),
                );
            }
            println!();
        }

        if regular_count > 0 {
            println!("📦 UPDATES ({}):", regular_count);
            for u in updates.iter().filter(|u| !u.is_security) {
                let icon = match u.update_type {
                    app_lib::updates::types::UpdateType::Major => "🟡",
                    app_lib::updates::types::UpdateType::Minor => "🔵",
                    app_lib::updates::types::UpdateType::Patch => "⚪",
                    app_lib::updates::types::UpdateType::Unknown => "⚫",
                };
                println!(
                    "  {} {} {} → {} ({:?}{})",
                    icon,
                    u.name,
                    u.current_version,
                    u.latest_version,
                    u.update_type,
                    if u.is_dev { ", dev" } else { "" }
                );
            }
        }
    }
}
