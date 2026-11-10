//! Browser-side notification tracker: re-emits a Karere-branded
//! `gio::Notification` so the desktop attributes the banner to Karere, not
//! Chromium. All methods run on the glib main thread.
//!
//! Every live banner is keyed by `(account_id, tag)`: WhatsApp reuses one tag
//! per chat, so the same chat in two accounts must not share a banner, and one
//! account's navigation or focus must never touch another account's banners.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use gettextrs::gettext;
use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use parking_lot::Mutex;
use std::sync::OnceLock;

use crate::application::APP_ID;

/// App action fired when a branded banner is clicked; target is
/// [`ACTIVATE_TARGET_TYPE`] `(account_id, tag)`.
pub const ACTIVATE_ACTION: &str = "app.notification-clicked";

/// GVariant type of the [`ACTIVATE_ACTION`] target: `(account_id, tag)`.
pub const ACTIVATE_TARGET_TYPE: &str = "(ss)";

/// App action behind a banner's "Mute Account" button; same
/// [`ACTIVATE_TARGET_TYPE`] target as [`ACTIVATE_ACTION`].
pub const MUTE_ACCOUNT_ACTION: &str = "app.notification-mute-account";

/// Upper bound on tracked live banners across all accounts; the oldest entry is
/// evicted beyond it so a misbehaving page cannot grow the map without limit.
const MAX_LIVE: usize = 256;

/// Per-account rate cap: at most `RATE_MAX` banners per `RATE_WINDOW`.
const RATE_WINDOW: Duration = Duration::from_secs(10);
const RATE_MAX: u32 = 20;

/// Process-global tracker.
pub fn tracker() -> &'static Tracker {
    static TRACKER: OnceLock<Tracker> = OnceLock::new();
    TRACKER.get_or_init(Tracker::new)
}

/// The `gio::Notification` id for `tag` in `account_id`.
pub fn notification_id(account_id: &str, tag: &str) -> String {
    format!("{account_id}:{tag}")
}

pub struct Tracker {
    live: Mutex<HashMap<(String, String), Instant>>,
    /// Per-account fixed rate window: `(window start, count in window)`.
    rate: Mutex<HashMap<String, (Instant, u32)>>,
}

impl Tracker {
    pub fn new() -> Self {
        Self {
            live: Mutex::new(HashMap::new()),
            rate: Mutex::new(HashMap::new()),
        }
    }

    /// Record the tag and emit the branded banner (unless notifications are off,
    /// i.e. Do Not Disturb, or the account is muted).
    pub fn on_seen(
        &self,
        tag: &str,
        title: &str,
        body: &str,
        icon: Option<&str>,
        account_id: &str,
    ) {
        log::debug!(
            "notifications: on_seen account={account_id} body_len={} has_icon={}",
            body.len(),
            icon.is_some()
        );
        let settings = gio::Settings::new(APP_ID);
        if !settings.boolean("notifications-enabled") || !settings.boolean("notify-messages") {
            log::debug!("notifications: suppressed by kill-switch (enabled/messages off)");
            return;
        }

        if !self.admit(account_id, Instant::now()) {
            log::warn!("notifications: rate cap reached for account {account_id}; dropping");
            return;
        }
        self.record(account_id, tag, Instant::now());

        // Unread counts come from the page title (`handlers::display`), not
        // from banners. Per-account mute only suppresses the banner.
        if crate::accounts::manager().is_muted(account_id) {
            log::debug!("notifications: suppressed (account {account_id} muted)");
            return;
        }

        // No separate Karere sound: WhatsApp's in-page ding is the only one (a
        // second sound here was the double-ding).
        self.emit(&settings, account_id, tag, title, body, icon);
    }

    /// The page in `account_id` closed the notification for `tag` (e.g. the
    /// chat was read on the phone): withdraw its banner.
    pub fn on_closed_for(&self, account_id: &str, tag: &str) {
        self.forget(account_id, tag);
        withdraw(account_id, tag);
    }

    /// Renderer-IPC fallback path, which carries no trusted account: withdraw
    /// `tag` in every account that has it live.
    pub fn on_closed(&self, tag: &str) {
        let accounts: Vec<String> = {
            let mut live = self.live.lock();
            let mut out = Vec::new();
            live.retain(|(a, t), _| {
                if t == tag {
                    out.push(a.clone());
                    false
                } else {
                    true
                }
            });
            out
        };
        for account_id in &accounts {
            withdraw(account_id, tag);
        }
    }

    /// The banner for `(account_id, tag)` was clicked: withdraw it and stop
    /// tracking it, so the account switch that follows does not close the stub
    /// the click is about to activate.
    pub fn on_activated(&self, account_id: &str, tag: &str) {
        self.forget(account_id, tag);
        withdraw(account_id, tag);
    }

