//! Browser-side notification tracker.
//!
//! The renderer's [`crate::ipc::RendererMessage::NotificationSeen`] /
//! `NotificationClosed` events feed this [`Tracker`], the single source of truth
//! for live notification tags in the browser process. It:
//!
//! - records seen tags (for unread counts — M20 — and focus-driven withdrawal),
//! - re-emits a Karere-branded `gio::Notification` so the desktop attributes the
//!   banner to Karere (app name + icon) rather than Chromium.
//!
//! All methods run on the glib main thread (the CEF UI thread under
//! `external_message_pump`), so the inner [`Mutex`] is only guarding against
//! re-entrancy, and the gio calls here are main-thread-safe.

use std::collections::HashMap;
use std::time::Instant;

use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use parking_lot::Mutex;
use std::sync::OnceLock;

use crate::application::APP_ID;

/// The application action fired when a branded banner is clicked. Its string
/// target is the notification tag (see [`crate::actions`]).
pub const ACTIVATE_ACTION: &str = "app.notification-clicked";

/// Process-global tracker. Everything runs on the main thread, so a single
/// shared instance is sufficient and avoids threading it through every handler.
pub fn tracker() -> &'static Tracker {
    static TRACKER: OnceLock<Tracker> = OnceLock::new();
    TRACKER.get_or_init(Tracker::new)
}

pub struct Tracker {
    /// Live notification tags → first-seen timestamp. Overwritten on repeat so
    /// WhatsApp's per-chat tag reuse collapses onto one entry.
    live: Mutex<HashMap<String, Instant>>,
}

impl Tracker {
    pub fn new() -> Self {
        Self {
            live: Mutex::new(HashMap::new()),
        }
    }

    /// A notification became visible in the page. Records the tag, sets the
    /// per-account unread dot (unless the account is already foregrounded),
    /// bumps the global tray count, and emits the branded banner (3.2 + 3.7).
    pub fn on_seen(
        &self,
        tag: &str,
        title: &str,
        body: &str,
        icon: Option<&str>,
        account_id: &str,
    ) {
        log::info!(
            "notifications: on_seen tag={tag:?} title={title:?} body_len={} has_icon={}",
            body.len(),
            icon.is_some()
        );
        let settings = gio::Settings::new(APP_ID);
        // Global kill-switch: no banner, no sound when notifications are off or
        // message notifications are disabled.
        if !settings.boolean("notifications-enabled") || !settings.boolean("notify-messages") {
            log::info!("notifications: suppressed by kill-switch (enabled/messages off)");
            return;
        }

        self.live.lock().insert(tag.to_owned(), Instant::now());

        // Per-account unread dot (switcher + tray). Skip the active account: a
        // banner for the account already in the foreground should not badge the
        // row the user is looking at. Cleared when an account becomes the focused
        // foreground (window focus / switch).
        let is_active = crate::accounts::manager()
            .active()
            .is_some_and(|a| a.id == account_id);
        if !is_active {
            crate::accounts::set_unread(account_id, true);
        }

        // M15: feed the tray unread indicator. Activate `app.set-unread` with
        // current+1 so the tray module stays the single owner of the count.
        // Gated on `notify-tray-icon`: when off, the tray icon must not change on
        // a new message.
        if settings.boolean("notify-tray-icon")
            && let Some(app) = gio::Application::default()
        {
            let next = crate::tray::unread_count().saturating_add(1);
            app.activate_action("set-unread", Some(&next.to_variant()));
        }

        // Notification sound is WhatsApp's own in-page ding, gated at the audio
        // layer (see `web_view::apply_audio_mute`, driven by `notify-sound-enabled`
        // + the master toggle). Karere plays no separate sound — that was the
        // source of the double-ding.

        self.emit(&settings, tag, title, body, icon);
    }

    /// A notification was dismissed in the page (3.3).
    pub fn on_closed(&self, tag: &str) {
        self.live.lock().remove(tag);
    }

