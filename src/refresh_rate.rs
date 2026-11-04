//! Monitor mode tracking, independent of CEF's hidden-browser throttling.
use gtk::{gdk, glib, prelude::*};
use std::rc::Rc;

pub fn frames_per_second(millihertz: i32) -> i32 {
    if millihertz <= 0 {
        60
    } else {
        ((i64::from(millihertz) + 500) / 1000).max(1) as i32
    }
}

pub fn for_widget(widget: &impl IsA<gtk::Widget>) -> i32 {
    widget
        .native()
        .and_then(|native| native.surface())
        .and_then(|surface| surface.display().monitor_at_surface(&surface))
        .map(|monitor| frames_per_second(monitor.refresh_rate()))
        .unwrap_or(60)
}

/// Own every signal connection so re-realizing a view cannot accumulate listeners.
#[derive(Default)]
pub struct Watch(Vec<(glib::Object, glib::SignalHandlerId)>);

impl Watch {
    pub fn new(
        surface: &gdk::Surface,
        changed: impl Fn() + 'static,
        scale_changed: impl Fn() + 'static,
    ) -> Self {
        let changed: Rc<dyn Fn()> = Rc::new(changed);
        let mut watch = Self::default();
        watch.0.push((
            surface.clone().upcast(),
            surface.connect_scale_notify(move |_| scale_changed()),
        ));
        let callback = changed.clone();
        watch.0.push((
            surface.clone().upcast(),
            surface.connect_enter_monitor(move |_, _| callback()),
        ));
        let callback = changed.clone();
        watch.0.push((
            surface.clone().upcast(),
            surface.connect_leave_monitor(move |_, _| callback()),
        ));
        let monitors = surface.display().monitors();
        let callback = changed.clone();
        watch.0.push((
            monitors.clone().upcast(),
            monitors.connect_items_changed(move |_, _, _, _| callback()),
        ));
        for monitor in monitors.iter::<gdk::Monitor>().filter_map(Result::ok) {
            let callback = changed.clone();
            let handler = monitor.connect_refresh_rate_notify(move |_| callback());
            watch.0.push((monitor.upcast(), handler));
        }
        watch
    }
}

impl Drop for Watch {
    fn drop(&mut self) {
        for (object, handler) in self.0.drain(..) {
            object.disconnect(handler);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_rates_are_rounded_without_a_sixty_or_240_hz_ceiling() {
        for (input, expected) in [
            (0, 60),
            (-1, 60),
            (59_940, 60),
            (119_880, 120),
            (144_000, 144),
            (239_760, 240),
            (360_000, 360),
            (500_000, 500),
            (1, 1),
            (i32::MAX, 2_147_484),
        ] {
            assert_eq!(frames_per_second(input), expected);
        }
    }
}
