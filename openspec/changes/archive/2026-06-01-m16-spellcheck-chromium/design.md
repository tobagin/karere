## Context

CEF/Chromium has an embedded spellchecker that downloads Hunspell-format `.bdic` dictionaries to `cache_path/dictionaries/` on first use. It is configured exclusively via command-line switches (`--disable-spell-checking`, `--spell-check-languages=<csv>`) read at browser startup; there is no public CEF API to change languages on a live browser. Karere v3 solved the equivalent problem with WebKit's Hunspell integration plus a custom GTK language dropdown that supported pin-to-top favorites. We want to reproduce the v3 UX on gtk-cef-shell while staying within CEF's startup-only constraint.

## Goals / Non-Goals

**Goals:**
- Honor GSettings for enable/disable, language list, auto-detect, and favorites.
- Reuse karere v3's `spellcheck.rs` verbatim for BCP-47 parsing, display names, region names, short-codes, and the `KNOWN_LANGUAGES` table.
- Provide a headerbar `gtk::DropDown` with star-pin favorites; mirror in Preferences (M22).
- Recreate the active browser when the language list changes, preserving URL, scroll, and zoom.
- Download dictionaries lazily via Chromium; offline-capable on second launch.

**Non-Goals:**
- Live language switching without browser recreation (locked decision: not pursued, includes DevTools-Protocol-driven toggles).
- Bundling Hunspell or `.bdic` files in the Flatpak.
- Custom suggestion menus (Chromium's context menu handles suggestions natively).

## Decisions

> **Revision (apply phase):** two premises here proved wrong and were corrected
> during implementation:
> 1. GSettings keys already existed under different names — M16 reuses
>    `enable-spell-checking` / `auto-detect-language` /
>    `favorite-spell-check-languages` / `spell-checking-languages` instead of
>    adding renamed duplicates.
> 2. "No public CEF API to change languages on a live browser" is **false**.
>    CEF 148 exposes `RequestContext::set_preference`; language switching is done
>    live via the `spellcheck.dictionaries` + `browser.enable_spellchecking`
>    preferences — **no browser recreation, no reload**. The recreation design
>    (and its URL/scroll/zoom capture-restore) is dropped.

- **Chromium auto-download over Hunspell**: avoids shipping dictionaries and a Hunspell module in the Flatpak; matches CEF's default path.
- **Startup language config + live switch**: append `--spell-check-languages=<csv>` (and `--disable-spell-checking`) in `on_before_command_line_processing` for the initial state; runtime language changes go through `KarereWebView::set_spellcheck_languages()` writing request-context preferences in place.
- **Auto-detect**: when `spell-check-auto-detect=true` and `spell-checking-languages` is empty, derive a single BCP-47 code from `glib::language_names()[0]` via `spellcheck::parse_locale`.
- **Verbatim port of karere v3 `spellcheck.rs`**: minimizes risk and keeps language metadata aligned with v3.
- **Star-pin favorites**: persisted as an `as` array in `spell-check-favorites`; favorites render at top of the dropdown via a custom row factory with a `gtk::ToggleButton` star.
- **State capture for recreation**: URL, scroll position (injected JS), and zoom level captured before `host.close_browser(0)`; restored after `on_load_end` of the respawned browser.
- **Page reload UX cost is accepted**: surfaced via a `gtk::Label` notice in Preferences.

## Risks / Trade-offs

- **First-launch dictionary download requires network**: if offline on first run, no underlines until next online launch. Mitigation: notice in Preferences.
- **Browser recreation loses ephemeral state** (open menus, partial form input outside captured fields). Mitigation: explicit UX notice; users acknowledge by changing language.
- **`KNOWN_LANGUAGES` drift from Chromium's supported list**: a chosen language may have no `.bdic` available. Mitigation: trust Chromium's silent fallback; consider future telemetry of download failures.
- **GSettings race during recreation**: writing the key and recreating must be ordered. Mitigation: write GSettings synchronously, then call `recreate_active_browser()`.
- **`RequestContext` lifecycle**: dictionaries live under `cache_path`; recreation uses a fresh `RequestContext` to pick up new switches without leaking state.
