use cef::{self, ImplBrowser, ImplFrame, ProcessId};
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

use super::imp::resolved_browser;
use super::input::mouse::widget_to_page;

/// Monotonic time (µs, `glib::monotonic_time`) of the last host-observed user
/// input on the web view: key press, button press, drag motion, touch, or a
/// context-menu activation. Gates page-initiated clipboard writes. (S9)
static LAST_USER_INPUT_US: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(i64::MIN);

/// Record that the user just interacted with the web view. Main thread.
pub(crate) fn note_user_input() {
    LAST_USER_INPUT_US.store(glib::monotonic_time(), std::sync::atomic::Ordering::Relaxed);
}

/// True when the user interacted with the web view within the last `within_ms`.
pub fn recent_user_input(within_ms: i64) -> bool {
    let last = LAST_USER_INPUT_US.load(std::sync::atomic::Ordering::Relaxed);
    last != i64::MIN && glib::monotonic_time().saturating_sub(last) <= within_ms * 1000
}

pub(super) fn is_copy_shortcut(keyval: gtk::gdk::Key, state: gtk::gdk::ModifierType) -> bool {
    use gtk::gdk::{Key, ModifierType};
    state.contains(ModifierType::CONTROL_MASK)
        && !state.intersects(ModifierType::ALT_MASK | ModifierType::SUPER_MASK)
        && matches!(keyval, Key::c | Key::C)
}

#[derive(Debug, Clone, Copy)]
pub(super) enum ExplicitCopyTrigger {
    Keyboard {
        keyval: gtk::gdk::Key,
        state: gtk::gdk::ModifierType,
    },
    ContextMenu(i32),
}

/// Shared production trigger used by both the installed key controller and
/// the CEF context-menu callback. The request sink is injectable so tests
/// cover command dispatch, not just shortcut/command classification.
pub(super) fn dispatch_explicit_copy(trigger: ExplicitCopyTrigger, request: impl FnOnce()) {
    // A menu item is picked in the popover, not on the web view, so no
    // press handler saw it; its Copy reply must still pass the gate. (S9)
    if matches!(trigger, ExplicitCopyTrigger::ContextMenu(_)) {
        super::note_user_input();
    }
    let requested = match trigger {
        ExplicitCopyTrigger::Keyboard { keyval, state } => is_copy_shortcut(keyval, state),
        ExplicitCopyTrigger::ContextMenu(command_id) => {
            crate::handlers::context_menu::is_copy_command(command_id)
        }
    };
    if requested {
        request();
    }
}

// ---- M17 paste / drop bridge ---------------------------------------

/// Send a [`crate::ipc::BrowserMessage`] to the renderer's main frame.
fn send_browser_message(widget: &super::KarereWebView, msg: crate::ipc::BrowserMessage) {
    if let Some(browser) = resolved_browser(widget.imp())
        && let Some(frame) = browser.main_frame()
        && let Some(mut m) = msg.to_cef_message()
    {
        frame.send_process_message(cef::ProcessId::RENDERER, Some(&mut m));
    }
}

/// Marshal a binary clipboard/drop payload into a `DispatchPasteEvent` (`kind` =
/// `"paste"` or `"drop"`). Large payloads round-trip via a tempfile (see
/// [`crate::paste::make_blob`]).
fn send_blob_paste(
    widget: &super::KarereWebView,
    mime: String,
    kind: &str,
    bytes: &[u8],
    name: Option<String>,
    coords: Option<(f64, f64)>,
) {
    let payload = match crate::paste::make_blob(bytes) {
        Ok(p) => p,
        Err(err) => {
            log::warn!("paste: {err}");
            toast_paste_too_large(widget);
            return;
        }
    };
    let (x, y) = match coords {
        Some((x, y)) => (Some(x), Some(y)),
        None => (None, None),
    };
    send_browser_message(
        widget,
        crate::ipc::BrowserMessage::DispatchPasteEvent {
            mime,
            kind: kind.to_string(),
            payload,
            name,
            x,
            y,
        },
    );
}

/// Tell the user an oversized paste/drop was refused (`make_blob` limit).
fn toast_paste_too_large(widget: &super::KarereWebView) {
    if let Some(window) = widget.root().and_downcast::<crate::window::KarereWindow>() {
        window.show_toast(&gettextrs::gettext("File is too large to attach (limit 48 MB)"));
    }
}

