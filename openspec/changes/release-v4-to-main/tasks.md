## 1. Pre-flight checks

- [x] 1.1 Confirm working tree clean on `karere-4-gtk-cef` (`git status`) — clean except the untracked openspec change dir
- [x] 1.2 Re-confirm orphan state: `git merge-base main karere-4-gtk-cef` returns empty — ORPHAN confirmed
- [x] 1.3 Record current v3 `main` tip SHA for rollback — `efd1a5fcec0087f8f78a819c7bb2564f9279919f`
- [x] 1.4 Confirm `origin/master` still mirrors the v3 line (safety net) — NOTE: `origin/master` is an older v3 *subset* (ancestor of `origin/main`, 223/0), not a full mirror; the `v3-final` tag (7.1) is the real full-tip net

## 2. Version bump (on karere-4-gtk-cef)

- [x] 2.1 `meson.build`: set `version: '4.0.0'` (drop `-beta7`)
- [x] 2.2 `Cargo.toml`: set `version = "4.0.0"`; `Cargo.lock` updated via `cargo update -p karere --offline`
- [x] 2.3 `git grep -n "4.0.0-beta"` — remaining refs resolved in §3/§4; the `update-flatpak.yml:7` ref is an illustrative comment of the exclude glob (left intentionally)

## 3. User-facing docs

- [x] 3.1 `CHANGELOG.md`: `[4.0.0]` stanza dated `2026-06-14`; no dangling Unreleased section (stanza already complete)
- [x] 3.2 `README.md`: removed the stale "preserved verbatim from karere v3" / `gtk-cef-shell` on-disk-name caveat blockquote; v4 facts already current in body
- [x] 3.3 `TESTING.md`: version ref `4.0.0-beta…` → `4.0.0`; also fixed stale `gtk-cef-shell` binary name → `karere` in the leak/orphan-process checks

## 4. Packaging / store metadata

- [x] 4.1 `data/…metainfo.xml.in`: most-recent `<release>` is `version="4.0.0"`, date corrected `2026-06-05` → `2026-06-14`
- [x] 4.2 **BLOCKER FOUND + FIXED:** the stable manifest's `karere` module had regressed to `type: dir, path: ..`, which `update-flatpak.yml` would publish to Flathub unmodified (its tag/commit `sed` is a no-op on a dir source) → unbuildable Flathub manifest. Restored v3's proven pattern: `type: git` + `url` + `tag: v4.0.0` + `commit:` placeholder (workflow overwrites at publish). Devel manifest correctly stays `type: dir`. `cargo-sources.json` regen not needed (no dep churn — only the local `karere` version line changed in `Cargo.lock`)
- [x] 4.3 `packaging/flathub-beta/…yml`: re-pinned `tag: v4.0.0` + placeholder `commit:` (synced to the real tag commit when the beta-branch PR is opened)
- [x] 4.4 `packaging/flathub-beta/README.md`: fixed the now-stale claim that the stable in-tree manifest is `type: dir` (corrected by 4.2) → points at the Devel manifest as the `type: dir` local one. The `4.0.0-betaN` steps are intentional recurring-process placeholders, left as-is
- [x] 4.5 Manifests (stable/Devel/beta) parse as YAML; metainfo XML well-formed and `appstreamcli validate` successful (14 info-level notices, pre-existing)

## 5. CI/CD continuity verification

- [x] 5.1 Confirmed: v4 carries `ci.yml`, `build-cef-codecs.yml`, `update-flatpak.yml`, `.github/scripts/smoke-test.sh`; v3-only `build-webkitgtk.yml` / `build-gst-plugin-audiofx.yml` absent (intentional — superseded by `build-cef-codecs.yml`)
- [x] 5.2 Confirmed: `update-flatpak.yml` copies only `cargo-sources.json` (no `cargo-sources-gst.json`) and excludes pre-releases via `- '!v*-*'`
- [x] 5.3 Confirmed: `ci.yml` builds `…Devel.yml` + runs `smoke-test.sh`; cache-key files (`…Devel.yml`, `cargo-sources.json`, `Cargo.lock`) all exist
- [ ] 5.4 After promotion (post §8): confirm `ci.yml` runs on the `main` push, and after tagging (post §9) confirm `update-flatpak.yml` fired on `v4.0.0`

## 6. Build verification (pre-promotion)

- [x] 6.1 Config verified: `cargo metadata` → `karere 4.0.0`, `meson.build` → `4.0.0`, no stray `4.0.0-beta` in shipping files. Full Flatpak build deferred to CI (`ci.yml` builds the unchanged Devel `type: dir` manifest on push); the stable `type: git` manifest is validated by the Flathub buildbot post-tag (a git-source build needs the tag to exist)
- [x] 6.2 Committed on `karere-4-gtk-cef`: `release: 4.0.0 — promote v4 to stable`

## 7. Preserve v3 history

- [ ] 7.1 `git tag v3-final <v3 main tip from 1.3>` and `git push origin v3-final`
- [ ] 7.2 (Optional, per Open Question) `git branch v3 <v3 main tip>` and `git push origin v3`
- [ ] 7.3 Verify every v3 commit is reachable from a ref (`git tag --contains` / `git branch --contains` spot check)

## 8. Promote v4 to main (DESTRUCTIVE — gate behind explicit confirmation)

- [ ] 8.1 `git checkout main && git reset --hard karere-4-gtk-cef`
- [ ] 8.2 Verify local `main` tree == v4 tip (`git diff karere-4-gtk-cef main` is empty)
- [ ] 8.3 `git push --force-with-lease origin main`
- [ ] 8.4 Confirm `origin/main` tip equals the v4 release commit

## 9. Tag stable release

- [ ] 9.1 `git tag -a v4.0.0 main -m "Karere 4.0.0"` on the new `main` tip
- [ ] 9.2 `git push origin v4.0.0`
- [ ] 9.3 Confirm the `update-flatpak` workflow fired and opened the Flathub stable PR (do not auto-merge)

## 10. Post-release verification

- [ ] 10.1 Fresh-clone smoke check or `git fetch && git reset --hard origin/main`; confirm version reads `4.0.0` everywhere
- [ ] 10.2 Confirm `v3-final` / `v3.*` tags / `origin/master` all still resolve (no v3 loss)
- [ ] 10.3 Confirm `ci.yml` ran green on the new `main` HEAD (build + headless smoke gate)
- [ ] 10.4 Note orphaned old-`main`-based branches (`feat/multi-account-completion`, `voip-fix-wip`, `account1009/main`) for the maintainer; no action required
