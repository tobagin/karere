#!/usr/bin/env bash
# Karere v4 (GTK4 + CEF / Rust) — local Flatpak build + install.
#
# v4 fetches CEF as a Flatpak module and builds the Rust app offline from the
# vendored crate manifest (packaging/cargo-sources.json). Regenerate that
# manifest with --regen-sources whenever Cargo.toml / Cargo.lock change.
set -euo pipefail
cd "$(dirname "$0")"

APP_ID="io.github.tobagin.karere"
MANIFEST="packaging/io.github.tobagin.karere.yml"
BUILD_DIR="build-dir"

INSTALL=1
REGEN_SOURCES=0

while [[ $# -gt 0 ]]; do
    case "$1" in
        --no-install)    INSTALL=0; shift ;;
        --regen-sources) REGEN_SOURCES=1; shift ;;
        --help|-h)
            cat <<EOF
Usage: $0 [--regen-sources] [--no-install]

  --regen-sources  Regenerate packaging/cargo-sources.json from Cargo.lock
                   first (run after any Cargo.toml / Cargo.lock change — the
                   Flatpak build is offline and needs every crate vendored).
  --no-install     Build only; do not install the resulting Flatpak.

Default: build the production manifest and install it for the current user.
Run with: flatpak run $APP_ID
EOF
            exit 0 ;;
        *) echo "Unknown option: $1 (see --help)" >&2; exit 1 ;;
    esac
done

if [[ "$REGEN_SOURCES" == 1 ]]; then
    echo "Regenerating $MANIFEST sources from Cargo.lock…"
    python3 tools/flatpak-cargo-generator.py Cargo.lock -o packaging/cargo-sources.json
fi

ARGS=(--force-clean --user)
[[ "$INSTALL" == 1 ]] && ARGS+=(--install)

echo "flatpak-builder ${ARGS[*]} $BUILD_DIR $MANIFEST"
flatpak-builder "${ARGS[@]}" "$BUILD_DIR" "$MANIFEST"

echo "Build complete."
[[ "$INSTALL" == 1 ]] && echo "Run with: flatpak run $APP_ID"