/// Notify the page that a file drag is hovering so it can pre-mount its
/// dropzone before the drop commits.
pub(super) fn send_drag_hover(widget: &super::KarereWebView, phase: &str, x: f64, y: f64) {
    // The bridge hit-tests these with elementFromPoint: CSS px. (I8)
    let (x, y) = widget_to_page(widget, x, y);
    send_browser_message(
        widget,
        crate::ipc::BrowserMessage::DragHover {
            phase: phase.to_string(),
            x,
            y,
        },
    );
}

/// Marshal text as an unnamed `text/plain` paste (`kind` "paste") or text
/// drop ("drop"). With coords the page targets the editable under them:
/// middle-click PRIMARY paste, or a dragged-in text/URL.
pub(super) fn send_text_paste(
    widget: &super::KarereWebView,
    text: &str,
    kind: &str,
    coords: Option<(f64, f64)>,
) {
    let (x, y) = match coords {
        Some((x, y)) => (Some(x), Some(y)),
        None => (None, None),
    };
    send_browser_message(
        widget,
        crate::ipc::BrowserMessage::DispatchPasteEvent {
            mime: "text/plain".to_string(),
            kind: kind.to_string(),
            payload: crate::ipc::PasteBlob::Base64(crate::paste::b64(text.as_bytes())),
            name: None,
            x,
            y,
        },
    );
}

/// On Ctrl+V, inspect the GDK clipboard. If it holds an image or files, async-read
/// and dispatch a synthetic paste, returning `true` so the caller swallows the key
/// (CEF's GDK-blind native paste must not also fire). Text-only/empty → `false`.
pub(super) fn try_intercept_paste(widget: &super::KarereWebView) -> bool {
    let Some(display) = gtk::gdk::Display::default() else {
        return false;
    };
    let clipboard = display.clipboard();
    let formats = clipboard.formats();

    let has_image = formats.contains_type(gtk::gdk::Texture::static_type())
        || formats.contain_mime_type("image/png")
        || formats.contain_mime_type("image/jpeg")
        || formats.contain_mime_type("image/gif")
        || formats.contain_mime_type("image/webp");
    if has_image {
        read_clipboard_image(widget, &clipboard);
        return true;
    }

    let has_files = formats.contains_type(gtk::gdk::FileList::static_type())
        || formats.contain_mime_type("text/uri-list");
    if has_files {
        read_clipboard_files(widget, &clipboard);
        return true;
    }

    // CEF's windowless clipboard doesn't consult GDK, so native Ctrl+V pastes
    // nothing. Read GDK text ourselves and synthesize, like the image/file paths.
    let has_text = formats.contains_type(glib::Type::STRING)
        || formats.contain_mime_type("text/plain")
        || formats.contain_mime_type("text/plain;charset=utf-8")
        || formats.contain_mime_type("UTF8_STRING");
    if has_text {
        read_clipboard_text(widget, &clipboard);
        return true;
    }

    false
}

fn read_clipboard_text(widget: &super::KarereWebView, clipboard: &gtk::gdk::Clipboard) {
    clipboard.read_text_async(
        gtk::gio::Cancellable::NONE,
        glib::clone!(
            #[weak]
            widget,
            move |res| match res {
                Ok(Some(text)) if !text.is_empty() => {
                    send_text_paste(&widget, text.as_str(), "paste", None)
                }
                Ok(_) => {}
                Err(err) => log::warn!("clipboard text read failed: {err}"),
            }
        ),
    );
}

/// Encoded image types taken from the clipboard as-is, most specific first:
/// an animated GIF or a WebP must not be flattened into a PNG still.
pub(super) const CLIPBOARD_IMAGE_MIMES: &[&str] = &["image/gif", "image/webp", "image/jpeg", "image/png"];

/// The first of [`CLIPBOARD_IMAGE_MIMES`] for which `offers` is true. A
/// source that copied a plain texture (a GTK screenshot tool) is offered in
/// every format GDK can convert to, BMP/TIFF/ICO included; those are not
/// originals, and PNG is the lossless one among them.
pub(super) fn preferred_image_mime(offers: impl Fn(&str) -> bool) -> Option<&'static str> {
    let converted = ["image/bmp", "image/tiff", "image/x-icon", "image/x-ico"]
        .iter()
        .any(|mime| offers(mime));
    if converted && offers("image/png") {
        return Some("image/png");
    }
    CLIPBOARD_IMAGE_MIMES.iter().copied().find(|mime| offers(mime))
}