    /// `account_id` is now in front of the user (window focus, account switch):
    /// withdraw its live banners and close the page stubs via `run_js` (which
    /// must run in that account's browser). Other accounts are untouched.
    pub fn on_account_viewed<F: Fn(&str)>(&self, account_id: &str, run_js: F) {
        for tag in self.take_account(account_id) {
            withdraw(account_id, &tag);
            run_js(&format!("window.__karereCloseNotif({})", js_string(&tag)));
        }
    }

    /// Reset on `account_id`'s main-frame navigation so cached tags never
    /// target a dead frame. Other accounts keep their tags.
    pub fn on_load_start(&self, account_id: &str) {
        self.live.lock().retain(|(a, _), _| a != account_id);
    }

    /// Count one banner against `account_id`'s rate window; `false` when the
    /// cap is already reached.
    fn admit(&self, account_id: &str, now: Instant) -> bool {
        let mut rate = self.rate.lock();
        let entry = rate.entry(account_id.to_owned()).or_insert((now, 0));
        if now.saturating_duration_since(entry.0) >= RATE_WINDOW {
            *entry = (now, 0);
        }
        if entry.1 >= RATE_MAX {
            return false;
        }
        entry.1 += 1;
        true
    }

    /// Track `(account_id, tag)` as live, evicting the oldest entry when full.
    fn record(&self, account_id: &str, tag: &str, now: Instant) {
        let mut live = self.live.lock();
        let key = (account_id.to_owned(), tag.to_owned());
        if !live.contains_key(&key)
            && live.len() >= MAX_LIVE
            && let Some(oldest) = live.iter().min_by_key(|(_, t)| **t).map(|(k, _)| k.clone())
        {
            live.remove(&oldest);
        }
        live.insert(key, now);
    }

    /// Stop tracking `(account_id, tag)`; `true` if it was live.
    fn forget(&self, account_id: &str, tag: &str) -> bool {
        self.live
            .lock()
            .remove(&(account_id.to_owned(), tag.to_owned()))
            .is_some()
    }

    /// Remove and return every live tag of `account_id`.
    fn take_account(&self, account_id: &str) -> Vec<String> {
        let mut live = self.live.lock();
        let mut tags = Vec::new();
        live.retain(|(a, t), _| {
            if a == account_id {
                tags.push(t.clone());
                false
            } else {
                true
            }
        });
        tags
    }

