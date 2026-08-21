fn main() {
    let payload = serde_json::json!({
        "axe_core_version": sitecmd_engine::browser::AXE_CORE_VERSION,
        "axe_run_script": sitecmd_engine::browser::axe_run_script(
            sitecmd_engine::browser::AxeEvidenceCaps::DEFAULT,
        ),
    });
    print!("{payload}");
}
