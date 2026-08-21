# macOS Code Signing and Notarization (Developer ID)

How SiteCMD Developer-ID-signs and notarizes the macOS app in CI. Distribution is
direct through the in-app updater and R2, not the Mac App Store.

## Summary

The release workflow (`.github/workflows/release.yml`) builds one universal
macOS `.app`, signs it with a **Developer ID Application** certificate under the
hardened runtime, and notarizes it with `notarytool` during `tauri build`. The
certificate is imported into an ephemeral keychain; notarization uses an App
Store Connect API key. Configuration lives in the protected `release-signing`
environment, and no Apple private key is committed.

Official releases fail closed when any required Apple value is missing. Local
development builds may remain unsigned, but the release workflow never ships an
unsigned or unnotarized macOS artifact.

## GitHub secrets

Repository > Settings > Secrets and variables > Actions.

| Secret                       | Value                                                            |
| ---------------------------- | ---------------------------------------------------------------- |
| `APPLE_CERTIFICATE`          | base64 of the Developer ID `.p12` (leaf + key + G2 intermediate) |
| `APPLE_CERTIFICATE_PASSWORD` | password protecting that `.p12`                                  |
| `APPLE_SIGNING_IDENTITY`     | `Developer ID Application: Brambleworks LLC (6ACNUQR5PK)`        |
| `APPLE_TEAM_ID`              | `6ACNUQR5PK`                                                     |
| `KEYCHAIN_PASSWORD`          | ephemeral CI keychain password                                   |
| `APPLE_API_ISSUER`           | App Store Connect API issuer id                                  |
| `APPLE_API_KEY_ID`           | App Store Connect API key id                                     |
| `APPLE_API_KEY_P8`           | base64 of the App Store Connect `.p8`                            |

## How the certificate was created

Apple restricts Developer ID certificate creation to the **Account Holder** - the
App Store Connect API returns `403 "This operation can only be performed by the
Account Holder"`, even for an Admin key. So the cert is created by CSR upload:

1. Generate an RSA 2048 key + CSR locally (`openssl req -new -newkey rsa:2048`).
2. The Account Holder uploads the CSR at
   developer.apple.com > Certificates > **Developer ID Application** (G2 Sub-CA)
   and downloads the `.cer`.
3. Assemble the `.p12` from the `.cer` + the private key + the Developer ID G2
   intermediate:
   `openssl pkcs12 -export -legacy -inkey key.pem -in leaf.pem -certfile intermediate.pem`.
   The **`-legacy` flag is required** - macOS `security import` cannot read the
   AES-256 encryption that OpenSSL 3 uses by default.

Keep any local backup outside the repository in encrypted, access-controlled
storage. The workflow receives only the protected environment copies.

## How it works in CI

- The macOS build legs decode `APPLE_CERTIFICATE` into a temporary keychain
  (`security create-keychain` / `import` / `set-key-partition-list`), decode the
  `.p8`, and export `APPLE_SIGNING_IDENTITY` + the `APPLE_API_*` notarization env.
- `tauri build` then codesigns the `.app` with hardened runtime and a secure
  timestamp, notarizes it via `notarytool`, and staples the ticket. The shipped
  `.app.tar.gz` updater bundle contains the notarized, stapled app.
- The certificate and notarization secrets are matrix-scoped to the `darwin` legs
  only, so they never enter the Windows / Linux runners.
- Tauri's notarization env vars: `APPLE_API_ISSUER` (issuer id), `APPLE_API_KEY`
  (the **Key ID**), `APPLE_API_KEY_PATH` (path to the decoded `.p8`).

This Developer ID signature is independent of the minisign updater signature
(`TAURI_SIGNING_PRIVATE_KEY`): the former is what macOS / Gatekeeper trust on
launch, the latter is what the in-app updater verifies.

## Troubleshooting

### `403. A required agreement is missing or has expired`

```
failed to bundle project: failed codesign application: failed to notarize app:
Error: HTTP status code: 403. A required agreement is missing or has expired.
```

Apple published a new Developer Program agreement (or an existing one lapsed), and
nothing notarizes until the **Account Holder** accepts it. This is account state,
not a build problem: the compile succeeds and the Developer ID signature succeeds.
Only the notarization request is rejected, at the very end of the job.

Fix: sign in to <https://developer.apple.com/account> as the Account Holder and
accept the pending agreement (also check App Store Connect > Business >
Agreements). Then re-run only the failed leg:

```bash
gh run rerun <run-id> --failed
```

Completed Windows and Linux legs retain their workflow artifacts, so a
`--failed` re-run rebuilds only the failed macOS job and its downstream release
jobs. Nothing reaches R2 until secretless verification succeeds. No re-tag is
needed because the tag and commit are unchanged.

Because Tauri only contacts the notary service **after** the roughly 20 minute
universal compile, release.yml runs a `Verify Apple notarization credentials` step
_before_ the build: one authenticated `xcrun notarytool history` call that surfaces
this 403 (and wrong `APPLE_API_KEY_ID` / `APPLE_API_ISSUER` / `APPLE_API_KEY_P8`
values) in seconds, rather than after a macOS job billed at the 10x multiplier.

## Verifying a signed build

Download the macOS `.app.tar.gz`, extract, and on a Mac:

```bash
codesign -dv --verbose=4 SiteCMD.app    # Authority: Developer ID Application: Brambleworks LLC
spctl -a -vvv -t exec SiteCMD.app       # accepted; source=Notarized Developer ID
xcrun stapler validate SiteCMD.app      # "The validate action worked!"
```

## Rotation and expiry

- **Certificate** expires **2027-02-01** (unusually short for Developer ID, which
  is normally 5 years; timestamped signatures already shipped stay valid past
  expiry). To renew, repeat the CSR-upload flow, rebuild the `.p12`, and update
  `APPLE_CERTIFICATE` + `APPLE_CERTIFICATE_PASSWORD`.
- **API key**: regenerate in App Store Connect, then update `APPLE_API_KEY_ID` +
  `APPLE_API_KEY_P8`. The `.p8` is download-once, so retain an encrypted backup
  outside the repository.
