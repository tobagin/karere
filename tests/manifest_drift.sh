#!/usr/bin/env bash
# Drift guard for the three hand-maintained Flatpak manifests (B7).
# Offline: normalises the expected differences (app-id, Debug extension name,
# -Dprofile, karere source stanza, comments, blank lines) and fails on anything else.
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

STABLE="packaging/io.github.tobagin.karere.yml"
DEVEL="packaging/io.github.tobagin.karere.Devel.yml"
BETA="packaging/flathub-beta/io.github.tobagin.karere.yml"

normalise() {
    # 1) drop whole-line comments and blank lines
    # 2) app-id / Debug extension name -> stable spelling
    # 3) -Dprofile value -> default
    # 4) drop the karere module's source stanza (git tag/commit vs local dir)
    sed -E -e '/^[[:space:]]*#/d' -e '/^[[:space:]]*$/d' \
           -e 's/io\.github\.tobagin\.karere\.Devel/io.github.tobagin.karere/g' \
           -e 's/^([[:space:]]*- -Dprofile=).*/\1PROFILE/' "$1" |
    awk '
        /^  - name: karere$/ { in_karere = 1 }
        in_karere && /^    sources:/ { print; in_src = 1; next }
        in_src && /^      - cargo-sources\.json/ { in_src = 0; in_karere = 0 }
        in_src { next }
        { print }
    '
}

for f in "$STABLE" "$DEVEL" "$BETA"; do
    if [[ ! -f "$f" ]]; then
        fail "missing manifest $f"
    fi
done
[[ "$FAIL" -eq 0 ]] || exit 1

# The normaliser must actually have found the karere source stanza, otherwise it
# is silently comparing more (or less) than intended.
for f in "$STABLE" "$DEVEL" "$BETA"; do
    if normalise "$f" | grep -Eq 'tag:|commit:|type: (git|dir)|path: \.\.'; then
        fail "normaliser left a karere source line in $f"
    fi
    if ! grep -q 'cargo-sources.json' "$f"; then
        fail "$f has no cargo-sources.json entry"
    fi
done

compare() {
    local label="$1" a="$2" b="$3" out
    if out="$(diff <(normalise "$a") <(normalise "$b"))"; then
        pass "$label manifests agree after normalisation"
    else
        fail "$label manifests drifted:"
        echo "$out" >&2
    fi
}

compare "stable vs Devel" "$STABLE" "$DEVEL"
compare "stable vs flathub-beta" "$STABLE" "$BETA"

# Expected differences really are present (guards against comparing a file with itself).
grep -q '^app-id: io.github.tobagin.karere.Devel$' "$DEVEL" && pass "Devel app-id" || fail "Devel app-id is not io.github.tobagin.karere.Devel"
grep -q '^app-id: io.github.tobagin.karere$' "$STABLE" && pass "stable app-id" || fail "stable app-id wrong"
grep -q '^app-id: io.github.tobagin.karere$' "$BETA" && pass "beta app-id" || fail "beta app-id wrong"
grep -q -- '-Dprofile=development' "$DEVEL" && pass "Devel profile=development" || fail "Devel profile is not development"
grep -q -- '-Dprofile=default' "$STABLE" && grep -q -- '-Dprofile=default' "$BETA" && pass "stable/beta profile=default" || fail "stable/beta profile is not default"

# Pinned release tag: manifests pin the PREVIOUS tag until the release commit,
# so report only; version_refs.sh warns on mismatch with the app version.
for f in "$STABLE" "$BETA"; do
    tag="$(sed -n 's/^[[:space:]]*tag: \(v[^[:space:]]*\).*/\1/p' "$f" | head -n 1)"
    echo "INFO: $f pins $tag"
done
if [[ "$(sed -n 's/^[[:space:]]*tag: \(v.*\)/\1/p' "$STABLE")" != "$(sed -n 's/^[[:space:]]*tag: \(v.*\)/\1/p' "$BETA")" ]]; then
    echo "WARN: stable and beta pin different tags"
fi

if [[ "$FAIL" -ne 0 ]]; then
    echo "---" >&2
    echo "$FAIL check(s) failed" >&2
    exit 1
fi
echo "---"
echo "Manifests agree"
