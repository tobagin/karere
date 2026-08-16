//! Browser-side notification tracker: re-emits a Karere-branded
//! `gio::Notification` so the desktop attributes the banner to Karere, not
//! Chromium. All methods run on the glib main thread.

use std::collections::HashMap;
use std::time::Instant;

use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use parking_lot::Mutex;
use std::sync::OnceLock;

use crate::application::APP_ID;

/// App action fired when a branded banner is clicked; target is the notif tag.
pub const ACTIVATE_ACTION: &str = "app.notification-clicked";

/// Process-global tracker.
pub fn tracker() -> &'static Tracker {
    static TRACKER: OnceLock<Tracker> = OnceLock::new();
    TRACKER.get_or_init(Tracker::new)
}

pub struct Tracker {
    live: Mutex<HashMap<String, Instant>>,
}

impl Tracker {
    pub fn new() -> Self {
        Self {
            live: Mutex::new(HashMap::new()),
        }
    }

    /// Record the tag, set the per-account unread dot, bump the tray count, emit
    /// the branded banner.
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
        if !settings.boolean("notifications-enabled") || !settings.boolean("notify-messages") {
            log::info!("notifications: suppressed by kill-switch (enabled/messages off)");
            return;
        }

        self.live.lock().insert(tag.to_owned(), Instant::now());

        // Skip the active account: don't badge the row the user is looking at.
        let is_active = crate::accounts::manager()
            .active()
            .is_some_and(|a| a.id == account_id);
        if !is_active {
            crate::accounts::set_unread(account_id, true);
        }

        // Per-account mute: keep the passive unread badge (set above) but
        // suppress the banner + tray-count bump for this account.
        if crate::accounts::manager().is_muted(account_id) {
            log::info!("notifications: suppressed (account {account_id} muted)");
            return;
        }

        if settings.boolean("notify-tray-icon")
            && let Some(app) = gio::Application::default()
        {
            let next = crate::tray::unread_count().saturating_add(1);
            app.activate_action("set-unread", Some(&next.to_variant()));
        }

        // No separate Karere sound: WhatsApp's in-page ding is the only one (a
        // second sound here was the double-ding).
        self.emit(&settings, tag, title, body, icon);
    }

    /// A notification was dismissed in the page.
    pub fn on_closed(&self, tag: &str) {
        self.live.lock().remove(tag);
    }

    /// Window regained focus: withdraw every live banner, close the page stubs,
    /// clear the cache so stale tags can't leak.
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

    /// Reset on main-frame navigation so cached tags never target a dead frame.
    pub fn on_load_start(&self) {
        self.live.lock().clear();
    }

    /// Build and publish the Karere-branded `gio::Notification`.
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

/// Body-preview character budget per the `notify-preview-length` enum.
fn preview_limit(settings: &gio::Settings) -> usize {
    match settings.string("notify-preview-length").as_str() {
        "short" => 40,
        "long" => 280,
        _ => 120, // "medium" and any unexpected value
    }
}

/// Truncate `s` to `limit` chars, appending an ellipsis when cut.
fn truncate(s: &str, limit: usize) -> String {
    if s.chars().count() <= limit {
        return s.to_owned();
    }
    let mut out: String = s.chars().take(limit.saturating_sub(1)).collect();
    out.push('…');
    out
}

/// Max decoded notification-icon size. Icons are attacker-influenceable and fed
/// to gdk-pixbuf; cap them so a page can't push huge blobs through the decoder.
const MAX_ICON_BYTES: usize = 4 * 1024 * 1024;

/// Decode a `data:image/...;base64,<payload>` URL to raw bytes; `None` otherwise.
/// Restricted to `image/*` base64 payloads and capped at [`MAX_ICON_BYTES`].
fn decode_data_url(url: &str) -> Option<Vec<u8>> {
    use base64::Engine;
    let rest = url.strip_prefix("data:")?;
    let comma = rest.find(',')?;
    let (meta, payload) = rest.split_at(comma);
    let payload = &payload[1..];
    // Only base64 image/* — never hand a non-image or unbounded blob to the decoder.
    if !meta.starts_with("image/") || !meta.contains(";base64") {
        return None;
    }
    // base64 inflates ~4:3; reject oversized payloads before allocating/decoding.
    if payload.len() > MAX_ICON_BYTES / 3 * 4 + 4 {
        log::warn!(
            "notif icon: data URL too large ({} b64 bytes), dropping",
            payload.len()
        );
        return None;
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(payload.as_bytes())
        .ok()?;
    if bytes.len() > MAX_ICON_BYTES {
        return None;
    }
    Some(bytes)
}

const AVATAR_SIZE: i32 = 96;

/// Mask `image_bytes` into a circular PNG; `None` on any decode/scale/encode error.
fn round_avatar(image_bytes: &[u8]) -> Option<Vec<u8>> {
    use gdk_pixbuf::prelude::PixbufLoaderExt;
    use gdk_pixbuf::{InterpType, PixbufLoader};

    let loader = PixbufLoader::new();
    // Fully-qualified to avoid `std::io::Write::write` (in scope via gtk prelude).
    PixbufLoaderExt::write(&loader, image_bytes).ok()?;
    PixbufLoaderExt::close(&loader).ok()?;
    let src = loader.pixbuf()?;

    let scaled = src.scale_simple(AVATAR_SIZE, AVATAR_SIZE, InterpType::Bilinear)?;

    let rgba = if scaled.has_alpha() {
        scaled
    } else {
        scaled.add_alpha(false, 0, 0, 0).ok()?
    };

    let size = AVATAR_SIZE;
    let r = size as f64 / 2.0;
    let stride = rgba.rowstride() as usize;
    let pixels = rgba.read_pixel_bytes();
    for y in 0..size {
        for x in 0..size {
            let dx = x as f64 + 0.5 - r;
            let dy = y as f64 + 0.5 - r;
            let dist = (dx * dx + dy * dy).sqrt();
            // 1px feathered edge between r-0.5 and r+0.5.
            let cover = if dist <= r - 0.5 {
                1.0
            } else if dist >= r + 0.5 {
                0.0
            } else {
                (r + 0.5 - dist).clamp(0.0, 1.0)
            };
            if cover < 1.0 {
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

/// Encode `s` as an escaped, double-quoted JS string literal.
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
        let bytes = decode_data_url("data:image/png;base64,AAAA").unwrap();
        assert_eq!(bytes, vec![0, 0, 0]);
    }

    #[test]
    fn decode_rejects_non_data_url() {
        assert!(decode_data_url("https://example.com/a.png").is_none());
        assert!(decode_data_url("data:text/plain,hello").is_none());
    }

    #[test]
    fn decode_rejects_non_image_base64() {
        // valid base64 but not an image MIME -> never reaches the decoder
        assert!(decode_data_url("data:application/octet-stream;base64,AAAA").is_none());
        assert!(decode_data_url("data:text/html;base64,AAAA").is_none());
    }

    #[test]
    fn decode_rejects_oversized_icon() {
        let huge = "A".repeat(MAX_ICON_BYTES / 3 * 4 + 8);
        let url = format!("data:image/png;base64,{huge}");
        assert!(decode_data_url(&url).is_none());
    }

    #[test]
    fn js_string_escapes_quotes_and_brackets() {
        assert_eq!(js_string("a\"b"), "\"a\\\"b\"");
        assert!(js_string("<script>").contains("\\u003c"));
    }
}
