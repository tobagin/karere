use cef::{self, EventFlags, ImplRunContextMenuCallback, RunContextMenuCallback};
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

use super::clipboard_dnd::{
    ExplicitCopyTrigger, dispatch_explicit_copy, request_live_selection, try_intercept_paste,
};
use super::imp::KarereWebView;
use super::input::keyboard::set_focus;

// ---- context-menu widget registry (main-thread only) ----------------------
//
// `run_context_menu` runs on the CEF UI thread = the glib main thread (external
// pump), so this registry and its non-`Send` `RunContextMenuCallback` never cross
// threads (deliberately NOT routed through the `Arc<Mutex>` `SharedRef`). Maps a
// CEF browser id to the widget rendering it.
thread_local! {
    static CTX_MENU_WIDGETS: std::cell::RefCell<
        std::collections::HashMap<i32, glib::WeakRef<super::KarereWebView>>,
    > = std::cell::RefCell::new(std::collections::HashMap::new());
}

pub(crate) fn register_context_menu_widget(id: i32, view: &super::KarereWebView) {
    use glib::prelude::ObjectExt;
    CTX_MENU_WIDGETS.with(|m| {
        m.borrow_mut().insert(id, view.downgrade());
    });
}

pub(crate) fn unregister_context_menu_widget(id: i32) {
    CTX_MENU_WIDGETS.with(|m| {
        m.borrow_mut().remove(&id);
    });
}

/// Route a snapshotted context menu to the widget owning `browser_id`. If the
/// widget is gone, cancel the callback so CEF doesn't leak pending-menu state.
/// Main-thread only.
pub fn dispatch_context_menu(
    browser_id: i32,
    items: Vec<crate::handlers::context_menu::MenuEntry>,
    x: i32,
    y: i32,
    callback: cef::RunContextMenuCallback,
) {
    let view = CTX_MENU_WIDGETS.with(|m| m.borrow().get(&browser_id).and_then(|w| w.upgrade()));
    match view {
        Some(v) => v.imp().show_context_menu(items, x, y, callback),
        None => {
            use cef::ImplRunContextMenuCallback;
            callback.cancel();
        }
    }
}

impl KarereWebView {
    pub(super) fn cancel_context_menu(&self) {
        self.pending_menu_command.set(None);
        if let Some(cb) = self.pending_menu_callback.borrow_mut().take() {
            cb.cancel();
        }
        if let Some(pop) = self.context_popover.borrow_mut().take() {
            pop.unparent();
        }
    }

    /// Present the snapshotted CEF context menu as a GTK `Popover` of buttons at
    /// the cursor over the `GLArea`. Main-thread only. Button click records the
    /// command + pops down; `closed` dispatches `cont`/`cancel` (exactly one).
    /// Manual button popover, not `PopoverMenu`+`gio::Menu`: the model menu's
    /// actions didn't activate when parented to the OSR `GLArea`.
    pub fn show_context_menu(
        &self,
        items: Vec<crate::handlers::context_menu::MenuEntry>,
        x_dev: i32,
        y_dev: i32,
        callback: RunContextMenuCallback,
    ) {
        // Replace any stale menu (shouldn't normally overlap).
        if let Some(old) = self.context_popover.borrow_mut().take() {
            old.unparent();
        }
        if let Some(cb) = self.pending_menu_callback.borrow_mut().take() {
            cb.cancel();
        }
        *self.pending_menu_callback.borrow_mut() = Some(callback);
        self.pending_menu_command.set(None);

        let obj = self.obj();
        let popover = gtk::Popover::new();
        popover.set_parent(&*obj);
        popover.set_has_arrow(false);
        popover.set_autohide(true);
        popover.set_position(gtk::PositionType::Bottom);
        popover.add_css_class("menu");

        let container = gtk::Box::new(gtk::Orientation::Vertical, 0);
        build_menu_box(&items, &container, &obj, &popover);
        popover.set_child(Some(&container));

        // CEF reports the cursor in view coords = physical pixels (view_rect
        // is physical, #158); GTK widget coords are logical, so divide by the
        // integer scale to anchor the popover at the cursor.
        let s = obj.scale_factor().max(1) as f64;
        let rect = gtk::gdk::Rectangle::new(
            (x_dev as f64 / s).round() as i32,
            (y_dev as f64 / s).round() as i32,
            1,
            1,
        );
        popover.set_pointing_to(Some(&rect));

        // Resolve the callback AFTER popdown (webview re-focused): dispatch the
        // activated command, or cancel if dismissed without a selection.
        popover.connect_closed(glib::clone!(
            #[weak]
            obj,
            move |pop| {
                let imp = obj.imp();
                let cmd = imp.pending_menu_command.take();
                if let Some(cb) = imp.pending_menu_callback.borrow_mut().take() {
                    // Re-assert CEF focus before the command (replace/cut/paste
                    // act on the focused frame's selection).
                    set_focus(&obj, true);
                    match cmd {
                        // CEF's windowless Paste only sees Chromium's internal
                        // clipboard, never GDK's. Route it through the same host
                        // paste path as Ctrl+V; cancel so CEF doesn't paste
                        // stale internal content on top. (#189)
                        Some(id)
                            if crate::handlers::context_menu::is_paste_command(id)
                                && try_intercept_paste(&obj) =>
                        {
                            log::debug!("context menu: host paste, cancel(id={id})");
                            cb.cancel();
                        }
                        Some(id) => {
                            log::debug!("context menu: cont(id={id})");
                            dispatch_explicit_copy(
                                ExplicitCopyTrigger::ContextMenu(id),
                                || request_live_selection(&obj),
                            );
                            cb.cont(id, EventFlags::default());
                        }
                        None => cb.cancel(),
                    }
                }
                pop.unparent();
                *imp.context_popover.borrow_mut() = None;
            }
        ));

        *self.context_popover.borrow_mut() = Some(popover.clone());
        popover.popup();
    }
}

/// Populate `container` per [`MenuEntry`]: flat `Button`s for commands (click →
/// record id + popdown), `Separator`s, dim heading labels for (flattened)
/// submenus. Disabled items are insensitive.
fn build_menu_box(
    entries: &[crate::handlers::context_menu::MenuEntry],
    container: &gtk::Box,
    obj: &super::KarereWebView,
    popover: &gtk::Popover,
) {
    use crate::handlers::context_menu::MenuEntry;
    use gtk::prelude::*;

    for e in entries {
        match e {
            MenuEntry::Separator => {
                container.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
            }
            MenuEntry::Submenu { label, items } => {
                let heading = gtk::Label::new(Some(label));
                heading.set_xalign(0.0);
                heading.add_css_class("dim-label");
                container.append(&heading);
                build_menu_box(items, container, obj, popover);
            }
            MenuEntry::Item {
                label,
                command_id,
                enabled,
            } => {
                let btn = gtk::Button::with_label(label);
                btn.set_has_frame(false);
                btn.add_css_class("flat");
                btn.set_sensitive(*enabled);
                if let Some(lbl) = btn.child().and_downcast::<gtk::Label>() {
                    lbl.set_xalign(0.0);
                }
                let cmd = *command_id;
                btn.connect_clicked(glib::clone!(
                    #[weak]
                    obj,
                    #[weak]
                    popover,
                    move |_| {
                        log::debug!("context menu: item activated id={cmd}");
                        // Record the choice; `closed` runs `cont` after popdown + refocus.
                        obj.imp().pending_menu_command.set(Some(cmd));
                        popover.popdown();
                    }
                ));
                container.append(&btn);
            }
        }
    }
}
