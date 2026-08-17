#!/bin/bash
set -euo pipefail

# tools/verify-po.sh — read-only catalog health gate for po/karere.pot + 72 locales.
# Fails non-zero with a clear message per check. Safe to run repeatedly.

cd "$(git rev-parse --show-toplevel)"

PO_DIR="po"
POT_FILE="$PO_DIR/karere.pot"
LINGUAS_FILE="$PO_DIR/LINGUAS"

fail() {
    echo "verify-po: FAIL — $*" >&2
    exit 1
}

pass() {
    echo "verify-po: OK — $*"
}

# Derive expected version from meson.build (same logic as update-po.sh).
EXPECTED_VERSION="$(sed -n "s/^ *version: '\([^']*\)'.*/\1/p" meson.build | head -n1)"
if [[ -z "$EXPECTED_VERSION" ]]; then
    fail "could not derive version from meson.build"
fi

# --- (a) POT exists and contains sentinel msgids ---
if [[ ! -f "$POT_FILE" ]]; then
    fail "pot missing: $POT_FILE not found"
fi
pass "pot exists: $POT_FILE"

SENTINELS=(
    "Karere starting"
    "About Karere"
    "unread"
    "Mute notifications"
    "Notifications muted — click to unmute"
    "GPU rendering takes effect after Karere restarts."
    "Match WhatsApp Colors"
    "Blend the window and header bar with WhatsApp Web's background"
    "GPU Rendering"
    "Experimental"
)

for s in "${SENTINELS[@]}"; do
    if ! grep -Fq "msgid \"$s\"" "$POT_FILE"; then
        # For msgids that may be split across lines (unlikely for these), also try raw search.
        if ! grep -Fq "$s" "$POT_FILE"; then
            fail "sentinel msgid missing in $POT_FILE: \"$s\""
        fi
    fi
done
pass "all ${#SENTINELS[@]} sentinel msgids present in $POT_FILE"

# --- (b) LINGUAS ↔ .po parity both ways ---
# Build normalized LINGUAS list (skip blank / # lines, trim whitespace).
LINGUAS_ENTRIES=()
while IFS= read -r line || [[ -n "$line" ]]; do
    [[ -z "$line" || "$line" == \#* ]] && continue
    trimmed="$(echo "$line" | xargs)"
    [[ -z "$trimmed" ]] && continue
    LINGUAS_ENTRIES+=("$trimmed")
done < "$LINGUAS_FILE"

LINGUAS_COUNT="${#LINGUAS_ENTRIES[@]}"
PO_COUNT="$(ls -1 "$PO_DIR"/*.po 2>/dev/null | wc -l)"
# Trim spaces from wc output.
PO_COUNT="$(echo "$PO_COUNT" | xargs)"

if [[ "$LINGUAS_COUNT" -ne 72 ]]; then
    fail "LINGUAS count is $LINGUAS_COUNT, expected 72"
fi
if [[ "$PO_COUNT" -ne 72 ]]; then
    fail ".po file count is $PO_COUNT, expected 72"
fi
pass "LINGUAS and .po counts both 72"

# Every LINGUAS entry must have a .po
for lang in "${LINGUAS_ENTRIES[@]}"; do
    if [[ ! -f "$PO_DIR/$lang.po" ]]; then
        fail "LINGUAS entry '$lang' has no $PO_DIR/$lang.po"
    fi
done
pass "every LINGUAS entry has a .po"

# No .po absent from LINGUAS
for po_file in "$PO_DIR"/*.po; do
    base="$(basename "$po_file" .po)"
    found=0
    for lang in "${LINGUAS_ENTRIES[@]}"; do
        if [[ "$lang" == "$base" ]]; then
            found=1
            break
        fi
    done
    if [[ $found -eq 0 ]]; then
        fail ".po file '$po_file' is absent from LINGUAS"
    fi
done
pass "no .po absent from LINGUAS"

# Sentinel msgids must be carried in every .po (as msgid, even if untranslated).
for lang in "${LINGUAS_ENTRIES[@]}"; do
    po_file="$PO_DIR/$lang.po"
    for s in "${SENTINELS[@]}"; do
        if ! grep -Fq "msgid \"$s\"" "$po_file"; then
            if ! grep -Fq "$s" "$po_file"; then
                fail "sentinel \"$s\" missing in $po_file"
            fi
        fi
    done
done
pass "all sentinel msgids present in every .po (72 locales)"

# --- (c) msgfmt --check passes for every locale ---
for lang in "${LINGUAS_ENTRIES[@]}"; do
    po_file="$PO_DIR/$lang.po"
    if ! msgfmt --check --statistics -o /dev/null "$po_file" 2>&1; then
        fail "msgfmt --check failed for $po_file"
    fi
done
pass "msgfmt --check clean for all 72 locales"

# --- (d) No tilde-suffix backup files under po/ ---
if ls "$PO_DIR"/*.po~ >/dev/null 2>&1 || ls "$PO_DIR"/*.pot~ >/dev/null 2>&1; then
    # List offenders
    offenders="$(ls "$PO_DIR"/*.po~ "$PO_DIR"/*.pot~ 2>/dev/null | tr '\n' ' ')"
    fail "tilde-suffix backup files exist under $PO_DIR: $offenders"
fi
pass "no tilde-suffix backup files under $PO_DIR"

# --- (e) POT header Project-Id-Version reflects derived version (not 2.0.0) ---
if grep -q 'Project-Id-Version:.*2\.0\.0' "$POT_FILE"; then
    fail "pot header still pins 2.0.0 (expected $EXPECTED_VERSION): $(grep 'Project-Id-Version' "$POT_FILE")"
fi
if ! grep -q "Project-Id-Version:.*$EXPECTED_VERSION" "$POT_FILE"; then
    fail "pot header Project-Id-Version does not reflect $EXPECTED_VERSION: $(grep 'Project-Id-Version' "$POT_FILE" || echo '(no header)')"
fi
pass "pot header Project-Id-Version reflects $EXPECTED_VERSION"

echo "verify-po: ALL CHECKS PASSED"
