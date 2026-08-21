set -euo pipefail
SRC="$GITHUB_WORKSPACE/apps/desktop/src-tauri"
BUNDLE="$SRC/target/${RUST_TARGET}/release/bundle"
APP=$(ls -d "$BUNDLE"/macos/*.app | head -1)
BG="$SRC/branding/dmg-background.tiff"
VOLICON="$SRC/icons/icon.icns"

python3 -m venv "$RUNNER_TEMP/dmgvenv"
# Hash-pin tooling that processes the signed artifact.
"$RUNNER_TEMP/dmgvenv/bin/pip" install --quiet --disable-pip-version-check \
  --require-hashes -r "$SRC/branding/dmgbuild-requirements.txt"

mkdir -p "$BUNDLE/dmg"
rm -f "$BUNDLE"/dmg/*.dmg
OUT="$BUNDLE/dmg/SiteCMD_universal.dmg"
"$RUNNER_TEMP/dmgvenv/bin/dmgbuild" \
  -s "$SRC/branding/dmgbuild_settings.py" \
  -D app="$APP" \
  -D background="$BG" \
  -D volicon="$VOLICON" \
  SiteCMD "$OUT"
echo "styled DMG built: $OUT"

# Sign, notarize, and staple the DMG separately from its app.
if [ -n "${APPLE_CERTIFICATE:-}" ]; then
  security unlock-keychain -p "$KEYCHAIN_PASSWORD" "$RUNNER_TEMP/sitecmd-signing.keychain-db"
  codesign --force --sign "$APPLE_SIGNING_IDENTITY" --timestamp "$OUT"
  xcrun notarytool submit "$OUT" \
    --key "$RUNNER_TEMP/asc_api_key.p8" \
    --key-id "$APPLE_API_KEY_ID" \
    --issuer "$APPLE_API_ISSUER" \
    --wait
  xcrun stapler staple "$OUT"
  xcrun stapler validate "$OUT"
  spctl -a -vv -t open --context context:primary-signature "$OUT" 2>&1 | sed -n '1,3p'
else
  echo "::warning::APPLE_CERTIFICATE unset; shipping styled but unsigned/un-notarized DMG."
fi
