use std::os::raw::c_int;

use cef::{
    self, AlphaType, Browser, CefString, ColorType, ContextMenuHandler, ContextMenuMediaType,
    ContextMenuParams, DownloadImageCallback, Frame, Image, ImplBinaryValue, ImplBrowser,
    ImplBrowserHost, ImplContextMenuHandler, ImplContextMenuParams, ImplDownloadImageCallback,
    ImplFrame, ImplImage, ImplMenuModel, ImplRunContextMenuCallback, MenuItemType, MenuModel,
    ProcessId, RunContextMenuCallback, WrapContextMenuHandler, WrapDownloadImageCallback, rc::Rc,
    wrap_context_menu_handler, wrap_download_image_callback,
};

/// Owned snapshot of a CEF `MenuModel` entry, decoupled from the non-`Send`
/// refcounted CEF objects so the GTK layer can render it on the main thread.
#[derive(Clone)]
pub enum MenuEntry {
    Separator,
    Item {
        label: String,
        command_id: i32,
        enabled: bool,
    },
    Submenu {
        label: String,
        items: Vec<MenuEntry>,
    },
}

/// Convert a CEF `CefStringUserfree` into a Rust `String`.
fn userfree_to_string(s: cef::CefStringUserfree) -> String {
    CefString::from(&s).to_string()
}

/// Strip Chromium's `&` mnemonic markers from a menu label. A lone `&` is a
/// dropped accelerator marker; `&&` is a literal `&`.
fn strip_mnemonics(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '&' {
            if chars.peek() == Some(&'&') {
                out.push('&');
                chars.next();
            }
            // Lone `&` is a mnemonic marker — drop it.
        } else {
            out.push(c);
        }
    }
    out
}

/// Walk a CEF `MenuModel` into a [`MenuEntry`] tree. Recurses into submenus
/// (the spellcheck suggestion list arrives as one).
fn snapshot_model(model: &MenuModel) -> Vec<MenuEntry> {
    let count = model.count();
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let ty = model.type_at(i);
        if ty == MenuItemType::SEPARATOR {
            out.push(MenuEntry::Separator);
            continue;
        }
        let label = strip_mnemonics(&userfree_to_string(model.label_at(i)));
        if ty == MenuItemType::SUBMENU
            && let Some(sub) = model.sub_menu_at(i)
        {
            out.push(MenuEntry::Submenu {
                label,
                items: snapshot_model(&sub),
            });
            continue;
        }
        out.push(MenuEntry::Item {
            label,
            command_id: model.command_id_at(i),
            enabled: model.is_enabled_at(i) != 0,
        });
    }
    out
}

/// Karere's own "Copy Link Address" entry. CEF's menu has no link items, so
/// links were uncopyable (#205).
pub(crate) const MENU_ID_COPY_LINK: i32 = cef::sys::cef_menu_id_t::MENU_ID_USER_FIRST as i32;

/// Karere's own "Copy Image" entry. Chromium's image commands are stripped by
/// the allowlist and CEF's OSR clipboard does not reach GDK (#215).
pub(crate) const MENU_ID_COPY_IMAGE: i32 = cef::sys::cef_menu_id_t::MENU_ID_USER_FIRST as i32 + 1;

/// Upper bound on raw BGRA pixel data accepted for a context-menu image copy
/// (128 MiB ≈ 5792×5792 px).
const MAX_CLIPBOARD_BITMAP_BYTES: usize = 128 * 1024 * 1024;

/// Schemes we allow `BrowserHost::download_image` to fetch for Copy Image.
/// Rejects local/internal schemes such as `file:`, `chrome:`, or `javascript:`.
fn is_copyable_image_url(url: &str) -> bool {
    let Some((scheme, rest)) = url.split_once(':') else {
        return false;
    };
    if rest.is_empty() {
        return false;
    }
    matches!(
        scheme.to_ascii_lowercase().as_str(),
        "https" | "http" | "blob" | "data"
    )
}

