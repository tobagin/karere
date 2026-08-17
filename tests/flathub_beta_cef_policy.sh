#!/usr/bin/env bash
# Offline policy regression test for flathub-beta CEF/engine pin (KARE-010).
# Ensures the beta manifest either mirrors stable or carries an explicit
# INTENTIONAL DIVERGENCE marker — never silently lags.
set -euo pipefail

# Policy selector: mirror | divergence-document
POLICY="mirror"

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

BETA_MANIFEST="packaging/flathub-beta/io.github.tobagin.karere.yml"
STABLE_MANIFEST="packaging/io.github.tobagin.karere.yml"
BETA_README="packaging/flathub-beta/README.md"

PASS=0
FAIL=0

pass() {
    PASS=$((PASS + 1))
    echo "PASS: $1"
}

fail() {
    FAIL=$((FAIL + 1))
    echo "FAIL: $1" >&2
}

# Helper: decode CEF tag from archive URLs in a manifest.
# Extracts the cef_binary...zip token, decodes %2B->+, strips prefix/suffix.
# Args: manifest path, arch suffix (e.g. linux64 | linuxarm64)
decode_cef_tag() {
    local manifest="$1"
    local arch="$2"
    # Grep the url line for this arch, extract the cef_binary...zip token
    local token
    token="$(grep -o 'cef_binary[^"]*\.zip' "$manifest" | grep "$arch" | head -n1 || true)"
    if [[ -z "$token" ]]; then
        echo ""
        return
    fi
    # Decode %2B -> +
    token="${token//%2B/+}"
    # Strip prefix cef_binary_ and suffix _${arch}_minimal.zip
    token="${token#cef_binary_}"
    token="${token%_${arch}_minimal.zip}"
    echo "$token"
}

# Helper: extract sorted unique archive.json .tar.bz2 names from a manifest
archive_names() {
    local manifest="$1"
    grep -o 'cef_binary[^"]*\.tar\.bz2' "$manifest" | sort -u || true
}

# Helper: extract karere git tag from a manifest (e.g. v4.2.2)
karere_tag() {
    local manifest="$1"
    grep -o 'tag: v[0-9.]*[^[:space:]]*' "$manifest" | head -n1 | sed 's/tag: //' || true
}

# --- YAML sanity (SKIP if PyYAML unavailable) ---
for f in "$BETA_MANIFEST" "$STABLE_MANIFEST"; do
    if python3 -c "import yaml" 2>/dev/null; then
        if python3 -c "import yaml,sys; yaml.safe_load(open(sys.argv[1]))" "$f" 2>/dev/null; then
            pass "yaml valid: $f"
        else
            fail "yaml invalid: $f"
        fi
    else
        echo "SKIP: PyYAML not available — yaml check for $f skipped"
    fi
done

if [[ "$POLICY" == "mirror" ]]; then
    echo "--- Policy: mirror (beta must match stable) ---"

    # 1. Decoded CEF tags for BOTH arches must match stable
    for arch in linux64 linuxarm64; do
        beta_tag="$(decode_cef_tag "$BETA_MANIFEST" "$arch")"
        stable_tag="$(decode_cef_tag "$STABLE_MANIFEST" "$arch")"
        if [[ -z "$beta_tag" ]]; then
            fail "decoded CEF tag empty for beta arch $arch"
        elif [[ -z "$stable_tag" ]]; then
            fail "decoded CEF tag empty for stable arch $arch"
        elif [[ "$beta_tag" == "$stable_tag" ]]; then
            pass "CEF tag $arch matches stable: $beta_tag"
        else
            fail "CEF tag $arch mismatch — beta: $beta_tag stable: $stable_tag"
        fi
    done

    # Also assert the two arches share the same tag within each manifest
    beta_x64="$(decode_cef_tag "$BETA_MANIFEST" "linux64")"
    beta_arm64="$(decode_cef_tag "$BETA_MANIFEST" "linuxarm64")"
    if [[ "$beta_x64" == "$beta_arm64" && -n "$beta_x64" ]]; then
        pass "beta CEF tags consistent across arches: $beta_x64"
    else
        fail "beta CEF tags inconsistent — x64: $beta_x64 arm64: $beta_arm64"
    fi

    # 2. Archive.json inline names must match stable's set and contain the decided tag
    beta_names="$(archive_names "$BETA_MANIFEST")"
    stable_names="$(archive_names "$STABLE_MANIFEST")"
    if [[ "$beta_names" == "$stable_names" ]]; then
        pass "archive.json names match stable set"
    else
        fail "archive.json names differ — beta: [$beta_names] stable: [$stable_names]"
    fi

    # The tag in archive.json should be the same decoded tag
    if echo "$beta_names" | grep -q "$beta_x64"; then
        pass "archive.json names contain decided tag $beta_x64"
    else
        fail "archive.json names do not contain decided tag $beta_x64 — got [$beta_names]"
    fi

    # 3. Karere git tag must equal stable's
    beta_karere="$(karere_tag "$BETA_MANIFEST")"
    stable_karere="$(karere_tag "$STABLE_MANIFEST")"
    if [[ "$beta_karere" == "$stable_karere" && -n "$beta_karere" ]]; then
        pass "karere git tag matches stable: $beta_karere"
    else
        fail "karere git tag mismatch — beta: $beta_karere stable: $stable_karere"
    fi

    # 4. GSK_RENDERER=gl must be present in beta
    if grep -q 'GSK_RENDERER=gl' "$BETA_MANIFEST"; then
        pass "GSK_RENDERER=gl present in beta manifest"
    else
        fail "GSK_RENDERER=gl missing in beta manifest"
    fi

    # 5. Zero chromium-148 references remain
    count_148="$(grep -c 'chromium-148' "$BETA_MANIFEST" || true)"
    if [[ "$count_148" -eq 0 ]]; then
        pass "zero chromium-148 references in beta manifest"
    else
        fail "chromium-148 still present $count_148 times in beta manifest"
    fi

    # Also check for stale 151fix marker
    count_151fix="$(grep -c '151fix' "$BETA_MANIFEST" || true)"
    if [[ "$count_151fix" -eq 0 ]]; then
        pass "zero 151fix references in beta manifest"
    else
        fail "151fix still present $count_151fix times in beta manifest"
    fi

elif [[ "$POLICY" == "divergence-document" ]]; then
    echo "--- Policy: divergence-document ---"

    if grep -q 'INTENTIONAL DIVERGENCE' "$BETA_MANIFEST"; then
        pass "beta manifest contains INTENTIONAL DIVERGENCE marker"
    else
        fail "beta manifest missing INTENTIONAL DIVERGENCE marker"
    fi

    if grep -q 'CEF version divergence' "$BETA_README"; then
        pass "README contains CEF version divergence section"
    else
        fail "README missing CEF version divergence section"
    fi

else
    fail "unknown POLICY value: $POLICY (expected mirror or divergence-document)"
fi

echo "---"
echo "Results: $PASS passed, $FAIL failed"
if [[ "$FAIL" -ne 0 ]]; then
    exit 1
fi
exit 0
