#!/usr/bin/env python3
import json
import subprocess
import hashlib
import re
import urllib.request
import argparse
import sys
import os
from typing import Optional, Tuple

MANIFEST_PATH = "packaging/io.github.tobagin.karere.yml"

def run_command(command, cwd=None, check=True):
    print(f"Running: {' '.join(command)}")
    result = subprocess.run(command, cwd=cwd, capture_output=True, text=True, check=check)
    return result

def get_remote_content(url):
    print(f"Fetching: {url}")
    with urllib.request.urlopen(url) as response:
        return response.read()

def get_remote_sha256(url):
    content = get_remote_content(url)
    return hashlib.sha256(content).hexdigest()

def update_manifest(pattern: str, replacement: str, path: str = MANIFEST_PATH) -> bool:
    with open(path, 'r') as f:
        content = f.read()
    
    new_content = re.sub(pattern, replacement, content)
    
    if new_content != content:
        with open(path, 'w') as f:
            f.write(new_content)
        return True
    return False

def update_cargo(dry_run: bool):
    print("Checking Cargo updates...")
    if dry_run:
        print("Dry run: Skipping actual cargo update")
        return False
    
    # Update Cargo.lock
    try:
        run_command(["cargo", "update"])
    except subprocess.CalledProcessError as e:
        print(f"Failed to update cargo: {e.stderr}")
        return False

    # Check if Cargo.lock changed
    run_command(["git", "diff", "--exit-code", "Cargo.lock"], check=False)
    # Actually we want to regenerate json regardless if we run update, or only if changed?
    # Usually regenerate if lock changed.
    # Let's check git status of Cargo.lock
    status = run_command(["git", "status", "--porcelain", "Cargo.lock"])
    if not status.stdout.strip():
        print("Cargo.lock did not change.")
        return False

    print("Cargo.lock updated. Regenerating cargo-sources.json...")
    run_command([
        "python3", "tools/flatpak-cargo-generator.py", 
        "Cargo.lock", "-o", "packaging/cargo-sources.json"
    ])
    return True

def update_hunspell(dry_run: bool) -> bool:
    print("Checking Hunspell dictionary updates...")
    # Get current version from manifest
    with open(MANIFEST_PATH, 'r') as f:
        content = f.read()
    
    # Extract current URL
    match = re.search(r'url: (https://github.com/LibreOffice/dictionaries/archive/refs/tags/libreoffice-(.*?).zip)', content)
    if not match:
        print("Could not find Hunspell URL in manifest.")
        return False
    
    current_url = match.group(1)
    current_version = match.group(2) # e.g. 26.2.0.1
    print(f"Current Hunspell version: {current_version}")

    # Check for latest tag on GitHub
    # Using GitHub API ideally, but let's try scraping tags page or using API with public access
    # Check for latest tag using git ls-remote
    try:
        cmd = ["git", "ls-remote", "--tags", "--refs", "https://github.com/LibreOffice/dictionaries.git"]
        result = run_command(cmd, check=True)
        
        tags = []
        for line in result.stdout.splitlines():
            # line is "sha256\trefs/tags/tagname"
            parts = line.split("\t")
            if len(parts) < 2: continue
            ref = parts[1]
            # remove refs/tags/ prefix
            tag_name = ref.replace("refs/tags/", "")
            if tag_name.startswith("libreoffice-"):
                tags.append(tag_name)
        
        if not tags:
             print("No libreoffice tags found via git ls-remote.")
             return False

        # Sort tags by version
        def version_key(tag):
             v_str = tag.replace("libreoffice-", "")
             # Handle possible non-numeric suffixes or irregularities
             # Simple split by dot and convert to int where possible
             return [int(x) if x.isdigit() else 0 for x in v_str.replace('_', '.').split('.')]

        latest_tag = sorted(tags, key=version_key)[-1]

    except subprocess.CalledProcessError as e:
        print(f"Failed to fetch tags via git: {e}")
        return False
    except Exception as e:
        print(f"Error parsing tags: {e}")
        return False
    
    latest_version = latest_tag.replace("libreoffice-", "")
    
    if latest_version == current_version:
        print("Hunspell is up to date.")
        return False
    
    print(f"Found new Hunspell version: {latest_version}")
    
    new_url = f"https://github.com/LibreOffice/dictionaries/archive/refs/tags/{latest_tag}.zip"
    
    if dry_run:
        print(f"Dry run: Would update Hunspell to {latest_version}")
        return True

    print(f"Calculating SHA256 for {new_url}...")
    new_sha256 = get_remote_sha256(new_url)
    
    # Update manifest
    updated_url = update_manifest(re.escape(current_url), new_url)
    
    # Determine current SHA from manifest to replace it uniquely
    # We scan for the SHA line following the URL
    # A bit naive regex, assuming sha256 follows url
    # Better: finding the block.
    # But let's try replacing the sha associated with the hunspell block.
    
    # We can use specific replacement by context.
    # Or just replace the generic sha if unique enough? No.
    # Let's read file, find the hunspell block, replace content.
    
    with open(MANIFEST_PATH, 'r') as f:
        lines = f.readlines()
    
    in_hunspell = False
    new_lines = []
    for line in lines:
        if "name: hunspell-dictionaries" in line:
            in_hunspell = True
        if in_hunspell and "name: " in line and "hunspell-dictionaries" not in line:
            in_hunspell = False
        
        if in_hunspell and "sha256:" in line:
             # This is the line to update
             new_lines.append(re.sub(r'sha256: .*', f'sha256: {new_sha256}', line))
             # Toggle off to avoid double replacement if logic was different, but here fine.
             # Only update once per block
             in_hunspell = False # effectively done for this block
        else:
            new_lines.append(line)
            
    with open(MANIFEST_PATH, 'w') as f:
        f.writelines(new_lines)

    return True

