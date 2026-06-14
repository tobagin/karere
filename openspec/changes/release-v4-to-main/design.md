## Context

Karere v4 is the GTK4 + CEF/Chromium rewrite, developed on the orphan branch `karere-4-gtk-cef`. Confirmed facts about the current repository state:

- `karere-4-gtk-cef` and `main` have **no merge base** (`git merge-base main karere-4-gtk-cef` is empty). They are independent root histories — v4 was imported as a fresh root commit (`dbdc0ad Karere v4: GTK4 + CEF (Rust) rewrite — initial import`).
- `main` currently holds the v3 (WebKitGTK) line; its tip is `efd1a5f`. The remote also has `origin/master` which mirrors the same v3 line (`origin/main...origin/master` = 223/0).
- v3 releases are tagged `v2.5.7 … v3.1.1`; v4 betas are tagged `v4.0.0-beta1 … v4.0.0-beta7`.
- Version is `4.0.0-beta7` in `meson.build:2` and `Cargo.toml`.
- `CHANGELOG.md` already contains a `[4.0.0] - 2026-06-05` stanza; `data/io.github.tobagin.karere.metainfo.xml.in` already has a `<release version="4.0.0" date="2026-06-05">` entry.
- `README.md` leads with caveat notes ("preserved verbatim from karere v3", "on-disk worktree is still named `gtk-cef-shell`").
- Beta version markers live in: `meson.build`, `Cargo.toml`, `TESTING.md`, `packaging/flathub-beta/README.md`, `packaging/flathub-beta/io.github.tobagin.karere.yml` (pins `tag: v4.0.0-beta7` + `commit: cb789bb…`).
- `.github/workflows/update-flatpak.yml` fires on stable tags only (excludes `v*-*` pre-releases) and opens a PR against `flathub/io.github.tobagin.karere` `master`; betas go to the flathub `beta` branch via manual PR.

Stakeholder: single maintainer (`tobagin`). This is a one-shot repository-state operation plus a documentation/versioning sweep — not application code.

## Goals / Non-Goals

**Goals:**
- Make `main` the canonical v4 line with v4's commit history fully preserved (no squash).
- Guarantee no v3 history becomes unreachable.
- Promote `4.0.0-beta7` → stable `4.0.0` across every source of truth and user/packaging-facing artifact.
- Cut a `v4.0.0` stable tag on the new `main`.
- Make the operation auditable and rollback-able.

**Non-Goals:**
- No application/runtime behavior changes.
- No rewrite of v4's internal commit history (no rebase/squash/filter).
- No deletion of `origin/master`, `v3.*` tags, or beta tags.
- Not publishing to Flathub stable in this change — that is driven automatically by the `v4.0.0` tag via the existing workflow; this change only ensures the manifests/pins are correct.

## Decisions

**Decision 1 — Replace `main` history via force-update, not merge.**
Because the histories are unrelated, `git merge` would refuse without `--allow-unrelated-histories` and, with it, would create a merge commit stitching two unrelated trees — producing a confusing dual-root `main` and dragging dead v3 files into the v4 tree unless resolved with `-X theirs`. Instead, point `main` at the v4 tip directly (`git checkout main && git reset --hard karere-4-gtk-cef`, then `git push --force-with-lease origin main`). This matches the user's "(re-write)" intent and keeps `main`'s tree exactly equal to v4.
- *Alternative considered:* `git merge -s ours` from v3 then replace tree — needlessly complex, still leaves a v3 root in `main`'s first-parent history.
- *Alternative considered:* squash v4 into one commit on top of v3 — rejected; loses v4 history, violates "keeping its history".

**Decision 2 — Preserve v3 with a dedicated ref before rewriting.**
Create `git tag v3-final main` (pointing at current v3 `main` tip `efd1a5f`) and optionally `git branch v3 main` before the reset, and push both. v3 is then reachable via `v3-final`, `v3` (optional), all `v3.*` tags, and `origin/master`. Belt and suspenders, because the rewrite is the only irreversible step.

**Decision 3 — Single canonical major branch.**
After promotion, `main` is the only development branch going forward. `karere-4-gtk-cef` is kept as-is (or pushed as a historical alias) but `main` becomes the default. No change to `origin/master` (leave the v3 safety net).

**Decision 4 — Version bump is a real commit on v4 before promotion.**
Make the `beta7 → 4.0.0` edits as a commit on `karere-4-gtk-cef` (so it is included in the promoted history and the `v4.0.0` tag points at it). Order: (a) commit version/doc updates on the v4 branch, (b) preserve v3 ref, (c) force-update `main` to the new v4 tip, (d) tag `v4.0.0` on `main`, (e) push.

**Decision 5 — Release date.**
The `4.0.0` CHANGELOG and metainfo entries are currently dated `2026-06-05`. Set the stable release date to the actual promotion date (today: `2026-06-14`) for both, so the published store entry matches the tag date.

