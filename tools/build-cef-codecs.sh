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
ARCH_BUILD="--x64-build"   # --arm64-build for aarch64

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

# 4. Build + package a minimal distribution (Release only, skip cefclient sample).
python3 "$AUTOMATE" \
  --download-dir="$DOWNLOAD_DIR" \
  --branch="$CEF_BRANCH" \
  --minimal-distrib \
  --no-debug-build \
  --build-target=cefsimple \
  $ARCH_BUILD \
  --force-clean

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
