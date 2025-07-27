#!/bin/bash

# Simple test script to verify translation setup is working
# This script tests the basic i18n infrastructure

echo "Testing Karere Translation Setup"
echo "================================"

# Check if LINGUAS file exists and has content
if [[ -f "LINGUAS" ]]; then
    echo "✓ LINGUAS file found"
    lang_count=$(grep -v '^#' LINGUAS | grep -v '^$' | wc -l)
    echo "  Languages supported: $lang_count"
else
    echo "✗ LINGUAS file missing"
    exit 1
fi

# Check if POTFILES.in exists and has content
if [[ -f "POTFILES.in" ]]; then
    echo "✓ POTFILES.in found"
    file_count=$(grep -v '^#' POTFILES.in | grep -v '^$' | wc -l)
    echo "  Source files listed: $file_count"
else
    echo "✗ POTFILES.in missing"
    exit 1
fi

# Check for .po files
po_count=$(ls -1 *.po 2>/dev/null | wc -l)
if [[ $po_count -gt 0 ]]; then
    echo "✓ Translation files found: $po_count"
else
    echo "✗ No .po files found"
fi

# Check if meson.build exists in po directory
if [[ -f "meson.build" ]]; then
    echo "✓ Meson i18n configuration found"
else
    echo "✗ meson.build missing in po directory"
fi

# Check for required tools (if available)
echo ""
echo "Tool availability:"
for tool in xgettext msgfmt msgmerge; do
    if command -v $tool >/dev/null 2>&1; then
        echo "✓ $tool available"
    else
        echo "✗ $tool not found (install gettext package)"
    fi
done

echo ""
echo "Translation Infrastructure Test Complete"
echo ""
echo "To test the setup:"
echo "1. From project root: meson setup build"
echo "2. Generate POT file: ninja -C build karere-pot"
echo "3. Update .po files: msgmerge po/es.po po/karere.pot -o po/es.po"
echo "4. Test with locale: LANG=es_ES.UTF-8 ./build/karere"