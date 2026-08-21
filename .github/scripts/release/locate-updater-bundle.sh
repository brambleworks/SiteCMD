set -euo pipefail
VERSION="${GITHUB_REF_NAME#v}"
BUNDLE="apps/desktop/src-tauri/target/${RUST_TARGET}/release/bundle"
# macOS wraps its app directory; other platforms sign installers directly.
case "$TARGET" in
  darwin-*)  PATTERN="$BUNDLE/macos/*.app.tar.gz" ;;
  linux-*)   PATTERN="$BUNDLE/appimage/*.AppImage" ;;
  windows-*) PATTERN="$BUNDLE/nsis/*-setup.exe" ;;
  *) echo "unknown target: $TARGET" >&2; exit 1 ;;
esac
ARTIFACT=$(ls $PATTERN | head -1)
SIG_FILE="${ARTIFACT}.sig"
if [ ! -f "$SIG_FILE" ]; then
  echo "missing signature file: $SIG_FILE" >&2
  exit 1
fi

# Mark the macOS updater archive as universal.
ARCH_SUFFIX=""
case "$TARGET" in
  darwin-universal) ARCH_SUFFIX="universal" ;;
esac
if [ -n "$ARCH_SUFFIX" ]; then
  DIR=$(dirname "$ARTIFACT")
  BASE=$(basename "$ARTIFACT" .app.tar.gz)
  NEW_NAME="${BASE}_${ARCH_SUFFIX}.app.tar.gz"
  mv "$ARTIFACT" "$DIR/$NEW_NAME"
  mv "$SIG_FILE" "$DIR/$NEW_NAME.sig"
  ARTIFACT="$DIR/$NEW_NAME"
  SIG_FILE="$DIR/$NEW_NAME.sig"
fi

ARTIFACT_NAME=$(basename "$ARTIFACT")

# Ship the DMG for installation and the app archive for updates.
DMG=""
DMG_NAME=""
if [[ "$TARGET" == darwin-* ]]; then
  DMG=$(ls "$BUNDLE"/dmg/*.dmg | head -1)
  DMG_DIR=$(dirname "$DMG")
  DMG_NAME="SiteCMD_${VERSION}_universal.dmg"
  if [ "$DMG" != "$DMG_DIR/$DMG_NAME" ]; then
    mv "$DMG" "$DMG_DIR/$DMG_NAME"
    DMG="$DMG_DIR/$DMG_NAME"
  fi
fi

{
  echo "artifact=$ARTIFACT"
  echo "artifact_name=$ARTIFACT_NAME"
  echo "sig_file=$SIG_FILE"
  echo "version=$VERSION"
  echo "dmg=$DMG"
  echo "dmg_name=$DMG_NAME"
} >> "$GITHUB_OUTPUT"
