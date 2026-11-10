//! Page-initiated drag-out (an image, link or selected text dragged from
//! WhatsApp into another app). CEF's `start_dragging` queues the data in the
//! shared state; this starts the matching GTK drag and reports its end back
//! to the page.

use cef::sys::cef_drag_operations_mask_t as Op;
use cef::{Browser, DragOperationsMask, ImplBrowser, ImplBrowserHost};
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, gio, glib};
use std::sync::atomic::Ordering;

use crate::handlers::render::DragOut;

/// GDK actions for the page's allowed CEF operations (COPY when none map).
pub(super) fn gdk_actions(allowed_ops: u32) -> gdk::DragAction {
    let mut actions = gdk::DragAction::empty();
    if allowed_ops & Op::DRAG_OPERATION_COPY.0 != 0 {
        actions |= gdk::DragAction::COPY;
    }
    if allowed_ops & Op::DRAG_OPERATION_LINK.0 != 0 {
        actions |= gdk::DragAction::LINK;
    }
    if allowed_ops & Op::DRAG_OPERATION_MOVE.0 != 0 {
        actions |= gdk::DragAction::MOVE;
    }
    if actions.is_empty() {
        gdk::DragAction::COPY
    } else {
        actions
    }
}

/// The CEF operation the drop target picked (`NONE` for a cancelled drag).
pub(super) fn cef_operation(action: gdk::DragAction) -> u32 {
    if action.contains(gdk::DragAction::MOVE) {
        Op::DRAG_OPERATION_MOVE.0
    } else if action.contains(gdk::DragAction::COPY) {
        Op::DRAG_OPERATION_COPY.0
    } else if action.contains(gdk::DragAction::LINK) {
        Op::DRAG_OPERATION_LINK.0
    } else {
        Op::DRAG_OPERATION_NONE.0
    }
}

/// One provider per form the drag carries; the union offers all of them,
/// the earlier one winning where two produce the same mime type.
fn content_for(drag: &DragOut) -> Option<gdk::ContentProvider> {
    let mut providers = Vec::new();
    if let Some(path) = &drag.file {
        let files = gdk::FileList::from_array(&[gio::File::for_path(path)]);
        providers.push(gdk::ContentProvider::for_value(&files.to_value()));
    }
    if let Some(uri) = &drag.uri {
        let bytes = glib::Bytes::from_owned(format!("{uri}\r\n").into_bytes());
        providers.push(gdk::ContentProvider::for_bytes("text/uri-list", &bytes));
    }
    if let Some(html) = &drag.html {
        let bytes = glib::Bytes::from_owned(html.clone().into_bytes());
        providers.push(gdk::ContentProvider::for_bytes("text/html", &bytes));
    }
    if let Some(text) = &drag.text {
        providers.push(gdk::ContentProvider::for_value(&text.to_value()));
    }
    (!providers.is_empty()).then(|| gdk::ContentProvider::new_union(&providers))
}

/// Tell the page its drag is over (`op` = the CEF operation performed).
fn end_page_drag(view: &crate::web_view::KarereWebView, browser: &Browser, op: u32) {
    let Some(host) = browser.host() else { return };
    // The same physical view coordinates the mouse path sends.
    let imp = view.imp();
    let x = imp.last_mouse_x.load(Ordering::Relaxed);
    let y = imp.last_mouse_y.load(Ordering::Relaxed);
    host.drag_source_ended_at(x, y, DragOperationsMask::from(Op(op)));
    host.drag_source_system_drag_ended();
}

impl crate::web_view::KarereWebView {
    /// Start the GTK drag for the drag-out `start_dragging` queued. Runs from
    /// a main-loop idle, never inside the CEF callback.
    pub(crate) fn begin_drag_out(&self) {
        let Some(shared) = self.imp().shared.lock().clone() else {
            return;
        };
        let Some(pending) = shared.lock().drag_out.take() else {
            return;
        };
        if let Some((drag, browser)) = self.start_drag(&pending) {
            self.imp().drag_out.replace(Some((drag, browser)));
        } else {
            end_page_drag(self, &pending.browser, Op::DRAG_OPERATION_NONE.0);
        }
    }

    fn start_drag(&self, pending: &DragOut) -> Option<(gdk::Drag, Browser)> {
        if self.imp().drag_out.borrow().is_some() {
            return None;
        }
        let surface = self.native()?.surface()?;
        let pointer = self.display().default_seat()?.pointer()?;
        let (x, y, state) = surface.device_position(&pointer)?;
        // A compositor only starts a drag during the press that began it.
        // Released already, or started by touch: nothing to drag with.
        if !state.contains(gdk::ModifierType::BUTTON1_MASK) {
            return None;
        }
        let content = content_for(pending)?;
        let drag = gdk::Drag::begin(
            &surface,
            &pointer,
            &content,
            gdk_actions(pending.allowed_ops),
            x,
            y,
        )?;
        let icon = gtk::Image::from_icon_name(if pending.file.is_some() {
            "image-x-generic"
        } else {
            "text-x-generic"
        });
        icon.set_pixel_size(32);
        gtk::DragIcon::for_drag(&drag).set_child(Some(&icon));

        let weak = self.downgrade();
        drag.connect_dnd_finished(move |drag| {
            if let Some(view) = weak.upgrade() {
                view.finish_drag_out(drag.selected_action(), true);
            }
        });
        let weak = self.downgrade();
        drag.connect_cancel(move |_drag, reason| {
            log::debug!("drag-out cancelled: {reason:?}");
            if let Some(view) = weak.upgrade() {
                view.finish_drag_out(gdk::DragAction::empty(), false);
            }
        });
        Some((drag, pending.browser.clone()))
    }

    /// Report the end once, whichever of finished/cancel comes first.
    fn finish_drag_out(&self, action: gdk::DragAction, success: bool) {
        let imp = self.imp();
        let Some((drag, browser)) = imp.drag_out.borrow_mut().take() else {
            return;
        };
        drag.drop_done(success);
        if let Some(forget) = imp.forget_mouse_buttons.borrow().as_ref() {
            forget();
        }
        end_page_drag(self, &browser, cef_operation(action));
    }
}
