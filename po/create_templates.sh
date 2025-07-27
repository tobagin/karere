#!/bin/bash

# Script to create template .po files for all languages in LINGUAS
# This script creates minimal template files that can be filled with translations

# Language data: language_code:language_name:plural_forms
declare -A LANG_DATA=(
    ["ko"]="Korean:nplurals=1; plural=0;"
    ["tr"]="Turkish:nplurals=2; plural=(n > 1);"
    ["pl"]="Polish:nplurals=3; plural=(n==1 ? 0 : n%10>=2 && n%10<=4 && (n%100<10 || n%100>=20) ? 1 : 2);"
    ["cs"]="Czech:nplurals=3; plural=(n==1) ? 0 : (n>=2 && n<=4) ? 1 : 2;"
    ["sv"]="Swedish:nplurals=2; plural=(n != 1);"
    ["nb"]="Norwegian Bokmål:nplurals=2; plural=(n != 1);"
    ["fi"]="Finnish:nplurals=2; plural=(n != 1);"
    ["el"]="Greek:nplurals=2; plural=(n != 1);"
    ["he"]="Hebrew:nplurals=2; plural=(n != 1);"
)

# Read LINGUAS file and create templates for missing .po files
while IFS= read -r lang || [[ -n "$lang" ]]; do
    # Skip comments and empty lines
    [[ "$lang" =~ ^#.*$ ]] && continue
    [[ -z "$lang" ]] && continue
    
    # Skip if .po file already exists
    [[ -f "${lang}.po" ]] && continue
    
    # Get language info
    if [[ -n "${LANG_DATA[$lang]}" ]]; then
        IFS=':' read -r lang_name plural_forms <<< "${LANG_DATA[$lang]}"
    else
        lang_name="$lang"
        plural_forms="nplurals=2; plural=(n != 1);"
    fi
    
    echo "Creating template for $lang ($lang_name)..."
    
    cat > "${lang}.po" << EOF
# ${lang_name} translations for karere.
# Copyright (C) 2025 Thiago Fernandes
# This file is distributed under the same license as the karere package.
# FIRST AUTHOR <EMAIL@ADDRESS>, YEAR.
#
msgid ""
msgstr ""
"Project-Id-Version: karere 0.6.0\\n"
"Report-Msgid-Bugs-To: https://github.com/tobagin/karere-vala/issues\\n"
"POT-Creation-Date: 2025-07-25 12:00+0000\\n"
"PO-Revision-Date: YEAR-MO-DA HO:MI+ZONE\\n"
"Last-Translator: FULL NAME <EMAIL@ADDRESS>\\n"
"Language-Team: ${lang_name}\\n"
"Language: ${lang}\\n"
"MIME-Version: 1.0\\n"
"Content-Type: text/plain; charset=UTF-8\\n"
"Content-Transfer-Encoding: 8bit\\n"
"Plural-Forms: ${plural_forms}\\n"

# Template file - translations needed
EOF

done < LINGUAS

echo "Template creation complete. Run 'meson setup build && ninja -C build karere-pot' to generate the POT file."
echo "Then use 'msgmerge' to update the .po files with actual translatable strings."