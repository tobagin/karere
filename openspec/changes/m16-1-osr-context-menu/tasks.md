## 1. Callback plumbing (main-thread only)

- [ ] 1.1 Add a non-`Send` per-widget slot on `KarereWebView` to hold a pending `RunContextMenuCallback` (e.g. `RefCell<Option<RunContextMenuCallback>>`), reachable from the context-menu handler on the main thread (registry keyed by browser id, or weak widget ref / `glib::Sender`)
- [ ] 1.2 Ensure the slot is cleared on `unrealize`/close so a destroyed widget calls `cancel()` for any in-flight menu
- [ ] 1.3 Confirm the path never crosses threads (do NOT route through `Arc<Mutex>` `SharedRef`)

## 2. run_context_menu rendering

- [ ] 2.1 Implement `ContextMenuHandler::run_context_menu` in `src/handlers/context_menu.rs`: snapshot the `MenuModel` (labels, command ids, item types, enabled state, submenus) into a plain data tree
- [ ] 2.2 Hand the snapshot + cursor position (`ContextMenuParams::x/y`) + the callback to the widget; return `1` to take ownership of display
- [ ] 2.3 Build a `gio::Menu` from the snapshot and present it as a `gtk::PopoverMenu` anchored at the cursor rectangle over the `GLArea`
- [ ] 2.4 Map each activated item to `RunContextMenuCallback::cont(command_id, 0)`; recurse for submenus
- [ ] 2.5 On popover `closed` without a selection, call `RunContextMenuCallback::cancel()`; guarantee exactly one of `cont`/`cancel`
- [ ] 2.6 Implement `on_context_menu_command` / `cancel_context_menu` as needed; keep `on_before_context_menu` link-strip policy intact

## 3. Cursor placement & scale

- [ ] 3.1 Translate `ContextMenuParams::x/y` through the view scale factor (reuse `last_mouse_x/last_mouse_y` handling in `web_view.rs`)
- [ ] 3.2 Verify placement on a HiDPI display (scale > 1) lands at the pointer

## 4. Spellcheck items

- [ ] 4.1 Read `ContextMenuParams::misspelled_word()` + `dictionary_suggestions()` and group the suggestion items + "Add to dictionary" in the menu
- [ ] 4.2 Verify choosing a suggestion replaces the word in the WhatsApp composer (via the dispatched command id)
- [ ] 4.3 Verify "Add to dictionary" stops the word being underlined and persists to Chromium's custom dictionary under `cef_user_data`

## 5. Standard editing commands

- [ ] 5.1 Ensure cut/copy/paste/select-all appear and work in editable fields via the host menu
- [ ] 5.2 Verify the m16 prerequisite `data/js/20-spellcheck-contextmenu.js` now lets the menu generate over the composer (no WhatsApp suppression)

## 6. Auto-correct (resolve open question first)

- [ ] 6.1 Decide the suggestion source for auto-correct (Chromium round-trip vs lightweight checker vs common-typos map) and the default on/off — record in design Open Questions
- [ ] 6.2 Add injected JS to detect word completion (space/punctuation) in editable fields and replace the just-typed word with the top suggestion, gated on `enable-auto-correct`
- [ ] 6.3 Wire the existing `enable-auto-correct` GSettings key to enable/disable the behavior at runtime
- [ ] 6.4 Skip replacement when no confident suggestion exists (leave the word underlined)

## 7. Preferences (M22)

- [ ] 7.1 Add an auto-correct toggle bound to `enable-auto-correct` in the M22 Preferences spellcheck section (depends on M22 building the dialog)

## 8. Verification

- [ ] 8.1 Right-click a misspelled word in the composer → host menu shows suggestions + "Add to dictionary"; choosing one replaces the word
- [ ] 8.2 Right-click plain text/link → menu shows copy/paste etc. with no "Open Link in New Tab/Window/Incognito" (link policy intact)
- [ ] 8.3 Dismiss menu with Escape/click-away → `cancel()` fires, no leak, no stuck menu
- [ ] 8.4 With auto-correct on, a misspelled word is corrected on space; with it off, only underlined
- [ ] 8.5 HiDPI: menu appears at the pointer
