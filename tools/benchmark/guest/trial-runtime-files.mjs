const protectionFiles = new Set([
  ".env",
  ".env.local",
  ".env.development",
  ".env.development.local",
  ".env.production",
  ".env.production.local",
  ".env.test",
  ".env.test.local",
  ".gitmodules",
  ".npmrc",
  ".yarnrc",
  ".yarnrc.yml",
  "bunfig.toml",
  "package.json",
  "package-lock.json",
  "pnpm-lock.yaml",
  "yarn.lock",
]);

export function captureRuntimeFiles(original, snapshot) {
  if (snapshot.violations.length) throw new Error("Unsafe client initialization files");
  for (const [name, contents] of Object.entries(original))
    if (!snapshot.files[name] || !Buffer.from(contents).equals(snapshot.files[name]))
      throw new Error(`Source changed during client initialization: ${name}`);
  const additions = Object.keys(snapshot.files).filter((name) => !Object.hasOwn(original, name));
  for (const name of additions)
    if (!protectionFiles.has(name) || snapshot.files[name].length !== 0)
      throw new Error(`Unexpected client initialization file: ${name}`);
  return additions.sort();
}

export function withoutRuntimeFiles(files, runtime) {
  return Object.fromEntries(
    Object.entries(files).filter(
      ([name, contents]) => !runtime.includes(name) || contents.length > 0,
    ),
  );
}
