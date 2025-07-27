#!/usr/bin/env python3

import sys
import re

def deduplicate_po_file(filename):
    """Remove duplicate msgid entries from a .po file"""
    
    with open(filename, 'r', encoding='utf-8') as f:
        lines = f.readlines()
    
    seen_msgids = set()
    result_lines = []
    i = 0
    skipping = False
    duplicates_found = 0
    
    while i < len(lines):
        line = lines[i].rstrip('\n')
        
        # Check if this is a msgid line
        if line.strip().startswith('msgid '):
            # Extract the msgid value
            match = re.match(r'^\s*msgid\s+"([^"]*)"', line)
            if match:
                msgid_value = match.group(1)
                
                # Skip empty msgid (header)
                if msgid_value == "":
                    result_lines.append(line + '\n')
                    i += 1
                    continue
                    
                # Check if we've seen this msgid before
                if msgid_value in seen_msgids:
                    print(f"Found duplicate msgid: '{msgid_value}' at line {i+1}")
                    duplicates_found += 1
                    skipping = True
                    # Skip until we find next msgid or comment
                    while i < len(lines):
                        current = lines[i].rstrip('\n')
                        if (current.strip().startswith('msgid ') or 
                            current.strip().startswith('#:') or
                            current.strip().startswith('# ') or
                            current.strip() == ''):
                            if current.strip().startswith('msgid ') or current.strip().startswith('#:'):
                                break
                        i += 1
                    skipping = False
                    continue
                else:
                    seen_msgids.add(msgid_value)
                    skipping = False
        
        if not skipping:
            result_lines.append(line + '\n')
        
        i += 1
    
    # Write the deduplicated content back
    with open(filename, 'w', encoding='utf-8') as f:
        f.writelines(result_lines)
    
    print(f"Removed {duplicates_found} duplicate entries from {filename}")
    return duplicates_found

if __name__ == "__main__":
    if len(sys.argv) != 2:
        print("Usage: python3 deduplicate.py <po_file>")
        sys.exit(1)
    
    filename = sys.argv[1]
    deduplicate_po_file(filename)