fn read_clipboard_image(widget: &super::KarereWebView, clipboard: &gtk::gdk::Clipboard) {
    let formats = clipboard.formats();
    let weak = widget.downgrade();
    let clipboard = clipboard.clone();
    glib::spawn_future_local(async move {
        // The original bytes when the source offers an encoded image.
        if let Some(mime) = preferred_image_mime(|m| formats.contain_mime_type(m)) {
            let read = match clipboard.read_future(&[mime], glib::Priority::DEFAULT).await {
                Ok((stream, _mime)) => read_stream_bytes(stream).await,
                Err(err) => Err(err),
            };
            match read {
                Ok(bytes) if !bytes.is_empty() => {
                    if let Some(widget) = weak.upgrade() {
                        send_blob_paste(&widget, mime.to_string(), "paste", &bytes, None, None);
                    }
                    return;
                }
                Ok(_) => log::debug!("clipboard {mime} read: empty"),
                Err(err) => log::warn!("clipboard {mime} read failed: {err}"),
            }
        }
        // Only a texture (another format GDK can decode): encode a PNG, off
        // the main thread — a large screenshot takes long enough to stall
        // the UI. A GdkTexture is immutable and Send.
        let texture = match clipboard.read_texture_future().await {
            Ok(Some(texture)) => texture,
            Ok(None) => return log::debug!("clipboard image read: empty"),
            Err(err) => return log::warn!("clipboard image read failed: {err}"),
        };
        let bytes = match gtk::gio::spawn_blocking(move || texture.save_to_png_bytes()).await {
            Ok(bytes) => bytes,
            Err(_) => return log::warn!("clipboard image PNG encode panicked"),
        };
        if let Some(widget) = weak.upgrade() {
            send_blob_paste(&widget, "image/png".to_string(), "paste", &bytes, None, None);
        }
    });
}

/// Read a GIO stream to its end.
async fn read_stream_bytes(stream: gtk::gio::InputStream) -> Result<glib::Bytes, glib::Error> {
    let out = gtk::gio::MemoryOutputStream::new_resizable();
    out.splice_future(
        &stream,
        gtk::gio::OutputStreamSpliceFlags::CLOSE_SOURCE
            | gtk::gio::OutputStreamSpliceFlags::CLOSE_TARGET,
        glib::Priority::DEFAULT,
    )
    .await?;
    Ok(out.steal_as_bytes())
}

fn read_clipboard_files(widget: &super::KarereWebView, clipboard: &gtk::gdk::Clipboard) {
    if clipboard.formats().contain_mime_type(PORTAL_FILE_TRANSFER) {
        clipboard.read_async(
            &[PORTAL_FILE_TRANSFER],
            glib::Priority::DEFAULT,
            gtk::gio::Cancellable::NONE,
            glib::clone!(
                #[weak]
                widget,
                move |res| match res {
                    Ok((stream, _mime)) => send_portal_files(&widget, stream, "paste", None),
                    Err(err) => log::warn!("clipboard transfer read failed: {err}"),
                }
            ),
        );
        return;
    }
    clipboard.read_value_async(
        gtk::gdk::FileList::static_type(),
        glib::Priority::DEFAULT,
        gtk::gio::Cancellable::NONE,
        glib::clone!(
            #[weak]
            widget,
            move |res| match res {
                Ok(value) => match value.get::<gtk::gdk::FileList>() {
                    Ok(list) => load_and_send_files(&widget, list.files(), "paste", None, true),
                    Err(err) => log::warn!("clipboard file list decode failed: {err}"),
                },
                Err(err) => log::warn!("clipboard file read failed: {err}"),
            }
        ),
    );
}

/// Offered by a source that registered its files with the document portal.
/// The payload is a key; the portal trades it for paths the sandbox can read.
pub(super) const PORTAL_FILE_TRANSFER: &str = "application/vnd.portal.filetransfer";

/// Text forms a text or URL drag offers (GTK, Qt, X11 and browser sources).
pub(super) const DROP_TEXT_MIMES: &[&str] = &[
    "text/plain;charset=utf-8",
    "text/plain",
    "UTF8_STRING",
    "STRING",
    "TEXT",
];

pub(super) fn drop_offers_text(formats: &gtk::gdk::ContentFormats) -> bool {
    formats.contains_type(glib::Type::STRING)
        || DROP_TEXT_MIMES.iter().any(|mime| formats.contain_mime_type(mime))
}

