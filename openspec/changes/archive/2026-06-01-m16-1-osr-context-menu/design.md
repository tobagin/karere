## Context

The shell renders WhatsApp Web windowless (OSR): CEF paints into a buffer that a `gtk::GLArea` (`KarereWebView`) blits. In OSR mode CEF has no native window to host a context menu, so it delegates display to the app via `ContextMenuHandler::run_context_menu`. That method is currently unimplemented (defaults to returning 0/false), and CEF cannot fall back to drawing the menu itself — so right-click produces nothing from CEF. The only menus the user sees are WhatsApp's own JS menus. `on_before_context_menu` already runs and edits the `MenuModel` (the m16 link-strip policy), but that work is invisible.

M16 added live spellcheck underlines plus `data/js/20-spellcheck-contextmenu.js`, which stops WhatsApp's composer from `preventDefault()`-ing the `contextmenu` event so a menu *can* be generated — but with no renderer it still shows nothing. This change supplies the renderer and the spellcheck/edit actions, and folds in the deferred auto-correct feature.

Key constraint: `run_context_menu` is invoked on the CEF UI thread, which in this app is the glib main thread (external message pump). GTK work is therefore legal directly. However, the `RunContextMenuCallback` handed to `run_context_menu` is a CEF ref-counted object that is **not `Send`**, so it cannot be stored in the existing `SharedRef = Arc<Mutex<SharedState>>` (which crosses threads). It needs a main-thread-only path to the GTK widget.

## Goals / Non-Goals

**Goals:**
- Render the CEF `MenuModel` as a real GTK menu over the `GLArea` at the cursor, and dispatch the chosen command back through `RunContextMenuCallback`.
- Show spellcheck suggestions + "Add to dictionary" and standard edit commands (cut/copy/paste/select-all) for editable fields; keep the existing link-entry strip policy.
- Implement optional auto-correct gated by `enable-auto-correct`, with an M22 preference.

**Non-Goals:**
- Re-theming or fully custom menu chrome beyond what GTK provides.
- Changing WhatsApp's own message/selection menus (only editable-field right-clicks unblock the native menu).
- Server-side/"enhanced" (cloud) spellcheck.
- Building the M22 Preferences dialog itself (this change only adds its spellcheck/autocorrect rows where M22 hosts them).

## Decisions

- **GTK `PopoverMenu` + `gio::Menu` from the CEF `MenuModel`.** Walk the model (`count`, `label_at`, `command_id_at`, `type_at`, `is_enabled_at`, submenus) into a `gio::Menu`, mapping each item to a `win`/local action whose activation calls `callback.cont(command_id, 0)`. Submenus (spellcheck suggestions live in one) recurse. Alternative considered: `gtk::PopoverMenu` built by hand with buttons — rejected as more code and worse a11y than a `gio::Menu` model.
- **Cursor positioning via the existing `last_mouse_x/last_mouse_y`.** `run_context_menu` provides `ContextMenuParams::x/y` in view coordinates; anchor the popover to that rectangle in the `GLArea`. Reuse the pointer tracking already in `web_view.rs`.
- **Main-thread-only callback handoff.** Add a non-`Send` per-widget slot (e.g. a `RefCell<Option<RunContextMenuCallback>>` owned by `KarereWebView`, reached via a main-thread registry keyed by browser identifier, or via the handler holding a `glib::Sender`/weak widget ref). Return `1` from `run_context_menu` to tell CEF the host will display the menu; call `cont`/`cancel` exactly once. This deliberately bypasses `SharedRef` because the callback is not `Send`.
- **Spellcheck items from params, not guesswork.** Use `ContextMenuParams::misspelled_word()` and `dictionary_suggestions()` to label/group the suggestion items and the "Add to dictionary" command, rather than parsing the model blindly.
- **Auto-correct as an injected-JS + host cooperation.** Chromium exposes no desktop autocorrect pref and no JS spellcheck API, so detect word boundaries in the composer via injected JS and replace the just-typed word with the top suggestion. The suggestion source is the open question below; the JS handles word-boundary detection and the in-place text replacement, gated on `enable-auto-correct`.

## Risks / Trade-offs

- **Non-`Send` callback lifetime** → if `cont`/`cancel` is never called (widget destroyed mid-menu), CEF leaks the menu state. Mitigation: ensure exactly one of `cont`/`cancel` fires (popover `closed` signal → `cancel` if no item chosen), and drop the slot on unrealize.
- **Coordinate/scale mismatch** (HiDPI, GLArea scale factor) → menu appears at the wrong spot. Mitigation: apply the same scale handling used for input forwarding in `web_view.rs`.
- **Auto-correct suggestion source** → without a host-queryable per-word checker, autocorrect quality is limited. Mitigation: scope to top-suggestion replace; surface accuracy expectations; keep it opt-in (default off may be safer — decide in specs).
- **WhatsApp re-suppressing the menu** → if the composer changes its event handling, the unblock JS may need updating. Mitigation: capture-phase `stopImmediatePropagation` already runs first; monitor.

## Open Questions (resolved)

- **Auto-correct suggestion source** → **small built-in common-typos map** in injected JS (`data/js/30-autocorrect.js`). Chromium exposes no JS spellcheck API and no desktop autocorrect preference, and a hidden right-click round-trip is hacky and racy; a conservative typo map gives unambiguous, high-confidence fixes with no host round-trip. The JS detects word completion (space/punctuation) and replaces via `execCommand('insertText')` (contenteditable) or `.value` splice (input/textarea), preserving capitalization; unknown words are left untouched (still underlined).
- **Auto-correct default** → **off** (`enable-auto-correct` schema default set to `false`). The limited typo-map source means auto-replace should be opt-in to avoid surprising substitutions; the user enables it in Preferences.
- **"Add to dictionary" persistence** → handled by Chromium itself: the spellcheck "Add to dictionary" command id arrives in the snapshotted `MenuModel` and round-trips through `RunContextMenuCallback::cont`, so Chromium writes its custom dictionary under `cef_user_data` and stops underlining the word. The host does not special-case it.

## Implementation notes

- Spellcheck suggestions + "Add to dictionary" + cut/copy/paste are surfaced by **mirroring the whole `MenuModel`** (which already contains them for an editable misspelled-word right-click), not by hand-building from `misspelled_word()`/`dictionary_suggestions()`. The params are still read (debug logging) to confirm the suggestion set, but the model is the source of truth so command ids are always correct.
- The non-`Send` `RunContextMenuCallback` reaches the widget via a **main-thread `thread_local` registry keyed by CEF browser id** (`web_view.rs::CTX_MENU_WIDGETS`); the widget registers on browser spawn and unregisters (cancelling any in-flight menu) on close/unrealize.