/// Validate decoded bitmap dimensions and return `(stride, expected_bytes)`.
fn validate_bitmap_layout(width: i32, height: i32) -> Option<(usize, usize)> {
    let w = usize::try_from(width).ok().filter(|&w| w > 0)?;
    let h = usize::try_from(height).ok().filter(|&h| h > 0)?;
    let stride = w.checked_mul(4)?;
    let total = stride.checked_mul(h)?;
    (total <= MAX_CLIPBOARD_BITMAP_BYTES).then_some((stride, total))
}

/// Commands the host menu may run: editing, spellcheck and Karere's own Copy
/// Link Address / Copy Image. Everything else CEF offers (navigation, print,
/// view source, open link in new tab/window) is removed from the menu and
/// refused.
pub(crate) fn is_allowed_command(command_id: i32) -> bool {
    use cef::sys::cef_menu_id_t as M;
    let id = command_id;
    (M::MENU_ID_UNDO as i32..=M::MENU_ID_SELECT_ALL as i32).contains(&id)
        || (M::MENU_ID_SPELLCHECK_SUGGESTION_0 as i32..=M::MENU_ID_ADD_TO_DICTIONARY as i32)
            .contains(&id)
        || id == MENU_ID_COPY_LINK
        || id == MENU_ID_COPY_IMAGE
}

/// Classify Chromium's Copy command by stable CEF command id, never by its
/// localized display label.
pub(crate) fn is_copy_command(command_id: i32) -> bool {
    command_id == cef::sys::cef_menu_id_t::MENU_ID_COPY as i32
}

/// Classify Chromium's Paste command by stable CEF command id.
pub(crate) fn is_paste_command(command_id: i32) -> bool {
    command_id == cef::sys::cef_menu_id_t::MENU_ID_PASTE as i32
}

#[derive(Clone, Default)]
pub struct ShellContextMenuHandler;

wrap_context_menu_handler! {
    pub struct ShellContextMenuHandlerBuilder {
        handler: ShellContextMenuHandler,
    }

    impl ContextMenuHandler {
        fn on_before_context_menu(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            params: Option<&mut ContextMenuParams>,
            model: Option<&mut MenuModel>,
        ) {
            let Some(model) = model else { return };
            strip_disallowed_entries(model);
            let has_image = params.as_ref().is_some_and(|p| {
                p.media_type() == ContextMenuMediaType::IMAGE
                    && p.has_image_contents() != 0
                    && is_copyable_image_url(&userfree_to_string(p.source_url()))
            });
            let has_link = params
                .as_ref()
                .is_some_and(|p| !userfree_to_string(p.link_url()).is_empty());
            let had_items = model.count() > 0;
            let mut pos = 0;
            if has_image {
                let label = CefString::from(gettextrs::gettext("Copy Image").as_str());
                model.insert_item_at(pos, MENU_ID_COPY_IMAGE, Some(&label));
                pos += 1;
            }
            if has_link {
                let label = CefString::from(gettextrs::gettext("Copy Link Address").as_str());
                model.insert_item_at(pos, MENU_ID_COPY_LINK, Some(&label));
                pos += 1;
            }
            if pos > 0 && had_items {
                model.insert_separator_at(pos);
            }
        }

        fn on_context_menu_command(
            &self,
            browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            params: Option<&mut ContextMenuParams>,
            command_id: c_int,
            _event_flags: cef::EventFlags,
        ) -> c_int {
            if !is_allowed_command(command_id) {
                log::debug!("context menu: refused command {command_id}");
                return 1;
            }
            if command_id == MENU_ID_COPY_LINK {
                if let Some(p) = params {
                    super::client::write_regular_clipboard(&userfree_to_string(p.link_url()));
                }
                return 1;
            }
            if command_id == MENU_ID_COPY_IMAGE {
                if let (Some(browser), Some(p)) = (browser, params) {
                    let url = userfree_to_string(p.source_url());
                    if is_copyable_image_url(&url) {
                        download_image_to_clipboard(browser, &url);
                    }
                }
                return 1;
            }
            0
        }

        // OSR has no platform window for CEF to draw the menu, so the host
        // renders it: snapshot the (link-stripped) model and hand it + cursor
        // pos + non-`Send` callback to the GTK widget keyed by browser id.
        // Returning 1 takes ownership; the widget guarantees one `cont`/`cancel`.
        fn run_context_menu(
            &self,
            browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            params: Option<&mut ContextMenuParams>,
            model: Option<&mut MenuModel>,
            callback: Option<&mut RunContextMenuCallback>,
        ) -> c_int {
            let (Some(model), Some(callback)) = (model, callback) else {
                // Nothing to manage → let CEF keep default ownership.
                return 0;
            };
            let callback = callback.clone();

            let items = snapshot_model(model);
            if items.is_empty() {
                callback.cancel();
                return 1;
            }

            // Cursor in view coords — DIP, matching the mouse events we feed CEF.
            let (x, y) = match &params {
                Some(p) => {
                    if log::log_enabled!(log::Level::Debug) {
                        let word = userfree_to_string(p.misspelled_word());
                        if !word.is_empty() {
                            let mut sugg = cef::CefStringList::new();
                            p.dictionary_suggestions(Some(&mut sugg));
                            log::debug!(
                                "context menu: misspelled {word:?} suggestions {:?}",
                                sugg.into_iter().collect::<Vec<_>>()
                            );
                        }
                    }
                    (p.xcoord(), p.ycoord())
                }
                None => (0, 0),
            };

            let browser_id = browser.as_ref().map(|b| b.identifier()).unwrap_or(-1);
            crate::web_view::dispatch_context_menu(browser_id, items, x, y, callback);
            1
        }
    }
}

