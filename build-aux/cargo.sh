#!/bin/sh

export MESON_BUILD_ROOT="$1"
export MESON_SOURCE_ROOT="$2"
export OUTPUT="$3"
shift 3

export CARGO_TARGET_DIR="$MESON_BUILD_ROOT/target"
if [ -z "$CARGO_HOME" ]; then
    export CARGO_HOME="$MESON_BUILD_ROOT/cargo-home"
fi

echo "DEBUG: Cargo target dir: $CARGO_TARGET_DIR"

cargo build --manifest-path "$MESON_SOURCE_ROOT/Cargo.toml" "$@" && \
cp "$CARGO_TARGET_DIR/release/karere" "$OUTPUT"
