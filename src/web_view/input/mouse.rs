use cef::{self, ImplBrowser, ImplBrowserHost, MouseButtonType, MouseEvent, sys};
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use std::cell::RefCell;
use std::sync::atomic::Ordering;

use crate::web_view::imp::{resolved_browser, with_host, with_resolved};

#[derive(Debug, Clone, Copy, PartialEq)]
struct ClickHistory {
    button: u32,
    x: f64,
    y: f64,
    time: u32,
    count: i32,
}

/// Button state shared by the real GTK legacy-event controller and motion
/// controller. Unlike GestureClick, raw button releases are not cancelled
/// when a pointer crosses GTK's drag threshold.
#[derive(Debug, Default)]
pub(super) struct MouseButtonTracker {
    pressed: u32,
    press_counts: [i32; 3],
    last_click: Option<ClickHistory>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum MouseInput {
    Press {
        button: u32,
        x: f64,
        y: f64,
        time: u32,
        double_time: u32,
        double_distance: f64,
        modifiers: u32,
    },
    Motion {
        x: f64,
        y: f64,
        modifiers: u32,
    },
    Release {
        button: u32,
        x: f64,
        y: f64,
        modifiers: u32,
    },
    Cancel {
        x: f64,
        y: f64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum MouseDispatch {
    Click {
        x: f64,
        y: f64,
        button: u32,
        down: bool,
        count: i32,
        modifiers: u32,
    },
    Move {
        x: f64,
        y: f64,
        modifiers: u32,
    },
}

impl MouseButtonTracker {
    pub(super) fn handle(&mut self, input: MouseInput) -> Vec<MouseDispatch> {
        match input {
            MouseInput::Press {
                button,
                x,
                y,
                time,
                double_time,
                double_distance,
                modifiers,
            } => self
                .press(button, x, y, time, double_time, double_distance)
                .map(|count| MouseDispatch::Click {
                    x,
                    y,
                    button,
                    down: true,
                    count,
                    modifiers: modifiers | self.active_modifiers(),
                })
                .into_iter()
                .collect(),
            MouseInput::Motion { x, y, modifiers } => vec![MouseDispatch::Move {
                x,
                y,
                modifiers: modifiers | self.active_modifiers(),
            }],
            MouseInput::Release {
                button,
                x,
                y,
                modifiers,
            } => self
                .release(button)
                .map(|count| MouseDispatch::Click {
                    x,
                    y,
                    button,
                    down: false,
                    count,
                    modifiers,
                })
                .into_iter()
                .collect(),
            MouseInput::Cancel { x, y } => self
                .cancel_all()
                .into_iter()
                .map(|(button, count)| MouseDispatch::Click {
                    x,
                    y,
                    button,
                    down: false,
                    count,
                    modifiers: 0,
                })
                .collect(),
        }
    }

    pub(super) fn press(
        &mut self,
        button: u32,
        x: f64,
        y: f64,
        time: u32,
        double_time: u32,
        double_distance: f64,
    ) -> Option<i32> {
        let mask = button_mask(button)?;
        if self.pressed & mask != 0 {
            return None;
        }
        let count = self
            .last_click
            .filter(|last| {
                last.button == button
                    && time.wrapping_sub(last.time) <= double_time
                    && (x - last.x).abs() <= double_distance
                    && (y - last.y).abs() <= double_distance
            })
            .map_or(1, |last| (last.count % 3) + 1);
        self.pressed |= mask;
        self.press_counts[(button - 1) as usize] = count;
        self.last_click = Some(ClickHistory {
            button,
            x,
            y,
            time,
            count,
        });
        Some(count)
    }

    pub(super) fn release(&mut self, button: u32) -> Option<i32> {
        let mask = button_mask(button)?;
        if self.pressed & mask == 0 {
            return None;
        }
        self.pressed &= !mask;
        Some(self.press_counts[(button - 1) as usize].max(1))
    }

    pub(super) fn active_modifiers(&self) -> u32 {
        self.pressed
    }

    pub(super) fn cancel_all(&mut self) -> Vec<(u32, i32)> {
        (1..=3)
            .filter_map(|button| self.release(button).map(|count| (button, count)))
            .collect()
    }
}

pub(super) fn should_forward_mouse(pointer_emulated: bool) -> bool {
    !pointer_emulated
}

fn button_mask(button: u32) -> Option<u32> {
    use sys::cef_event_flags_t as F;
    match button {
        1 => Some(F::EVENTFLAG_LEFT_MOUSE_BUTTON.0),
        2 => Some(F::EVENTFLAG_MIDDLE_MOUSE_BUTTON.0),
        3 => Some(F::EVENTFLAG_RIGHT_MOUSE_BUTTON.0),
        _ => None,
    }
}

pub(super) fn dispatch_mouse(widget: &crate::web_view::KarereWebView, event: MouseDispatch) {
    dispatch_mouse_with(
        event,
        |x, y, button, down, count, modifiers| {
            send_click(widget, x, y, button, down, count, modifiers)
        },
        |x, y, modifiers| send_move(widget, x, y, modifiers, false),
    );
}

/// Production dispatch seam shared by the installed GTK controllers and the
/// headless regression harness. Keeping the sink injectable proves that the
/// adapter calls the click/move forwarding path rather than merely mutating
/// button state.
pub(super) fn dispatch_mouse_with(
    event: MouseDispatch,
    mut click: impl FnMut(f64, f64, u32, bool, i32, u32),
    mut motion: impl FnMut(f64, f64, u32),
) {
    match event {
        MouseDispatch::Click {
            x,
            y,
            button,
            down,
            count,
            modifiers,
        } => click(x, y, button, down, count, modifiers),
        MouseDispatch::Move { x, y, modifiers } => motion(x, y, modifiers),
    }
}

pub(super) fn release_stuck_mouse_buttons(
    widget: &crate::web_view::KarereWebView,
    state: &std::rc::Rc<RefCell<MouseButtonTracker>>,
) {
    let scale = widget.scale_factor().max(1) as f64;
    let x = widget.imp().last_mouse_x.load(Ordering::Relaxed) as f64 / scale;
    let y = widget.imp().last_mouse_y.load(Ordering::Relaxed) as f64 / scale;
    for event in state.borrow_mut().handle(MouseInput::Cancel { x, y }) {
        dispatch_mouse(widget, event);
    }
}

/// Surface-unit (touchpad) scroll distance worth one Ctrl+scroll zoom step.
pub(super) const ZOOM_SCROLL_SURFACE_STEP: f64 = 40.0;

/// Whole zoom steps (+ = in) for a Ctrl+scroll `dy`, accumulating smooth
/// deltas in `accum` until they reach `threshold`. A direction change
/// drops the partial step. (I10)
pub(super) fn zoom_scroll_steps(accum: &mut f64, dy: f64, threshold: f64) -> i32 {
    let delta = -dy;
    if delta == 0.0 || threshold <= 0.0 {
        return 0;
    }
    if *accum != 0.0 && accum.signum() != delta.signum() {
        *accum = 0.0;
    }
    *accum += delta;
    let steps = (*accum / threshold).trunc();
    *accum -= steps * threshold;
    steps as i32
}

/// Widget logical px → page CSS px. CEF's view is physical px and the page
/// zoom folds the display scale in (#158), so CSS = logical × scale / zoom.
/// The mouse path needs no such step: CEF maps its physical coords itself.
/// Only coordinates handed to page script (`elementFromPoint`, synthetic
/// event `clientX/Y`) go through here. (I8)
pub(super) fn page_coords(x: f64, y: f64, scale: f64, page_zoom: f64) -> (f64, f64) {
    if !(page_zoom.is_finite() && page_zoom > 0.0) {
        return (x, y);
    }
    let s = scale.max(1.0);
    (x * s / page_zoom, y * s / page_zoom)
}

pub(in crate::web_view) fn widget_to_page(widget: &crate::web_view::KarereWebView, x: f64, y: f64) -> (f64, f64) {
    let scale = widget.scale_factor().max(1) as f64;
    match resolved_browser(widget.imp()).and_then(|browser| browser.host()) {
        Some(host) => page_coords(x, y, scale, crate::web_view::cef_to_linear(host.zoom_level())),
        None => (x, y),
    }
}

/// True when the controller's current event is a pointer event GTK
/// synthesised from a touch sequence. Those are handled by the touch drag
/// gesture, so the mouse controllers skip them to avoid double-feeding the
/// page. (#162)
pub(super) fn is_pointer_emulated(ctrl: &impl IsA<gtk::EventController>) -> bool {
    ctrl.current_event()
        .map(|e| e.is_pointer_emulated())
        .unwrap_or(false)
}

pub(super) fn physical_mouse_coordinates(x: f64, y: f64, scale: i32) -> (i32, i32) {
    let s = scale.max(1) as f64;
    ((x * s).round() as i32, (y * s).round() as i32)
}

/// CEF mouse-host operations. Production implements this on BrowserHost;
/// tests inject an in-memory host only at this final API boundary.
pub(super) trait MouseEventSink {
    fn send_mouse_move(&mut self, event: &MouseEvent, leave: bool);
    fn send_mouse_click(
        &mut self,
        event: &MouseEvent,
        button: MouseButtonType,
        mouse_up: bool,
        click_count: i32,
    );
}

impl MouseEventSink for cef::BrowserHost {
    fn send_mouse_move(&mut self, event: &MouseEvent, leave: bool) {
        self.send_mouse_move_event(Some(event), leave as i32);
    }

    fn send_mouse_click(
        &mut self,
        event: &MouseEvent,
        button: MouseButtonType,
        mouse_up: bool,
        click_count: i32,
    ) {
        self.send_mouse_click_event(Some(event), button, mouse_up as i32, click_count);
    }
}

pub(super) fn send_move_with<S: MouseEventSink>(
    x: f64,
    y: f64,
    scale: i32,
    modifiers: u32,
    leave: bool,
    resolve: impl FnOnce() -> Option<S>,
    mut update_position: impl FnMut(i32, i32),
) {
    // CEF view coords are physical pixels (view_rect is physical, #158).
    let (px, py) = physical_mouse_coordinates(x, y, scale);
    log::debug!(
        "coord: J1/J6 move logical={:.1},{:.1} physical={},{} scale={} leave={}",
        x,
        y,
        px,
        py,
        scale,
        leave
    );
    if !leave {
        update_position(px, py);
    }
    let event = MouseEvent {
        x: px,
        y: py,
        modifiers,
    };
    with_resolved(resolve, |mut sink| sink.send_mouse_move(&event, leave));
}

pub(super) fn send_move(widget: &crate::web_view::KarereWebView, x: f64, y: f64, modifiers: u32, leave: bool) {
    let imp = widget.imp();
    send_move_with(
        x,
        y,
        widget.scale_factor(),
        modifiers,
        leave,
        || resolved_browser(imp).and_then(|browser| browser.host()),
        |px, py| {
            imp.last_mouse_x.store(px, Ordering::Relaxed);
            imp.last_mouse_y.store(py, Ordering::Relaxed);
        },
    );
}

pub(super) fn send_click_with<S: MouseEventSink>(
    position: (f64, f64),
    scale: i32,
    button: (u32, bool, i32),
    modifiers: u32,
    resolve: impl FnOnce() -> Option<S>,
) {
    let (x, y) = position;
    let (button, down, n_press) = button;
    let btn = match button {
        1 => MouseButtonType::LEFT,
        2 => MouseButtonType::MIDDLE,
        3 => MouseButtonType::RIGHT,
        _ => return,
    };
    let (px, py) = physical_mouse_coordinates(x, y, scale);
    log::debug!(
        "coord: J1/J6 click logical={:.1},{:.1} physical={},{} scale={} button={} down={} count={}",
        x,
        y,
        px,
        py,
        scale,
        button,
        down,
        n_press
    );
    let (x, y) = (px, py);
    let event = MouseEvent { x, y, modifiers };
    with_resolved(resolve, |mut sink| {
        sink.send_mouse_click(&event, btn, !down, n_press.max(1));
    });
}

fn send_click(
    widget: &crate::web_view::KarereWebView,
    x: f64,
    y: f64,
    button: u32,
    down: bool,
    n_press: i32,
    modifiers: u32,
) {
    send_click_with(
        (x, y),
        widget.scale_factor(),
        (button, down, n_press),
        modifiers,
        || resolved_browser(widget.imp()).and_then(|browser| browser.host()),
    );
}

/// Live pointer position in physical widget coords, for wheel hit-testing —
/// touchpad scrolls arrive without motion events, so cached coords go stale. (#161)
fn pointer_pos_physical(widget: &crate::web_view::KarereWebView) -> Option<(i32, i32)> {
    let pointer = widget.display().default_seat()?.pointer()?;
    let (sx, sy, _) = widget.native()?.surface()?.device_position(&pointer)?;
    let (x, y) = surface_to_widget(widget, sx, sy)?;
    let s = widget.scale_factor().max(1) as f64;
    Some(((x * s).round() as i32, (y * s).round() as i32))
}

/// Surface → widget coords: strip the native's window-decoration offset, then
/// map through the widget tree. Raw `GdkEvent` positions (and
/// `device_position`) are surface-relative — feeding them to CEF unmapped
/// offsets every click by the header-bar height (KARE-002 regression).
pub(super) fn surface_to_widget(widget: &crate::web_view::KarereWebView, sx: f64, sy: f64) -> Option<(f64, f64)> {
    let native = widget.native()?;
    let (tx, ty) = native.surface_transform();
    let p = native.compute_point(
        widget,
        &gtk::graphene::Point::new((sx - tx) as f32, (sy - ty) as f32),
    )?;
    Some((p.x() as f64, p.y() as f64))
}

pub(super) fn send_wheel(widget: &crate::web_view::KarereWebView, dx: f64, dy: f64, modifiers: u32, precise: bool) {
    // CEF wants pixel deltas. A mouse wheel reports notch clicks (±1) → use a
    // browser-like notch size. A touchpad (Surface unit) reports surface-unit
    // deltas; Chromium's own Wayland backend scales those by
    // kWheelDelta(53) / kAxisValueScale(10) = 5.3 (wayland_pointer.cc), so
    // match it — 1:1 passthrough was ~5x slower than every browser. (#161)
    let step = if precise { 5.3 } else { 100.0 };
    let s = widget.scale_factor().max(1) as f64;
    let imp = widget.imp();

    // Accumulate sub-pixel deltas so fine touchpad motion isn't truncated to
    // zero (the old `as i32` dropped small deltas → jumpy/slow scroll). Keep
    // the fractional remainder for the next event. (#161)
    let ax = imp.scroll_accum_x.get() + (-dx * step * s);
    let ay = imp.scroll_accum_y.get() + (-dy * step * s);
    let ix = ax.trunc();
    let iy = ay.trunc();
    imp.scroll_accum_x.set(ax - ix);
    imp.scroll_accum_y.set(ay - iy);
    if ix == 0.0 && iy == 0.0 {
        return; // no whole pixel yet — keep accumulating
    }

    // CEF hit-tests the cursor to pick the scroll target; (0,0) scrolls nothing.
    // Touchpad scrolling doesn't move the pointer, so the cached motion coords
    // can be stale (pointer entered without motion — window presented under
    // the cursor, workspace switch): query the live position, fall back to
    // last_mouse_* (already physical). (#161)
    let (hx, hy) = pointer_pos_physical(widget).unwrap_or_else(|| {
        (
            imp.last_mouse_x.load(Ordering::Relaxed),
            imp.last_mouse_y.load(Ordering::Relaxed),
        )
    });
    let event = MouseEvent {
        x: hx,
        y: hy,
        modifiers,
    };
    with_host(widget, |host| {
        host.send_mouse_wheel_event(Some(&event), ix as i32, iy as i32);
    });
}
