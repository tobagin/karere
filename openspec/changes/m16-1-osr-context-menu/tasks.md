## 1. Callback plumbing (main-thread only)

- [x] 1.1 Add a non-`Send` per-widget slot on `KarereWebView` to hold a pending `RunContextMenuCallback` (e.g. `RefCell<Option<RunContextMenuCallback>>`), reachable from the context-menu handler on the main thread (registry keyed by browser id, or weak widget ref / `glib::Sender`)
- [x] 1.2 Ensure the slot is cleared on `unrealize`/close so a destroyed widget calls `cancel()` for any in-flight menu
- [x] 1.3 Confirm the path never crosses threads (do NOT route through `Arc<Mutex>` `SharedRef`)

## 2. run_context_menu rendering

- [x] 2.1 Implement `ContextMenuHandler::run_context_menu` in `src/handlers/context_menu.rs`: snapshot the `MenuModel` (labels, command ids, item types, enabled state, submenus) into a plain data tree
- [x] 2.2 Hand the snapshot + cursor position (`ContextMenuParams::x/y`) + the callback to the widget; return `1` to take ownership of display
- [x] 2.3 Build a `gio::Menu` from the snapshot and present it as a `gtk::PopoverMenu` anchored at the cursor rectangle over the `GLArea`
- [x] 2.4 Map each activated item to `RunContextMenuCallback::cont(command_id, 0)`; recurse for submenus
- [x] 2.5 On popover `closed` without a selection, call `RunContextMenuCallback::cancel()`; guarantee exactly one of `cont`/`cancel`
- [x] 2.6 Implement `on_context_menu_command` / `cancel_context_menu` as needed; keep `on_before_context_menu` link-strip policy intact (neither extra override needed — `cont` dispatches the command through CEF directly; link-strip `on_before_context_menu` kept)

## 3. Cursor placement & scale

- [x] 3.1 Translate `ContextMenuParams::x/y` through the view scale factor (reuse `last_mouse_x/last_mouse_y` handling in `web_view.rs`)
- [x] 3.2 Verify placement on a HiDPI display (scale > 1) lands at the pointer

## 4. Spellcheck items

- [x] 4.1 Read `ContextMenuParams::misspelled_word()` + `dictionary_suggestions()` and group the suggestion items + "Add to dictionary" in the menu (suggestions + "Add to dictionary" arrive in the mirrored `MenuModel`; params read for debug confirmation)
- [x] 4.2 Verify choosing a suggestion replaces the word in the WhatsApp composer (via the dispatched command id)
- [x] 4.3 Verify "Add to dictionary" stops the word being underlined and persists to Chromium's custom dictionary under `cef_user_data`

## 5. Standard editing commands

- [x] 5.1 Ensure cut/copy/paste/select-all appear and work in editable fields via the host menu (mirrored from the model; activation dispatches the command id via `cont`)
- [x] 5.2 Verify the m16 prerequisite `data/js/20-spellcheck-contextmenu.js` now lets the menu generate over the composer (no WhatsApp suppression)

## 6. Auto-correct (resolve open question first)

- [x] 6.1 Decide the suggestion source for auto-correct (Chromium round-trip vs lightweight checker vs common-typos map) and the default on/off — record in design Open Questions (chosen: built-in common-typos map, default off — see design "Open Questions (resolved)")
- [x] 6.2 Add injected JS to detect word completion (space/punctuation) in editable fields and replace the just-typed word with the top suggestion, gated on `enable-auto-correct` (`data/js/30-autocorrect.js`)
- [x] 6.3 Wire the existing `enable-auto-correct` GSettings key to enable/disable the behavior at runtime (load handler re-seeds on navigation; `window.rs` pushes on change)
- [x] 6.4 Skip replacement when no confident suggestion exists (leave the word underlined) (`correct()` returns null for words absent from the map)

## 7. Preferences (M22)

- [x] 7.1 Add an auto-correct toggle bound to `enable-auto-correct` in the M22 Preferences spellcheck section — DESCOPED to M22, which already owns it: M22 `tasks.md` 2.7 (bind the `enable-auto-correct` switch) + 5.7 (verify), and spec scenario "Auto-correct toggle binds the GSetting". The behavior side (the `enable-auto-correct` runtime wiring + JS) is implemented in this change; M22 only binds the switch when it builds the dialog. `row_auto_correct` already exists in `data/ui/preferences.blp`.

## 8. Verification

- [x] 8.1 Right-click a misspelled word in the composer → host menu shows suggestions + "Add to dictionary"; choosing one replaces the word
- [x] 8.2 Right-click plain text/link → menu shows copy/paste etc. with no "Open Link in New Tab/Window/Incognito" (link policy intact)
- [x] 8.3 Dismiss menu with Escape/click-away → `cancel()` fires, no leak, no stuck menu
- [x] 8.4 With auto-correct on, a misspelled word is corrected on space; with it off, only underlined (limited to the built-in common-typos map by design — Chromium exposes no JS spellcheck API)
- [x] 8.5 HiDPI: menu appears at the pointer
