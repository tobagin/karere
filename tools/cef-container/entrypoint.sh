#!/usr/bin/env bash
# Container entrypoint: resolve the CEF branch, build each arch via
# build-cef-codecs.sh, move only the distrib tarball + sha256 to /out.
set -euo pipefail

# Default branch = whatever the latest Rust `cef` crate binds against
# (its +N.N.N build metadata → chromium branch, 3rd version component).
if [[ -z "${CEF_BRANCH:-}" ]]; then
  echo ">> resolving CEF branch from the latest cef crate…"
  CEF_VER=$(curl -fsSL -A cef-container https://crates.io/api/v1/crates/cef \
    | python3 -c 'import json,sys; print(json.load(sys.stdin)["crate"]["max_version"].split("+")[1])')
  # "150.0.10+g8042e43+chromium-150.0.7871.101" → branch 7871, checkout 8042e43.
  # The exact commit matters: the binaries must match the crate's pinned CEF
  # version, and branch tip can be a release ahead.
  read -r CEF_BRANCH CEF_CHECKOUT < <(curl -fsSL https://cef-builds.spotifycdn.com/index.json \
    | python3 -c "
import json, sys
d = json.load(sys.stdin)
v = next(x for x in d['linux64']['versions'] if x['cef_version'].startswith('$CEF_VER+'))
print(v['chromium_version'].split('.')[2], v['cef_version'].split('+')[1].lstrip('g'))")
  export CEF_CHECKOUT
  echo ">> cef crate $CEF_VER -> chromium branch $CEF_BRANCH, cef commit $CEF_CHECKOUT"
fi
export CEF_BRANCH

# Fresh tree every run — skip full git history (saves tens of GB).
export AUTOMATE_EXTRA="--no-chromium-history"

DIST=/work/chromium/src/cef/binary_distrib
mkdir -p /out
for arch in ${ARCHES:-x64 arm64}; do
  echo ">> ===== building $arch ====="
  CEF_ARCH=$arch /opt/karere/build-cef-codecs.sh /work
  # Subshell + `|| true` so the unmatched-extension glob doesn't trip pipefail.
  tarball=$( (ls -t "$DIST"/cef_binary_*_linux*.tar.bz2 "$DIST"/cef_binary_*_linux*.zip 2>/dev/null || true) | head -1)
  [[ -n "$tarball" ]] || { echo "error: no distrib archive found in $DIST" >&2; exit 1; }
  # Ship the full + minimal pair for this arch (basename without extension prefix-matches both).
  for f in "${tarball%_minimal.*}"*.zip "${tarball%_minimal.*}"*.tar.bz2; do
    [[ -e "$f" ]] || continue
    (cd "$DIST" && sha256sum "$(basename "$f")" > "$f.sha256")
    mv "$f" "$f.sha256" /out/
  done
done

echo ">> done — results in /out:"
ls -l /out
cat /out/*.sha256