    /// Build and publish the Karere-branded `gio::Notification`.
    fn emit(
        &self,
        settings: &gio::Settings,
        account_id: &str,
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

        let sender = (show_name && !title.is_empty()).then_some(title);
        let label = account_label(account_id);
        let display_title = banner_title(sender, label.as_deref());

        let notif = gio::Notification::new(&display_title);

        if show_message && !body.is_empty() {
            let limit = preview_limit(settings);
            notif.set_body(Some(&truncate(body, limit)));
        } else {
            notif.set_body(Some(&gettext("New message")));
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

        let target = activate_target(account_id, tag);
        notif.set_default_action_and_target_value(ACTIVATE_ACTION, Some(&target));
        notif.add_button_with_target_value(
            &gettext("View message"),
            ACTIVATE_ACTION,
            Some(&target),
        );
        // Muting one account only makes sense when there are others.
        if crate::accounts::manager().get_accounts_sorted().len() > 1 {
            notif.add_button_with_target_value(
                &gettext("Mute Account"),
                MUTE_ACCOUNT_ACTION,
                Some(&target),
            );
        }

        log::debug!("notifications: send_notification account={account_id}");
        app.send_notification(Some(&notification_id(account_id, tag)), &notif);
    }
}

/// The [`ACTIVATE_ACTION`] target for `(account_id, tag)`.
pub fn activate_target(account_id: &str, tag: &str) -> glib::Variant {
    (account_id.to_owned(), tag.to_owned()).to_variant()
}

/// Decode an [`ACTIVATE_ACTION`] target back into `(account_id, tag)`.
pub fn parse_activate_target(v: &glib::Variant) -> Option<(String, String)> {
    v.get::<(String, String)>()
}

fn withdraw(account_id: &str, tag: &str) {
    if let Some(app) = gio::Application::default() {
        app.withdraw_notification(&notification_id(account_id, tag));
    }
}

/// The account's label for the banner title, only when more than one account
/// exists (with a single account it would be noise).
fn account_label(account_id: &str) -> Option<String> {
    let mgr = crate::accounts::manager();
    if mgr.get_accounts_sorted().len() < 2 {
        return None;
    }
    let account = mgr.get(account_id)?;
    let nonempty = |s: &Option<String>| s.clone().filter(|s| !s.trim().is_empty());
    crate::accounts::display_label(&account).or_else(|| nonempty(&account.pushname))
}

/// Banner title from the (privacy-filtered) sender and the optional account
/// label. The label names the user's own account, never message content, so it
/// is shown even when the sender name is hidden.
fn banner_title(sender: Option<&str>, account: Option<&str>) -> String {
    match (sender, account) {
        (Some(s), Some(a)) => format!("{s} ({a})"),
        (Some(s), None) => s.to_owned(),
        (None, Some(a)) => a.to_owned(),
        (None, None) => gettext("Karere"),
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
/// Restricted to `image/*` base64 payloads that decode to PNG/JPEG/WebP, capped
/// at [`MAX_ICON_BYTES`].
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
    if bytes.len() > MAX_ICON_BYTES || !is_supported_image(&bytes) {
        return None;
    }
    Some(bytes)
}

/// PNG, JPEG or WebP by magic bytes; the MIME label in the data URL is
/// page-controlled, so only these formats ever reach the pixbuf loader.
fn is_supported_image(b: &[u8]) -> bool {
    b.starts_with(b"\x89PNG\r\n\x1a\n")
        || b.starts_with(&[0xFF, 0xD8, 0xFF])
        || (b.len() >= 12 && &b[0..4] == b"RIFF" && &b[8..12] == b"WEBP")
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
        // The 8-byte PNG signature.
        let bytes = decode_data_url("data:image/png;base64,iVBORw0KGgo=").unwrap();
        assert_eq!(bytes, b"\x89PNG\r\n\x1a\n".to_vec());
    }

    #[test]
    fn decode_rejects_unsupported_image_bytes() {
        // Claims image/png but decodes to zeros (not PNG/JPEG/WebP).
        assert!(decode_data_url("data:image/png;base64,AAAA").is_none());
        // GIF magic under an image/* label.
        assert!(decode_data_url("data:image/gif;base64,R0lGODlh").is_none());
    }

    #[test]
    fn supported_image_magic() {
        assert!(is_supported_image(&[0xFF, 0xD8, 0xFF, 0xE0]));
        assert!(is_supported_image(b"RIFF\0\0\0\0WEBPVP8 "));
        assert!(!is_supported_image(b"RIFF\0\0\0\0AVI "));
        assert!(!is_supported_image(b"<svg"));
    }

    #[test]
    fn notification_id_is_namespaced_by_account() {
        assert_eq!(notification_id("acct-a", "chat1"), "acct-a:chat1");
        assert_ne!(
            notification_id("acct-a", "chat1"),
            notification_id("acct-b", "chat1")
        );
    }

    #[test]
    fn activate_target_round_trips() {
        let v = activate_target("acct-a", "chat:1");
        assert_eq!(v.type_().as_str(), ACTIVATE_TARGET_TYPE);
        assert_eq!(
            parse_activate_target(&v),
            Some(("acct-a".to_owned(), "chat:1".to_owned()))
        );
        assert_eq!(parse_activate_target(&"chat1".to_variant()), None);
    }

    #[test]
    fn same_tag_in_two_accounts_is_tracked_separately() {
        let t = Tracker::new();
        let now = Instant::now();
        t.record("a", "chat", now);
        t.record("b", "chat", now);
        assert!(t.forget("a", "chat"));
        assert!(!t.forget("a", "chat"));
        assert!(t.forget("b", "chat"));
    }

    #[test]
    fn load_start_clears_only_that_account() {
        let t = Tracker::new();
        let now = Instant::now();
        t.record("a", "x", now);
        t.record("a", "y", now);
        t.record("b", "x", now);
        t.on_load_start("a");
        assert!(!t.forget("a", "x"));
        assert!(!t.forget("a", "y"));
        assert!(t.forget("b", "x"));
    }

    #[test]
    fn take_account_returns_only_that_account() {
        let t = Tracker::new();
        let now = Instant::now();
        t.record("a", "x", now);
        t.record("a", "y", now);
        t.record("b", "z", now);
        let mut tags = t.take_account("a");
        tags.sort();
        assert_eq!(tags, vec!["x".to_owned(), "y".to_owned()]);
        assert!(t.take_account("a").is_empty());
        assert_eq!(t.take_account("b"), vec!["z".to_owned()]);
    }

    #[test]
    fn live_map_is_bounded() {
        let t = Tracker::new();
        let start = Instant::now();
        for i in 0..(MAX_LIVE + 10) {
            t.record("a", &format!("t{i}"), start + Duration::from_millis(i as u64));
        }
        assert_eq!(t.live.lock().len(), MAX_LIVE);
        // The oldest entries were evicted first.
        assert!(!t.forget("a", "t0"));
        assert!(t.forget("a", &format!("t{}", MAX_LIVE + 9)));
    }

    #[test]
    fn rate_cap_is_per_account_and_resets() {
        let t = Tracker::new();
        let now = Instant::now();
        for _ in 0..RATE_MAX {
            assert!(t.admit("a", now));
        }
        assert!(!t.admit("a", now));
        assert!(t.admit("b", now));
        assert!(t.admit("a", now + RATE_WINDOW));
    }

    #[test]
    fn banner_title_respects_privacy_and_accounts() {
        assert_eq!(banner_title(Some("Ann"), None), "Ann");
        assert_eq!(banner_title(Some("Ann"), Some("Work")), "Ann (Work)");
        assert_eq!(banner_title(None, Some("Work")), "Work");
        assert_eq!(banner_title(None, None), "Karere");
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
