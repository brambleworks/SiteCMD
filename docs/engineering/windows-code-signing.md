# Windows Code Signing (Azure Artifact Signing)

How SiteCMD Authenticode-signs the Windows desktop installer in CI. macOS
Developer ID signing + notarization is the separate, live path documented in
`macos-code-signing.md`.

## Summary

The release workflow (`.github/workflows/release.yml`) signs the Windows app
executable and NSIS installer during `tauri build`, using Tauri's
`bundle.windows.signCommand` driving [`artifact-signing-cli`] against an Azure
Artifact Signing certificate profile. Authentication uses a Microsoft Entra
service principal. All configuration lives in GitHub repository settings; no
Azure identifiers are committed to the repo.

Official releases fail closed if any setting below is absent. Local development
builds may remain unsigned, but the release workflow never publishes an unsigned
Windows executable or installer.

> Azure renamed "Trusted Signing" to "Artifact Signing" in 2026. The CLI
> follows: `artifact-signing-cli` is current; `trusted-signing-cli` is
> deprecated. Some portal labels and RBAC role names may still read "Trusted
> Signing".

## One-time Azure setup

1. **Account** - Obtain an approved Trusted/Artifact Signing account. Identity
   validation is normally the longest setup step.
2. **Certificate profile** - In the account, create a certificate profile of
   type **Public Trust** (required for public distribution). Note its name.
3. **Service principal** - In Microsoft Entra ID > App registrations > New
   registration, create an app for CI. Under Certificates & secrets, create a
   client secret. Record the **Directory (tenant) ID**, **Application (client)
   ID**, and the **client secret value** (shown once).
4. **Role assignment** - On the signing account (Access control / IAM > Add role
   assignment), grant that service principal the **Trusted Signing Certificate
   Profile Signer** role (the Artifact Signing equivalent after the rename).
5. **Endpoint** - From the account overview, note the regional URI, e.g.
   `https://eus.codesigning.azure.net`.

## GitHub configuration

Repository > Settings > Secrets and variables > Actions.

**Secrets** (sensitive):

| Name                  | Value                   |
| --------------------- | ----------------------- |
| `AZURE_TENANT_ID`     | Directory (tenant) ID   |
| `AZURE_CLIENT_ID`     | Application (client) ID |
| `AZURE_CLIENT_SECRET` | client secret value     |

**Variables** (non-secret identifiers):

| Name                  | Value                                                |
| --------------------- | ---------------------------------------------------- |
| `AZURE_SIGN_ENDPOINT` | region URI, e.g. `https://eus.codesigning.azure.net` |
| `AZURE_SIGN_ACCOUNT`  | signing account name                                 |
| `AZURE_SIGN_PROFILE`  | certificate profile name                             |

Set all six. The next `v*` release tag produces a signed Windows installer.

## How it works in CI

- The release gate requires `AZURE_SIGN_ENDPOINT`; the Windows matrix leg then
  installs the pinned `artifact-signing-cli` with `--locked`.
- The build step assembles a `bundle.windows.signCommand` overlay and passes it
  via `tauri build --config`. Tauri then calls the CLI once per artifact (the
  app `.exe` and the NSIS `-setup.exe`), substituting each file path for `%1`.
- Tauri runs `signCommand` without a shell and hides the CLI's stderr, so a
  failed sign only surfaces as the opaque `failed to bundle project 'failed to
run artifact-signing-cli'` deep into the build. Do NOT try to fix this by
  wrapping the CLI in `bash` from the signCommand: a bare `bash` on the runner
  resolves to WSL (no distro), so Tauri reports `failed to run bash` and never
  reaches the CLI. Instead, a dedicated **Verify Azure signing** step runs the
  CLI directly in the job's shell (where stdout AND stderr are visible) against
  a throwaway copy of a system exe, before the long build. A broken
  credential / role / profile fails there in about a minute with the real Azure
  error, instead of wasting a ~45-minute build.
- The service-principal secrets are matrix-scoped to the Windows leg only, so
  they never enter the macOS/Linux runners (defense-in-depth, mirroring the
  license-env narrowing from the security audit).
- Auth is via the standard `AZURE_TENANT_ID` / `AZURE_CLIENT_ID` /
  `AZURE_CLIENT_SECRET` environment variables read by the CLI's Azure
  credential.

This Authenticode signature is independent of the minisign updater signature
(`TAURI_SIGNING_PRIVATE_KEY`): the former is what Windows / SmartScreen trust on
install, the latter is what the in-app updater verifies.

## Verifying a signed build

Download the `-setup.exe` artifact and, on Windows:

```powershell
Get-AuthenticodeSignature .\SiteCMD_<version>_x64-setup.exe | Format-List
```

Status should be `Valid` with the signer chaining to the Microsoft-managed
publisher identity. Or: right-click > Properties > Digital Signatures.

SmartScreen reputation accrues with download volume even with a valid
certificate, so early downloads may still show a warning that fades over time.

## Troubleshooting: `failed to run artifact-signing-cli`

Tauri reports this when the sign command exits non-zero, with the CLI's real
error hidden. The **Verify Azure signing** step earlier in the job surfaces that
error directly (it runs the CLI in the job's own shell before the build); read
its output first. A sign that fails in under a second (versus ~10s for a
successful network sign) died at Azure authentication, before submitting
anything, which points at one of:

- An expired or rotated `AZURE_CLIENT_SECRET`. Entra client secrets have an
  expiry; rotate as in Notes below.
- The service principal lost its Trusted Signing role (`Trusted Signing
Certificate Profile Signer`) on the account, or the certificate profile is no
  longer in a usable state.
- A transient Azure outage or throttle at the token or signing endpoint. A plain
  job re-run distinguishes this from the above two.

The signing config is byte-identical across tags, so when a tag fails to sign
after a previous tag signed cleanly, the cause is on the Azure side, not in the
repo.

## Notes

- Cost: ~$9.99/month for the signing account plus a negligible per-signature
  charge.
- Rotating the client secret: create a new secret in Entra, update
  `AZURE_CLIENT_SECRET`, remove the old one. No code change required.
- The deprecated `trusted-signing-cli` still works but should not be
  reintroduced; track `artifact-signing-cli` releases for the pinned version.

[`artifact-signing-cli`]: https://crates.io/crates/artifact-signing-cli
