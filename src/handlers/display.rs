use cef::{
    self, Browser, CefString, CursorInfo, CursorType, DisplayHandler, ImplBrowser,
    ImplDisplayHandler, WrapDisplayHandler, rc::Rc, wrap_display_handler,
};

use super::SharedRef;

/// Map a CEF `CursorType` to a GTK/CSS cursor name (`gdk::Cursor::from_name`).
/// OSR gives no platform cursor, so we translate and let the GLArea set it.
/// Unmapped/custom types fall back to `"default"`.
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
        fn on_title_change(&self, browser: Option<&mut Browser>, title: Option<&CefString>) {
            let t = title.map(CefString::to_string).unwrap_or_default();
            log::debug!("title: {t}");
            // WhatsApp titles the page "(N) WhatsApp" while N chats are unread:
            // the page's own count is the source of truth for the badges.
            let count = unread_from_title(&t);
            self.handler.shared.lock().title = t;
            if let Some(account_id) =
                browser.and_then(|b| crate::accounts::account_for_browser(b.identifier()))
            {
                // Deferred: the change signal rebuilds GTK widgets, which must not
                // run inside a CEF callback.
                glib::idle_add_local_once(move || {
                    crate::accounts::set_unread_count(&account_id, count);
                });
            }
        }

        // M21: WhatsApp's `element.requestFullscreen()` surfaces here. On the CEF
        // UI thread, so it must NOT touch GTK widgets — just record the request in
        // `SharedState`; the window poll loop drains it and calls
        // `window.fullscreen()`/`unfullscreen()`.
        fn on_fullscreen_mode_change(
            &self,
            _browser: Option<&mut Browser>,
            fullscreen: ::std::os::raw::c_int,
        ) {
            self.handler.shared.lock().fullscreen_request = Some(fullscreen != 0);
        }

        // OSR: CEF never sets the platform cursor, so record it (as a CSS name)
        // for the view flush (`wake_view`) to apply. Returns 1 = handled.
        fn on_cursor_change(
            &self,
            browser: Option<&mut Browser>,
            _cursor: ::std::os::raw::c_ulong,
            type_: CursorType,
            _custom_cursor_info: Option<&CursorInfo>,
        ) -> ::std::os::raw::c_int {
            let name = cursor_css_name(type_);
            let mut s = self.handler.shared.lock();
            // Same gate as paints: a background account's page must not change
            // the cursor of the one in front (0 = pool not wired yet).
            let from = browser.as_ref().map(|b| b.identifier()).unwrap_or(0);
            if s.foreground_browser_id != 0 && from != s.foreground_browser_id {
                return 1;
            }
            if s.cursor_name != name {
                s.cursor_name = name;
                s.cursor_dirty = true;
                super::wake_view(s);
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

/// Unread-chat count from a WhatsApp page title: the leading "(N)" of
/// "(N) WhatsApp". Only that prefix matters, so localized titles work too;
/// anything else (no prefix, non-digits) is 0.
pub(crate) fn unread_from_title(title: &str) -> u32 {
    let Some(rest) = title.trim_start().strip_prefix('(') else {
        return 0;
    };
    let Some((digits, _)) = rest.split_once(')') else {
        return 0;
    };
    let digits = digits.trim();
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return 0;
    }
    digits.parse().unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::unread_from_title;

    #[test]
    fn parses_leading_count() {
        assert_eq!(unread_from_title("(3) WhatsApp"), 3);
        assert_eq!(unread_from_title("(128) WhatsApp Business"), 128);
        assert_eq!(unread_from_title("  (1) WhatsApp"), 1);
    }

    #[test]
    fn no_count_is_zero() {
        assert_eq!(unread_from_title("WhatsApp"), 0);
        assert_eq!(unread_from_title(""), 0);
        assert_eq!(unread_from_title("()"), 0);
        assert_eq!(unread_from_title("(x) WhatsApp"), 0);
        assert_eq!(unread_from_title("(3 WhatsApp"), 0);
        assert_eq!(unread_from_title("WhatsApp (3)"), 0);
        assert_eq!(unread_from_title("(-1) WhatsApp"), 0);
    }

    #[test]
    fn huge_count_saturates() {
        assert_eq!(unread_from_title("(99999999999) WhatsApp"), u32::MAX);
    }
}
