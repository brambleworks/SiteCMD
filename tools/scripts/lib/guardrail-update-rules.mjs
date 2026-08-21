export function desktopUpdateCommandFailures(read, sourceFiles) {
  const failures = [];
  const updateCommandsSource = read("apps/desktop/src/components/dashboard/update-commands.ts");
  const inlineUpdateCommandFiles = sourceFiles.filter((file) => {
    if (!file.startsWith("apps/desktop/src/components/") || !/\.(ts|tsx)$/.test(file)) {
      return false;
    }
    if (file === "apps/desktop/src/components/dashboard/update-commands.ts") return false;
    if (file.includes(".test.")) return false;
    const source = read(file);
    return /npm install \$\{|composer require \$\{|pip install \$\{|go get \$\{|cargo update -p \$\{|wp plugin update \$\{/.test(
      source,
    );
  });

  if (
    !updateCommandsSource.includes("export function buildCommand") ||
    inlineUpdateCommandFiles.length > 0
  ) {
    failures.push(
      `Desktop package-update command strings must use components/dashboard/update-commands.ts buildCommand instead of local ecosystem maps: ${inlineUpdateCommandFiles.join(", ")}`,
    );
  }

  const dependencyParserFiles = [
    "apps/desktop/src-tauri/src/updates/npm.rs",
    // Apply the bounded-read guarantee to lockfile and workspace parsers.
    "apps/desktop/src-tauri/src/updates/npm_lockfiles.rs",
    "apps/desktop/src-tauri/src/updates/npm_workspaces.rs",
    "apps/desktop/src-tauri/src/updates/composer.rs",
    "apps/desktop/src-tauri/src/updates/golang.rs",
    "apps/desktop/src-tauri/src/updates/python.rs",
    "apps/desktop/src-tauri/src/updates/ruby.rs",
    "apps/desktop/src-tauri/src/updates/rust_crates.rs",
    "apps/desktop/src-tauri/src/updates/wordpress.rs",
    "apps/desktop/src-tauri/src/updates/drupal.rs",
    "apps/desktop/src-tauri/src/core/code_scan/package_inventory/registry.rs",
  ];
  const unboundedDependencyReaders = dependencyParserFiles.filter((file) =>
    read(file).includes("read_to_string"),
  );
  if (
    !read("apps/desktop/src-tauri/src/updates/mod.rs").includes(
      "pub(crate) fn read_dependency_file",
    ) ||
    unboundedDependencyReaders.length > 0
  ) {
    failures.push(
      `Dependency manifests and lockfiles must use updates::read_dependency_file so project-controlled files stay within the shared byte budget: ${unboundedDependencyReaders.join(", ")}`,
    );
  }

  return failures;
}
