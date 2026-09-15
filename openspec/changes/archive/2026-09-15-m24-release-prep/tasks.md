## 1. Version bumps

- [ ] 1.1 Change `Cargo.toml` `[package]` `version` from `4.0.0-dev` to `4.0.0`.
- [ ] 1.2 Change `meson.build` `project()` `version` to `'4.0.0'`.
- [ ] 1.3 Run `cargo check` and a Meson reconfigure to confirm both build systems still resolve.
- [ ] 1.4 Commit "release: bump version to 4.0.0".

## 2. Metainfo release entry

- [ ] 2.1 In `data/io.github.tobagin.karere.metainfo.xml.in.in`, add a `<release version="4.0.0" date="YYYY-MM-DD">` element above prior releases.
- [ ] 2.2 Fill the description with user-facing bullets covering: engine switch to CEF/Chromium 148, multi-account auto-discovery, tray, portal notifications, spell-check, downloads, find-in-page, DevTools, zoom-per-account, paste bridge, accessibility, mobile responsive UI, preferences dialog.
- [ ] 2.3 Validate the metainfo with `appstream-util validate-relax` (or the project's existing metainfo validator).
- [ ] 2.4 Replace the `YYYY-MM-DD` placeholder with the actual tag date during the tag step (Task 8.2).

## 3. CHANGELOG finalization

- [ ] 3.1 In `CHANGELOG.md`, replace any `Unreleased` heading for the v4 line with `## 4.0.0 - YYYY-MM-DD`.
- [ ] 3.2 Add the locked headline verbatim: "Switched rendering engine from WebKitGTK to CEF (Chromium 148); account identity now auto-discovered from WhatsApp Web; tray + portal notifications + spell-check via Chromium."
- [ ] 3.3 Append the locked migration note paragraph explaining re-link and on-phone history.
- [ ] 3.4 List the M7-M22 user-facing changes underneath the headline.
- [ ] 3.5 Replace the date placeholder during the tag step (Task 8.2).

## 4. README rewrite

- [ ] 4.1 Copy the v3 `README.md` structure as the v4 base.
- [ ] 4.2 Add a top-level section "Now built on CEF/Chromium 148" describing the engine switch and what it changes for users.
- [ ] 4.3 Add a "Locked architectural decisions" list summarizing the per-milestone locked decisions from M1-M23.
- [ ] 4.4 Refresh install instructions to point at the Flathub install command.

## 5. Migration dialog implementation

- [ ] 5.1 Add a new GSettings key `migration-acknowledged-v4` of type `b` defaulting to `false` to the project's GSettings schema.
- [ ] 5.2 On application activation, before showing the main window content, check whether `$XDG_DATA_HOME/karere/sessions/` exists, `$XDG_DATA_HOME/karere/accounts/accounts.json` does NOT exist, and `migration-acknowledged-v4` is `false`.
- [ ] 5.3 If all three conditions hold, construct an `AdwAlertDialog` titled "Welcome to Karere 4.0".
- [ ] 5.4 Set the body to: "Karere 4 uses a new web engine. You'll need to re-link your WhatsApp account(s). Existing chat history stays on your phone; old session data can be removed safely."
- [ ] 5.5 Add responses `open-settings` (label "Open Settings") and `got-it` (label "Got it").
- [ ] 5.6 Connect the `response` signal so any response, including window-close dismissal, sets `migration-acknowledged-v4` to `true`.
- [ ] 5.7 On `open-settings`, present the existing add-account dialog after the migration dialog closes.
- [ ] 5.8 Ensure the migration flow never reads, writes, moves, or deletes anything under `sessions/`.

## 6. Migration dialog tests

- [ ] 6.1 Unit-test the trigger predicate against all 8 boolean combinations of (sessions present, accounts.json present, acknowledged) and confirm only the (true, false, false) case fires.
- [ ] 6.2 Manually verify on a clean v4 install that the dialog does not appear.
- [ ] 6.3 Manually verify with a synthetic `sessions/` directory and no `accounts.json` that the dialog appears.
- [ ] 6.4 Manually verify that after dismissal the dialog does not appear on the next launch.
- [ ] 6.5 Manually verify that "Open Settings" presents the add-account dialog.

## 7. Flathub submission packet preparation

- [ ] 7.1 Clone or check out the `flathub/io.github.tobagin.karere` repository.
- [ ] 7.2 Copy `packaging/io.github.tobagin.karere.yml` from this repo to the Flathub repo root as `io.github.tobagin.karere.yml`.
- [ ] 7.3 Copy `packaging/cargo-sources.json` from this repo to the Flathub repo root as `cargo-sources.json`.
- [ ] 7.4 Verify byte-equality with `cmp` against the in-tree files at the `v4.0.0` tag.

## 8. Tag and Flathub PR

- [ ] 8.1 Confirm master CI is green at the head commit.
- [ ] 8.2 Update the metainfo `<release date>` and the `CHANGELOG.md` heading date to today's date and commit as "release: lock 4.0.0 date".
- [ ] 8.3 Create an annotated tag `v4.0.0` at that commit.
- [ ] 8.4 Push the tag.
- [ ] 8.5 Open the Flathub PR with title "Add Karere 4.0.0".
- [ ] 8.6 In the PR body, link the upstream `v4.0.0` tag and state: "Chromium ships under BSD with LGPL libraries; `libcef.so` is dynamically loaded by the application."

## 9. Smoke-test matrix

- [ ] 9.1 Run the smoke-test flow on KDE Plasma 6 / Wayland / x86_64: pair account, send + receive text, send + receive media, accept a call, close-to-tray, quit.
- [ ] 9.2 Run the smoke-test flow on KDE Plasma 6 / X11 / x86_64.
- [ ] 9.3 Run the smoke-test flow on GNOME 50 / Wayland / x86_64.
- [ ] 9.4 Run the smoke-test flow on GNOME 50 / X11 / x86_64.
- [ ] 9.5 Run the smoke-test flow on XFCE / X11 / x86_64.
- [ ] 9.6 Run the smoke-test flow on aarch64 KDE via CI or QEMU.
- [ ] 9.7 Record pass/fail for each target before requesting a Flathub merge.

## 10. Upgrade verification

- [ ] 10.1 On a test machine running v3.1.1, perform a Flatpak upgrade to the candidate v4.0.0 build.
- [ ] 10.2 Confirm the migration dialog appears on first launch.
- [ ] 10.3 Confirm the dialog does NOT reappear on the second launch.
- [ ] 10.4 Confirm `flatpak install --user flathub io.github.tobagin.karere` succeeds once the Flathub PR is merged.
