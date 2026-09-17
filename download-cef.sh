#!/usr/bin/env bash
# Download + extract CEF Linux binary distribution into ./cef-binaries/
# Version must match the 'cef' crate's bundled CEF version (152.0.6 / chromium 152.0.7977.83 at time of writing).
# See Cargo.toml / Cargo.lock (cef crate) and packaging/io.github.tobagin.karere*.yml (cef-binaries URLs) for the sources of truth.
set -euo pipefail

CEF_VERSION="${CEF_VERSION:-152.0.6+g708dc14+chromium-152.0.7977.83}"
ARCH="$(uname -m)"

case "$ARCH" in
    x86_64) CEF_PLATFORM="linux64" ;;
    aarch64|arm64) CEF_PLATFORM="linuxarm64" ;;
    *) echo "unsupported arch: $ARCH" >&2; exit 1 ;;
esac

URL_TAG="$(echo "$CEF_VERSION" | sed 's/+/%2B/g')"
TARBALL="cef_binary_${URL_TAG}_${CEF_PLATFORM}_minimal.tar.bz2"
URL="https://cef-builds.spotifycdn.com/${TARBALL}"

OUT_DIR="${OUT_DIR:-cef-binaries}"
mkdir -p "$OUT_DIR"
cd "$OUT_DIR"

if [ ! -f "$TARBALL" ]; then
    echo "fetch $URL"
    curl -fL -o "$TARBALL" "$URL"
fi

echo "extract"
tar -xjf "$TARBALL"

EXTRACTED_DIR="cef_binary_${CEF_VERSION}_${CEF_PLATFORM}_minimal"
ln -sfn "$EXTRACTED_DIR" current

echo "done. set CEF_PATH=$(pwd)/current/Release"
echo "and copy *.pak, locales/, icudtl.dat, *.bin, libcef.so, libEGL.so, libGLESv2.so, libvk_swiftshader.so, libvulkan.so.1, vk_swiftshader_icd.json next to the binary at runtime."
