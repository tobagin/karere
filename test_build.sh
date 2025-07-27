#!/bin/bash
cd /home/tobagin/Documents/Projects/karere-vala
meson setup build --wipe 2>&1
meson compile -C build 2>&1