/// Whether a drag may carry files, so its hover should prime the page's file
/// dropzone. A plain text drag must not: the overlay would cover the composer
/// the text is meant for.
pub(super) fn drop_may_carry_files(formats: &gtk::gdk::ContentFormats) -> bool {
    formats.contain_mime_type(PORTAL_FILE_TRANSFER) || formats.contain_mime_type("text/uri-list")
}

/// What a dropped `text/uri-list` stands for.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum UriDrop {
    /// Every entry is a `file://` URI: attach the files.
    Files(Vec<String>),
    /// Web links (or a mix): insert them as text, one per line.
    Text(String),
}

/// Classify a `text/uri-list` payload (RFC 2483: CRLF lines, `#` comments).
/// `None` when it holds no URI.
pub(super) fn classify_uri_list(list: &str) -> Option<UriDrop> {
    let uris: Vec<&str> = list
        .lines()
        .map(|line| line.trim_matches(|c: char| c == '\0' || c.is_whitespace()))
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect();
    if uris.is_empty() {
        return None;
    }
    let is_file = |uri: &&str| uri.get(..7).is_some_and(|s| s.eq_ignore_ascii_case("file://"));
    if uris.iter().all(is_file) {
        Some(UriDrop::Files(uris.iter().map(|u| u.to_string()).collect()))
    } else {
        Some(UriDrop::Text(uris.join("\n")))
    }
}

/// Drop of a `text/uri-list`: local files go through the file attach path
/// (the same `gio::File`s GDK's file-list conversion would build), web links
/// are inserted as text at the drop point.
pub(super) fn read_uri_list_drop(
    widget: &super::KarereWebView,
    drop: &gtk::gdk::Drop,
    coords: Option<(f64, f64)>,
) {
    let weak = widget.downgrade();
    let drop = drop.clone();
    glib::spawn_future_local(async move {
        let read = match drop.read_future(&["text/uri-list"], glib::Priority::DEFAULT).await {
            Ok((stream, _mime)) => read_stream_bytes(stream).await,
            Err(err) => Err(err),
        };
        drop.finish(gtk::gdk::DragAction::COPY);
        let bytes = match read {
            Ok(bytes) => bytes,
            Err(err) => return log::warn!("drop uri list read failed: {err}"),
        };
        let Some(widget) = weak.upgrade() else { return };
        match classify_uri_list(&String::from_utf8_lossy(&bytes)) {
            Some(UriDrop::Files(uris)) => {
                let files = uris.iter().map(|u| gtk::gio::File::for_uri(u)).collect();
                load_and_send_files(&widget, files, "drop", coords, true);
            }
            Some(UriDrop::Text(text)) => send_text_paste(&widget, &text, "drop", coords),
            None => log::debug!("drop uri list: empty"),
        }
    });
}

/// Drop of plain text: inserted into the editable under the drop point.
pub(super) fn read_text_drop(
    widget: &super::KarereWebView,
    drop: &gtk::gdk::Drop,
    coords: Option<(f64, f64)>,
) {
    drop.read_value_async(
        glib::Type::STRING,
        glib::Priority::DEFAULT,
        gtk::gio::Cancellable::NONE,
        glib::clone!(
            #[weak]
            widget,
            #[strong]
            drop,
            move |res| {
                match res.map(|value| value.get::<String>()) {
                    Ok(Ok(text)) if !text.is_empty() => {
                        send_text_paste(&widget, &text, "drop", coords)
                    }
                    Ok(Ok(_)) => {}
                    Ok(Err(err)) => log::warn!("drop text decode failed: {err}"),
                    Err(err) => log::warn!("drop text read failed: {err}"),
                }
                drop.finish(gtk::gdk::DragAction::COPY);
            }
        ),
    );
}

/// Resolve a portal file transfer and dispatch each file as a paste/drop.
/// This is what lets a drop attach without any filesystem permission. (#192)
pub(super) fn send_portal_files(
    widget: &super::KarereWebView,
    stream: gtk::gio::InputStream,
    kind: &'static str,
    coords: Option<(f64, f64)>,
) {
    let widget = widget.downgrade();
    glib::spawn_future_local(async move {
        let files = match retrieve_portal_files(stream).await {
            Ok(files) => files,
            Err(err) => return log::warn!("portal file transfer failed: {err}"),
        };
        let Some(widget) = widget.upgrade() else {
            return;
        };
        let files = files.into_iter().map(gtk::gio::File::for_path).collect();
        load_and_send_files(&widget, files, kind, coords, true);
    });
}

