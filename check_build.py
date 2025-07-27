#!/usr/bin/env python3
import subprocess
import os
import sys

os.chdir('/home/tobagin/Documents/Projects/karere-vala')

try:
    # Setup build
    result = subprocess.run(['meson', 'setup', 'build', '--wipe'], 
                          capture_output=True, text=True)
    if result.returncode != 0:
        print("MESON SETUP FAILED:")
        print(result.stderr)
        sys.exit(1)
    
    # Compile
    result = subprocess.run(['meson', 'compile', '-C', 'build'], 
                          capture_output=True, text=True)
    if result.returncode != 0:
        print("COMPILATION FAILED:")
        print(result.stderr)
        sys.exit(1)
    else:
        print("BUILD SUCCESSFUL!")
        print(result.stdout)
        
except Exception as e:
    print(f"Error: {e}")
    sys.exit(1)