#!/usr/bin/env python3
"""
Simple script to remove duplicate msgid entries from .po files
"""

import re

def clean_po_content(content):
    """Remove duplicate msgid entries, keeping only the first occurrence"""
    
    # Split into lines
    lines = content.split('\n')
    result_lines = []
    seen_msgids = set()
    
    i = 0
    while i < len(lines):
        line = lines[i]
        
        # Check if this line starts a msgid entry
        if line.strip().startswith('msgid '):
            # Extract the msgid value
            msgid_match = re.match(r'^\s*msgid\s+"([^"]*)"', line)
            if msgid_match:
                msgid_value = msgid_match.group(1)
                
                # Skip empty msgid (header)
                if msgid_value == "":
                    result_lines.append(line)
                    i += 1
                    continue
                
                # If we've seen this msgid before, skip this entire entry
                if msgid_value in seen_msgids:
                    # Skip until we find the next msgid or end of file
                    while i < len(lines) and not lines[i].strip().startswith('msgid '):
                        i += 1
                    continue
                else:
                    # First time seeing this msgid, add it to seen set
                    seen_msgids.add(msgid_value)
        
        result_lines.append(line)
        i += 1
    
    return '\n'.join(result_lines)

# List of files to process
po_files = [
    'po/de.po', 'po/it.po', 'po/es.po', 'po/fr.po', 'po/ru.po', 
    'po/nl.po', 'po/ko.po', 'po/hi.po', 'po/pt_BR.po', 'po/ar.po',
    'po/zh_CN.po', 'po/ja.po', 'po/tr.po', 'po/pl.po', 'po/cs.po',
    'po/sv.po', 'po/nb.po', 'po/fi.po', 'po/el.po', 'po/he.po'
]

for po_file in po_files:
    try:
        print(f"Processing {po_file}...")
        
        # Read the file
        with open(po_file, 'r', encoding='utf-8') as f:
            content = f.read()
        
        # Clean duplicates
        cleaned_content = clean_po_content(content)
        
        # Write back
        with open(po_file, 'w', encoding='utf-8') as f:
            f.write(cleaned_content)
        
        print(f"  ✓ Cleaned {po_file}")
        
    except FileNotFoundError:
        print(f"  ⚠ File not found: {po_file}")
    except Exception as e:
        print(f"  ✗ Error processing {po_file}: {e}")

print("Done!")