async fn retrieve_portal_files(
    stream: gtk::gio::InputStream,
) -> Result<Vec<String>, glib::Error> {
    let bytes = read_stream_bytes(stream).await?;
    let key = String::from_utf8_lossy(&bytes)
        .trim_matches(|c: char| c == '\0' || c.is_whitespace())
        .to_owned();

    let bus = gtk::gio::bus_get_future(gtk::gio::BusType::Session).await?;
    let options = glib::VariantDict::new(None).end();
    let reply = bus
        .call_future(
            Some("org.freedesktop.portal.Documents"),
            "/org/freedesktop/portal/documents",
            "org.freedesktop.portal.FileTransfer",
            "RetrieveFiles",
            Some(&glib::Variant::tuple_from_iter([key.to_variant(), options])),
            Some(glib::VariantTy::new("(as)").expect("valid variant type")),
            gtk::gio::DBusCallFlags::NONE,
            -1,
        )
        .await?;
    Ok(reply
        .get::<(Vec<String>,)>()
        .map(|(files,)| files)
        .unwrap_or_default())
}

/// Raw bytes one batched paste/drop message may carry; base64 makes that
/// ~64 MB on the wire, the same ceiling a single file always had.
const PASTE_BATCH_BYTES: usize = 48 * 1024 * 1024;

/// Split files of `sizes` bytes, in order, into consecutive batches of at
/// most `budget` bytes. A file larger than the budget gets a batch alone
/// (and is then refused by `make_blob`).
pub(super) fn batch_by_size(sizes: &[usize], budget: usize) -> Vec<std::ops::Range<usize>> {
    let mut batches = Vec::new();
    let (mut start, mut total) = (0, 0usize);
    for (i, &size) in sizes.iter().enumerate() {
        if i > start && total.saturating_add(size) > budget {
            batches.push(start..i);
            start = i;
            total = 0;
        }
        total = total.saturating_add(size);
    }
    if start < sizes.len() {
        batches.push(start..sizes.len());
    }
    batches
}

/// Load every file, then dispatch them together so the page builds one
/// `DataTransfer` holding all of them: one paste/drop, one attach preview,
/// instead of one per file. (J4)
pub(super) fn load_and_send_files(
    widget: &super::KarereWebView,
    files: Vec<gtk::gio::File>,
    kind: &'static str,
    coords: Option<(f64, f64)>,
    offer_chooser: bool,
) {
    if files.is_empty() {
        return;
    }
    let weak = widget.downgrade();
    glib::spawn_future_local(async move {
        let mut loaded = Vec::new();
        let mut unreadable = None;
        for file in files {
            let name = file.basename().map(|p| p.to_string_lossy().into_owned());
            match file.load_contents_future().await {
                Ok((bytes, _etag)) => {
                    let mime = guess_mime(name.as_deref(), &bytes);
                    loaded.push((name, mime, bytes));
                }
                Err(err) => {
                    log::warn!("read dropped/pasted file failed: {err}");
                    unreadable.get_or_insert(file);
                }
            }
        }
        let Some(widget) = weak.upgrade() else {
            return;
        };
        let sizes: Vec<usize> = loaded.iter().map(|(_, _, bytes)| bytes.len()).collect();
        let mut too_large = false;
        for batch in batch_by_size(&sizes, PASTE_BATCH_BYTES) {
            let files: Vec<crate::ipc::PasteFile> = loaded[batch]
                .iter()
                .filter_map(|(name, mime, bytes)| match crate::paste::make_blob(bytes) {
                    Ok(payload) => Some(crate::ipc::PasteFile {
                        mime: mime.clone(),
                        name: name.clone(),
                        payload,
                    }),
                    Err(err) => {
                        log::warn!("paste: {err}");
                        too_large = true;
                        None
                    }
                })
                .collect();
            if files.is_empty() {
                continue;
            }
            let (x, y) = coords.map_or((None, None), |(x, y)| (Some(x), Some(y)));
            send_browser_message(
                &widget,
                crate::ipc::BrowserMessage::DispatchPasteFiles {
                    kind: kind.to_string(),
                    files,
                    x,
                    y,
                },
            );
        }
        if too_large {
            toast_paste_too_large(&widget);
        }
        if offer_chooser && let Some(file) = unreadable {
            choose_unreadable_file(&widget, &file, kind, coords);
        }
    });
}

