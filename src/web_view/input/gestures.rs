use gtk::glib;
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use super::mouse::{send_wheel, wheel_pixel_scale};

/// Pinch scale change worth one zoom step, as a factor of the scale at the
/// last step (both directions).
pub(super) const PINCH_STEP_FACTOR: f64 = 1.1;

/// Turns a pinch gesture's running scale (relative to the gesture start)
/// into whole zoom steps. Each step re-bases on the scale it fired at, so a
/// slow pinch and a fast one give the same number of steps.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct PinchSteps {
    last: f64,
}

impl Default for PinchSteps {
    fn default() -> Self {
        Self { last: 1.0 }
    }
}

impl PinchSteps {
    /// Zoom steps (+ = in) to emit for the gesture's current `scale`.
    pub(super) fn update(&mut self, scale: f64) -> i32 {
        if !(scale.is_finite() && scale > 0.0) {
            return 0;
        }
        let mut steps = 0;
        while scale / self.last >= PINCH_STEP_FACTOR {
            self.last *= PINCH_STEP_FACTOR;
            steps += 1;
        }
        while scale / self.last <= 1.0 / PINCH_STEP_FACTOR {
            self.last /= PINCH_STEP_FACTOR;
            steps -= 1;
        }
        steps
    }

}

/// Activate `steps` window zoom actions (+ = in), the same per-account zoom
/// levels Ctrl+scroll and Ctrl+plus/minus use. (I10)
pub(super) fn activate_zoom_steps(widget: &crate::web_view::KarereWebView, steps: i32) {
    let action = if steps > 0 { "win.zoom-in" } else { "win.zoom-out" };
    for _ in 0..steps.unsigned_abs() {
        if let Err(err) = widget.activate_action(action, None) {
            log::debug!("zoom step: {action}: {err}");
            break;
        }
    }
}

/// Kinetic scroll frame interval.
pub(super) const KINETIC_FRAME: Duration = Duration::from_millis(16);
/// Velocity kept per frame, close to GTK's own kinetic scrolling friction.
pub(super) const KINETIC_FRICTION: f64 = 0.95;
/// Hard stop for one fling, however fast it started.
pub(super) const KINETIC_MAX: Duration = Duration::from_millis(1500);

/// Exponentially decaying fling started from GTK's `decelerate` velocity
/// (surface units per second, the same units as the scroll deltas).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct KineticDecay {
    vx: f64,
    vy: f64,
    elapsed: Duration,
}

impl KineticDecay {
    pub(super) fn new(vx: f64, vy: f64) -> Self {
        Self {
            vx: if vx.is_finite() { vx } else { 0.0 },
            vy: if vy.is_finite() { vy } else { 0.0 },
            elapsed: Duration::ZERO,
        }
    }

    /// The scroll delta (surface units) to send for the next frame, or
    /// `None` once the fling has finished: the frame's delta is below
    /// `min_delta` on both axes, or the time limit is reached.
    pub(super) fn step(&mut self, min_delta: f64) -> Option<(f64, f64)> {
        if self.elapsed >= KINETIC_MAX {
            return None;
        }
        let dt = KINETIC_FRAME.as_secs_f64();
        let (dx, dy) = (self.vx * dt, self.vy * dt);
        if dx.abs() < min_delta && dy.abs() < min_delta {
            return None;
        }
        self.vx *= KINETIC_FRICTION;
        self.vy *= KINETIC_FRICTION;
        self.elapsed += KINETIC_FRAME;
        Some((dx, dy))
    }
}

/// The running fling of one view, stopped by any new input. CEF's
/// windowless wheel events carry no momentum phase, so Chromium never
/// flings a touchpad scroll by itself; this feeds the decay as wheel deltas.
#[derive(Default)]
pub(super) struct KineticScroll {
    source: RefCell<Option<glib::SourceId>>,
}

impl KineticScroll {
    pub(super) fn stop(&self) {
        if let Some(id) = self.source.borrow_mut().take() {
            id.remove();
        }
    }

    pub(super) fn start(self: &Rc<Self>, widget: &crate::web_view::KarereWebView, vx: f64, vy: f64) {
        self.stop();
        let mut decay = KineticDecay::new(vx, vy);
        // Under a whole page pixel per frame the scroll is no longer visible.
        let min_delta = 1.0 / wheel_pixel_scale(true);
        let this = Rc::downgrade(self);
        let widget = widget.downgrade();
        let id = glib::timeout_add_local(KINETIC_FRAME, move || {
            let Some(this) = this.upgrade() else {
                return glib::ControlFlow::Break;
            };
            let next = widget.upgrade().and_then(|w| decay.step(min_delta).map(|d| (w, d)));
            match next {
                Some((widget, (dx, dy))) => {
                    send_wheel(&widget, dx, dy, 0, true);
                    glib::ControlFlow::Continue
                }
                None => {
                    // Finished on its own: forget the id so a later stop()
                    // never removes a source that no longer exists.
                    this.source.borrow_mut().take();
                    glib::ControlFlow::Break
                }
            }
        });
        *self.source.borrow_mut() = Some(id);
    }
}