def update_noto(dry_run: bool) -> bool:
    print("Checking Noto Color Emoji updates...")
    with open(MANIFEST_PATH, 'r') as f:
        content = f.read()
    
    match = re.search(r'name: fonts-noto-color-emoji.*?sha256: ([a-f0-9]+)', content, re.DOTALL)
    if not match:
         print("Could not find Noto SHA in manifest.")
         return False
    current_sha256 = match.group(1)
    
    url = "https://github.com/googlefonts/noto-emoji/raw/main/fonts/NotoColorEmoji.ttf"
    print(f"Checking {url}...")
    latest_sha256 = get_remote_sha256(url)
    
    if current_sha256 == latest_sha256:
        print("Noto Color Emoji is up to date.")
        return False
        
    print("Found new Noto Color Emoji version (SHA256 changed).")
    
    if dry_run:
        print(f"Dry run: Would update Noto SHA to {latest_sha256}")
        return True

    # Update manifest
    # We need to target the Noto block again
    with open(MANIFEST_PATH, 'r') as f:
        lines = f.readlines()
        
    in_noto = False
    new_lines = []
    for line in lines:
        if "name: fonts-noto-color-emoji" in line:
            in_noto = True
        if in_noto and "name: " in line and "fonts-noto-color-emoji" not in line:
            in_noto = False
            
        if in_noto and "sha256:" in line:
             new_lines.append(re.sub(r'sha256: .*', f'sha256: {latest_sha256}', line))
             in_noto = False
        else:
            new_lines.append(line)
            
    with open(MANIFEST_PATH, 'w') as f:
        f.writelines(new_lines)
    
    return True

def update_runtime(dry_run: bool) -> bool:
    print("Checking Runtime updates...")
    with open(MANIFEST_PATH, 'r') as f:
        content = f.read()
        
    match = re.search(r"runtime-version: '(\d+)'", content)
    if not match:
        print("Could not find runtime-version in manifest.")
        return False
    
    current_version = int(match.group(1))
    next_version = current_version + 1
    
    print(f"Current runtime: {current_version}. Checking for {next_version}...")
    
    # Check if org.gnome.Sdk//next_version exist
    # If we are in an environment with flatpak installed:
    # run_command(["flatpak", "remote-ls", "--system", "flathub", f"org.gnome.Sdk//{next_version}"])
    # But usually we are not.
    # Let's check Flathub's git repo for the branch.
    # https://github.com/flathub/org.gnome.Sdk/tree/
    # Actually runtime version 49 corresponds to org.gnome.Sdk//49
    
    url = f"https://api.github.com/repos/flathub/org.gnome.Sdk/branches/{next_version}"
    try:
        # If branch exists, this returns 200 with data
        # If not, 404
        # Note: Github API rate limits might hit us, but for one call it is usually fine.
        urllib.request.urlopen(url)
        print(f"Runtime {next_version} is available!")
        exists = True
    except urllib.error.HTTPError as e:
        if e.code == 404:
            print(f"Runtime {next_version} not found.")
            exists = False
        else:
            print(f"Error checking runtime: {e}")
            return False

    if not exists:
        return False
        
    if dry_run:
        print(f"Dry run: Would update runtime to {next_version}")
        return True

    update_manifest(f"runtime-version: '{current_version}'", f"runtime-version: '{next_version}'")
    return True

