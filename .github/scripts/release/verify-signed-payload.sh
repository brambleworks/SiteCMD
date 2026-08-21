set -euo pipefail
sha256_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}
test "$(sha256_file release-candidate/manifest.json)" = \
  "$EXPECTED_CANDIDATE_HASH"
dir="payload/$TARGET"
if command -v sha256sum >/dev/null 2>&1; then
  (cd "$dir" && sha256sum -c SHA256SUMS)
else
  (cd "$dir" && shasum -a 256 -c SHA256SUMS)
fi
test "$(jq -r '.target' "$dir/fragment.json")" = "$TARGET"
test "$(jq -r '.candidate_hash' "$dir/fragment.json")" = "$EXPECTED_CANDIDATE_HASH"
test "$(jq -r '.source_commit' "$dir/fragment.json")" = "$EXPECTED_SOURCE_COMMIT"
filename=$(jq -r '.filename' "$dir/fragment.json")
test "$(sha256_file "$dir/$filename")" = \
  "$(jq -r '.artifact_sha256' "$dir/fragment.json")"

# Normalize jq CRLF before GNU base64 decoding.
jq -r '.plugins.updater.pubkey' apps/desktop/src-tauri/tauri.conf.json | \
  tr -d '\r' | base64 --decode > updater-public-key.pub
tr -d '\r' < "$dir/$filename.sig" | base64 --decode > updater-signature.sig
verifier=".github/updater-verifier/target/release/sitecmd-updater-verifier"
if [ -x "${verifier}.exe" ]; then verifier="${verifier}.exe"; fi
"$verifier" updater-public-key.pub "$dir/$filename" updater-signature.sig

cli_archive=$(jq -r '.cli_archive' "$dir/fragment.json")
tr -d '\r' < "$dir/$cli_archive.sig" | base64 --decode > cli-signature.sig
"$verifier" updater-public-key.pub "$dir/$cli_archive" cli-signature.sig
