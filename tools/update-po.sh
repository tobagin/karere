#!/bin/bash
set -euo pipefail

# tools/update-po.sh — regenerate po/karere.pot and merge all LINGUAS locales.
# Canonical procedure documented in po/README.md ("Regenerating the POT").
# Must materialize data/ui/*.ui from .blp via blueprint-compiler, then run a
# split xgettext extraction (non-Rust + Rust --language=C join) and
# LINGUAS-driven msgmerge --backup=none.

cd "$(git rev-parse --show-toplevel)"

VERSION="$(sed -n "s/^ *version: '\([^']*\)'.*/\1/p" meson.build | head -n1)"
if [[ -z "$VERSION" ]]; then
    echo "error: failed to derive version from meson.build" >&2
    exit 1
fi

DOMAIN="karere"
PO_DIR="po"
POT_FILE="$PO_DIR/$DOMAIN.pot"

# --- Blueprint materialization (mirrors data/meson.build blueprint_files) ---
BLUEPRINT_FILES=(window preferences keyboard-shortcuts account_switcher)
CREATED_UI=()

cleanup_ui() {
    for f in "${CREATED_UI[@]}"; do
        rm -f "$f"
    done
}
trap cleanup_ui EXIT

for name in "${BLUEPRINT_FILES[@]}"; do
    blp="data/ui/${name}.blp"
    ui="data/ui/${name}.ui"
    # Track only files this run created so we remove exactly those.
    was_present=0
    if [[ -f "$ui" ]]; then
        was_present=1
    fi
    echo "Compiling $blp -> $ui ..."
    blueprint-compiler compile --output "$ui" "$blp"
    if [[ $was_present -eq 0 ]]; then
        CREATED_UI+=("$ui")
    fi
done

# --- Extraction pass 1: non-Rust sources (Glade/Desktop/ITS auto-detected) ---
NON_RS_LIST="$(mktemp)"
RS_LIST="$(mktemp)"
trap 'rm -f "$NON_RS_LIST" "$RS_LIST"; cleanup_ui' EXIT

grep -v '\.rs$' "$PO_DIR/POTFILES.in" > "$NON_RS_LIST"
grep '\.rs$' "$PO_DIR/POTFILES.in" > "$RS_LIST"

if [[ ! -s "$NON_RS_LIST" ]]; then
    echo "error: non-Rust POTFILES subset is empty" >&2
    exit 1
fi
if [[ ! -s "$RS_LIST" ]]; then
    echo "error: Rust POTFILES subset is empty" >&2
    exit 1
fi

echo "Extracting strings (non-Rust) ..."
xgettext --package-name="$DOMAIN" \
         --package-version="$VERSION" \
         --default-domain="$DOMAIN" \
         --from-code=UTF-8 \
         --add-comments \
         --keyword=tr \
         --keyword=gettext \
         --keyword=_ \
         --output="$POT_FILE" \
         --files-from="$NON_RS_LIST" \
         --sort-output

# --- Extraction pass 2: Rust sources via C parser join ---
# xgettext has no Rust support; --language=C forces the C parser which
# extracts gettext/tr strings from .rs files. Rust char literals (e.g. 'x')
# trigger benign "warning: unterminated character constant" diagnostics from
# the C parser — these are expected and do not indicate failure.
echo "Extracting strings (Rust join via --language=C) ..."
# shellcheck disable=SC2046
xgettext --language=C \
         --join-existing \
         --from-code=UTF-8 \
         --add-comments \
         --keyword=tr \
         --keyword=gettext \
         --keyword=_ \
         --package-name="$DOMAIN" \
         --package-version="$VERSION" \
         --output="$POT_FILE" \
         $(cat "$RS_LIST")

# Clean up temp file lists early (trap handles remainder).
rm -f "$NON_RS_LIST" "$RS_LIST"
# Adjust trap to only clean up UI files from here on.
trap cleanup_ui EXIT

# --- Merge: LINGUAS-driven msgmerge ---
echo "Merging translations ..."
while IFS= read -r lang || [[ -n "$lang" ]]; do
    # Skip blank lines and comments.
    [[ -z "$lang" || "$lang" == \#* ]] && continue
    # Trim whitespace.
    lang="$(echo "$lang" | xargs)"
    [[ -z "$lang" ]] && continue
    po_file="$PO_DIR/$lang.po"
    if [[ ! -f "$po_file" ]]; then
        echo "warning: LINGUAS entry '$lang' has no $po_file — skipping" >&2
        continue
    fi
    echo "Updating $po_file ..."
    msgmerge -U --backup=none "$po_file" "$POT_FILE"
done < "$PO_DIR/LINGUAS"

echo "Done."
