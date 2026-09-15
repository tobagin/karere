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
- [x] 5.4 Confirmed: `ci.yml` fired on the `main` push (run 27496857898); `update-flatpak.yml` fired on `v4.0.0` (run 27496861032, success in 8s) and opened Flathub PR #162 with the correct rewritten pin (`type: git`, `tag: v4.0.0`, `commit: dcfe128`) — end-to-end proof the 4.2 fix works

## 6. Build verification (pre-promotion)

- [x] 6.1 Config verified: `cargo metadata` → `karere 4.0.0`, `meson.build` → `4.0.0`, no stray `4.0.0-beta` in shipping files. Full Flatpak build deferred to CI (`ci.yml` builds the unchanged Devel `type: dir` manifest on push); the stable `type: git` manifest is validated by the Flathub buildbot post-tag (a git-source build needs the tag to exist)
- [x] 6.2 Committed on `karere-4-gtk-cef`: `release: 4.0.0 — promote v4 to stable`

## 7. Preserve v3 history

- [x] 7.1 `v3-final` tag created at `efd1a5f` (pure v3 tip) and pushed
- [x] 7.2 `v3` branch created at `efd1a5f` and pushed (`origin/v3`)
- [x] 7.3 Verified v3 reachable (222 commits). **DISCOVERY:** `origin/main`'s true tip was `679545f` — 4 commits *beyond* `efd1a5f`: stray **v4 CEF-CI** commits (`build-cef-codecs.yml` + `tools/build-cef-codecs.sh`) committed onto the v3 main by mistake. These are parallel duplicates of v4's own `afaae5b/4d276ae/d788405/be6a564`, which v4 then *superseded* with 3 newer commits (`1a37861` shallow-Chromium-sync, `8a26eae` comment trim, `e6ed9f7` #151 patch). No unique content lost. Preserved `679545f` anyway under tag `pre-v4-promotion` (pushed) before force-push

## 8. Promote v4 to main (DESTRUCTIVE — gate behind explicit confirmation)

- [x] 8.1 `git checkout main && git reset --hard karere-4-gtk-cef` → main at `dcfe128`
- [x] 8.2 Verified local `main` tree == v4 tip (empty diff)
- [x] 8.3 `git push --force-with-lease=main:679545f origin main` → forced update `679545f...dcfe128`
- [x] 8.4 Confirmed `origin/main` == `dcfe128` (v4 release commit)

## 9. Tag stable release

- [x] 9.1 `git tag -a v4.0.0 main -m "Karere 4.0.0"` → at `dcfe128`
- [x] 9.2 `git push origin v4.0.0`
- [x] 9.3 `update-flatpak` fired and opened Flathub PR #162 — left open (not merged)

## 10. Post-release verification

- [x] 10.1 Verified at `origin/main`: meson `4.0.0`, Cargo `4.0.0`, metainfo top release `4.0.0` (2026-06-14), CHANGELOG `[4.0.0] - 2026-06-14`
- [x] 10.2 Confirmed all v3 refs resolve: `v3` branch + `origin/v3`, tags `v3-final` / `pre-v4-promotion` / `v3.0.0–3.1.1`, and `origin/master` (`ff52459`) — no v3 loss
- [x] 10.3 `ci.yml` on new `main` HEAD (run 27496857898) — build + headless smoke gate **passed (success)**
- [x] 10.4 Orphaned old-`main`-based branches noted for the maintainer (no action): `feat/multi-account-completion` (`origin` d248d87), `voip-fix-wip`, `account1009/main`. They predate the v4 line
