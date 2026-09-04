set -euo pipefail
node tools/scripts/check-google-oauth-config.mjs
EPHEMERAL_KEY="$RUNNER_TEMP/sitecmd-updater-ephemeral"
pnpm --filter @sitecmd/desktop exec tauri signer generate \
  --ci --write-keys "$EPHEMERAL_KEY" --password ""
# `tauri build` reads TAURI_SIGNING_PRIVATE_KEY, not the _PATH variant.
export TAURI_SIGNING_PRIVATE_KEY="$EPHEMERAL_KEY"
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD=""
trap 'rm -f "$EPHEMERAL_KEY" "$EPHEMERAL_KEY.pub"' EXIT

# Inject Azure signing only on the Windows leg.
EXTRA_ARGS=()
if [ "$RUST_TARGET" = "x86_64-pc-windows-msvc" ] \
  && [ -n "${AZURE_SIGN_ENDPOINT:-}" ] \
  && [ -n "${AZURE_CLIENT_ID:-}" ]; then
  # Invoke the signer directly; `bash` resolves to WSL on this runner.
  SIGN_CMD="artifact-signing-cli -e ${AZURE_SIGN_ENDPOINT} -a ${AZURE_SIGN_ACCOUNT} -c ${AZURE_SIGN_PROFILE} %1"
  EXTRA_ARGS+=(--config "$(jq -nc --arg cmd "$SIGN_CMD" '{bundle:{windows:{signCommand:$cmd}}}')")
  echo "Windows code signing: ENABLED (Azure Artifact Signing)"
elif [ "$RUST_TARGET" = "x86_64-pc-windows-msvc" ]; then
  echo "::error::Windows code signing is required for a production release."
  exit 1
fi
# Stage macOS signing material in a temporary keychain.
if [[ "$RUST_TARGET" == *apple-darwin ]] && [ -n "${APPLE_CERTIFICATE:-}" ]; then
  KC="$RUNNER_TEMP/sitecmd-signing.keychain-db"
  printf '%s' "$APPLE_CERTIFICATE" | tr -d '\r' | base64 --decode > "$RUNNER_TEMP/cert.p12"
  security create-keychain -p "$KEYCHAIN_PASSWORD" "$KC"
  security set-keychain-settings -lut 21600 "$KC"
  security unlock-keychain -p "$KEYCHAIN_PASSWORD" "$KC"
  security import "$RUNNER_TEMP/cert.p12" -k "$KC" -P "$APPLE_CERTIFICATE_PASSWORD" -T /usr/bin/codesign
  security set-key-partition-list -S apple-tool:,apple:,codesign: -s -k "$KEYCHAIN_PASSWORD" "$KC" >/dev/null
  security list-keychains -d user -s "$KC"
  security default-keychain -s "$KC"
  printf '%s' "$APPLE_API_KEY_P8_B64" | tr -d '\r' | base64 --decode > "$APPLE_API_KEY_PATH"
  chmod 600 "$APPLE_API_KEY_PATH"
  rm -f "$RUNNER_TEMP/cert.p12"
  echo "macOS code signing: ENABLED (Developer ID + notarization)"
  security find-identity -v -p codesigning "$KC"
elif [[ "$RUST_TARGET" == *apple-darwin ]]; then
  echo "::error::macOS signing and notarization are required for a production release."
  exit 1
fi
pnpm --filter @sitecmd/desktop exec tauri build --target "$RUST_TARGET" --bundles "$BUNDLES" ${EXTRA_ARGS[@]+"${EXTRA_ARGS[@]}"}
