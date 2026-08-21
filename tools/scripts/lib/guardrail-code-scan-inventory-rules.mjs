export function codeScanInventoryFailures(read) {
  const codeScan = read("apps/desktop/src-tauri/src/core/code_scan/mod.rs");
  const safeFs = read("apps/desktop/src-tauri/src/core/safe_fs.rs");
  // The inventory contract spans production code and its sibling test module.
  const filesystem =
    read("apps/desktop/src-tauri/src/core/code_scan/filesystem.rs") +
    read("apps/desktop/src-tauri/src/core/code_scan/filesystem_tests.rs");
  const operations = read("apps/desktop/src-tauri/src/core/code_scan/operations.rs");
  // Config-secret inventory is implemented in the supply-chain submodule.
  const supplyChain =
    read("apps/desktop/src-tauri/src/core/code_scan/supply_chain.rs") +
    read("apps/desktop/src-tauri/src/core/code_scan/supply_chain/config_secrets.rs");
  const inventoryReaders = [
    "apps/desktop/src-tauri/src/core/code_scan/project_inventory.rs",
    "apps/desktop/src-tauri/src/core/code_scan/database_analysis/artifacts.rs",
    "apps/desktop/src-tauri/src/core/code_scan/database_analysis/env_files.rs",
    "apps/desktop/src-tauri/src/core/code_scan/package_inventory/manifests.rs",
    "apps/desktop/src-tauri/src/core/code_scan/operations.rs",
  ].map(read);
  const reusesInventory =
    codeScan.includes("let inventory = collect_project_inventory(root)?;") &&
    codeScan.includes("let manifests = collect_package_manifests(&project_files);") &&
    operations.includes("collect_project_paths(project_files)") &&
    supplyChain.includes("collect_ai_config_files(project_files)") &&
    !operations.includes("collect_package_manifests(root)") &&
    !supplyChain.includes("collect_package_manifests(root)");
  const usesBoundedInventoryReads =
    filesystem.includes("pub(super) fn read_project_file") &&
    safeFs.includes("libc::O_NOFOLLOW") &&
    safeFs.includes("initial_metadata.ino() != opened_metadata.ino()") &&
    filesystem.includes("security_regression_inventory_read_rejects_file_replaced_by_symlink") &&
    inventoryReaders.every(
      (source) =>
        source.includes("read_project_file(") &&
        !source.includes("fs::read(&file.absolute_path)") &&
        !source.includes("fs::read_to_string(&file.absolute_path)"),
    );

  const fixedPathReaders = [
    "apps/desktop/src-tauri/src/core/code_scan/ai_scaffolding.rs",
    "apps/desktop/src-tauri/src/core/code_scan/operations/project_hygiene.rs",
  ].map(read);
  const usesGuardedFixedPathReads =
    filesystem.includes("pub(super) fn read_text_under_root") &&
    filesystem.includes("pub(super) fn read_under_root") &&
    filesystem.includes("security_regression_read_text_under_root_rejects_symlink_escape") &&
    fixedPathReaders.every(
      (source) =>
        source.includes("read_text_under_root(") &&
        !source.includes("fs::read_to_string") &&
        !/fs::read\(/.test(source),
    );

  const projectDetectionSources = [
    "apps/desktop/src-tauri/src/core/project.rs",
    "apps/desktop/src-tauri/src/core/project/ecosystems.rs",
    "apps/desktop/src-tauri/src/core/project/environments.rs",
    "apps/desktop/src-tauri/src/core/project/helpers.rs",
    "apps/desktop/src-tauri/src/ai/code_prompt.rs",
  ].map(read);
  const usesGuardedProjectMetadataReads =
    safeFs.includes("pub(crate) fn read_bounded_file_under_root") &&
    safeFs.includes("pub(crate) fn read_bounded_text_under_root") &&
    projectDetectionSources.some((source) => source.includes("read_bounded_text_under_root")) &&
    projectDetectionSources.every(
      (source) =>
        !source.includes("fs::read_to_string") &&
        !source.includes("std::fs::read_to_string") &&
        !/\bfs::read\(/.test(source) &&
        !/\bstd::fs::read\(/.test(source),
    );

  const failures = [];
  if (!reusesInventory) {
    failures.push(
      "Code Scan analyzers must reuse one bounded project inventory instead of recursively walking the project again.",
    );
  }
  if (!usesBoundedInventoryReads) {
    failures.push(
      "Code Scan inventory readers must use the bounded no-follow helper and reject files replaced by symlinks.",
    );
  }
  if (!usesGuardedFixedPathReads) {
    failures.push(
      "Code Scan readers of fixed project paths (AI scaffolding, project hygiene) must use filesystem::read_text_under_root (symlink-safe, size-bounded), not raw fs::read_to_string.",
    );
  }
  if (!usesGuardedProjectMetadataReads) {
    failures.push(
      "Project detection and AI framework detection must use the shared bounded no-follow reader for repository-controlled metadata.",
    );
  }
  return failures;
}
