## Why

Karere v3 (`window.rs:2211-2257`) bound several accessibility preferences to GTK/CSS state so that users could opt into reduced motion, enhanced focus rings, and screen-reader-friendly Chromium flags. v4 currently has no equivalent surface. M19 restores parity by re-binding these prefs to the v4 GTK4/libadwaita window and to the embedded CEF runtime. High-contrast theming is intentionally left to libadwaita's automatic detection (read-only, no override).

## What Changes

- Introduce GSettings keys `reduce-motion`, `focus-indicators-enhanced`, and `screen-reader-opts` (all `b`, default `false`).
- `reduce-motion` binds to `GtkSettings::gtk-enable-animations` (inverted), with live updates via `connect_changed`.
- `focus-indicators-enhanced` toggles a CSS class `enhanced-focus` on the root window.
- Ship a CSS resource defining `.enhanced-focus *:focus` rules (3 px accent-color outline + 2 px offset); load once at application startup via `gtk::CssProvider::load_from_resource`.
- `screen-reader-opts` appends `--enable-caret-browsing` in `cef_runtime::on_before_command_line_processing`; restart required (documented in preferences).
- Preferences page (M22) will expose all three switches.

## Capabilities

### New Capabilities
- `a11y-reduce-motion`: binds the `reduce-motion` GSetting to `GtkSettings::gtk-enable-animations` and propagates runtime changes.
- `a11y-focus-indicators`: toggles a CSS class on the root window driven by `focus-indicators-enhanced`, backed by a bundled stylesheet resource.
- `a11y-screen-reader`: appends Chromium caret-browsing flag when `screen-reader-opts` is enabled, with restart-required semantics.

### Modified Capabilities
<!-- none -->

## Impact

- Code: `src/window.rs`, `src/application.rs` (CSS provider loading at `startup`), `src/cef_runtime.rs`, `data/resources/style.css` (new), `data/resources/resources.gresource.xml`, gschema XML.
- UX: new switches surfaced by M22 preferences. `screen-reader-opts` is restart-required.
- Non-goals: high-contrast theme override (libadwaita auto-detects; we do not expose an override).