    /// The window regained focus: withdraw every live banner from the desktop
    /// and ask the page to close its matching stubs, then clear the cache so
    /// stale tags can't leak (3.4). `run_js` runs a script in the page's main
    /// frame.
    pub fn on_focus_gained<F: Fn(&str)>(&self, run_js: F) {
        let tags: Vec<String> = {
            let mut live = self.live.lock();
            live.drain().map(|(tag, _)| tag).collect()
        };
        if tags.is_empty() {
            return;
        }
        let app = gio::Application::default();
        for tag in &tags {
            if let Some(app) = app.as_ref() {
                app.withdraw_notification(tag);
            }
            run_js(&format!("window.__karereCloseNotif({})", js_string(tag)));
        }
    }

    /// Reset on main-frame navigation so cached tags never target a dead frame
    /// (3.5).
    pub fn on_load_start(&self) {
        self.live.lock().clear();
    }

    /// Build and publish the Karere-branded `gio::Notification` (3.7). Title and
    /// body honour the `notify-preview-*` keys; the avatar (when resolvable)
    /// becomes the notification image, falling back to the Karere icon. The
    /// default action raises the window and routes the click back to the page.
    fn emit(
        &self,
        settings: &gio::Settings,
        tag: &str,
        title: &str,
        body: &str,
        icon: Option<&str>,
    ) {
        let Some(app) = gio::Application::default() else {
            log::warn!("notifications: no default application; cannot emit banner");
            return;
        };

        let show_name = settings.boolean("notify-preview-name");
        let show_message = settings.boolean("notify-preview-message");

        let display_title = if show_name && !title.is_empty() {
            title.to_owned()
        } else {
            "Karere".to_owned()
        };

        let notif = gio::Notification::new(&display_title);

        if show_message && !body.is_empty() {
            let limit = preview_limit(settings);
            notif.set_body(Some(&truncate(body, limit)));
        } else {
            notif.set_body(Some("New message"));
        }

        // Avatar image: decode the renderer-supplied data URL to bytes, mask to
        // a circle (matching WhatsApp Web), re-encode PNG. On any failure fall
        // back to the raw bytes, then the themed Karere icon.
        match icon.and_then(decode_data_url) {
            Some(bytes) => {
                let final_bytes = round_avatar(&bytes).unwrap_or(bytes);
                let g = glib::Bytes::from_owned(final_bytes);
                notif.set_icon(&gio::BytesIcon::new(&g));
            }
            None => {
                notif.set_icon(&gio::ThemedIcon::new(APP_ID));
            }
        }

        // Clicking the banner body, or the explicit "View message" button,
        // raises the window and re-enters the page via the tag target. (The
        // native Chromium banner's "Settings" button — which opened Chrome site
        // settings — is gone now that we suppress that banner and render our own.)
        notif.set_default_action_and_target_value(ACTIVATE_ACTION, Some(&tag.to_variant()));
        notif.add_button_with_target_value(
            "View message",
            ACTIVATE_ACTION,
            Some(&tag.to_variant()),
        );

        log::info!(
            "notifications: send_notification id={tag:?} app_id={:?}",
            app.application_id().map(|s| s.to_string())
        );
        app.send_notification(Some(tag), &notif);
    }
}

impl Default for Tracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Character budget for the body preview, per the `notify-preview-length` enum.
fn preview_limit(settings: &gio::Settings) -> usize {
    match settings.string("notify-preview-length").as_str() {
        "short" => 40,
        "long" => 280,
        _ => 120, // "medium" and any unexpected value
    }
}

/// Truncate `s` to at most `limit` characters, appending an ellipsis when cut.
fn truncate(s: &str, limit: usize) -> String {
    if s.chars().count() <= limit {
        return s.to_owned();
    }
    let mut out: String = s.chars().take(limit.saturating_sub(1)).collect();
    out.push('…');
    out
}

/// Decode a `data:[<mime>][;base64],<payload>` URL to raw bytes. Returns `None`
/// for non-data URLs or malformed payloads (the caller falls back to the app
/// icon).
fn decode_data_url(url: &str) -> Option<Vec<u8>> {
    use base64::Engine;
    let rest = url.strip_prefix("data:")?;
    let comma = rest.find(',')?;
    let (meta, payload) = rest.split_at(comma);
    let payload = &payload[1..];
    if meta.contains(";base64") {
        base64::engine::general_purpose::STANDARD
            .decode(payload.as_bytes())
            .ok()
    } else {
        // Percent-encoded text data URLs aren't used for avatars; ignore.
        None
    }
}

