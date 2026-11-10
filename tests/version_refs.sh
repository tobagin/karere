#!/usr/bin/env bash
# Release version consistency (B5).
# Offline: the app version must agree across Cargo.toml, Cargo.lock, meson.build,
# metainfo (top <release>), CHANGELOG.md (top versioned heading), README
# "What's New in X" and po/karere.pot. The Flatpak manifests pin the PREVIOUS tag
# until the release commit, so for those only the pinned tag is reported and a
# mismatch is a warning.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

FAIL=0
fail() {
    echo "FAIL: $1" >&2
    FAIL=$((FAIL + 1))
}
pass() {
    echo "PASS: $1"
}

# Source of truth: Cargo.toml [package] version.
BASE="$(sed -n 's/^version = "\([^"]*\)".*/\1/p' Cargo.toml | head -n 1)"
if [[ -z "$BASE" ]]; then
    fail "could not read version from Cargo.toml"
    echo "---" >&2
    echo "$FAIL check(s) failed" >&2
    exit 1
fi
pass "Cargo.toml version is $BASE"

check() {
    # check <label> <found>
    local label="$1" found="$2"
    if [[ -z "$found" ]]; then
        fail "$label: version not found"
    elif [[ "$found" != "$BASE" ]]; then
        fail "$label is $found, expected $BASE"
    else
        pass "$label is $BASE"
    fi
}

check "Cargo.lock karere entry" "$(grep -A1 '^name = "karere"$' Cargo.lock | sed -n 's/^version = "\([^"]*\)".*/\1/p' | head -n 1)"
check "meson.build project version" "$(sed -n "s/^[[:space:]]*version:[[:space:]]*'\([^']*\)'.*/\1/p" meson.build | head -n 1)"
check "metainfo top <release>" "$(sed -n 's/.*<release version="\([^"]*\)".*/\1/p' data/io.github.tobagin.karere.metainfo.xml.in | head -n 1)"
check "CHANGELOG.md top versioned heading" "$(grep -m1 -E '^## \[[0-9]' CHANGELOG.md | sed -n 's/^## \[\([^]]*\)\].*/\1/p' || true)"
check "README What's New heading" "$(sed -n "s/.*What's New in \([0-9][0-9.]*\).*/\1/p" README.md | head -n 1)"
check "po/karere.pot Project-Id-Version" "$(sed -n 's/^"Project-Id-Version: karere \([^\\"]*\).*/\1/p' po/karere.pot | head -n 1)"

# Manifests: report the pinned tag, warn (never fail) on mismatch.
for f in packaging/io.github.tobagin.karere.yml packaging/flathub-beta/io.github.tobagin.karere.yml; do
    pinned="$(sed -n 's/^[[:space:]]*tag:[[:space:]]*v\([^[:space:]]*\).*/\1/p' "$f" | head -n 1)"
    if [[ -z "$pinned" ]]; then
        echo "WARN: $f has no pinned tag"
    elif [[ "$pinned" == "$BASE" ]]; then
        echo "INFO: $f pins v$pinned"
    else
        echo "WARN: $f pins v$pinned, app version is $BASE (expected until the release commit)"
    fi
done

if [[ "$FAIL" -ne 0 ]]; then
    echo "---" >&2
    echo "$FAIL check(s) failed" >&2
    exit 1
fi
echo "---"
echo "All version refs agree on $BASE"
