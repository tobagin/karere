#!/usr/bin/env bash
#
# build-cef-codecs.sh — build CEF WITH proprietary codecs (H.264/AAC) so WhatsApp
# MP4 videos play (default CDN builds strip these). Emits cef_binary_*_minimal.tar.bz2
# + sha256 for the packaging manifest archive source.
#
# Needs ~100-150 GB disk, 16 GB+ RAM, several hours. Distributing H.264/AAC binaries
# carries patent-licensing obligations, falling on whoever hosts the tarball.
#
# Usage: CEF_BRANCH=<n> tools/build-cef-codecs.sh [download-dir]
#   CEF_BRANCH MUST match Karere's Chromium 148 line (chromium-148.0.7778.96); wrong
#   branch wastes the whole build, so the script refuses to start without it.
#
set -euo pipefail

if [[ -z "${CEF_BRANCH:-}" ]]; then
  cat >&2 <<'EOF'
error: CEF_BRANCH is required.

  Set it to the CEF branch number for Chromium 148 (the line Karere targets,
  chromium-148.0.7778.96). Look it up at:
    https://bitbucket.org/chromiumembedded/cef/wiki/BranchesAndBuilding

  Then re-run, e.g.:
    CEF_BRANCH=NNNN tools/build-cef-codecs.sh ~/cef-build
EOF
  exit 2
fi

DOWNLOAD_DIR="${1:-$HOME/cef-build}"
DEPOT_TOOLS_DIR="$DOWNLOAD_DIR/depot_tools"
# CEF_ARCH=x64 (default) or arm64. Karere ships both; build each separately.
case "${CEF_ARCH:-x64}" in
  x64)   ARCH_BUILD="--x64-build" ;;
  arm64) ARCH_BUILD="--arm64-build" ;;
  *) echo "error: CEF_ARCH must be x64 or arm64 (got '${CEF_ARCH}')" >&2; exit 2 ;;
esac

echo ">> CEF_BRANCH      = $CEF_BRANCH"
echo ">> DOWNLOAD_DIR    = $DOWNLOAD_DIR"
echo ">> arch            = $ARCH_BUILD"
echo

# 1. depot_tools
mkdir -p "$DOWNLOAD_DIR"
if [[ ! -d "$DEPOT_TOOLS_DIR" ]]; then
  echo ">> cloning depot_tools"
  git clone --depth 1 https://chromium.googlesource.com/chromium/tools/depot_tools.git \
    "$DEPOT_TOOLS_DIR"
fi
export PATH="$DEPOT_TOOLS_DIR:$PATH"

# 2. automate-git.py
AUTOMATE="$DOWNLOAD_DIR/automate-git.py"
if [[ ! -f "$AUTOMATE" ]]; then
  echo ">> fetching automate-git.py"
  curl -fsSL \
    "https://bitbucket.org/chromiumembedded/cef/raw/master/tools/automate/automate-git.py" \
    -o "$AUTOMATE"
fi

# 3. GN args: proprietary_codecs + ffmpeg_branding=Chrome compile in H.264/AAC;
#    is_official_build = release-grade optimized build matching upstream.
export GN_DEFINES="is_official_build=true proprietary_codecs=true ffmpeg_branding=Chrome"
export CEF_USE_GN=1  # CEF flag mirror, kept in sync for older trees

echo ">> GN_DEFINES = $GN_DEFINES"
echo

# 4a. Checkout/sync only (no build yet) so we can patch the tree before building.
#     --force-clean wipes the tree first, so it MUST run on this phase only —
#     running it on the build phase (4c) would revert the patch below.
python3 "$AUTOMATE" \
  --download-dir="$DOWNLOAD_DIR" \
  --branch="$CEF_BRANCH" \
  --minimal-distrib \
  --no-debug-build \
  --build-target=cefsimple \
  $ARCH_BUILD \
  --force-clean \
  --no-build

# 4b. Apply Karere CEF source patches (idle-CPU fix #151, etc.). Patches live in
#     tools/cef-patches/ and apply from the Chromium src root (paths cover both
#     base/ and cef/). Idempotent: skip a patch already present in the tree.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PATCH_DIR="$SCRIPT_DIR/cef-patches"
CHROMIUM_SRC="$DOWNLOAD_DIR/chromium/src"
if compgen -G "$PATCH_DIR/*.patch" >/dev/null; then
  for patch in "$PATCH_DIR"/*.patch; do
    name="$(basename "$patch")"
    if patch -p1 -d "$CHROMIUM_SRC" --dry-run --reverse --force <"$patch" >/dev/null 2>&1; then
      echo ">> patch already applied, skipping: $name"
      continue
    fi
    echo ">> applying patch: $name"
    patch -p1 -d "$CHROMIUM_SRC" --forward <"$patch"
  done
else
  echo ">> no patches found in $PATCH_DIR"
fi

# 4c. Build + package a minimal distribution (Release only, skip cefclient sample).
#     --no-update keeps the patched tree as-is (no re-sync, no clean).
python3 "$AUTOMATE" \
  --download-dir="$DOWNLOAD_DIR" \
  --branch="$CEF_BRANCH" \
  --minimal-distrib \
  --no-debug-build \
  --build-target=cefsimple \
  $ARCH_BUILD \
  --no-update \
  --force-build \
  --force-distrib

# 5. Locate the tarball + print sha256.
DISTRIB_DIR="$DOWNLOAD_DIR/chromium/src/cef/binary_distrib"
TARBALL="$(ls -t "$DISTRIB_DIR"/cef_binary_*_linux*_minimal.tar.bz2 2>/dev/null | head -1 || true)"

if [[ -z "$TARBALL" ]]; then
  echo "error: build finished but no minimal tarball found under $DISTRIB_DIR" >&2
  exit 1
fi

echo
echo "=============================================================="
echo " DONE. Proprietary-codec CEF distribution:"
echo "   $TARBALL"
echo
echo " sha256:"
sha256sum "$TARBALL"
echo
echo " Next: host this tarball (e.g. a GitHub release), then update"
echo " packaging/io.github.tobagin.karere.yml lines 51-52 to point at"
echo " its URL + this sha256 (and the archive.json inline name on line"
echo " 60 if the filename differs). Ping Claude with the URL+sha256 and"
echo " it'll make the edit."
echo "=============================================================="