/// A drop, or a paste of a file copied in a file manager, hands over a plain
/// path with no portal involved, and the sandbox cannot read most of those.
/// Only the file chooser portal can grant access, so say why nothing was
/// attached and offer the chooser, opened in the file's folder. (#192)
fn choose_unreadable_file(
    widget: &super::KarereWebView,
    file: &gtk::gio::File,
    kind: &'static str,
    coords: Option<(f64, f64)>,
) {
    let Some(window) = widget.root().and_downcast::<crate::window::KarereWindow>() else {
        return;
    };
    if widget.imp().attach_chooser_open.replace(true) {
        return;
    }
    let toast = libadwaita::Toast::new(&gettextrs::gettext(
        "Karere cannot read files from this folder",
    ));
    toast.set_button_label(Some(&gettextrs::gettext("Choose File…")));
    toast.set_timeout(8);

    let chosen = std::rc::Rc::new(std::cell::Cell::new(false));
    toast.connect_button_clicked(glib::clone!(
        #[weak]
        widget,
        #[weak]
        window,
        #[strong]
        file,
        #[strong]
        chosen,
        move |_| {
            chosen.set(true);
            let dialog = gtk::FileDialog::builder()
                .title(gettextrs::gettext("Select the File to Attach"))
                .accept_label(gettextrs::gettext("Attach"))
                .modal(true)
                .build();
            dialog.set_initial_folder(file.parent().as_ref());
            dialog.open_multiple(
                Some(&window),
                gtk::gio::Cancellable::NONE,
                glib::clone!(
                    #[weak]
                    widget,
                    move |res| {
                        widget.imp().attach_chooser_open.set(false);
                        // An error here is the user dismissing the chooser.
                        let Ok(files) = res else { return };
                        let files = files.iter::<gtk::gio::File>().flatten().collect();
                        load_and_send_files(&widget, files, kind, coords, false);
                    }
                ),
            );
        }
    ));
    toast.connect_dismissed(glib::clone!(
        #[weak]
        widget,
        move |_| {
            if !chosen.get() {
                widget.imp().attach_chooser_open.set(false);
            }
        }
    ));
    window.imp().toast_overlay.add_toast(toast);
}

/// Best-effort MIME from filename + leading bytes; defaults to octet-stream.
fn guess_mime(name: Option<&str>, bytes: &[u8]) -> String {
    let (content_type, _certain) = gtk::gio::content_type_guess(name, bytes);
    gtk::gio::content_type_get_mime_type(content_type.as_str())
        .map(|m| m.to_string())
        .unwrap_or_else(|| "application/octet-stream".to_string())
}

/// Final browser→renderer transport used by explicit Copy. Keeping the seam
/// at `Frame::send_process_message` lets the headless regression capture the
/// real typed CEF envelope while production still resolves the live frame.
pub(super) trait RendererMessageSink {
    fn send_to_renderer(&mut self, message: crate::ipc::BrowserMessage);
}

impl RendererMessageSink for cef::Frame {
    fn send_to_renderer(&mut self, message: crate::ipc::BrowserMessage) {
        if let Some(mut message) = message.to_cef_message() {
            self.send_process_message(ProcessId::RENDERER, Some(&mut message));
        }
    }
}

pub(super) fn request_live_selection_with<S: RendererMessageSink>(
    resolve: impl FnOnce() -> Option<S>,
) -> bool {
    let Some(mut sink) = resolve() else {
        return false;
    };
    sink.send_to_renderer(crate::ipc::BrowserMessage::CopySelection);
    true
}

/// Ask only the current foreground browser's main renderer frame to copy its
/// live selection. The response returns through RendererMessage::SetClipboard.
pub(super) fn request_live_selection(widget: &super::KarereWebView) {
    request_live_selection_with(|| {
        resolved_browser(widget.imp()).and_then(|browser| browser.main_frame())
    });
}

/// Middle-click: paste primary-clipboard text into the element at `(x, y)` if
/// editable (page-enforced). Empty selection sends nothing.
pub(super) fn read_primary_clipboard_paste(widget: &super::KarereWebView, x: f64, y: f64) {
    let Some(display) = gtk::gdk::Display::default() else {
        return;
    };
    display.primary_clipboard().read_text_async(
        gtk::gio::Cancellable::NONE,
        glib::clone!(
            #[weak]
            widget,
            move |res| match res {
                Ok(Some(text)) if !text.is_empty() => {
                    send_text_paste(&widget, text.as_str(), "paste", Some((x, y)))
                }
                Ok(_) => {}
                Err(err) => log::debug!("primary clipboard read failed: {err}"),
            }
        ),
    );
}
