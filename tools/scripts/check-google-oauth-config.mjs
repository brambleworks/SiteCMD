const required = ["GOOGLE_CLIENT_ID", "GOOGLE_CLIENT_SECRET"];
const missing = required.filter((name) => !process.env[name]?.trim());

if (missing.length > 0) {
  console.error(
    `Release Google OAuth configuration is missing: ${missing.join(", ")}. Set both secrets from the same Desktop OAuth client in the release-signing environment.`,
  );
  process.exitCode = 1;
}