impl ShellContextMenuHandlerBuilder {
    pub fn build() -> ContextMenuHandler {
        Self::new(ShellContextMenuHandler)
    }
}

#[derive(Clone)]
struct CopyImageCallback {
    browser: Browser,
    url: String,
}

wrap_download_image_callback! {
    struct CopyImageCallbackBuilder {
        inner: CopyImageCallback,
    }

    impl DownloadImageCallback {
        fn on_download_image_finished(
            &self,
            _image_url: Option<&CefString>,
            http_status_code: c_int,
            image: Option<&mut Image>,
        ) {
            if let Some(image) = image.filter(|img| img.is_empty() == 0)
                && copy_cef_image_to_clipboard(image)
            {
                return;
            }
            log::debug!(
                "copy image: DownloadImage returned no usable bitmap (status {http_status_code}); falling back to renderer fetch"
            );
            send_renderer_copy_image(&self.inner.browser, &self.inner.url);
        }
    }
}

impl CopyImageCallbackBuilder {
    fn build(browser: Browser, url: String) -> DownloadImageCallback {
        Self::new(CopyImageCallback { browser, url })
    }
}

/// Fetch `url` through Blink's frame-scoped image downloader and copy the
/// decoded bitmap onto the GDK clipboard (#215). Falls back to renderer-side
/// `fetch` IPC if `DownloadImage` cannot deliver a usable bitmap. Runs on the
/// CEF UI thread.
fn download_image_to_clipboard(browser: &Browser, url: &str) {
    let Some(host) = browser.host() else {
        log::warn!("copy image: browser has no host");
        send_renderer_copy_image(browser, url);
        return;
    };
    let mut callback = CopyImageCallbackBuilder::build(browser.clone(), url.to_owned());
    host.download_image(
        Some(&CefString::from(url)),
        0, // is_favicon = false (keep session context)
        0, // max_image_size = 0 (unlimited / full resolution)
        0, // bypass_cache = false (reuse already-loaded image)
        Some(&mut callback),
    );
}

