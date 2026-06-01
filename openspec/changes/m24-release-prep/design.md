## Context

After M7-M23 the application is feature-complete: CEF/Chromium 148 rendering, multi-account auto-discovery, tray, portal notifications, spell-check, downloads, DevTools, find-in-page, zoom-per-account, paste bridge, accessibility, mobile responsive UI, preferences dialog, and a finalized Flatpak manifest. There is no shipped v4 yet — `Cargo.toml` carries `4.0.0-dev` and the metainfo lacks a `4.0.0` release entry. v3.1.1 users in the field have local data under `$XDG_DATA_HOME/karere/sessions/` that the CEF engine cannot consume; they need a clear, one-time prompt explaining that they must re-link. The release also requires a Flathub submission and a smoke-test pass across the targets the project supports.

## Goals / Non-Goals

**Goals:**
- Make every version-bearing file say `4.0.0`: `Cargo.toml`, `meson.build`, the metainfo `<release>` block.
- Translate M7-M22 engineering milestones into a single user-facing CHANGELOG entry and metainfo release notes.
- Show upgrading v3 users a clear, one-shot migration dialog so they understand the engine switch and the need to re-link.
- Land the v4.0.0 build on Flathub.
- Confirm the build works on the supported desktop matrix before publish.

**Non-Goals:**
- Marketing site updates.
- App-index listings beyond Flathub.
- Any new code-level features beyond the migration dialog.
- Migrating v3 session data into the v4 accounts store (the dialog explicitly tells the user to re-link instead).

## Decisions

**Version single-source: `Cargo.toml` first, then `meson.build`.** `Cargo.toml` is the source of truth for the Rust crate version; `meson.build`'s `project()` version is bumped to match so packaging tooling and the metainfo template stay aligned. Both must change in the same commit.

**Metainfo release entry covers M7-M22, not M23.** M23 was a packaging milestone with no user-visible features. The `<release version="4.0.0">` block enumerates only what users will notice: engine switch, multi-account, tray, notifications, spell-check, downloads, find-in-page, DevTools, zoom, mobile responsive UI, preferences, accessibility.

**CHANGELOG headline is locked verbatim.** "Switched rendering engine from WebKitGTK to CEF (Chromium 148); account identity now auto-discovered from WhatsApp Web; tray + portal notifications + spell-check via Chromium." This phrasing was approved upstream and must not be paraphrased.

**Migration dialog trigger condition is conjunctive, not disjunctive.** The dialog fires only when BOTH `$XDG_DATA_HOME/karere/sessions/` exists AND `$XDG_DATA_HOME/karere/accounts/accounts.json` does NOT exist. The first half detects "this is a v3 user". The second half detects "they have not already paired under v4". Either half alone would either spam fresh installs (first half) or annoy users who paired before reading the dialog (second half).

**One-shot latch via GSettings, not a file marker.** A new `migration-acknowledged-v4` boolean GSetting (default `false`) latches the dialog off after dismissal. Using GSettings rather than a sentinel file keeps the state in the same store the rest of the app already uses and survives the `~/.var/app/io.github.tobagin.karere/` sandbox cleanly.

**Two-action dialog: "Open Settings" and "Got it".** "Open Settings" opens the add-account flow directly so the user is one click from re-linking. "Got it" simply dismisses. Both set the GSetting; the dialog never re-fires regardless of which action was chosen.

**Body text does not promise data migration.** The body says "history stays on your phone; old session data can be removed safely" — it deliberately does not offer to migrate or delete the legacy directory automatically. Users can remove `sessions/` manually if they want; the app does not touch it.

**Flathub submission is a separate-repo PR, not a manifest update in-tree.** The in-tree `packaging/` directory holds the canonical manifest and `cargo-sources.json`; the Flathub repo gets copies of both. Keeping the canonical files in this repo means future updates only require copying — no diverging edits in two places.

**CEF licensing is called out in the Flathub PR body.** Chromium ships under BSD with a constellation of LGPL libraries; `libcef.so` is loaded dynamically. Reviewers need to see this stated explicitly to clear license review without round-trips.

**Smoke-test matrix is the publish gate.** No Flathub merge until the matrix passes. aarch64 may run in QEMU or CI rather than on bare metal; the other targets are bare-metal or VM, not CI.

**Tag after CI is green on master, before the Flathub PR opens.** The Flathub PR body needs to link an upstream tag, so the tag must exist first. CI must be green at the tagged commit.

## Risks / Trade-offs

**A v3 user might dismiss the dialog without re-linking and then forget what it said.** Mitigated by the "Open Settings" action wiring directly to the add-account dialog, and by the CHANGELOG/README also explaining the change. Not mitigated for users who installed v4 without reading anything — there is no second prompt by design.

**Detecting v3 data by directory existence is heuristic.** A user who manually created `$XDG_DATA_HOME/karere/sessions/` for some unrelated reason would see the dialog spuriously. Acceptable: it dismisses in one click and never re-fires.

**Flathub review may ask for extra license metadata.** The CEF licensing note in the PR body is a best-effort pre-empt. If review pushes back, the manifest or appdata may need additional `<provides>` or license tags; we treat that as in-scope follow-up but not a blocker to opening the PR.

**The smoke-test matrix is large enough to slow the release.** Six targets is the minimum to cover the supported desktop landscape; cutting any target risks a regression report on day one. We accept the wall-clock cost.

**Pinning the metainfo `date` is fragile.** The release date in `<release date="YYYY-MM-DD">` must match the tag date. The tag-and-PR task explicitly checks this; if the date slips between bump and tag, the metainfo is updated again before tagging.
