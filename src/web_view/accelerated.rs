//! GPU-OSR glue: CEF's accelerated paints become owned GTK textures, and the
//! windowless frame rate follows the monitor. (gpu-osr)
use cef::{ImplBrowser, ImplBrowserHost};
use gtk::prelude::*;
use gtk::subclass::prelude::*;

use super::imp::KarereWebView;

impl KarereWebView {
    /// Copy CEF's accelerated frame (valid only during this call) into an
    /// owned texture. On a transfer failure the view falls back to CPU paint
    /// once; a failed popup copy only costs the popup.
    pub(super) fn accept_accelerated(&self, info: &cef::AcceleratedPaintInfo, popup: bool) {
        let widget = self.obj();
        let result = if popup {
            self.popup_presenter
                .borrow_mut()
                .accelerated(&*widget, info)
        } else {
            self.presenter.borrow_mut().accelerated(&*widget, info)
        };
        match result {
            Ok(true) => {
                if popup {
                    self.popup_gpu_frame.set(true);
                } else {
                    self.gpu_frame.set(true);
                }
                if let Some(shared) = self.shared.lock().as_ref() {
                    let mut s = shared.lock();
                    s.frame_serial += 1;
                    if !popup {
                        s.view_frame_serial += 1;
                    }
                }
                widget.queue_render();
            }
            Ok(false) => {
                // Every owned buffer is still held by GTK. Drop this frame (a
                // newer one supersedes it) and let GSK release its textures.
                log::debug!("graphics: all owned transfer buffers are in use; coalescing frame");
                widget.queue_render();
            }
            Err(error) if popup => {
                log::warn!("accelerated popup transfer failed: {error:#}; popup not shown");
                self.popup_gpu_frame.set(false);
                self.popup_presenter.borrow_mut().clear();
            }
            Err(error) => {
                log::warn!(
                    "graphics: accelerated transfer failed: {error:#}; falling back to CPU paint"
                );
                self.drop_accelerated_frames();
                // CEF's shared-texture choice is fixed at browser creation;
                // recreate the pool after this callback returns.
                self.schedule_software_osr_fallback();
            }
        }
    }

    /// A CPU paint replaces the accelerated copy of that layer.
    pub(super) fn accept_cpu(&self, popup: bool) {
        if popup {
            if self.popup_gpu_frame.replace(false) {
                self.popup_presenter.borrow_mut().clear();
            }
        } else if self.gpu_frame.replace(false) {
            self.presenter.borrow_mut().clear();
        }
    }

    /// Release every owned accelerated frame (CPU fallback, unrealize).
    pub(super) fn drop_accelerated_frames(&self) {
        self.gpu_frame.set(false);
        self.popup_gpu_frame.set(false);
        self.presenter
            .replace(crate::presenter::Presenter::default());
        self.popup_presenter
            .replace(crate::presenter::Presenter::default());
    }

    /// Present the owned accelerated view frame and the popup layer.
    pub(super) fn snapshot_accelerated(&self, snapshot: &gtk::Snapshot) {
        let widget = self.obj();
        let bounds =
            gtk::graphene::Rect::new(0.0, 0.0, widget.width() as f32, widget.height() as f32);
        let [r, g, b] = super::page_background(&*widget);
        snapshot.append_color(&gtk::gdk::RGBA::new(r, g, b, 1.0), &bounds);
        let scale = widget.scale_factor().max(1) as f32;
        snapshot.push_clip(&bounds);
        // Physical frame, logical widget: draw the frame at its own size,
        // anchored top-left, so a previous-size frame during a resize stays
        // 1:1 instead of stretching (as the GL and software paths do).
        let presenter = self.presenter.borrow();
        if let Some((w, h)) = presenter.visible_size() {
            presenter.snapshot(
                snapshot,
                &gtk::graphene::Rect::new(0.0, 0.0, w as f32 / scale, h as f32 / scale),
            );
        }
        drop(presenter);
        let popup_rect = self.shared.lock().as_ref().and_then(|shared| {
            let s = shared.lock();
            s.popup.visible_rect(s.foreground_browser_id)
        });
        match popup_rect {
            Some((x, y, w, h)) => {
                let rect = gtk::graphene::Rect::new(
                    x as f32 / scale,
                    y as f32 / scale,
                    w as f32 / scale,
                    h as f32 / scale,
                );
                if self.popup_gpu_frame.get() {
                    self.popup_presenter.borrow().snapshot(snapshot, &rect);
                } else if let Some(shared) = self.shared.lock().as_ref().cloned() {
                    let texture = super::gl::cached_frame_texture(
                        &self.sw_popup_texture,
                        &mut shared.lock().popup.frame,
                    );
                    if let Some(texture) = texture {
                        snapshot.append_texture(&texture, &rect);
                    }
                }
            }
            None if self.popup_gpu_frame.replace(false) => {
                self.popup_presenter.borrow_mut().clear();
            }
            None => {}
        }
        snapshot.pop();
    }

    /// Frame-pacing diagnostics: one line per GTK draw with the content serial.
    pub(super) fn log_draw(&self) {
        if !log::log_enabled!(log::Level::Debug) {
            return;
        }
        let Some(shared) = self.shared.lock().as_ref().cloned() else {
            return;
        };
        let (width, height, serial) = {
            let s = shared.lock();
            (s.frame.width, s.frame.height, s.frame_serial)
        };
        let widget = self.obj();
        let (width, height) = if self.gpu_frame.get() {
            self.presenter
                .borrow()
                .visible_size()
                .unwrap_or((width, height))
        } else {
            (width, height)
        };
        log::debug!(
            "coord: J4 draw frame={width}x{height} serial={serial} clock_frame={}",
            widget
                .frame_clock()
                .map(|clock| clock.frame_counter())
                .unwrap_or(-1)
        );
    }

    /// Follow the monitor this view is on; reconnects on hotplug.
    pub(super) fn watch_refresh_rate(&self) {
        self.refresh_watch.borrow_mut().take();
        if let Some(surface) = self.obj().native().and_then(|n| n.surface()) {
            let weak = self.obj().downgrade();
            *self.refresh_watch.borrow_mut() =
                Some(crate::refresh_rate::Watch::new(&surface, move || {
                    if let Some(view) = weak.upgrade() {
                        // Hotplug can replace the monitor list: reconnect as
                        // well as re-reading the current monitor's mode.
                        view.imp().watch_refresh_rate();
                    }
                }));
        }
        self.refresh_frame_rate();
    }

    /// Send the current monitor's refresh rate to every live browser.
    pub(super) fn refresh_frame_rate(&self) {
        let fps = crate::refresh_rate::for_widget(&*self.obj());
        if self.target_fps.replace(fps) == fps {
            return;
        }
        log::info!("render cadence: monitor mode target={fps} FPS");
        for browser in self.browsers.lock().values() {
            if let Some(host) = browser.host() {
                host.set_windowless_frame_rate(fps);
            }
        }
        if let Some(browser) = self.browser.lock().as_ref()
            && let Some(host) = browser.host()
        {
            host.set_windowless_frame_rate(fps);
        }
    }
}
