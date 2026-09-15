## Why

M7-M23 delivered a working Karere v4: CEF rendering, multi-account, notifications, tray, spell-check, downloads, accessibility, mobile responsive UI, preferences, and a finalized Flatpak. The work is done; v3.1.1 users now need an actual release. M24 is the release pipeline: bump versions, finalize the changelog with every M7-M22 feature in user-facing language, ship a one-shot first-run migration AdwAlertDialog that warns v3 users they must re-link, replace the README with v4 content, tag `v4.0.0`, and submit to Flathub. The migration dialog is the only user-visible behavior change in M24 — everything else is metadata, tagging, and packaging.

## What Changes

- Bump `Cargo.toml` from `4.0.0-dev` to `4.0.0` and bump the `meson.build` `project()` version to match.
- Add a `<release version="4.0.0" date="YYYY-MM-DD">` entry to `data/io.github.tobagin.karere.metainfo.xml.in.in` listing every M7-M22 user-facing feature in plain language (engine switch, account auto-discovery, tray, portal notifications, spell-check, downloads, find-in-page, DevTools, zoom per account, mobile responsive UI, preferences dialog, accessibility).
- Finalize the `4.0.0` entry in `CHANGELOG.md` with the locked headline ("Switched rendering engine from WebKitGTK to CEF (Chromium 148); account identity now auto-discovered from WhatsApp Web; tray + portal notifications + spell-check via Chromium.") and the locked migration note.
- Add a one-shot first-run migration `AdwAlertDialog` that fires on launch when `$XDG_DATA_HOME/karere/sessions/` exists from v3 AND `$XDG_DATA_HOME/karere/accounts/accounts.json` does not yet exist. Title "Welcome to Karere 4.0", body explaining the engine switch and that history stays on the phone, with "Open Settings" and "Got it" actions. After dismissal a GSetting `migration-acknowledged-v4` is set to `true` so the dialog never re-fires.
- Replace `README.md` with v4 content: the v3 README plus a "Now built on CEF/Chromium 148" section and the locked decisions list.
- Tag `v4.0.0` once master CI is green.
- Build a Flathub submission packet against the separate `flathub/io.github.tobagin.karere` repo: copy `packaging/io.github.tobagin.karere.yml` and `packaging/cargo-sources.json` into the Flathub repo root, then open a PR titled "Add Karere 4.0.0" with a body linking the upstream tag and noting CEF licensing (Chromium BSD + LGPL libraries; `libcef.so` dynamically loaded).
- Run a smoke-test matrix against KDE Plasma 6 (Wayland and X11, x86_64), GNOME 50 (Wayland and X11, x86_64), XFCE (X11, x86_64), and aarch64 KDE via CI or QEMU.

## Capabilities

### New Capabilities

- `release-tagging`: version bump in `Cargo.toml` and `meson.build`, metainfo `<release>` entry covering M7-M22, finalized CHANGELOG `4.0.0` entry, README rewrite, and the `v4.0.0` git tag.
- `release-migration-notice`: one-shot first-run `AdwAlertDialog` for upgrading v3 users, gated by detection of legacy `sessions/` data and absence of new `accounts/accounts.json`, latched off by a `migration-acknowledged-v4` GSetting.
- `release-flathub-submission`: Flathub submission packet — manifest and cargo-sources copied to `flathub/io.github.tobagin.karere`, PR opened with a CEF licensing note, and a documented smoke-test matrix run before publish.

### Modified Capabilities

(none — M24 only introduces release-pipeline capabilities; existing capabilities stay as shipped through M23.)

## Impact

- Files: `Cargo.toml`, `meson.build`, `data/io.github.tobagin.karere.metainfo.xml.in.in`, `CHANGELOG.md`, `README.md`, new migration dialog source under the existing app module, a new GSettings key in the project schema, `packaging/io.github.tobagin.karere.yml`, `packaging/cargo-sources.json`.
- External: a new PR against the `flathub/io.github.tobagin.karere` repository.
- Users: upgrading v3.1.1 installs show the migration dialog exactly once; fresh v4 installs never see it.
- Non-goals: updating any marketing site, refreshing app-index listings beyond Flathub itself, or shipping additional code-level features.
