#!/bin/bash

cd /home/tobagin/Documents/Projects/karere-vala

echo "Testing compilation of window fixes..."

# Try to setup and compile
meson setup build --wipe -Dprofile=development 2>&1 | head -20
echo "--- Setup complete, now compiling ---"
meson compile -C build 2>&1 | head -30

echo "Compilation test complete!"