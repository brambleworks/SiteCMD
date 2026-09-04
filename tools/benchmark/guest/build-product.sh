#!/bin/bash
set -euo pipefail
test "$(id -un)" = builder
test "$(uname -s)" = Linux
test "$(uname -m)" = aarch64
cd "$1"
test -f pnpm-lock.yaml
test ! -e .env
export CARGO_BUILD_JOBS=2
export CARGO_INCREMENTAL=0
export CARGO_PROFILE_DEV_DEBUG=0
export CARGO_PROFILE_RELEASE_DEBUG=0
git init --quiet --initial-branch=feature/benchmark-build
pnpm install --frozen-lockfile
pnpm tauri:build:contributor --bundles deb
cargo build --locked --release --manifest-path apps/desktop/src-tauri/Cargo.toml -p sitecmd-cli
