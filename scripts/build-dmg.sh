#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "DMG bundles can only be built on macOS." >&2
  exit 1
fi

if [[ ! -d src-web/node_modules ]]; then
  echo "Missing src-web/node_modules. Run: npm --prefix src-web install" >&2
  exit 1
fi

args=(build --bundles dmg --ci)

if [[ -n "${TAURI_TARGET:-}" ]]; then
  args+=(--target "$TAURI_TARGET")
fi

if [[ "${TAURI_NO_SIGN:-0}" == "1" ]]; then
  args+=(--no-sign)
fi

if [[ "${TAURI_SKIP_STAPLING:-0}" == "1" ]]; then
  args+=(--skip-stapling)
fi

echo "Building Space Lenser DMG..."
src-web/node_modules/.bin/tauri "${args[@]}"

echo
echo "DMG artifact(s):"
find target src-tauri/target -path '*/bundle/dmg/*.dmg' -type f -print 2>/dev/null || true
