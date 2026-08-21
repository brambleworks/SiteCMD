#!/usr/bin/env bash

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
TAURI_TARGET_DIR="$REPO_ROOT/apps/desktop/src-tauri/target"
TAURI_TARGET_MAX_GB="${SITECMD_TAURI_TARGET_MAX_GB:-25}"
TAURI_FORCE_CLEAN="${SITECMD_TAURI_FORCE_CLEAN:-0}"
APP_PID=""

APP_PROCESS_PATTERNS=(
  "tools/scripts/dev\\.sh"
  "$REPO_ROOT/tools/scripts/dev.sh"
  "pnpm tauri:dev"
  "@tauri-apps/cli/.*/tauri\\.js dev"
  "pnpm dev:tauri"
  "$REPO_ROOT/node_modules/.*/vite/bin/vite\\.js --host 127\\.0\\.0\\.1"
  "target/debug/sitecmd"
  "SiteCMD\\.app/Contents/MacOS"
)

kill_matching_processes() {
  local signal="$1"
  shift

  local pattern pid
  for pattern in "$@"; do
    while IFS= read -r pid; do
      [ -n "$pid" ] || continue
      [ "$pid" = "$$" ] && continue
      [ "${PPID:-}" = "$pid" ] && continue
      kill "-$signal" "$pid" 2>/dev/null || true
    done < <(pgrep -f "$pattern" 2>/dev/null || true)
  done
}

stop_app_dev_processes() {
  kill_matching_processes TERM "${APP_PROCESS_PATTERNS[@]}"
  sleep 1
  kill_matching_processes KILL "${APP_PROCESS_PATTERNS[@]}"
}

free_app_ports() {
  for port in 5173 5174; do
    local pids
    pids="$(lsof -ti:"$port" 2>/dev/null || true)"
    if [ -n "$pids" ]; then
      echo "Freeing port $port..."
      echo "$pids" | xargs kill -9 2>/dev/null || true
    fi
  done
}

cleanup_tauri_target_if_needed() {
  [ -d "$TAURI_TARGET_DIR" ] || return 0

  local size_kb
  size_kb="$(du -sk "$TAURI_TARGET_DIR" 2>/dev/null | awk '{print $1}')"
  [ -n "$size_kb" ] || return 0

  local max_kb
  max_kb=$((TAURI_TARGET_MAX_GB * 1024 * 1024))

  if [ "$TAURI_FORCE_CLEAN" = "1" ] || [ "$size_kb" -ge "$max_kb" ]; then
    local size_gb
    size_gb="$(awk "BEGIN { printf \"%.1f\", $size_kb / 1024 / 1024 }")"
    echo "Cleaning Rust build cache at $TAURI_TARGET_DIR (${size_gb} GiB)..."
    echo "Set SITECMD_TAURI_FORCE_CLEAN=1 to force this, or"
    echo "SITECMD_TAURI_TARGET_MAX_GB to change the ${TAURI_TARGET_MAX_GB} GiB threshold."
    (cd "$REPO_ROOT" && cargo clean --manifest-path apps/desktop/src-tauri/Cargo.toml)
  fi
}

load_repo_env_if_present() {
  local env_file="$REPO_ROOT/.env"
  [ -f "$env_file" ] || return 0

  set -a
  set +u
  # shellcheck disable=SC1090
  source "$env_file"
  set -u
  set +a
}

cleanup() {
  trap - EXIT INT TERM

  if [ -n "${APP_PID:-}" ]; then
    kill "$APP_PID" 2>/dev/null || true
  fi

  stop_app_dev_processes
  free_app_ports
}

echo "Stopping existing processes..."

stop_app_dev_processes
free_app_ports

sleep 1

cleanup_tauri_target_if_needed

cd "$REPO_ROOT"
load_repo_env_if_present
trap cleanup EXIT INT TERM

echo "Starting pnpm tauri:dev (desktop app)..."
pnpm tauri:dev &
APP_PID=$!

# Wait for the app rather than `wait`, so the EXIT trap still runs its cleanup
# when this script is interrupted.
while kill -0 "$APP_PID" 2>/dev/null; do
  sleep 1
done