**Decision 6 — Packaging pins.**
The stable Flathub manifest (`packaging/io.github.tobagin.karere.yml`) is consumed by the `update-flatpak` workflow on the `v4.0.0` tag — verify its source module resolves to the new tag/release assets. The `flathub-beta` manifest + README remain beta artifacts; either re-pin them to `v4.0.0` (so beta == stable at release) or leave them as the last beta. Chosen: re-pin `flathub-beta` to `v4.0.0` for consistency, matching the established `re-pin flathub-beta to v4.0.0-betaN` commit pattern.

**Decision 7 — CI/CD: verify, don't port.**
The v3 and v4 workflow sets were compared. v3 had three workflows: `build-webkitgtk.yml` and `build-gst-plugin-audiofx.yml` (both `workflow_dispatch`-only dependency-binary builders, WebKit-specific) and `update-flatpak.yml` (release automation). v4 carries `build-cef-codecs.yml` (the CEF dependency-binary builder — direct replacement for the WebKitGTK builder), `ci.yml` (a build + headless-smoke gate v3 never had), and an adapted `update-flatpak.yml`. Because `main` is *replaced* by the v4 tree (Decision 1), the v4 workflows become `main`'s workflows automatically and the obsolete WebKit builders simply do not exist in the new tree — correct, since v4 has no WebKitGTK or gst-audiofx dependency. The only cross-version workflow, `update-flatpak.yml`, is already ported and improved on v4 (it drops the `cargo-sources-gst.json` copy step and excludes `v*-*` pre-release tags). Therefore no workflow needs authoring or porting; the work is to **verify** the v4 workflows fire correctly once on `main`.
- *Alternative considered:* keep `build-webkitgtk.yml`/`build-gst-plugin-audiofx.yml` on `main` for "continuity" — rejected; they reference WebKit tooling absent from the v4 tree and would be permanently dead/red workflows.
- *Note:* the v3 dependency-asset tags (`webkitgtk-2.50.5-*`) and their release binaries are unaffected by the `main` rewrite (tags are independent refs); they remain available but unused by v4.

## Risks / Trade-offs

- **Force-push to `main` is destructive on the remote** → Mitigation: create+push `v3-final` tag (and `v3` branch) first; `origin/master` already mirrors v3; use `--force-with-lease` to avoid clobbering unseen remote commits.
- **Local clones / CI with the old `main` will diverge** → Mitigation: single maintainer; document that fresh clone or `git fetch && git reset --hard origin/main` is required after the rewrite.
- **Open branches based on old `main`** (`feat/multi-account-completion`, `voip-fix-wip`, `account1009/main`) become orphaned relative to new `main` → Mitigation: out of scope; note them. They were already unrelated to v4 work.
- **`ci.yml` runs on every branch push (`branches: ['**']`)** → after promotion it will run on `main` pushes too; the Flatpak build+smoke takes up to 90 min → Mitigation: expected and desired (it is the startup-crash gate); caching keys on manifest+lockfile keep it warm. No change needed.
- **Stale v3 webkitgtk dependency-asset tags remain** but are unused by v4 → Mitigation: harmless; leave them (other consumers / reproducibility).
- **Stable Flathub PR fires automatically on `v4.0.0` tag** → Mitigation: ensure manifest + `cargo-sources.json` are correct *before* tagging; the workflow only opens a PR (manual merge gate remains).
- **Date drift** between CHANGELOG/metainfo/tag → Mitigation: Decision 5 sets one date everywhere.

## Migration Plan

1. On `karere-4-gtk-cef`: bump version `4.0.0-beta7 → 4.0.0`, finalize CHANGELOG/metainfo dates, rewrite README, re-pin packaging. Commit.
2. `git tag v3-final <current main tip>` and `git push origin v3-final` (optionally `git branch v3 main && git push origin v3`).
3. `git checkout main && git reset --hard karere-4-gtk-cef`.
4. `git push --force-with-lease origin main`.
5. `git tag v4.0.0 main && git push origin v4.0.0` (triggers the stable Flathub PR workflow).
6. Verify: `origin/main` tip == v4 tip; `v3-final` reachable; CI/Flathub PR opened.

**Rollback:** before step 4, nothing is irreversible. After step 4, restore with `git push --force-with-lease origin v3-final:main` (or from `origin/master`). Delete `v4.0.0` tag locally+remote if the tag must be re-cut.

## Open Questions

- Keep `origin/master` indefinitely as the v3 archive, or eventually retire it? (Default: keep.)
- Create the optional `v3` branch in addition to the `v3-final` tag, or tag-only? (Default: tag-only is sufficient; branch is optional.)
- Re-pin `flathub-beta` to `v4.0.0`, or freeze it at `v4.0.0-beta7`? (Default per Decision 6: re-pin to `v4.0.0`.)
