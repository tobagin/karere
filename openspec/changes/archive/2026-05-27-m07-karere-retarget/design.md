## Context

`gtk-cef-shell` was an experimental generic GTK4 + CEF shell. Karere v3.1.1 (`/home/tobagin/Projects/karere`) is a shipping WebKitGTK WhatsApp client. We are collapsing the two: gtk-cef-shell becomes Karere v4, swapping WebKitGTK for CEF while keeping Karere's UI surface, gschema, i18n, icons, sounds, and app-id. M7 establishes the new identity and tooling pipeline; subsequent milestones port behavior (M8 persistence, M14 notifications, M15 tray, M20 multi-account, M22 prefs).

Constraints:
- Directory name on disk stays `gtk-cef-shell` (we do not rename the worktree).
- Karere assets (icons, sounds, blueprints, gschema, po files) must be copied **verbatim** — no value edits, no key renames — so v3 user settings migrate cleanly.
- License must become GPL-3.0-or-later because copied karere code is GPL.
- Flatpak SDK provides `blueprint-compiler`; host developers may not, so build.rs must surface a clear error.

## Goals / Non-Goals

**Goals:**
- Rename Cargo package, binary, app-id, gschema, desktop, metainfo, gresource paths to `karere` / `io.github.tobagin.karere`.
- Copy karere icons, sounds, blueprints, gschema, po tree, LICENSE, README verbatim.
- Wire build.rs to compile `.blp` → `.ui` at build time, with host detection.
- Wire meson `i18n.gettext('karere', preset: 'glib')` + `subdir('po')`; re-enable icon cache update.
- Initialize gettext in `src/main.rs` so translated strings load at runtime.
- Rename Rust types (`ShellApplication` → `KarereApplication`, `ShellWindow` → `KarereWindow`, `CefGtkArea` → `KarereWebView`).
- Produce a flatpak that runs `flatpak run io.github.tobagin.karere --url=https://web.whatsapp.com` and shows karere branding + WhatsApp QR page.

**Non-Goals:**
- Window geometry persistence (M8).
- GAction wiring beyond what compiles (M8).
- Preferences dialog implementation (M22).
- Tray icon (M15).
- Native notifications via `notify-send` / portal (M14).
- Multi-account / `KarereSession` model (M20).
- Modifying any karere gschema key values or blueprint contents.
- Renaming the on-disk worktree from `gtk-cef-shell`.

## Decisions

### Decision: Hard-fork over template/feature-flag

We discussed three models: (a) keep gtk-cef-shell generic + add karere as a feature flag, (b) gtk-cef-shell stays a template and karere becomes a downstream consumer via `app-config.toml`, (c) hard-fork — gtk-cef-shell *becomes* karere. We chose (c).

Rationale: karere has too many bespoke behaviors (spell-check, notification batching, tray, sounds, multi-account) to express as flags without the config surface dwarfing the feature itself. The "generic shell" abstraction was an early hedge; we have one downstream and it is karere.

Alternatives rejected: (a) leaks every karere feature into the generic shell as conditional code; (b) requires a stable plugin ABI before we even know what the plugin points are.

### Decision: Verbatim asset copy, no migration shim

Karere v3 ships `io.github.tobagin.karere.gschema.xml.in` with a specific key set. We copy it byte-for-byte. Reason: v3 users upgrading to v4 keep their dconf settings (notification volume, spell-check languages, window geometry) without a migration step. The CEF code in later milestones must read the existing keys; it does not get to invent new key names.

Alternative rejected: "clean slate" gschema designed around CEF. Would force every v3 user to reconfigure.

### Decision: Blueprint compiled at build.rs, gresource picks $OUT_DIR first

The blueprint sources live in `data/ui/*.blp`. We could either (a) commit pre-compiled `.ui` alongside `.blp`, or (b) compile at build time. We pick (b) — committing generated artifacts invites drift. build.rs runs `blueprint-compiler compile-file` for each blp; gresource paths in `data/karere.gresource.xml` reference `$OUT_DIR/ui/<name>.ui` via the `prefix`/`alias` mechanism; if `$OUT_DIR/ui/<name>.ui` is absent (e.g., docs build with no compiler), fall back to `data/ui/<name>.ui` if present.

Host detection: `which blueprint-compiler`. On miss, `panic!` with `apt install blueprint-compiler` / `dnf install blueprint-compiler` / `flatpak run org.gnome.Sdk//47` hints. We do not auto-download.

### Decision: gettext-rs with `gettext-system` feature

Two viable crates: `gettextrs` and `gettext-rs`. We pick `gettext-rs = { version = "0.7", features = ["gettext-system"] }` because the `gettext-system` feature links against the system libintl (matching what the flatpak runtime ships) instead of vendoring. Karere v3 uses the same crate; copy/paste of `src/main.rs:23-80` works unchanged.

### Decision: Rename Rust types in this milestone (not deferred)

`ShellApplication` etc. are renamed here even though some fields (CEF lifecycle handles, account context) will be reshuffled in M20. Reason: every subsequent milestone references the type names; landing the rename now avoids churn in M8–M22 PRs. The M20 field shuffle is a separate concern from the type name.

### Decision: Keep `gtk-cef-shell` as on-disk worktree name

Renaming the directory would invalidate every absolute path in tooling, hooks, IDE state, and the OpenSpec history. The Cargo package, binary, app-id, etc. inside the directory all become `karere`; the directory label is cosmetic.

## Risks / Trade-offs

- [Karere gschema schema-id collision on dev machines that already have karere v3 installed] → flatpak isolates dconf per-app-id, but on a non-flatpak dev `cargo run`, both v3 and v4 would write to the same dconf path. Mitigation: dev runs go through `flatpak-builder --user --install`, not host `cargo run`. Document this in README.
- [Blueprint compiler version skew between host and flatpak SDK] → blueprint-compiler is API-stable for the syntax karere uses; pin SDK runtime version in the manifest (already done at M6).
- [GPL-3.0-or-later relicensing of pre-existing gtk-cef-shell code] → we authored gtk-cef-shell ourselves; we can relicense.
- [Verbatim copy of karere `.blp` references widget IDs that the CEF rewrite has not yet wired] → downstream milestones (M8, M22) wire them. M7 must not modify the blueprints to "fit" current code; instead, current code uses what it uses and the rest sits dormant in gresource until later milestones consume it.
- [gettext bindtextdomain path hard-coded to `/app/share/locale`] → fine for flatpak (the only supported run target after M6); non-flatpak dev gets untranslated strings, which is acceptable.

## Migration Plan

1. Land Cargo.toml + meson.build + LICENSE renames in one commit (build still passes with old assets).
2. Copy assets verbatim from `/home/tobagin/Projects/karere` in a second commit (no code changes, just files).
3. Wire build.rs blueprint step + gresource path updates in a third commit.
4. Rename Rust types + gettext init in a fourth commit.
5. Update packaging manifest + verify flatpak build.
6. No rollback story needed — this is forward-only fork work; if it breaks, revert the commits.

## Open Questions

- Do we want `karere-cef` or just `karere` as the binary name? Locked: `karere` (matches v3, lets users replace their v3 install in-place).
- Should `app-id` include a `.cef` suffix for parallel install with v3? Locked: no. v4 replaces v3.
