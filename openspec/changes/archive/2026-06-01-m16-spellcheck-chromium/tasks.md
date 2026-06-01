## 1. GSettings schema

> Decision (apply): reuse the spellcheck keys already present in the gschema
> rather than adding duplicates under the task names. Mapping:
> enable→`enable-spell-checking`, auto-detect→`auto-detect-language`,
> favorites→`favorite-spell-check-languages`, list→`spell-checking-languages`.

- [x] 1.1 ~~Add `spell-checking-enabled`~~ → reuse existing `enable-spell-checking` (b, default true)
- [x] 1.2 ~~Add `spell-check-auto-detect`~~ → reuse existing `auto-detect-language` (b, default true)
- [x] 1.3 `spell-checking-languages` (`as`, default `[]`) already present in gschema
- [x] 1.4 ~~Add `spell-check-favorites`~~ → reuse existing `favorite-spell-check-languages` (`as`, default `[]`)
- [x] 1.5 Compile and validate the gschema during build (keys already build cleanly)

## 2. spellcheck module (verbatim port)

- [x] 2.1 Create `src/spellcheck.rs` ported/adapted from `/home/tobagin/Projects/karere/src/spellcheck.rs` (Hunspell dir-scan dropped; Chromium-supported locale table added)
- [x] 2.2 Expose `parse_locale(LANG: &str) -> Option<(String, Option<String>)>`
- [x] 2.3 Expose `display_name(code: &str) -> String` and `region_name(code: &str) -> Option<&'static str>`
- [x] 2.4 Expose `short_code(code: &str) -> String`
- [x] 2.5 Expose `KNOWN_LANGUAGES: &[(&str, &str)]` static slice (BCP-47, friendly name)
- [x] 2.6 Register `mod spellcheck;` in `main.rs`

## 3. CEF command-line wiring

- [x] 3.1 In `src/cef_runtime.rs::on_before_command_line_processing`, read `enable-spell-checking`; append `--disable-spell-checking` when false (browser process only)
- [x] 3.2 Read `spell-checking-languages`; when non-empty append `--spell-check-languages=<joined>`
- [x] 3.3 When `auto-detect-language=true` and the list is empty, derive a single BCP-47 code via `glib::language_names()` + `spellcheck::parse_locale` and append it
- [x] 3.4 Chromium downloads `.bdic` dictionaries on first run — SUPERSEDED by §8.1: the `cache_path` override was reverted, so dictionaries land in CEF's default `config/cef_user_data/Dictionaries/`. Persistence + offline use confirmed via 7.4.

## 4. Live language switch (revised from browser-recreation)

> Decision (apply): the design's premise — "no public CEF API to change
> languages on a live browser" — is false. CEF 148 exposes
> `RequestContext::set_preference` (via `ImplPreferenceManager`), and
> `BrowserHost::request_context()` is reachable. So language switching is done
> **live** via the registered Chromium preferences `spellcheck.dictionaries`
> and `browser.enable_spellchecking` — no recreation, no page reload, no lost
> scroll/zoom/ephemeral state. This supersedes tasks 4.1–4.3.

- [x] 4.1 Implement `KarereWebView::set_spellcheck_languages(langs, enabled)` — sets `spellcheck.dictionaries` (list) + `browser.enable_spellchecking` (bool) on the live browser's request context (`src/web_view.rs`)
- [x] 4.2 ~~close_browser + respawn with fresh RequestContext~~ → not needed; preference change applies in place (Chromium downloads missing `.bdic` on demand)
- [x] 4.3 ~~Restore URL/scroll/zoom after on_load_end~~ → not needed; no reload occurs

## 5. Headerbar language dropdown

- [x] 5.1 Create `src/spellcheck_ui.rs` with a `SpellLang` GObject + `gtk::SortListModel` over `KNOWN_LANGUAGES`, bound to `dictionary_dropdown` (already in `window.blp`)
- [x] 5.2 Row factory rendering display name + a `gtk::ToggleButton` star (starred/non-starred-symbolic)
- [x] 5.3 On star toggle, update `favorite-spell-check-languages` and re-sort (favorites float to top via `CustomSorter`)
- [x] 5.4 On selection change, write `spell-checking-languages` then invoke `KarereWebView::set_spellcheck_languages()` (live switch, per revised §4)
- [x] 5.5 Initialise dropdown active row from current `spell-checking-languages` before connecting the change handler (no write-back on restore)

