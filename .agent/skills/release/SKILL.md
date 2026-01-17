---
name: Release
description: Create a new version release by analyzing changes, bumping version, updating changelogs, committing, tagging, and pushing
---

# Release Skill

This skill automates the entire release process for this project.

## Workflow

### Step 1: Analyze Changes Since Last Release

```bash
# Get the last release tag
git describe --tags --abbrev=0

# List all commits since the last tag
git log $(git describe --tags --abbrev=0)..HEAD --oneline --no-merges

# Check for uncommitted changes
git status --short
```

Categorize all changes into:
- **Added**: New features
- **Changed**: Modifications to existing functionality
- **Fixed**: Bug fixes
- **Removed**: Removed features
- **Breaking**: Breaking changes (triggers major version bump)

### Step 2: Determine Version Bump

Follow [Semantic Versioning](https://semver.org/):
- **MAJOR** (X.0.0): Breaking changes or major rewrites
- **MINOR** (x.Y.0): New features, backward compatible
- **PATCH** (x.y.Z): Bug fixes, metadata updates, backward compatible

Current version can be found in:
- `Cargo.toml` (line ~3): `version = "X.Y.Z"`
- `meson.build` (line ~2): `version: 'X.Y.Z'`

### Step 3: Update Version Numbers

Update the version in these files:
1. **`Cargo.toml`**: `version = "X.Y.Z"`
2. **`meson.build`**: `version: 'X.Y.Z'`

### Step 4: Update CHANGELOG.md

Move content from `[Unreleased]` section to a new version section:

```markdown
## [X.Y.Z] - YYYY-MM-DD

### Added
- Feature description

### Changed
- Change description

### Fixed
- Fix description
```

Reset the `[Unreleased]` section to empty placeholders.

### Step 5: Update README.md

Update the version header section (around line 14):
```markdown
## 🎉 Version X.Y.Z - [Short Title]

**Karere X.Y.Z** brings [brief description of main changes].

### 🆕 What's New in X.Y.Z

- **Feature 1**: Description
- **Feature 2**: Description
```

### Step 6: Update metainfo.xml.in

Add a new `<release>` entry at the TOP of the `<releases>` section (after line 97):

```xml
<release version="X.Y.Z" date="YYYY-MM-DD">
  <description>
    <ul>
      <li>Change description 1</li>
      <li>Change description 2</li>
    </ul>
  </description>
</release>
```

**File location**: `data/io.github.tobagin.karere.metainfo.xml.in`

### Step 7: Update Cargo Lock and Sources

// turbo
```bash
cargo generate-lockfile
python3 tools/flatpak-cargo-generator.py Cargo.lock -o packaging/cargo-sources.json
```

### Step 8: Commit All Changes

Stage and commit with a detailed message:

```bash
git add .
git commit -m "Release vX.Y.Z

Changes in this release:
- [List main changes]
- [One per line]

Files updated:
- Cargo.toml, meson.build (version bump)
- CHANGELOG.md (release notes)
- README.md (version header)
- metainfo.xml.in (AppStream release)
- Cargo.lock, cargo-sources.json (dependencies)"
```

### Step 9: Create and Push Tag

```bash
# Create annotated tag
git tag -a vX.Y.Z -m "Release vX.Y.Z"

# Push commits and tags
git push origin HEAD --tags
```

## Important Notes

- Always use the format `vX.Y.Z` for tags (with the `v` prefix)
- Date format in CHANGELOG.md and metainfo.xml is `YYYY-MM-DD`
- The metainfo.xml release entries should be concise (no emojis in `<li>` items)
- Keep README.md "What's New" section brief and user-friendly
- CHANGELOG.md can be more detailed and technical

## File Locations Summary

| File | Version Location | Purpose |
|------|------------------|---------|
| `Cargo.toml` | Line ~3 | Rust package version |
| `meson.build` | Line ~2 | Build system version |
| `CHANGELOG.md` | New section after line 8 | Detailed release notes |
| `README.md` | Lines 14-22 | User-facing highlights |
| `data/io.github.tobagin.karere.metainfo.xml.in` | After line 97 | AppStream metadata |
