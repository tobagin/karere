## Why

Karere v4 (the GTK4 + CEF/Chromium rewrite) has been developed on the orphan branch `karere-4-gtk-cef` and shipped through seven betas (`v4.0.0-beta1` … `v4.0.0-beta7`). It is now ready for stable release, but `main` still points at the v3 (WebKitGTK) history, which shares **no common ancestor** with the v4 branch. We need `main` to become the canonical v4 line — with v4's full commit history preserved — while keeping the v3 history reachable, and we need every user- and packaging-facing artifact (README, CHANGELOG, version strings, AppStream metainfo, Flatpak manifests) to describe the stable `4.0.0` release rather than a beta.

## What Changes

- **BREAKING (repository history):** `main` is rewritten to be the v4 line. Because `karere-4-gtk-cef` is an orphan branch with no common ancestor, `main` cannot fast-forward or three-way merge — its history is *replaced* with the v4 history (force-update), not merged. v4's own commit history is preserved verbatim (no squash).
- The pre-rewrite v3 `main` tip is preserved under a permanent ref (tag `v3-final` and/or branch `v3`) before the rewrite, in addition to existing `v3.*` tags and `origin/master`, so no v3 history is lost.
- Version bumped from `4.0.0-beta7` to stable `4.0.0` across all sources of truth (`meson.build`, `Cargo.toml`, AppStream metainfo, any packaging pins).
- CHANGELOG finalized: the existing `[4.0.0]` stanza is confirmed/dated for the stable release and the beta churn is summarized rather than left dangling.
- README updated to stand on its own as the v4 README — the "this README is preserved verbatim from karere v3 / on-disk worktree is still `gtk-cef-shell`" framing notes are removed or rewritten, and v4 facts (CEF backend, multi-account, no v3 migration) are presented as the current state.
- AppStream metainfo `<release>` list confirmed to lead with the stable `4.0.0` entry with a correct date.
- Flatpak / Flathub packaging updated to track the stable `v4.0.0` tag instead of `v4.0.0-beta7`.
- CI/CD continuity verified: v4 already carries its own workflow set (`ci.yml` build+smoke gate, `build-cef-codecs.yml` dependency-binary builder, `update-flatpak.yml` release automation). v3's WebKit-specific builders (`build-webkitgtk.yml`, `build-gst-plugin-audiofx.yml`) are intentionally *not* carried forward — they are superseded by `build-cef-codecs.yml`. After promotion, confirm the v4 workflows trigger correctly on `main` (branch push → `ci.yml`; stable tag → `update-flatpak.yml`).
- A stable `v4.0.0` tag is created on the new `main` tip.

## Capabilities

### New Capabilities
- `release-process`: Defines the repository's release and branch-promotion policy — how an orphan major-version line is promoted to `main`, how prior-major history is preserved, and which artifacts must be synchronized (version strings, CHANGELOG, README, AppStream metainfo, packaging pins) before a stable tag is cut.

### Modified Capabilities
<!-- None — no existing app behavior/spec requirements change; this is a release/repository-process change. -->

## Impact

- **Git history / refs:** `main` (force-updated), new `v3-final` tag and/or `v3` branch, new `v4.0.0` tag. Remote `origin/main` requires a force-push; `origin/master` (v3) is left intact as an additional safety net.
- **Version sources:** `meson.build`, `Cargo.toml`.
- **User-facing docs:** `README.md`, `CHANGELOG.md`.
- **Packaging / store:** `data/io.github.tobagin.karere.metainfo.xml.in`, Flatpak manifests, Flathub / flathub-beta pin references.
- **CI/CD:** `.github/workflows/{ci.yml,build-cef-codecs.yml,update-flatpak.yml}` (already on v4, become the `main` workflows after promotion). v3-only `build-webkitgtk.yml` and `build-gst-plugin-audiofx.yml` drop out with the v3 tree (superseded). No new workflow files are authored; this is a verification, not a port.
- **No application source/behavior changes** — runtime code is unchanged; this is a promotion + documentation + versioning change.