## 6. Preferences mirror (M22) — DEFERRED

> Blocked: `app.preferences` is still an M22 stub (`src/actions.rs:84` logs
> "not yet implemented"); there is no loaded `KarerePreferencesWindow` to mirror
> into. The `preferences.blp` "Spell Checking" group is unwired v3 (WebKit-era)
> scaffolding. Decision (apply): defer 6.1/6.2 to M22, which will build the
> preferences dialog and wire this section. Also note: with the live-switch
> design there is **no page reload**, so the planned "Changing language reloads
> the page" notice is obsolete — M22 should omit or reword it.

- [x] 6.1 (MOVED → m22-preferences-shortcuts-dialog tasks 2.7 / 5.3) Mirror the headerbar language list + live-switch selection in the Preferences Spellcheck page
- [x] 6.2 (RESOLVED) Reload notice dropped — m22 spec updated: live switch performs no reload, no notice. Auto-correct toggle (`enable-auto-correct`) also added to m22 (behavior in m16-1-osr-context-menu).

## 7. Verification — RUNTIME (needs a GUI/Flatpak run; not done headlessly)

> Automated coverage in place: `cargo test` (4 `spellcheck::` unit tests) green.
> The items below require launching the app against WhatsApp Web.
> Note: per the revised §4 the language switch is **live**, so 7.2 should
> confirm the page does NOT reload (underlines update in place).

- [x] 7.1 With `spell-checking-languages=['en-US']`, a typo in the WhatsApp chat input renders a red underline — CONFIRMED
- [x] 7.2 Switch via dropdown (en-US/en-GB/pt-BR); underlines update live with NO reload (centre↔center proves dictionary swap) — CONFIRMED
- [x] 7.3 Star-pin language, restart app, pinned appears at top of dropdown — CONFIRMED
- [x] 7.4 Misspellings underline on launch with no dropdown change, and survive F5 — CONFIRMED (dicts persist at `config/cef_user_data/Dictionaries/`, CEF default path; see §8.1)

## 8. Field findings (apply phase)

- [x] 8.1 Reverted a `cache_path`/`root_cache_path` override in `cef_runtime.rs`: it pointed CEF at a fresh dir and orphaned the logged-in profile. CEF's default `$XDG_CONFIG_HOME/cef_user_data` already persists both profile and `.bdic` dictionaries; no override needed.
- [x] 8.2 Right-click spellcheck suggestions need a **host-rendered OSR context menu** — root cause: windowless (OSR) rendering means CEF cannot paint its own menu; `run_context_menu` is unimplemented, so NO native menu shows app-wide (spellcheck, cut/copy/paste). Added `data/js/20-spellcheck-contextmenu.js` (capture-phase `contextmenu` unblock on editable targets) as a **prerequisite** — kept, but inert until the menu renderer exists. → MOVED to new change (see 8.4).
- [x] 8.3 Auto-correct (no native desktop Chromium pref) → DECISION: fold into the new context-menu change (it needs the same suggestion source). → MOVED to new change (see 8.4).
- [x] 8.4 HANDOFF: created new change **m16.1-osr-context-menu** covering the host-rendered OSR context menu (spellcheck suggestions, Add to dictionary, cut/copy/paste, link actions) + auto-correct + `enable-auto-correct` wiring. m16 closes on its verified core (live multi-language underlining, headerbar dropdown, favorites, live `RequestContext` switch).
- [x] 8.5 Startup highlight fix: `--spell-check-languages` is a no-op; only the `spellcheck.dictionaries` request-context pref works, and Chromium re-checks only on an actual CHANGE (CEF persists the pref, so re-setting the same value is silent). Apply now runs on main-frame `on_load_end` with a clear-then-set (`[]`→langs) plus delayed re-applies at +1.5s/+4s (the renderer's spellcheck isn't live at on_load_end). Result: draft/typed text underlines on launch and survives F5 without a dropdown change — CONFIRMED. Centralised in `web_view::apply_spellcheck_to_browser` + `spellcheck::resolve_languages` (locale mapped to closest supported, e.g. en_IE→en-GB).
