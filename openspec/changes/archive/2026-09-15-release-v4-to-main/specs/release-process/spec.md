## ADDED Requirements

### Requirement: Major-version line promotion to main

When a new major version is developed on a separate branch whose history does not share a common ancestor with `main` (an orphan branch), promoting it to `main` SHALL replace `main`'s history with the new line's history rather than attempting a fast-forward or three-way merge. The promoted line's own commit history SHALL be preserved without squashing.

#### Scenario: Orphan branch promoted to main

- **WHEN** the release line branch and `main` have no merge base (orphan histories)
- **THEN** `main` is force-updated to the release line's tip
- **AND** the release line's commit history remains intact (not squashed or rewritten)

#### Scenario: Fast-forward is impossible

- **WHEN** an operator attempts to merge the orphan release branch into `main` with a standard merge
- **THEN** the process instead force-updates `main`, because `git merge` would require `--allow-unrelated-histories` and produce a misleading merge commit

### Requirement: Prior-major history preservation

Before `main` is rewritten, the previous `main` tip SHALL be preserved under at least one permanent ref so no prior-major history becomes unreachable.

#### Scenario: v3 tip preserved before rewrite

- **WHEN** `main` is about to be force-updated from v3 to v4
- **THEN** the pre-rewrite v3 tip is captured under a permanent tag (e.g. `v3-final`) and/or branch (e.g. `v3`)
- **AND** existing `v3.*` release tags and the `master` ref remain intact as additional safety nets

#### Scenario: No history is lost

- **WHEN** the rewrite completes
- **THEN** every pre-rewrite v3 commit remains reachable from at least one ref (tag or branch)

### Requirement: Release artifact synchronization

Before a stable version tag is cut, all sources of truth for the version number and all user- and packaging-facing release artifacts SHALL agree on the same stable version and SHALL contain no pre-release (beta) markers for that version.

#### Scenario: Version strings agree

- **WHEN** a stable release is prepared
- **THEN** `meson.build` and `Cargo.toml` declare the same stable version with no `-beta` suffix

#### Scenario: CHANGELOG finalized

- **WHEN** the stable version is cut
- **THEN** `CHANGELOG.md` has a dated stanza for that stable version and no unreleased/beta-only dangling entries for it

#### Scenario: README reflects current major

- **WHEN** the stable version is cut
- **THEN** `README.md` describes the current major as the present state (no "preserved verbatim from the previous major" or stale on-disk-name caveats)

#### Scenario: AppStream metainfo leads with stable release

- **WHEN** the stable version is cut
- **THEN** the AppStream metainfo `<releases>` list contains a dated `<release>` entry for the stable version as its most recent entry

#### Scenario: Packaging pins track the stable tag

- **WHEN** the stable version is cut
- **THEN** Flatpak / Flathub packaging references point at the stable version tag, not a beta tag

### Requirement: CI/CD continuity across major promotion

The release line's CI/CD workflows SHALL be self-contained on the promoted branch so that, once it becomes `main`, automated build/test gates and release automation continue to function without relying on the prior major's workflows. Prior-major workflows that are specific to a replaced backend SHALL NOT be carried forward, and their removal SHALL be accounted for (the capability they provided either no longer applies or is provided by a replacement workflow).

#### Scenario: Release-line workflows function on main

- **WHEN** the release line is promoted to `main`
- **THEN** the build/test gate workflow runs on branch pushes and pull requests against `main`
- **AND** the release-automation workflow triggers on a stable version tag and opens the downstream packaging PR

#### Scenario: Obsolete prior-major workflows are accounted for

- **WHEN** prior-major workflows are tied to a backend the new major has replaced
- **THEN** those workflows drop out with the prior-major tree (not merged into `main`)
- **AND** any still-needed capability they provided is confirmed covered by a replacement workflow on the release line

#### Scenario: No release automation regresses

- **WHEN** comparing the prior-major and release-line workflow sets
- **THEN** every cross-version workflow (e.g. downstream packaging update) exists on the release line and is adapted to the new backend's inputs (no dangling references to removed sources)

### Requirement: Stable tag creation

A stable version tag SHALL be created on the new `main` tip only after artifact synchronization is verified.

#### Scenario: Stable tag on promoted main

- **WHEN** artifact synchronization is complete and `main` points at the release line
- **THEN** a stable version tag (e.g. `v4.0.0`) is created on the `main` tip
