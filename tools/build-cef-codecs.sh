#!/usr/bin/env bash
#
# build-cef-codecs.sh — build CEF WITH proprietary codecs (H.264/AAC) so WhatsApp
# MP4 videos play (default CDN builds strip these). Emits cef_binary_*_minimal.tar.bz2
# + sha256 for the packaging manifest archive source.
#
# Needs ~100-150 GB disk, 16 GB+ RAM, several hours. Distributing H.264/AAC binaries
# carries patent-licensing obligations, falling on whoever hosts the tarball.
#
# Usage: CEF_BUILD_MODE=upgrade CEF_VERSION=<full-version> CEF_CHECKOUT=<full-sha>
#   CEF_BRANCH=<n> tools/build-cef-codecs.sh [download-dir]
# Use legacy-current-pin explicitly to rebuild the current Chromium 152 line
# (chromium-152.0.7977.83). Upgrade mode waits for a verified fixed release.
#
set -euo pipefail

if [[ -z "${CEF_BRANCH:-}" ]]; then
  cat >&2 <<'EOF'
error: CEF_BRANCH is required.

  Set it to the CEF branch number for Chromium 152 (the line Karere targets,
  chromium-152.0.7977.83). Look it up at:
    https://bitbucket.org/chromiumembedded/cef/wiki/BranchesAndBuilding

  Then re-run, e.g.:
    CEF_BRANCH=NNNN tools/build-cef-codecs.sh ~/Projects/cef-build
EOF
  exit 2
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CEF_BUILD_MODE="${CEF_BUILD_MODE:-upgrade}"
if [[ "$CEF_BUILD_MODE" == legacy-current-pin ]]; then
  CEF_VERSION="${CEF_VERSION:-152.0.6+g708dc14+chromium-152.0.7977.83}"
  CEF_CHECKOUT="${CEF_CHECKOUT:-708dc140cbc3286826a8abef89dc23a44ff9ea72}"
fi
# This is deliberately before mkdir, cloning, hooks or expensive work.
PREFLIGHT="$(python3 "$SCRIPT_DIR/cef-release.py" preflight \
  --mode "$CEF_BUILD_MODE" --cef-version "${CEF_VERSION:-}" \
  --cef-checkout "${CEF_CHECKOUT:-}" --cef-branch "$CEF_BRANCH" \
  --arch "${CEF_ARCH:-x64}")" || { printf '%s\n' "$PREFLIGHT" >&2; exit 2; }
export CEF_ARCHIVE_FORMAT="${CEF_ARCHIVE_FORMAT:-tar.bz2}"
case "$CEF_ARCHIVE_FORMAT" in zip|tar.bz2) ;; *) echo 'error: unsupported CEF_ARCHIVE_FORMAT' >&2; exit 2 ;; esac

DOWNLOAD_DIR="${1:-$HOME/Projects/cef-build}"
DEPOT_TOOLS_DIR="$DOWNLOAD_DIR/depot_tools"
# CEF_ARCH=x64 (default) or arm64. Karere ships both; build each separately.
case "${CEF_ARCH:-x64}" in
  x64)   ARCH_BUILD="--x64-build" ;;
  arm64) ARCH_BUILD="--arm64-build" ;;
  *) echo "error: CEF_ARCH must be x64 or arm64 (got '${CEF_ARCH}')" >&2; exit 2 ;;
esac

# FORCE_CLEAN=1 wipes + re-syncs the tree from scratch (first build, or to recover
# a corrupt tree). Default OFF so a re-run RESUMES an interrupted sync (e.g. after a
# googlesource HTTP 429 or an out-of-disk) instead of throwing away the checkout.
FORCE_CLEAN_FLAG=""
[[ "${FORCE_CLEAN:-0}" == "1" ]] && FORCE_CLEAN_FLAG="--force-clean"

# Extra automate-git.py flags (e.g. --no-chromium-history in throwaway containers).
AUTOMATE_EXTRA="${AUTOMATE_EXTRA:-}"

