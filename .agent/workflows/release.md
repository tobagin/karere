---
description: How to release a new version of Karere
---

1. Bump version in `Cargo.toml`.
2. Update `CHANGELOG.md`.
3. Update `data/io.github.tobagin.karere.metainfo.xml` with `<release>` entry.
// turbo
4. Update Cargo dependencies and sources:
   `cargo generate-lockfile` # Updates Cargo.lock
   `python3 tools/flatpak-cargo-generator.py Cargo.lock -o packaging/cargo-sources.json`
5. Commit changes (Ensure generated sources are included):
   `git add Cargo.lock packaging/cargo-sources.json .`
   `git commit -m "Release vX.Y.Z"`
6. Tag release:
   `git tag -f vX.Y.Z`
7. Push:
   `git push origin HEAD --tags --force`