/// Fallback when `BrowserHost::download_image` does not return an image: ask
/// the renderer copy bridge to fetch `url`, convert to PNG if needed, and
/// return it via `RendererMessage::SetClipboardImage`.
fn send_renderer_copy_image(browser: &Browser, url: &str) {
    let Some(frame) = browser.main_frame() else {
        log::warn!("copy image fallback: browser has no main frame");
        return;
    };
    let msg = crate::ipc::BrowserMessage::CopyImage {
        url: url.to_owned(),
    };
    if let Some(mut cef_msg) = msg.to_cef_message() {
        frame.send_process_message(ProcessId::RENDERER, Some(&mut cef_msg));
    }
}

/// Extract the decoded bitmap from `image` and place it on the GDK clipboard.
/// Returns `true` when the texture was written to the clipboard.
fn copy_cef_image_to_clipboard(image: &Image) -> bool {
    use gtk::prelude::Cast;

    let mut width: c_int = 0;
    let mut height: c_int = 0;
    let Some(binary) = image.as_bitmap(
        1.0,
        ColorType::BGRA_8888,
        AlphaType::PREMULTIPLIED,
        Some(&mut width),
        Some(&mut height),
    ) else {
        log::warn!("copy image: failed to extract bitmap");
        return false;
    };
    let Some((stride, expected_bytes)) = validate_bitmap_layout(width, height) else {
        log::warn!("copy image: invalid or oversized dimensions ({width}x{height})");
        return false;
    };
    let size = binary.size();
    if size != expected_bytes {
        log::warn!("copy image: unexpected bitmap size ({size} != {expected_bytes})");
        return false;
    }
    let mut buf = vec![0u8; size];
    let copied = binary.data(Some(&mut buf), 0);
    if copied != size {
        log::warn!("copy image: short bitmap read ({copied} != {size})");
        return false;
    }
    let texture = gtk::gdk::MemoryTexture::new(
        width,
        height,
        gtk::gdk::MemoryFormat::B8g8r8a8Premultiplied,
        &gtk::glib::Bytes::from_owned(buf),
        stride,
    );
    super::client::write_clipboard_texture(texture.upcast_ref());
    true
}

/// Remove every entry whose command is not [`is_allowed_command`] (recursing
/// into submenus, dropping ones left empty) and any separator that orphans
/// (leading, trailing, or doubled).
fn strip_disallowed_entries(model: &MenuModel) {
    let count = model.count();
    if count == 0 {
        return;
    }

    let types: Vec<MenuItemType> = (0..count).map(|i| model.type_at(i)).collect();
    let mut remove = vec![false; count];
    for (i, slot) in remove.iter_mut().enumerate() {
        *slot = if types[i] == MenuItemType::SEPARATOR {
            false
        } else if types[i] == MenuItemType::SUBMENU {
            match model.sub_menu_at(i) {
                Some(sub) => {
                    strip_disallowed_entries(&sub);
                    sub.count() == 0
                }
                None => true,
            }
        } else {
            !is_allowed_command(model.command_id_at(i))
        };
    }

    // Leave menus with nothing to remove untouched.
    if remove.iter().any(|&r| r) {
        normalize_separators(&types, &mut remove);
    }

    // Remove from the back so earlier indices stay valid.
    for i in (0..count).rev() {
        if remove[i] {
            model.remove_at(i);
        }
    }
}

