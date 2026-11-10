use cef::{ImplBrowserHost, PointerType, TouchEvent, TouchEventType};
use gtk::prelude::*;

use crate::web_view::imp::with_host;

/// Forward a touch point to CEF so the page gets native touchstart/move/end
/// — required for touch scrolling on touchscreens (Phosh), which emulated
/// pointer events don't drive. Coords are physical (× scale), like the mouse
/// path. (#162)
pub(super) fn send_touch(widget: &crate::web_view::KarereWebView, id: i32, x: f64, y: f64, type_: TouchEventType) {
    let s = widget.scale_factor().max(1) as f32;
    let event = TouchEvent {
        id,
        x: x as f32 * s,
        y: y as f32 * s,
        radius_x: 0.0,
        radius_y: 0.0,
        rotation_angle: 0.0,
        pressure: 1.0,
        type_,
        modifiers: 0,
        pointer_type: PointerType::TOUCH,
    };
    with_host(widget, |host| host.send_touch_event(Some(&event)));
}