# Pin the exact CEF commit (e.g. 708dc14 from "152.0.6+g708dc14+…") instead of
# branch tip — the produced binaries must match the Rust cef crate's pinned
# version, and branch tip can be a release ahead.
CHECKOUT_FLAG=""
[[ -n "${CEF_CHECKOUT:-}" ]] && CHECKOUT_FLAG="--checkout=$CEF_CHECKOUT"

echo ">> CEF_BRANCH      = $CEF_BRANCH"
echo ">> DOWNLOAD_DIR    = $DOWNLOAD_DIR"
echo ">> arch            = $ARCH_BUILD"
echo

# 1. depot_tools
mkdir -p "$DOWNLOAD_DIR"
printf '%s\n' "$PREFLIGHT" > "$DOWNLOAD_DIR/cef-preflight-${CEF_ARCH:-x64}.json"
if [[ ! -d "$DEPOT_TOOLS_DIR" ]]; then
  echo ">> cloning depot_tools"
  git clone --depth 1 https://chromium.googlesource.com/chromium/tools/depot_tools.git \
    "$DEPOT_TOOLS_DIR"
fi
export PATH="$DEPOT_TOOLS_DIR:$PATH"
# Pinned/offline-update builds still need depot_tools' Python bootstrap. In a
# fresh container DEPOT_TOOLS_UPDATE=0 otherwise leaves gn unable to start.
if [[ ! -f "$DEPOT_TOOLS_DIR/python3_bin_reldir.txt" ]]; then
  "$DEPOT_TOOLS_DIR/ensure_bootstrap"
fi

# 2. automate-git.py
AUTOMATE="$DOWNLOAD_DIR/automate-git.py"
if [[ ! -f "$AUTOMATE" ]]; then
  echo ">> fetching automate-git.py"
  curl -fsSL \
    "https://bitbucket.org/chromiumembedded/cef/raw/master/tools/automate/automate-git.py" \
    -o "$AUTOMATE"
fi

# 3. GN args: proprietary_codecs + ffmpeg_branding=Chrome compile in H.264/AAC;
#    is_official_build = release-grade optimized build matching upstream. The rest
#    are hard-won fixes for building on a modern Fedora host — the bare
#    official+codecs set alone fails to compile/link here:
#    - is_cfi=false                    : official build enables CFI which asserts "CFI requires ThinLTO"; we disable LTO.
#    - use_sysroot=true                : Fedora glibc 2.42 too new for Chromium headers; build against the bundled sysroot.
#    - treat_warnings_as_errors=false  : bundled sqlite3.c trips -Wincompatible-pointer-types-discards-qualifiers under -Werror.
#    - concurrent_links=1              : the libcef.so link OOM-kills on 31 GB RAM (exit 247); serialize links.
#    - use_system_libffi=false / use_cups=false : avoid host -lffi_pic link error / cups-config dep.
export GN_DEFINES="is_official_build=true is_component_build=false proprietary_codecs=true ffmpeg_branding=Chrome use_thin_lto=false chrome_pgo_phase=false is_cfi=false use_cups=false use_system_libffi=false use_sysroot=true treat_warnings_as_errors=false concurrent_links=1 symbol_level=0 blink_symbol_level=0"
export CEF_USE_GN=1  # CEF flag mirror, kept in sync for older trees

echo ">> GN_DEFINES = $GN_DEFINES"
echo

# 4a. Checkout/sync only (no build yet) so we can patch the tree before building.
#     --force-clean wipes the tree first, so it MUST run on this phase only —
#     running it on the build phase (4c) would revert the patch below.
#     Bootstrapping quirk: automate's checkout phase ends by running CEF's
#     gclient_hook.py, which needs the gn/clang toolchain that only `gclient
#     runhooks` fetches — so the FIRST pass on a fresh tree always fails there.
#     Run the hooks and retry once; the retry's sync is a fast no-op.
checkout() {
  python3 "$AUTOMATE" \
    --download-dir="$DOWNLOAD_DIR" \
    --branch="$CEF_BRANCH" \
    --minimal-distrib \
    --no-debug-build \
    --build-target=cefsimple \
    $ARCH_BUILD \
    $FORCE_CLEAN_FLAG \
    $CHECKOUT_FLAG \
    $AUTOMATE_EXTRA \
    --no-build \
    --no-distrib
}
if ! checkout; then
  echo ">> checkout pass failed (expected on a fresh tree: gclient_hook needs"
  echo ">> the hook-fetched toolchain) — running gclient hooks and retrying"
  ( cd "$DOWNLOAD_DIR/chromium" && gclient runhooks )
  checkout
