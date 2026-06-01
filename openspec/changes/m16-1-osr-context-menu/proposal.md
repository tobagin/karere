## Why

The app renders the page windowless (OSR), and in that mode CEF cannot draw its own context menu — it relies on the host to display one. `ContextMenuHandler::run_context_menu` is unimplemented, so **no native right-click menu appears anywhere in the page**: no spellcheck suggestions, no cut/copy/paste, no link actions. The `cef-context-menu` capability today only edits the menu model (`on_before_context_menu`), which is invisible because nothing ever renders it. M16 shipped live spellcheck underlines but the suggestions that go with them are unreachable for exactly this reason.

## What Changes

- Implement `ContextMenuHandler::run_context_menu` to build a **GTK menu from the CEF `MenuModel`**, position it at the cursor over the `GLArea`, and on activation invoke `RunContextMenuCallback::cont(command_id, event_flags)` (and `cancel()` on dismiss). This makes the context menu actually appear in OSR mode.
- Surface Chromium's **spellcheck suggestions + "Add to dictionary"** from `ContextMenuParams::misspelled_word()` / `dictionary_suggestions()`, plus standard editing commands (cut/copy/paste/select-all), respecting the existing `strip_open_link_entries` link policy.
- Add main-thread-only plumbing to carry the non-`Send` `RunContextMenuCallback` from the handler to the GTK menu interaction (it cannot ride the cross-thread `Arc<Mutex>` `SharedRef`).
- Add **auto-correct** (deferred from m16 §8.3): a per-word, suggestion-driven replace (suggestion source, word-boundary detection, replace-on-space), wiring the existing `enable-auto-correct` GSettings key, with an M22 preference to toggle it.
- The m16 prerequisite `data/js/20-spellcheck-contextmenu.js` (unblocks WhatsApp's `contextmenu` `preventDefault` on editable fields) becomes functional once the menu renders.

## Capabilities

### New Capabilities
- `spellcheck-autocorrect`: Optional automatic correction of misspelled words in editable fields as the user types, driven by Chromium's spellcheck suggestions, gated by the `enable-auto-correct` GSettings key.

### Modified Capabilities
- `cef-context-menu`: Extend from "edit the menu model" to "render and display the menu in OSR" — implement `run_context_menu` (GTK menu from `MenuModel`, cursor positioning, callback dispatch) so right-click menus (including spellcheck suggestions and cut/copy/paste) actually appear in the windowless view.

## Impact

- `src/handlers/context_menu.rs`: implement `run_context_menu` / `on_context_menu_command` / `cancel_context_menu`; keep `on_before_context_menu` link-strip policy.
- `src/web_view.rs` + `src/handlers/mod.rs` (`SharedRef`): main-thread-only channel to hand the menu request (model snapshot + non-`Send` callback) and cursor position to the GTK widget; reuse `last_mouse_x/y`.
- New menu rendering in the `KarereWebView`/window layer (GTK `PopoverMenu`/`gio::Menu` from the CEF model).
- Auto-correct: new logic (suggestion source + replace-on-space, likely an injected JS bridge in `data/js/` cooperating with host spellcheck), `enable-auto-correct` wiring, and an M22 Preferences toggle.
- No new Cargo dependencies expected; spellcheck data comes from Chromium (`libcef.so`).
- Depends on m16 (live spellcheck + the `20-spellcheck-contextmenu.js` unblock prerequisite).
