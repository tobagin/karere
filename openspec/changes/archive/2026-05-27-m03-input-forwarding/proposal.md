## Why

After M2 the CEF browser renders into the GtkGLArea but does not respond to any user input — pointer movement, clicks, scroll, keys, and focus are dropped on the floor. Without input forwarding the embedded browser is unusable for any real app.

## What Changes

- Install GTK4 event controllers on `CefGtkArea` (motion, click x3, scroll, key, focus) and forward every event to the active `BrowserHost`.
- Make the widget focusable (`set_focusable(true)`, `set_can_focus(true)`) so it receives keyboard focus on click.
- Translate GTK `ModifierType` and `gdk::Key` values into CEF `EVENTFLAG_*` bits and Windows virtual-key codes.
- Apply the widget's HiDPI scale factor to pointer coordinates and wheel deltas.
- For text input, emit a `KEYEVENT_CHAR` after `RAWKEYDOWN` whenever the key produces a Unicode character.

## Capabilities

### New Capabilities
- `cef-input-forwarding`: Forwarding of GTK pointer, scroll, keyboard, and focus events into the CEF off-screen browser host, including modifier, virtual-key, and HiDPI-scale translation.

### Modified Capabilities
<!-- none -->

## Impact

- `src/cef_gtk_area.rs`: adds `install_input_controllers` plus helpers `send_move`, `send_click`, `send_wheel`, `send_key`, `modifiers_from_state`, `gdk_key_to_vk`, `scale`, `with_host`.
- Depends on `cef::sys::cef_event_flags_t::EVENTFLAG_*` constants and `cef::{MouseEvent, MouseButtonType, KeyEvent, KeyEventType}` types already pulled in by M1.
- Non-goals (deferred): touch events, IME pre-edit composition, drag-and-drop (tracked under M17).