fi

CHROMIUM_SRC="$DOWNLOAD_DIR/chromium/src"

# 4a-bis. Fetch the build toolchain (clang, rust, gn helpers, ...) AND the sysroot via
#     gclient hooks. This MUST run AFTER the 4a sync — automate-git syncs with
#     --nohooks, which leaves third_party/llvm-build/.../clang absent, and a re-run of
#     4a's `gclient sync --reset` wipes any clang fetched earlier — and BEFORE the 4c
#     build, which uses --no-update (no re-sync) so it won't clobber what we fetch here.
#     Without this, cef/tools/gclient_hook.py dies with FileNotFoundError: clang. Idempotent.
echo ">> running gclient hooks (clang/rust/toolchain)"
( cd "$DOWNLOAD_DIR/chromium" && gclient runhooks )

# GCS-fetched tools can land without exec bits when hooks run as root (container
# builds); every devtools-frontend ninja action then dies with EACCES on node.
chmod +x "$DOWNLOAD_DIR"/chromium/src/third_party/node/linux/node-linux-*/bin/node 2>/dev/null || true

# Belt-and-suspenders: ensure the matching Debian sysroot (use_sysroot=true needs it;
# a fresh glibc host fails header checks without it). install-sysroot is idempotent.
case "${CEF_ARCH:-x64}" in
  x64)   SYSROOT_ARCH=amd64 ;;
  arm64) SYSROOT_ARCH=arm64 ;;
esac
echo ">> ensuring $SYSROOT_ARCH sysroot"
python3 "$CHROMIUM_SRC/build/linux/sysroot_scripts/install-sysroot.py" --arch="$SYSROOT_ARCH"
# The x64 HOST toolchain (protoc, codegen) always needs the amd64 sysroot, and
# a target_cpu=arm64 gclient sync removes it (DEPS condition no longer matches).
if [[ "$SYSROOT_ARCH" != "amd64" ]]; then
  echo ">> ensuring amd64 host sysroot"
  python3 "$CHROMIUM_SRC/build/linux/sysroot_scripts/install-sysroot.py" --arch=amd64
fi

# 4b. Apply Karere CEF source patches (idle-CPU fix #151, etc.). Patches live in
#     tools/cef-patches/ and apply from the Chromium src root (paths cover both
#     base/ and cef/). Idempotent: skip a patch already present in the tree.
PATCH_DIR="$SCRIPT_DIR/cef-patches"
PATCHES=(karere-151-detach-worksource.patch karere-173-pump-continuation.patch)
if [[ "$CEF_BUILD_MODE" == legacy-current-pin ]]; then
  PATCHES+=(karere-nvidia-native-shared-images.patch)
fi
# The old native-handle backport is specific to the legacy engine. Fixed releases
# already contain that work and must not receive it again.
for name in "${PATCHES[@]}"; do
  patch="$PATCH_DIR/$name"
  if patch -p1 -d "$CHROMIUM_SRC" --dry-run --reverse --force <"$patch" >/dev/null 2>&1; then
    echo ">> patch already applied, skipping: $name"
    continue
  fi
  echo ">> applying patch: $name"
  patch -p1 -d "$CHROMIUM_SRC" --forward <"$patch"
done

# VersionFormatter needs CEF branch history to generate the exact release name.
if [[ "$(git -C "$CHROMIUM_SRC/cef" rev-parse --is-shallow-repository)" == true ]]; then
  git -C "$CHROMIUM_SRC/cef" fetch --unshallow origin
