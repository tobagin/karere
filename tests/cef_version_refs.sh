#!/usr/bin/env bash
# Regression test for CEF version consistency (KARE-009).
# Offline: ties download-cef.sh default to manifests, Cargo.lock, spec, and metainfo.
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

# 1) Extract script default
CEF_DEFAULT="$(sed -n 's/^CEF_VERSION="${CEF_VERSION:-\([^"]*\)}"$/\1/p' download-cef.sh)"
if [[ -z "$CEF_DEFAULT" ]]; then
    fail "download-cef.sh default is empty or pattern did not match"
else
    pass "download-cef.sh default is $CEF_DEFAULT"
fi

# 2) Decode tags from both Flatpak manifests (URL-decoded %2B -> +)
decode_manifest_tag() {
    local file="$1"
    # Grab first cef_binary_* URL, strip to version+platform minimal, decode %2B
    local url
    url="$(grep -o 'cef_binary[^"]*\.zip' "$file" | head -n 1)"
    if [[ -z "$url" ]]; then
        echo ""
        return
    fi
    # url like cef_binary_150.0.10%2Bg8042e43%2Bchromium-150.0.7871.101_linux64_minimal.zip
    local decoded
    decoded="$(echo "$url" | sed 's/%2B/+/g')"
    # Strip prefix cef_binary_ and suffix _linux*_minimal.zip
    decoded="${decoded#cef_binary_}"
    decoded="${decoded%_linux64_minimal.zip}"
    decoded="${decoded%_linuxarm64_minimal.zip}"
    echo "$decoded"
}

TAG_YML="$(decode_manifest_tag packaging/io.github.tobagin.karere.yml)"
TAG_DEVEL="$(decode_manifest_tag packaging/io.github.tobagin.karere.Devel.yml)"

if [[ -z "$TAG_YML" ]]; then
    fail "could not extract CEF tag from packaging/io.github.tobagin.karere.yml"
else
    pass "manifest stable tag is $TAG_YML"
fi
if [[ -z "$TAG_DEVEL" ]]; then
    fail "could not extract CEF tag from packaging/io.github.tobagin.karere.Devel.yml"
else
    pass "manifest Devel tag is $TAG_DEVEL"
fi

if [[ -n "$CEF_DEFAULT" && -n "$TAG_YML" ]]; then
    if [[ "$CEF_DEFAULT" != "$TAG_YML" ]]; then
        fail "download-cef.sh default ($CEF_DEFAULT) != stable manifest tag ($TAG_YML)"
    else
        pass "download-cef.sh default matches stable manifest"
    fi
fi
if [[ -n "$CEF_DEFAULT" && -n "$TAG_DEVEL" ]]; then
    if [[ "$CEF_DEFAULT" != "$TAG_DEVEL" ]]; then
        fail "download-cef.sh default ($CEF_DEFAULT) != Devel manifest tag ($TAG_DEVEL)"
    else
        pass "download-cef.sh default matches Devel manifest"
    fi
fi

# Also ensure manifests agree with each other
if [[ -n "$TAG_YML" && -n "$TAG_DEVEL" && "$TAG_YML" != "$TAG_DEVEL" ]]; then
    fail "stable manifest tag ($TAG_YML) != Devel manifest tag ($TAG_DEVEL)"
fi

# 3) Cargo.lock cef build metadata vs first three dot-components of default
# Cargo.lock: version = "150.0.0+150.0.10" after name = "cef"
LOCK_VERSION="$(grep -A1 '^name = "cef"$' Cargo.lock | grep 'version' | sed -n 's/.*version = "\([^"]*\)".*/\1/p' | head -n 1)"
if [[ -z "$LOCK_VERSION" ]]; then
    fail "could not extract cef version from Cargo.lock"
else
    pass "Cargo.lock cef version is $LOCK_VERSION"
    LOCK_META="${LOCK_VERSION#*+}"
    if [[ "$LOCK_META" == "$LOCK_VERSION" ]]; then
        fail "Cargo.lock cef version has no build metadata (+): $LOCK_VERSION"
    else
        # First three dot-components of CEF_DEFAULT (before first +)
        CEF_PREFIX="${CEF_DEFAULT%%+*}"
        if [[ "$LOCK_META" != "$CEF_PREFIX" ]]; then
            fail "Cargo.lock build metadata ($LOCK_META) != CEF prefix ($CEF_PREFIX) from default ($CEF_DEFAULT)"
        else
            pass "Cargo.lock build metadata matches CEF prefix"
        fi
    fi
fi

# 4) Openspec contains full tag
if [[ -n "$CEF_DEFAULT" ]]; then
    if ! grep -Fq "$CEF_DEFAULT" openspec/specs/cef-binary-provisioning/spec.md; then
        fail "openspec/specs/cef-binary-provisioning/spec.md does not contain $CEF_DEFAULT"
    else
        pass "openspec spec contains $CEF_DEFAULT"
    fi
fi

# 5) Metainfo current description says 150, not 148
if grep -Fq "Now built on CEF/Chromium 150." data/io.github.tobagin.karere.metainfo.xml.in; then
    pass "metainfo current description contains CEF/Chromium 150"
else
    fail "metainfo current description missing 'Now built on CEF/Chromium 150.'"
fi
if grep -Fq "Now built on CEF/Chromium 148" data/io.github.tobagin.karere.metainfo.xml.in; then
    # Count occurrences: only historical release entries should contain 148, not the current paragraph.
    # The current paragraph check above ensures 150 is present; but if 148 also appears outside history, fail.
    # Simpler: ensure the exact stale sentence is gone (it would be in the current paragraph).
    # Check that the description paragraph no longer has the stale text by ensuring no match before <releases>
    # Extract up to <releases> and check.
    BEFORE_RELEASES="$(sed -n '1,/<releases>/p' data/io.github.tobagin.karere.metainfo.xml.in)"
    if echo "$BEFORE_RELEASES" | grep -Fq "Now built on CEF/Chromium 148"; then
        fail "metainfo current-description still contains 'Now built on CEF/Chromium 148'"
    else
        pass "metainfo stale 148 only in historical releases (acceptable)"
    fi
fi

# 6) Historical entries survive
if grep -Fq "CEF 149" data/io.github.tobagin.karere.metainfo.xml.in; then
    pass "historical CEF 149 still present"
else
    fail "historical CEF 149 missing from metainfo"
fi
if grep -Fq "CEF/Chromium 148" data/io.github.tobagin.karere.metainfo.xml.in; then
    pass "historical CEF/Chromium 148 still present"
else
    fail "historical CEF/Chromium 148 missing from metainfo"
fi

# 7) xmllint well-formed
if command -v xmllint >/dev/null 2>&1; then
    if xmllint --noout data/io.github.tobagin.karere.metainfo.xml.in 2>&1; then
        pass "xmllint well-formed"
    else
        fail "xmllint --noout failed"
    fi
else
    echo "SKIP: xmllint not found, skipping well-formed check"
fi

# 8) tools/build-cef-codecs.sh documents the shipping Chromium line (KARE-011)
if [[ -n "$CEF_DEFAULT" ]]; then
    CHROMIUM_LINE="chromium-${CEF_DEFAULT##*chromium-}"
    if grep -Fq "$CHROMIUM_LINE" tools/build-cef-codecs.sh; then
        pass "build-cef-codecs.sh contains shipping Chromium line $CHROMIUM_LINE"
    else
        fail "build-cef-codecs.sh does not contain shipping Chromium line $CHROMIUM_LINE"
    fi
    if grep -Eq 'Chromium 148|chromium-148' tools/build-cef-codecs.sh; then
        fail "build-cef-codecs.sh still contains stale Chromium 148 reference"
    else
        pass "build-cef-codecs.sh has no stale Chromium 148 reference"
    fi
    # Runtime error surface: missing CEF_BRANCH prints the Chromium line and exits 2
    TMPFILE="$(mktemp)"
    set +e
    # env -u CEF_BRANCH: strip any leaked CEF_BRANCH so a stray export in the
    # invoking shell cannot push the script past its guard into a real build.
    env -u CEF_BRANCH bash tools/build-cef-codecs.sh 2>"$TMPFILE"
    CODE=$?
    set -e
    if [[ "$CODE" -ne 2 ]]; then
        fail "build-cef-codecs.sh without CEF_BRANCH exited $CODE, expected 2"
    else
        pass "build-cef-codecs.sh without CEF_BRANCH exits 2"
    fi
    if grep -Fq "$CHROMIUM_LINE" "$TMPFILE"; then
        pass "build-cef-codecs.sh error message contains $CHROMIUM_LINE"
    else
        fail "build-cef-codecs.sh error message does not contain $CHROMIUM_LINE"
    fi
    rm -f "$TMPFILE"
fi

# Summary
if [[ "$FAIL" -ne 0 ]]; then
    echo "---" >&2
    echo "$FAIL check(s) failed" >&2
    exit 1
fi

echo "---"
echo "All CEF version refs agree on $CEF_DEFAULT"
exit 0