/// Mark separators that become leading, trailing, or doubled once the forbidden
/// entries are gone.
fn normalize_separators(types: &[MenuItemType], remove: &mut [bool]) {
    // Treat menu start as a separator boundary so a leading separator drops.
    let mut prev_kept_is_separator = true;
    let mut last_kept: Option<usize> = None;

    for i in 0..types.len() {
        if remove[i] {
            continue;
        }
        if types[i] == MenuItemType::SEPARATOR {
            if prev_kept_is_separator {
                remove[i] = true;
                continue;
            }
            prev_kept_is_separator = true;
        } else {
            prev_kept_is_separator = false;
        }
        last_kept = Some(i);
    }

    if let Some(i) = last_kept
        && types[i] == MenuItemType::SEPARATOR
    {
        remove[i] = true;
    }
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_CLIPBOARD_BITMAP_BYTES, MENU_ID_COPY_IMAGE, MENU_ID_COPY_LINK, is_allowed_command,
        is_copy_command, is_copyable_image_url, is_paste_command, validate_bitmap_layout,
    };

    #[test]
    fn only_edit_spellcheck_and_copy_commands_are_allowed() {
        use cef::sys::cef_menu_id_t as M;
        for id in [
            M::MENU_ID_UNDO,
            M::MENU_ID_CUT,
            M::MENU_ID_COPY,
            M::MENU_ID_PASTE,
            M::MENU_ID_SELECT_ALL,
            M::MENU_ID_SPELLCHECK_SUGGESTION_0,
            M::MENU_ID_SPELLCHECK_SUGGESTION_4,
            M::MENU_ID_NO_SPELLING_SUGGESTIONS,
            M::MENU_ID_ADD_TO_DICTIONARY,
        ] {
            assert!(is_allowed_command(id as i32), "{id:?}");
        }
        assert_ne!(MENU_ID_COPY_LINK, MENU_ID_COPY_IMAGE);
        assert!(is_allowed_command(MENU_ID_COPY_LINK));
        assert!(is_allowed_command(MENU_ID_COPY_IMAGE));
        for id in [
            M::MENU_ID_BACK,
            M::MENU_ID_RELOAD,
            M::MENU_ID_PRINT,
            M::MENU_ID_VIEW_SOURCE,
            M::MENU_ID_FIND,
        ] {
            assert!(!is_allowed_command(id as i32), "{id:?}");
        }
        // Chromium's "Open link in new tab/window/incognito" and native image commands.
        for id in [50100, 50101, 50102, 50112, 50113, 50114, 0, -1] {
            assert!(!is_allowed_command(id));
        }
    }

    #[test]
    fn copyable_image_url_scheme_allowlist() {
        for ok in [
            "blob:https://web.whatsapp.com/1234-abcd",
            "BLOB:https://web.whatsapp.com/1234-abcd",
            "data:image/png;base64,iVBORw0KGgo=",
            "https://pps.whatsapp.net/v/t61.24694-24/avatar.jpg",
            "http://127.0.0.1:8080/test.png",
        ] {
            assert!(is_copyable_image_url(ok), "{ok}");
        }
        for bad in [
            "",
            "blob:",
            "data:",
            "relative/image.png",
            "file:///etc/passwd",
            "chrome://settings",
            "javascript:alert(1)",
            "about:blank",
        ] {
            assert!(!is_copyable_image_url(bad), "{bad}");
        }
    }

    #[test]
    fn bitmap_layout_validation() {
        assert_eq!(validate_bitmap_layout(10, 20), Some((40, 800)));
        assert_eq!(validate_bitmap_layout(0, 10), None);
        assert_eq!(validate_bitmap_layout(10, 0), None);
        assert_eq!(validate_bitmap_layout(-1, 10), None);
        assert_eq!(validate_bitmap_layout(i32::MAX, i32::MAX), None);
        // 6000 * 6000 * 4 = 144 MiB > 128 MiB cap.
        const { assert!(6000 * 6000 * 4 > MAX_CLIPBOARD_BITMAP_BYTES) };
        assert_eq!(validate_bitmap_layout(6000, 6000), None);
    }

    #[test]
    fn copy_is_classified_by_cef_command_id_not_label() {
        let copy = cef::sys::cef_menu_id_t::MENU_ID_COPY as i32;
        let paste = cef::sys::cef_menu_id_t::MENU_ID_PASTE as i32;
        assert!(is_copy_command(copy));
        assert!(!is_copy_command(paste));
        assert!(!is_copy_command(0));
        assert!(is_paste_command(paste));
        assert!(!is_paste_command(copy));
    }
}
