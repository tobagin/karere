## ADDED Requirements

### Requirement: Pointer motion forwarding
The widget SHALL forward pointer motion and pointer-leave events from GTK to the CEF browser host as `send_mouse_move_event` calls, with coordinates multiplied by the widget's scale factor and modifiers translated to CEF event flags.

#### Scenario: Pointer moves over the widget
- **WHEN** the user moves the pointer across `CefGtkArea`
- **THEN** the widget calls `host.send_mouse_move_event` with the scaled x/y, the translated modifiers, and `leave = 0`

#### Scenario: Pointer leaves the widget
- **WHEN** the pointer exits the widget bounds
- **THEN** the widget calls `host.send_mouse_move_event` with `leave = 1`

### Requirement: Mouse button forwarding
The widget SHALL forward presses and releases for mouse buttons 1, 2, and 3 to the CEF browser host as `send_mouse_click_event` calls, mapping button 1 to LEFT, 2 to MIDDLE, 3 to RIGHT, and grabbing keyboard focus on press.

#### Scenario: Left button press
- **WHEN** the user presses mouse button 1 on the widget
- **THEN** the widget grabs focus and calls `host.send_mouse_click_event` with `MouseButtonType::LEFT`, `mouse_up = 0`, and `click_count = max(n_press, 1)`

#### Scenario: Right button release
- **WHEN** the user releases mouse button 3 on the widget
- **THEN** the widget calls `host.send_mouse_click_event` with `MouseButtonType::RIGHT` and `mouse_up = 1`

### Requirement: Scroll wheel forwarding
The widget SHALL forward two-axis scroll events to `send_mouse_wheel_event` with deltas multiplied by a fixed step (40) and the widget scale factor, with the sign flipped so natural scrolling matches Chromium's expectation, and SHALL consume the GTK scroll event.

#### Scenario: Vertical scroll
- **WHEN** the user scrolls the wheel downward by `dy`
- **THEN** the widget calls `host.send_mouse_wheel_event` with `delta_y = -dy * 40 * scale` and returns `Propagation::Stop`

### Requirement: Keyboard event forwarding
The widget SHALL forward key presses and releases as CEF `KeyEvent`s of type `RAWKEYDOWN` / `KEYUP`, populating `windows_key_code` via a GDK-to-VK mapping, `native_key_code` from the hardware keycode, `character` and `unmodified_character` from the keyval's Unicode value, and SHALL additionally dispatch a `KEYEVENT_CHAR` after every key-down that produces a non-zero character.

#### Scenario: Printable character typed
- **WHEN** the user presses the `a` key
- **THEN** the widget dispatches one `RAWKEYDOWN` followed by one `KEYEVENT_CHAR` whose `windows_key_code` equals the Unicode character

#### Scenario: Navigation key pressed
- **WHEN** the user presses `Left`
- **THEN** the widget dispatches a `RAWKEYDOWN` with `windows_key_code = 0x25`

#### Scenario: Key released
- **WHEN** the user releases any key
- **THEN** the widget dispatches a `KEYUP` event with matching `windows_key_code` and modifiers

### Requirement: Focus forwarding
The widget SHALL be focusable and SHALL inform the CEF browser host whenever it gains or loses GTK focus via `BrowserHost::set_focus`.

#### Scenario: Widget gains focus
- **WHEN** the widget receives focus-enter
- **THEN** the widget calls `host.set_focus(1)`

#### Scenario: Widget loses focus
- **WHEN** the widget receives focus-leave
- **THEN** the widget calls `host.set_focus(0)`

### Requirement: Modifier and scale translation
The widget SHALL translate GTK `ModifierType` to CEF event flags (`SHIFT`, `CONTROL`, `ALT`, `SUPER`/COMMAND, `LEFT_MOUSE_BUTTON`, `MIDDLE_MOUSE_BUTTON`, `RIGHT_MOUSE_BUTTON`) and SHALL multiply pointer coordinates and wheel deltas by `widget.scale_factor().max(1)` before sending them to CEF.

#### Scenario: Shift+Ctrl held during click
- **WHEN** the user clicks with Shift and Ctrl pressed on a HiDPI display with scale 2
- **THEN** the dispatched `MouseEvent` carries `EVENTFLAG_SHIFT_DOWN | EVENTFLAG_CONTROL_DOWN` and coordinates equal to the logical x/y multiplied by 2
