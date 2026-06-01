#!/bin/bash
set -e

DOMAIN="karere"
PO_DIR="po"
POT_FILE="$PO_DIR/$DOMAIN.pot"

echo "Extracting strings..."
xgettext --package-name="Karere" \
         --package-version="2.0.0" \
         --default-domain="$DOMAIN" \
         --output="$POT_FILE" \
         --from-code=UTF-8 \
         --keyword=_ \
         --keyword=gettext \
         --files-from="$PO_DIR/POTFILES.in" \
         --sort-output

for po_file in "$PO_DIR"/*.po; do
    if [ -f "$po_file" ]; then
        echo "Updating $po_file..."
        msgmerge --update --backup=none "$po_file" "$POT_FILE"
    fi
done

echo "Done."