/// Side length (px) of the rendered circular avatar.
const AVATAR_SIZE: i32 = 96;

/// Mask `image_bytes` (any format gdk-pixbuf decodes — WhatsApp avatars are
/// JPEG/PNG/WebP) into a circular PNG matching WhatsApp Web's round avatars.
/// Returns `None` on any decode/scale/encode failure so the caller falls back to
/// the original bytes.
fn round_avatar(image_bytes: &[u8]) -> Option<Vec<u8>> {
    use gdk_pixbuf::prelude::PixbufLoaderExt;
    use gdk_pixbuf::{InterpType, PixbufLoader};

    let loader = PixbufLoader::new();
    // Fully-qualified to avoid `std::io::Write::write` (in scope via gtk prelude).
    PixbufLoaderExt::write(&loader, image_bytes).ok()?;
    PixbufLoaderExt::close(&loader).ok()?;
    let src = loader.pixbuf()?;

    // Scale to AVATAR_SIZE square. WhatsApp avatars are already square; a
    // non-square source just gets squished slightly, which is fine for a 96px
    // round badge.
    let scaled = src.scale_simple(AVATAR_SIZE, AVATAR_SIZE, InterpType::Bilinear)?;

    // Ensure an alpha channel, then punch a circular mask. `add_alpha` returns a
    // fresh pixbuf with 4 channels (RGBA8).
    let rgba = if scaled.has_alpha() {
        scaled
    } else {
        scaled.add_alpha(false, 0, 0, 0).ok()?
    };

    let size = AVATAR_SIZE;
    let r = size as f64 / 2.0;
    for y in 0..size {
        for x in 0..size {
            // Distance from pixel center to circle center.
            let dx = x as f64 + 0.5 - r;
            let dy = y as f64 + 0.5 - r;
            let dist = (dx * dx + dy * dy).sqrt();
            // 1px feathered edge for anti-aliasing: fully opaque inside r-0.5,
            // fully transparent past r+0.5, linear in between.
            let cover = if dist <= r - 0.5 {
                1.0
            } else if dist >= r + 0.5 {
                0.0
            } else {
                (r + 0.5 - dist).clamp(0.0, 1.0)
            };
            if cover < 1.0 {
                // Read existing RGB, rewrite with scaled alpha.
                let pixels = unsafe { rgba.pixels() };
                let stride = rgba.rowstride() as usize;
                let idx = y as usize * stride + x as usize * 4;
                if idx + 3 < pixels.len() {
                    let a = pixels[idx + 3] as f64 * cover;
                    rgba.put_pixel(
                        x as u32,
                        y as u32,
                        pixels[idx],
                        pixels[idx + 1],
                        pixels[idx + 2],
                        a.round() as u8,
                    );
                }
            }
        }
    }

    rgba.save_to_bufferv("png", &[]).ok()
}

/// Encode `s` as a JS string literal (double-quoted, escaped) for safe
/// interpolation into an `execute_java_script` call.
pub(crate) fn js_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '<' => out.push_str("\\u003c"),
            '>' => out.push_str("\\u003e"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_keeps_short_strings() {
        assert_eq!(truncate("hello", 40), "hello");
    }

    #[test]
    fn truncate_cuts_with_ellipsis() {
        let out = truncate("abcdef", 4);
        assert_eq!(out.chars().count(), 4);
        assert!(out.ends_with('…'));
    }

    #[test]
    fn decode_base64_data_url() {
        // "AAAA" base64 -> three zero bytes.
        let bytes = decode_data_url("data:image/png;base64,AAAA").unwrap();
        assert_eq!(bytes, vec![0, 0, 0]);
    }

    #[test]
    fn decode_rejects_non_data_url() {
        assert!(decode_data_url("https://example.com/a.png").is_none());
        assert!(decode_data_url("data:text/plain,hello").is_none());
    }

    #[test]
    fn js_string_escapes_quotes_and_brackets() {
        assert_eq!(js_string("a\"b"), "\"a\\\"b\"");
        assert!(js_string("<script>").contains("\\u003c"));
    }
}
