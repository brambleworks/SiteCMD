import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const packageRoot = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const migrationsPath = join(
  packageRoot,
  "..",
  "desktop",
  "src-tauri",
  "src",
  "db",
  "migrations.rs",
);
const versionPath = join(packageRoot, "src", "version.ts");

export function latestMigrationVersion(source) {
  const versions = [...source.matchAll(/\(\s*(\d+),\s*include_str!\("migrations\//g)].map((match) =>
    Number(match[1]),
  );
  if (versions.length === 0) throw new Error("Could not read the desktop migration registry");
  return Math.max(...versions);
}

export function supportedSchemaRange(source) {
  const match = source.match(
    /SUPPORTED_SCHEMA_VERSIONS\s*=\s*\{\s*min:\s*(\d+),\s*max:\s*(\d+)\s*\}/,
  );
  if (!match) throw new Error("Could not read the MCP schema compatibility range");
  return { min: Number(match[1]), max: Number(match[2]) };
}

export function assertSchemaContract(migrationsSource, versionSource) {
  const latest = latestMigrationVersion(migrationsSource);
  const supported = supportedSchemaRange(versionSource);
  if (supported.min > supported.max) {
    throw new Error(
      `MCP schema compatibility is invalid: minimum ${supported.min} exceeds maximum ${supported.max}`,
    );
  }
  if (supported.max !== latest) {
    throw new Error(
      `MCP schema compatibility is stale: desktop migration ${latest} is registered, but the MCP server supports through ${supported.max}. Review the migration, then update SUPPORTED_SCHEMA_VERSIONS before bundling.`,
    );
  }
  return { latest, supported };
}

export function assertRepositorySchemaContract() {
  return assertSchemaContract(
    readFileSync(migrationsPath, "utf8"),
    readFileSync(versionPath, "utf8"),
  );
}
