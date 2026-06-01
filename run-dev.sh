#!/usr/bin/env bash
# Run the local (cargo) build of Karere with the dev GSettings schema available.
#
# The gschema is normally installed only inside the flatpak. For a bare
# `cargo run` the schema must be compiled into a directory on GSETTINGS_SCHEMA_DIR,
# otherwise `gio::Settings::new(APP_ID)` panics ("no such schema").
set -euo pipefail
cd "$(dirname "$0")"

APP_ID="io.github.tobagin.karere"
APP_PATH="/io/github/tobagin/karere/"
SCHEMA_DIR="${XDG_CACHE_HOME:-$HOME/.cache}/karere-dev-schemas"

mkdir -p "$SCHEMA_DIR"
sed "s/@APP_ID@/$APP_ID/g; s|@APP_PATH@|$APP_PATH|g" \
  data/io.github.tobagin.karere.gschema.xml.in \
  > "$SCHEMA_DIR/$APP_ID.gschema.xml"
glib-compile-schemas "$SCHEMA_DIR"

# Prepend so the dev schema wins over any system install.
export GSETTINGS_SCHEMA_DIR="$SCHEMA_DIR:${GSETTINGS_SCHEMA_DIR:-}"

# Surface our info-level notification logs.
exec cargo run "$@"
