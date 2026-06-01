## 1. Plumbing

- [x] 1.1 Make `CefGtkArea` focusable (`set_focusable(true)`, `set_can_focus(true)`) and call `install_input_controllers(widget)` from `constructed`
- [x] 1.2 Add `with_host(widget, f)` helper that locks the browser mutex and yields `&BrowserHost`
- [x] 1.3 Add `scale(widget)` helper returning `widget.scale_factor().max(1)`
- [x] 1.4 Add `modifiers_from_state(ModifierType) -> u32` mapping SHIFT/CONTROL/ALT/SUPER/BUTTON1/2/3 to `EVENTFLAG_*` bits

## 2. Pointer and wheel

- [x] 2.1 Install `EventControllerMotion`; wire `connect_motion` → `send_move(.., leave=false)` and `connect_leave` → `send_move(.., leave=true)`
- [x] 2.2 Install three `GestureClick`s (buttons 1..=3); on press grab focus and call `send_click(down=true, n_press)`; on release call `send_click(down=false)`
- [x] 2.3 Install `EventControllerScroll::new(BOTH_AXES)`; `connect_scroll` returns `Stop` and calls `send_wheel(dx, dy, modifiers)` with `STEP = 40` and flipped sign

## 3. Keyboard and focus

- [x] 3.1 Add `gdk_key_to_vk(Key) -> i32` with the navigation-key table and Unicode-uppercase fallback
- [x] 3.2 Install `EventControllerKey`; `connect_key_pressed` calls `send_key(down=true)` (returns `Stop`); `connect_key_released` calls `send_key(down=false)`
- [x] 3.3 In `send_key`, after dispatching `RAWKEYDOWN` and when `character != 0`, dispatch a second `KEYEVENT_CHAR` with `windows_key_code = character`
- [x] 3.4 Install `EventControllerFocus`; `connect_enter` → `host.set_focus(1)`; `connect_leave` → `host.set_focus(0)`

## 4. Verify

- [x] 4.1 Load `https://html5demos.com/forms`, type into a text field, confirm characters appear
- [x] 4.2 Confirm scroll wheel scrolls the page in the expected direction
- [x] 4.3 Confirm right-click shows Chromium's default context menu (stripping deferred to M9)
