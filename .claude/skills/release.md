---
description: Create a new version release by analyzing changes, bumping version, updating changelogs, committing, tagging, and pushing
---

# Release Workflow

You are helping create a new release for Karere. Follow these steps:

## 1. Analyze Changes

Read the git log since the last release to understand what changed:
- Run `git log --oneline $(git describe --tags --abbrev=0)..HEAD` to see commits since last tag
- Categorize changes as: features (feat:), fixes (fix:), chores (chore:), breaking changes
- Determine appropriate version bump (major, minor, patch) based on semantic versioning

## 2. Determine New Version

- Read current version from `Cargo.toml`
- Ask user what version to release (suggest appropriate bump based on changes)
- Version format: X.Y.Z (e.g., 2.4.3)

## 3. Update Version Files

Update version in these files:
- `Cargo.toml`: Update `version = "X.Y.Z"` in [package] section
- `packaging/io.github.tobagin.karere.yml`: Update `tag: vX.Y.Z` in the karere module sources

## 4. Update CHANGELOG.md

- Read existing `CHANGELOG.md`
- Add new section at top with format:
  ```markdown
  ## [X.Y.Z] - YYYY-MM-DD

  ### Added
  - New features

  ### Fixed
  - Bug fixes

  ### Changed
  - Changes

  ### Removed
  - Removals
  ```
- Populate sections based on git log analysis from step 1
- Use current date

## 5. Update AppStream Metadata

Update `data/io.github.tobagin.karere.metainfo.xml`:
- Add new `<release>` entry at the top of the `<releases>` section
- Format:
  ```xml
  <release version="X.Y.Z" date="YYYY-MM-DD">
    <description>
      <p>Brief summary of changes</p>
      <ul>
        <li>Key feature or fix 1</li>
        <li>Key feature or fix 2</li>
      </ul>
    </description>
  </release>
  ```

## 6. Update Cargo Lock and Flatpak Sources

Run these commands:
```bash
cargo generate-lockfile
python3 tools/flatpak-cargo-generator.py Cargo.lock -o packaging/cargo-sources.json
```

## 7. Commit Changes (Without Co-Author)

Stage and commit all changes:
```bash
git add Cargo.toml Cargo.lock CHANGELOG.md data/io.github.tobagin.karere.metainfo.xml packaging/cargo-sources.json packaging/io.github.tobagin.karere.yml
git commit -m "Release vX.Y.Z"
```

**IMPORTANT**: Do NOT add `Co-Authored-By: Claude` to the commit. Use only the user's name.

## 8. Tag Release

Create annotated tag:
```bash
git tag -a vX.Y.Z -m "Release vX.Y.Z"
```

## 9. Push to Remote

Push commits and tags:
```bash
git push origin main --tags
```

## 10. Verify

Confirm with user:
- Show the git tag created
- Show the commit hash
- Confirm push was successful
- Remind user to create GitHub release if needed: `gh release create vX.Y.Z --generate-notes`

## Notes

- Always use semantic versioning (MAJOR.MINOR.PATCH)
- Breaking changes = major bump
- New features = minor bump
- Bug fixes only = patch bump
- Never force push tags unless explicitly requested
- Ensure CHANGELOG follows Keep a Changelog format
