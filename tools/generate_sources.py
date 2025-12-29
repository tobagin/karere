#!/usr/bin/env python3
import json
import sys
import re

def parse_cargo_lock(lock_path):
    packages = []
    current_package = None
    
    with open(lock_path, 'r') as f:
        # Simple parser for Cargo.lock which is TOML
        # We look for [[package]] blocks
        lines = f.readlines()
        
    for line in lines:
        line = line.strip()
        if line == "[[package]]":
            if current_package:
                packages.append(current_package)
            current_package = {}
            continue
            
        if current_package is None:
            if line.startswith("name ="):
                # Handle root package maybe? Cargo.lock usually implies [[package]]
                pass
            continue
            
        # Parse key = "value"
        # We need name, version, source, checksum
        if "=" in line:
            parts = line.split("=", 1)
            key = parts[0].strip()
            value = parts[1].strip()
            # Remove quotes
            if value.startswith('"') and value.endswith('"'):
                value = value[1:-1]
            
            if key == "name":
                current_package["name"] = value
            elif key == "version":
                current_package["version"] = value
            elif key == "source":
                current_package["source"] = value
            elif key == "checksum":
                current_package["checksum"] = value
                
    if current_package:
        packages.append(current_package)
        
    return packages

def generate_sources(cargo_lock_path):
    packages = parse_cargo_lock(cargo_lock_path)
    real_sources = []
    
    # Source for cargo config
    real_sources.append({
        "type": "shell",
        "commands": [
            "mkdir -p .cargo",
            "echo '[source.crates-io]' > .cargo/config",
            "echo 'replace-with = \"vendored-sources\"' >> .cargo/config",
            "echo '[source.vendored-sources]' >> .cargo/config",
            "echo 'directory = \"cargo/vendor\"' >> .cargo/config",
            "mkdir -p cargo/vendor"  # Ensure dir exists
        ]
    })

    for package in packages:
        source = package.get('source', '')
        if not source.startswith('registry+'):
            continue
            
        name = package.get('name')
        version = package.get('version')
        checksum = package.get('checksum')
        
        if not name or not version or not checksum:
            continue
            
        url = f"https://crates.io/api/v1/crates/{name}/{version}/download"
        
        # Use archive type
        real_sources.append({
            "type": "archive",
            "archive-type": "tar-gzip",
            "url": url,
            "sha256": checksum,
            "dest": f"cargo/vendor/{name}-{version}"
        })
        
        # Write checksum file
        real_sources.append({
             "type": "shell",
             "commands": [
                 f"echo '{{\"package\":\"{checksum}\",\"files\":{{}}}}' > cargo/vendor/{name}-{version}/.cargo-checksum.json"
             ]
        })

    return real_sources

if __name__ == "__main__":
    if len(sys.argv) < 2:
        print("Usage: generate_sources.py Cargo.lock")
        sys.exit(1)
        
    sources = generate_sources(sys.argv[1])
    print(json.dumps(sources, indent=4))