fi
git -C "$CHROMIUM_SRC/cef" fetch origin \
  "refs/heads/master:refs/remotes/origin/master" \
  "refs/heads/$CEF_BRANCH:refs/remotes/origin/$CEF_BRANCH"

# 4c. Build. automate-git's own build phase shells out to `autoninja` with no -j
#     and prepends depot_tools to PATH (so a PATH shim can't cap it); siso then
#     runs one local job per core, which on a 16-core/30 GB box swaps hard and
#     gets build children SIGKILLed. Drive ninja ourselves: CEF_NINJA_JOBS caps
#     parallelism (unset = one job per core), and the loop retries because siso
#     resumes exactly where it died — a killed step costs seconds, not a re-sync.
#     gclient_hook.py (gn gen) already ran via runhooks; rerunning is a cheap no-op.
OUT_DIR="out/Release_GN_${CEF_ARCH:-x64}"
JOBS_FLAG=""
[[ -n "${CEF_NINJA_JOBS:-}" ]] && JOBS_FLAG="-j $CEF_NINJA_JOBS"
( cd "$CHROMIUM_SRC/cef" && python3 tools/gclient_hook.py )
python3 "$SCRIPT_DIR/cef-release.py" source \
  --preflight "$DOWNLOAD_DIR/cef-preflight-${CEF_ARCH:-x64}.json" \
  --chromium-src "$CHROMIUM_SRC" --patch-dir "$PATCH_DIR" \
  > "$DOWNLOAD_DIR/cef-source-${CEF_ARCH:-x64}.json"
attempt=0
until ( cd "$CHROMIUM_SRC" && autoninja -C "$OUT_DIR" $JOBS_FLAG cefsimple chrome_sandbox ); do
  attempt=$((attempt + 1))
  if (( attempt > ${CEF_NINJA_RETRIES:-2} )); then
    echo "error: ninja failed $attempt times, giving up" >&2
    exit 1
  fi
  echo ">> ninja attempt $attempt failed — retrying in 10 s"
  sleep 10
done

# 4d. Package a minimal distribution from the tree built above.
#     --no-update keeps the patched tree as-is; --no-build skips automate's
#     uncapped autoninja (targets are already up to date).
python3 "$AUTOMATE" \
  --download-dir="$DOWNLOAD_DIR" \
  --branch="$CEF_BRANCH" \
  --minimal-distrib \
  --no-debug-build \
  --build-target=cefsimple \
  $ARCH_BUILD \
  $CHECKOUT_FLAG \
  $AUTOMATE_EXTRA \
  --no-update \
  --no-build \
  --force-distrib

# 5. Verify the exact version/architecture, headers, resources and ELF machine.
# Old caches may contain other versions, architectures and incorrect shallow-git
# version names. Never select the newest (or first) matching glob.
DISTRIB_DIR="$DOWNLOAD_DIR/chromium/src/cef/binary_distrib"
python3 "$SCRIPT_DIR/cef-release.py" artifact \
  --provenance "$DOWNLOAD_DIR/cef-source-${CEF_ARCH:-x64}.json" \
  --distrib-dir "$DISTRIB_DIR" --format "$CEF_ARCHIVE_FORMAT" \
  > "$DOWNLOAD_DIR/cef-artifact-${CEF_ARCH:-x64}.json"
TARBALL="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["artifact"])' \
  "$DOWNLOAD_DIR/cef-artifact-${CEF_ARCH:-x64}.json")"

echo
echo "=============================================================="
echo " DONE. Proprietary-codec CEF distribution:"
echo "   $TARBALL"
echo
echo " sha256:"
sha256sum "$TARBALL"
echo
echo " Next: host this tarball (e.g. a GitHub release), then update"
echo " packaging/io.github.tobagin.karere.yml to point at its URL + this"
echo " sha256 (and the archive.json inline name if the filename differs)."
echo "=============================================================="
