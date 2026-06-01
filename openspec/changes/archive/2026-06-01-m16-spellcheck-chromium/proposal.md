## Why

Karere v3 relied on WebKit's Hunspell-backed spellcheck. CEF/Chromium ships its own spellchecker that downloads `.bdic` dictionaries to `cache_path` on first need, so we do not need to bundle Hunspell in the Flatpak. We need to wire CEF's checker to GSettings and surface a karere v3-style language picker so users can match the v3 spell-check experience on gtk-cef-shell. Locked decision: use Chromium auto-download (no Hunspell module in Flatpak); accept that language changes require browser recreation since Chromium reads `--spell-check-languages` only at startup.

## What Changes

- Append `--disable-spell-checking` and `--spell-check-languages=<joined>` to the Chromium command line based on GSettings (`spell-checking-enabled`, `spell-checking-languages`).
- Auto-detect mode derives a single language from `glib::language_names()[0]`.
- Add `src/spellcheck.rs` ported verbatim from karere v3 (`parse_locale`, `display_name`, `region_name`, `short_code`, `KNOWN_LANGUAGES`).
- Add `src/spellcheck_ui.rs` (or extend `window.rs`) with a headerbar `gtk::DropDown` + star-pin row factory; selection updates GSettings and triggers `KarereWebView::recreate_active_browser()`.
- Preferences page (M22 hosts) mirrors the dropdown and shows a "Changing language reloads the page" notice.
- New GSettings keys: `spell-checking-enabled`, `spell-check-auto-detect`, `spell-checking-languages`, `spell-check-favorites`.
- `KarereWebView::recreate_active_browser()` captures URL, scroll position, zoom; closes the host; respawns a browser with a fresh `RequestContext`; restores state on `on_load_end`.

## Capabilities

### New Capabilities
- `spellcheck-chromium`: Wire Chromium's built-in spellchecker into the CEF runtime via command-line switches sourced from GSettings, including auto-detect from `glib::language_names()` and language-aware browser recreation.
- `spellcheck-ui`: GTK4/libadwaita headerbar language dropdown with star-pin favorites and a Preferences mirror, persisting selection and favorites in GSettings.

### Modified Capabilities
<!-- none -->

## Impact

- `src/cef_runtime.rs::on_before_command_line_processing`: read GSettings, append Chromium switches.
- New `src/spellcheck.rs` and `src/spellcheck_ui.rs` modules; possible additions to `src/window.rs`.
- `KarereWebView`: new `recreate_active_browser()` flow capturing/restoring URL, scroll, zoom.
- New gschema keys (`spell-checking-enabled`, `spell-check-auto-detect`, `spell-checking-languages`, `spell-check-favorites`).
- No new Cargo dependencies (Chromium spellchecker ships in libcef.so).
- UX: changing language reloads the active page; first launch downloads `.bdic` files to `cache_path/dictionaries/`.
