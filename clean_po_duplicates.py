#!/usr/bin/env python3
"""
Script to remove duplicate msgid entries from .po translation files.
Keeps the first occurrence of each msgid and removes subsequent duplicates.
"""

import os
import re
import sys
from pathlib import Path

def clean_po_file(file_path):
    """Clean a single .po file by removing duplicate msgid entries."""
    print(f"Processing {file_path}...")
    
    try:
        with open(file_path, 'r', encoding='utf-8') as f:
            content = f.read()
    except Exception as e:
        print(f"Error reading {file_path}: {e}")
        return False
    
    # Split content into entries (separated by empty lines)
    entries = []
    current_entry = []
    lines = content.split('\n')
    
    # Track seen msgids to detect duplicates
    seen_msgids = set()
    cleaned_entries = []
    duplicates_removed = 0
    
    i = 0
    while i < len(lines):
        line = lines[i].strip()
        
        # If we hit an empty line or end of file, process the current entry
        if line == "" or i == len(lines) - 1:
            if i == len(lines) - 1 and line != "":
                current_entry.append(lines[i])
            
            if current_entry:
                # Extract msgid from the entry
                msgid = None
                for entry_line in current_entry:
                    if entry_line.strip().startswith('msgid '):
                        msgid_match = re.match(r'msgid\s+"([^"]*)"', entry_line.strip())
                        if msgid_match:
                            msgid = msgid_match.group(1)
                            break
                
                # Only add entry if we haven't seen this msgid before
                if msgid is not None:
                    if msgid not in seen_msgids:
                        seen_msgids.add(msgid)
                        cleaned_entries.extend(current_entry)
                        if current_entry and i < len(lines) - 1:  # Add empty line separator
                            cleaned_entries.append("")
                    else:
                        duplicates_removed += 1
                        print(f"  Removed duplicate msgid: '{msgid}'")
                else:
                    # Keep entries without msgid (like headers)
                    cleaned_entries.extend(current_entry)
                    if current_entry and i < len(lines) - 1:  # Add empty line separator
                        cleaned_entries.append("")
            
            current_entry = []
        else:
            current_entry.append(lines[i])
        
        i += 1
    
    # Write cleaned content back to file
    try:
        cleaned_content = '\n'.join(cleaned_entries)
        # Remove multiple consecutive empty lines
        cleaned_content = re.sub(r'\n\n\n+', '\n\n', cleaned_content)
        
        with open(file_path, 'w', encoding='utf-8') as f:
            f.write(cleaned_content)
        
        print(f"  ✓ Removed {duplicates_removed} duplicate entries from {file_path}")
        return True
    except Exception as e:
        print(f"Error writing {file_path}: {e}")
        return False

def main():
    """Main function to clean all .po files in the po directory."""
    po_dir = Path("po")
    
    if not po_dir.exists():
        print("Error: po directory not found!")
        sys.exit(1)
    
    po_files = list(po_dir.glob("*.po"))
    
    if not po_files:
        print("No .po files found in po directory!")
        sys.exit(1)
    
    print(f"Found {len(po_files)} .po files to process...")
    
    success_count = 0
    for po_file in sorted(po_files):
        if clean_po_file(po_file):
            success_count += 1
    
    print(f"\n✓ Successfully processed {success_count}/{len(po_files)} files")
    
    if success_count == len(po_files):
        print("All translation files have been cleaned of duplicates!")
    else:
        print("Some files had errors. Please check the output above.")
        sys.exit(1)

if __name__ == "__main__":
    main()