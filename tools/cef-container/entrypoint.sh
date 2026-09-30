#!/usr/bin/env bash
# Container entrypoint: resolve the CEF branch, build each arch via
# build-cef-codecs.sh, move only the distrib tarball + sha256 to /out.
set -euo pipefail

# Match Cargo.lock and the source patches, not a moving crates.io release.
if [[ -z "${CEF_BRANCH:-}" && -z "${CEF_CHECKOUT:-}" ]]; then
  CEF_BRANCH=7977
  CEF_CHECKOUT=708dc140cbc3286826a8abef89dc23a44ff9ea72
elif [[ -z "${CEF_BRANCH:-}" || -z "${CEF_CHECKOUT:-}" ]]; then
  echo "error: set both CEF_BRANCH and CEF_CHECKOUT when changing the pinned engine" >&2
  exit 2
fi
export CEF_BRANCH CEF_CHECKOUT

# Fresh tree every run — skip full git history (saves tens of GB).
export AUTOMATE_EXTRA="--no-chromium-history"

DIST=/work/chromium/src/cef/binary_distrib
mkdir -p /out
for arch in ${ARCHES:-x64 arm64}; do
  echo ">> ===== building $arch ====="
  CEF_ARCH=$arch /opt/karere/build-cef-codecs.sh /work
  # Subshell + `|| true` so the unmatched-extension glob doesn't trip pipefail.
  # Match the _minimal archive explicitly: `ls -t` over both archives can pick
  # the full one, and then the _minimal prefix-strip below matches nothing.
  tarball=$( (ls -t "$DIST"/cef_binary_*_linux*_minimal.tar.bz2 "$DIST"/cef_binary_*_linux*_minimal.zip 2>/dev/null || true) | head -1)
  [[ -n "$tarball" ]] || { echo "error: no distrib archive found in $DIST" >&2; exit 1; }
  # Ship the full + minimal pair for this arch (basename without extension prefix-matches both).
  for f in "${tarball%_minimal.*}"*.zip "${tarball%_minimal.*}"*.tar.bz2; do
    [[ -e "$f" ]] || continue
    (cd "$DIST" && sha256sum "$(basename "$f")" > "$f.sha256")
    mv "$f" "$f.sha256" /out/
  done
  # Both arches' out dirs (~100 GB each) don't fit a ~140 GB volume: free this one.
  rm -rf "/work/chromium/src/out/Release_GN_$arch"
done

echo ">> done — results in /out:"
ls -l /out
cat /out/*.sha256