def update_extensions(dry_run: bool) -> bool:
    print("Checking Runtime Extensions updates...")
    with open(MANIFEST_PATH, 'r') as f:
        content = f.read()

    # Regex to find extensions with version
    # Pattern: org.freedesktop.Platform.ffmpeg-full: ... version: '25.08'
    # We look for the block. simpler: scan for version lines inside add-extensions
    
    # We assume standard formatting as seen in file.
    # add-extensions:
    #   org.freedesktop.Platform.ffmpeg-full:
    #     directory: lib/ffmpeg
    #     version: '25.08'
    
    extensions_block = re.search(r'add-extensions:(.*?)(?=\n\S)', content, re.DOTALL)
    if not extensions_block:
        print("No add-extensions found.")
        return False
        
    block_content = extensions_block.group(1)
    
    # Find all extensions and their versions
    # We iterate over the block lines manually for better context parsing
    lines = block_content.split('\n')
    current_extension = None
    updates_found = False
    
    for line in lines:
        ext_match = re.search(r'^\s\s([\w\.\-]+):', line)
        if ext_match:
            current_extension = ext_match.group(1)
            continue
            
        ver_match = re.search(r"version: '(\d+\.\d+)'", line)
        if ver_match and current_extension:
            current_ver = ver_match.group(1)
            # Assumption: Version is YY.MM, usually YY.08 for freedesktop sdk extensions
            try:
                major, minor = map(int, current_ver.split('.'))
                next_major = major + 1
                next_ver = f"{next_major:02d}.{minor:02d}"
                
                print(f"Checking extension {current_extension}: {current_ver} -> {next_ver}...")
                
                # Check Flathub for branch
                # Repo: https://github.com/flathub/<extension_id>
                repo_url = f"https://github.com/flathub/{current_extension}.git"
                
                try:
                    cmd = ["git", "ls-remote", "--heads", repo_url, next_ver]
                    result = run_command(cmd, check=False)
                    if result.returncode == 0 and result.stdout.strip():
                        print(f"Found new version {next_ver} for {current_extension}!")
                        if dry_run:
                            print(f"Dry run: Would update {current_extension} to {next_ver}")
                            updates_found = True
                        else:
                            # Replace in manifest
                            # Be careful to replace only for this extension context
                            # Since we don't have a sophisticated yaml parser/editor that preserves comments,
                            # we'll use a regex that includes the extension name if possible, or context.
                            
                            # Searching specifically for the version line *under* the extension line is hard with global replace.
                            # But formatted as:
                            #   org.freedesktop.Platform.ffmpeg-full:
                            #     directory: lib/ffmpeg
                            #     version: '25.08'
                            
                            pattern = rf"({current_extension}:.*?version: '){current_ver}(')"
                            if update_manifest(pattern, rf"\g<1>{next_ver}\g<2>"):
                                print(f"Updated {current_extension} to {next_ver}")
                                updates_found = True
                    else:
                        print(f"Version {next_ver} not found for {current_extension}.")

                except Exception as e:
                    print(f"Error checking extension {current_extension}: {e}")

            except ValueError:
                print(f"Skipping non-standard version format for {current_extension}: {current_ver}")
                
    return updates_found

def main():
    parser = argparse.ArgumentParser(description="Check for updates for Karere dependencies.")
    parser.add_argument("--dry-run", action="store_true", help="Do not make changes, just check.")
    args = parser.parse_args()
    
    cargo_updated = update_cargo(args.dry_run)
    hunspell_updated = update_hunspell(args.dry_run)
    noto_updated = update_noto(args.dry_run)
    runtime_updated = update_runtime(args.dry_run)
    extensions_updated = update_extensions(args.dry_run)
    
    if any([cargo_updated, hunspell_updated, noto_updated, runtime_updated, extensions_updated]):
        print("Updates applied.")
        sys.exit(0) # Changes made
    else:
        print("No updates found.")
        sys.exit(0) # No changes

if __name__ == "__main__":
    main()
