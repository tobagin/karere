use cef::{
    self, Browser, CefString, CursorInfo, CursorType, DisplayHandler, ImplDisplayHandler,
    WrapDisplayHandler, rc::Rc, wrap_display_handler,
};

use super::SharedRef;

/// Map a CEF `CursorType` to a GTK/CSS cursor name (the names
/// `gdk::Cursor::from_name` understands). OSR gives us no platform cursor, so we
/// translate and let the GLArea set it. Unmapped/custom types fall back to
/// `"default"`.
fn cursor_css_name(t: CursorType) -> &'static str {
    use cef::sys::cef_cursor_type_t as C;
    match *t.as_ref() {
        C::CT_POINTER => "default",
        C::CT_CROSS => "crosshair",
        C::CT_HAND => "pointer",
        C::CT_IBEAM => "text",
        C::CT_WAIT => "wait",
        C::CT_HELP => "help",
        C::CT_EASTRESIZE => "e-resize",
        C::CT_NORTHRESIZE => "n-resize",
        C::CT_NORTHEASTRESIZE => "ne-resize",
        C::CT_NORTHWESTRESIZE => "nw-resize",
        C::CT_SOUTHRESIZE => "s-resize",
        C::CT_SOUTHEASTRESIZE => "se-resize",
        C::CT_SOUTHWESTRESIZE => "sw-resize",
        C::CT_WESTRESIZE => "w-resize",
        C::CT_NORTHSOUTHRESIZE => "ns-resize",
        C::CT_EASTWESTRESIZE => "ew-resize",
        C::CT_NORTHEASTSOUTHWESTRESIZE => "nesw-resize",
        C::CT_NORTHWESTSOUTHEASTRESIZE => "nwse-resize",
        C::CT_COLUMNRESIZE => "col-resize",
        C::CT_ROWRESIZE => "row-resize",
        C::CT_MOVE => "move",
        C::CT_VERTICALTEXT => "vertical-text",
        C::CT_CELL => "cell",
        C::CT_CONTEXTMENU => "context-menu",
        C::CT_ALIAS => "alias",
        C::CT_PROGRESS => "progress",
        C::CT_NODROP => "no-drop",
        C::CT_COPY => "copy",
        C::CT_NONE => "none",
        C::CT_NOTALLOWED => "not-allowed",
        C::CT_ZOOMIN => "zoom-in",
        C::CT_ZOOMOUT => "zoom-out",
        C::CT_GRAB => "grab",
        C::CT_GRABBING => "grabbing",
        C::CT_DND_NONE => "default",
        C::CT_DND_MOVE => "move",
        C::CT_DND_COPY => "copy",
        C::CT_DND_LINK => "alias",
        _ => "default",
    }
}

#[derive(Clone)]
pub struct ShellDisplayHandler {
    shared: SharedRef,
}

impl ShellDisplayHandler {
    pub fn new(shared: SharedRef) -> Self {
        Self { shared }
    }
}

wrap_display_handler! {
    pub struct ShellDisplayHandlerBuilder {
        handler: ShellDisplayHandler,
    }

    impl DisplayHandler {
        fn on_title_change(&self, _browser: Option<&mut Browser>, title: Option<&CefString>) {
            let t = title.map(CefString::to_string).unwrap_or_default();
            self.handler.shared.lock().title = t.clone();
            log::debug!("title: {t}");
        }

        // OSR: CEF never sets the platform cursor, so record the requested
        // cursor (as a CSS name) for the widget tick callback to apply. Returns
        // 1 to signal we handled the cursor.
        fn on_cursor_change(
            &self,
            _browser: Option<&mut Browser>,
            _cursor: ::std::os::raw::c_ulong,
            type_: CursorType,
            _custom_cursor_info: Option<&CursorInfo>,
        ) -> ::std::os::raw::c_int {
            let name = cursor_css_name(type_);
            let mut s = self.handler.shared.lock();
            if s.cursor_name != name {
                s.cursor_name = name;
                s.cursor_dirty = true;
            }
            1
        }
    }
}

impl ShellDisplayHandlerBuilder {
    pub fn build(handler: ShellDisplayHandler) -> DisplayHandler {
        Self::new(handler)
    }
}
