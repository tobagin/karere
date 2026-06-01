## Context

Karere v3 wired accessibility prefs directly to GTK runtime state and to Chromium command-line flags. In v4 these bindings have not yet been ported; users currently have no in-app control over animation suppression, focus ring strength, or caret-browsing. M19 introduces the minimal surface needed for parity, deferring the actual UI controls to M22 (preferences page).

## Goals / Non-Goals

**Goals:**
- Add three GSettings keys with sensible defaults (`false`) and bind two of them to live GTK/CSS state.
- Provide a single bundled stylesheet that scopes enhanced focus styling under a `.enhanced-focus` class so the rules only apply when opted in.
- Append `--enable-caret-browsing` to the CEF command line when `screen-reader-opts` is `true`, with restart-required semantics surfaced to users.

**Non-Goals:**
- Overriding libadwaita's automatic high-contrast detection.
- Live-toggling Chromium flags without restart.
- Exposing the preferences UI (covered by M22).
- Per-account or per-window scoping; settings are application-global.

## Decisions

- **GtkSettings binding for motion**: invert the boolean and write to `gtk-enable-animations` via `GtkSettings::default().set_property(...)`. Connect `connect_changed("reduce-motion", ...)` to propagate runtime changes; no widget rebuild required.
- **CSS class toggle for focus**: avoid runtime stylesheet swapping. Load the stylesheet once at `GtkApplication::startup` via `gtk::CssProvider::load_from_resource("/.../style.css")` added to the default display with `STYLE_PROVIDER_PRIORITY_APPLICATION`. Toggle `enhanced-focus` on the root window via `add_css_class` / `remove_css_class` driven by the setting.
- **CEF flag injection**: read the GSetting inside `on_before_command_line_processing` (where the command line is mutable) and append the switch. Document the restart requirement in the preferences page (M22).
- **gschema location**: extend the existing application gschema rather than introducing a new schema, keeping all user-visible prefs in one place.

## Risks / Trade-offs

- `gtk-enable-animations` is a process-wide GtkSettings property; changing it affects all top-level windows, which is the intended behavior.
- CSS resource priority must be `APPLICATION` (not `USER`) so user theme overrides still win where appropriate.
- Caret-browsing requires a restart because CEF command-line switches are read once at subprocess launch; we accept this and surface it in the UI rather than implementing a more expensive live-reconfig path.
- Default `false` for all three keys preserves current behavior for existing users on upgrade.
