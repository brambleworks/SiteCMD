# Updater Signing Key Incident Procedure

**Status:** Supported procedure for the current single-key updater.

SiteCMD embeds one minisign public key in
`apps/desktop/src-tauri/tauri.conf.json`. A running build accepts updater
artifacts signed by the matching private key and no other key. The application
cannot add or revoke updater trust roots dynamically.

That boundary has two consequences:

- Deleting `TAURI_SIGNING_PRIVATE_KEY` from GitHub stops the official workflow
  from using it, but does not revoke a copy held elsewhere.
- After a suspected private-key compromise, existing installations cannot be
  moved safely to a new key through the in-app updater. Recovery requires a
  fresh installer authenticated through channels independent of the old key.

## Suspected compromise

1. Freeze releases and disable the production updater manifest.
2. Remove the signing secret from the `release-updater-signing` GitHub
   environment. Preserve audit logs and record the time and suspected exposure.
3. Rotate any credentials that could have exposed the key, including release
   environment access and maintainer sessions.
4. Generate a new keypair offline on a clean machine:

   ```bash
   pnpm --filter @sitecmd/desktop exec tauri signer generate \
     --write-keys ./sitecmd-signing-NEW \
     --password "<long-random-password>"
   ```

5. Store the new encrypted private key and password in the protected
   `release-updater-signing` environment. Replace `plugins.updater.pubkey` with
   the new public key in a reviewed pull request.
6. Cut a release through the normal workflow. The release must pass the
   updater-key probe, secretless artifact verification, platform signing, and
   CLI signature verification before publication.
7. Publish a security notice and fresh installer through separately
   authenticated channels. Use the HTTPS download site, a signed source tag,
   and Apple or Windows platform signatures where available. Publish the new
   minisign key fingerprint through more than one channel for Linux users.
8. Tell affected users that automatic update is not a recovery path. They must
   download and install the new release manually.

Do not publish an unsigned updater, and do not use a suspected key to authorize
a replacement trust root.

## Routine rotation

Routine key rotation without requiring reinstalls is not supported. Keep the
current key while it remains trusted.

Before adding routine rotation, implement and test a real multi-key or
independent-root trust design in the application. The acceptance test must
start from an already released build, cross the transition using only shipped
update behavior, reject the retired key afterward, and cover users who skipped
the transition release. Until that exists, changing the embedded key creates an
update cliff for every build carrying the previous key.

## Recovery verification

For every recovery release:

- Confirm the checked-in public key matches the new private key using the
  release workflow's `validate-updater-key` job.
- Verify updater and CLI signatures in the secretless `verify-release` job.
- Install the fresh package on each supported platform and confirm the app can
  discover and verify a later test release signed by the same new key.
- Confirm a package signed by the retired key is rejected.
- Confirm the public download page, release metadata, and published key
  fingerprint all name the same release.

## Key record

- `FF14638274D8CCED` is the current updater signing-key identifier.

Record every future key change here with the old and new key identifiers, the
first release carrying the new key, the reason for the change, and whether
existing installations require manual recovery.
