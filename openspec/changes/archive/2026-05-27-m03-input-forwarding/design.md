## Context

`CefGtkArea` (built in M1/M2) hosts an off-screen CEF browser inside a `GtkGLArea`. CEF off-screen rendering has no native window, so the embedder must hand every input event to `BrowserHost` via `send_mouse_*_event`, `send_key_event`, and `set_focus`. GTK4 expresses these events through `EventController*` objects attached to the widget.

## Goals / Non-Goals

**Goals:**
- Make the offscreen browser fully interactive for pointer, scroll, keyboard, and focus on X11 and Wayland.
- Match Chromium's expectations for modifier bits, virtual-key codes, scroll sign, and HiDPI scaling.
- Keep all input glue in one place (`cef_gtk_area.rs`) behind a single `install_input_controllers` entry point.

**Non-Goals:**
- Touch events (CEF supports them; deferred until needed).
- IME pre-edit / composition (handled in a later milestone).
- Drag-and-drop, which has its own surface area in M17.

## Decisions

- **Use GTK4 `EventController*` rather than legacy `connect_*_event` signals.** They are the only supported path on GTK4 and they expose modifier state cleanly.
- **One `GestureClick` per button (1..=3) instead of a single controller with button=0.** `GestureClick` only emits press/release for the button it was configured with; three controllers map cleanly to LEFT/MIDDLE/RIGHT.
- **Flip the sign of scroll deltas and multiply by `STEP = 40`.** GTK reports positive `dy` when scrolling down; CEF expects negative `delta_y` to scroll content down. `40` matches Chromium's wheel-tick magnitude well enough that page scrolling feels native.
- **Always emit `KEYEVENT_CHAR` after `RAWKEYDOWN` for printable keys.** Off-screen CEF does not synthesise char events from raw key-downs; text fields stay empty without this second dispatch.
- **Map common navigation keys to Windows VK codes by hand and fall back to `to_unicode().to_ascii_uppercase()`.** CEF's `windows_key_code` is documented as a Windows VK; the small handcrafted table covers BackSpace/Tab/Return/Escape/Page*/End/Home/arrows/Insert/Delete; everything else uses the upper-cased Unicode value, which matches Chromium's behaviour for ASCII keys.
- **`with_host(widget, f)` helper locks the browser mutex once per event.** Centralises the unwrap-or-skip pattern and avoids re-implementing locking inside every controller closure.

## Risks / Trade-offs

- `STEP = 40` is a magic constant. Different mice/trackpads may feel too fast or slow → tune later; a per-event multiplier from GTK's smooth-scroll deltas already smooths most of this out.
- The VK fallback `to_unicode().to_ascii_uppercase() as i32` is ASCII-only; non-Latin layouts will dispatch the bare Unicode codepoint, which Chromium tolerates for `CHAR` events but may misbehave for shortcuts → revisit when IME work begins.
- No touch / pen support → users on touch devices get no input on the surface. Acceptable for the desktop target.
- Modifier translation only covers the documented `cef_event_flags_t` set; CapsLock and NumLock are ignored (matches Chromium's renderer-side handling).

## Migration Plan

Pure additive change inside `CefGtkArea::constructed`. No data on disk, no API consumers outside this crate. Rollback is a single revert of `src/cef_gtk_area.rs`.

## Open Questions

- Should we expose `STEP` as a setting once preferences land (M14)?
- Do we want to suppress the browser context menu here or strictly in M9?
