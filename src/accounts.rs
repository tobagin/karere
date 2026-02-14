use glib;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use gtk::prelude::*;
use gtk::gdk;
use libadwaita as adw;
use cairo;
use pango;
use pangocairo;

pub const DEFAULT_ACCOUNT_ID: &str = "default";
pub const DEFAULT_COLOR: &str = "#3584e4";
pub const DEFAULT_EMOJI: &str = "💬";

/// Recursively copy a directory and its contents
fn copy_dir_recursive(src: &PathBuf, dst: &PathBuf) -> anyhow::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

/// Represents a WhatsApp account
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    /// Unique identifier for the account (based on phone number or similar)
    pub id: String,
    /// Display name (phone number or contact name)
    pub name: String,
    /// Timestamp when an account was added
    pub created_at: i64,
    /// Whether this is the currently active account
    #[serde(default)]
    pub is_active: bool,
    /// Whether this account has session data (is logged in)
    #[serde(default)]
    pub has_session: bool,
    /// Avatar background color (hex string, e.g. "#3584e4")
    #[serde(default = "default_color")]
    pub color: String,
    /// Avatar emoji identifier
    #[serde(default = "default_emoji")]
    pub emoji: String,
    /// Display order (lower = first)
    #[serde(default)]
    pub order: i32,
    /// Whether this account has unread notifications
    #[serde(default)]
    pub has_unread: bool,
    /// Per-account zoom level (default 1.0)
    #[serde(default = "default_zoom")]
    pub zoom_level: f64,
    /// Per-account permission state
    #[serde(default)]
    pub notification_permission_asked: bool,
    #[serde(default)]
    pub notification_permission_granted: bool,
    #[serde(default)]
    pub microphone_permission_asked: bool,
    #[serde(default)]
    pub microphone_permission_granted: bool,
    #[serde(default)]
    pub camera_permission_asked: bool,
    #[serde(default)]
    pub camera_permission_granted: bool,
}

fn default_color() -> String { DEFAULT_COLOR.to_string() }
fn default_emoji() -> String { DEFAULT_EMOJI.to_string() }
fn default_zoom() -> f64 { 1.0 }

impl Account {
    pub fn new(id: String, name: String, color: String, emoji: String) -> Self {
        Self {
            id,
            name,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64,
            is_active: false,
            has_session: false,
            color,
            emoji,
            order: 0,
            has_unread: false,
            zoom_level: 1.0,
            notification_permission_asked: false,
            notification_permission_granted: false,
            microphone_permission_asked: false,
            microphone_permission_granted: false,
            camera_permission_asked: false,
            camera_permission_granted: false,
        }
    }
}

/// Manages multiple WhatsApp accounts
#[derive(Debug, Clone)]
pub struct AccountManager {
    config_dir: PathBuf,
    accounts_file: PathBuf,
}

impl AccountManager {
    pub fn new() -> Self {
        let config_dir = glib::user_data_dir().join("karere").join("accounts");
        let accounts_file = config_dir.join("accounts.json");

        // Create directories if they don't exist
        if !config_dir.exists() {
            let _ = std::fs::create_dir_all(&config_dir);
        }

        Self {
            config_dir,
            accounts_file,
        }
    }

    /// Get all stored accounts
    pub fn get_accounts(&self) -> anyhow::Result<Vec<Account>> {
        if !self.accounts_file.exists() {
            return Ok(Vec::new());
        }

        let content = std::fs::read_to_string(&self.accounts_file)?;
        let accounts: Vec<Account> = serde_json::from_str(&content)?;
        Ok(accounts)
    }

    /// Get the currently active account
    pub fn get_active_account(&self) -> anyhow::Result<Option<Account>> {
        let accounts = self.get_accounts()?;
        Ok(accounts.into_iter().find(|a| a.is_active))
    }

    /// Add a new account
    pub fn add_account(&self, mut account: Account) -> anyhow::Result<()> {
        let mut accounts = self.get_accounts()?;

        // Check if account already exists
        if accounts.iter().any(|a| a.id == account.id) {
            return Err(anyhow::anyhow!("Account already exists"));
        }

        // If this is the first account or explicitly set, make it active
        if accounts.is_empty() {
            account.is_active = true;
        }

        // Set order to be after the last account
        let max_order = accounts.iter().map(|a| a.order).max().unwrap_or(-1);
        account.order = max_order + 1;

        accounts.push(account);
        self.save_accounts(&accounts)?;
        Ok(())
    }

    /// Remove an account (the default account can never be removed)
    pub fn remove_account(&self, account_id: &str) -> anyhow::Result<()> {
        // Protect the default account from removal
        if account_id == DEFAULT_ACCOUNT_ID {
            return Err(anyhow::anyhow!("The default account cannot be removed"));
        }

        let mut accounts = self.get_accounts()?;
        accounts.retain(|a| a.id != account_id);

        // If we removed the active account, make the first one active
        if accounts.iter().all(|a| !a.is_active) && !accounts.is_empty() {
            accounts[0].is_active = true;
        }

        self.save_accounts(&accounts)?;

        // Also remove any saved session data for this account
        let session_dir = self.session_dir_for(account_id);
        if session_dir.exists() {
            let _ = std::fs::remove_dir_all(&session_dir);
        }

        Ok(())
    }

    /// Set the active account
    pub fn set_active_account(&self, account_id: &str) -> anyhow::Result<()> {
        let mut accounts = self.get_accounts()?;

        for account in &mut accounts {
            account.is_active = account.id == account_id;
        }

        self.save_accounts(&accounts)?;
        Ok(())
    }

    /// Update account display name
    pub fn update_account_name(&self, account_id: &str, name: &str) -> anyhow::Result<()> {
        let mut accounts = self.get_accounts()?;

        if let Some(account) = accounts.iter_mut().find(|a| a.id == account_id) {
            account.name = name.to_string();
            self.save_accounts(&accounts)?;
        } else {
            return Err(anyhow::anyhow!("Account not found"));
        }

        Ok(())
    }

    /// Update account name, emoji, and color
    pub fn update_account_identity(&self, account_id: &str, name: &str, emoji: &str, color: &str) -> anyhow::Result<()> {
        let mut accounts = self.get_accounts()?;

        if let Some(account) = accounts.iter_mut().find(|a| a.id == account_id) {
            account.name = name.to_string();
            account.emoji = emoji.to_string();
            account.color = color.to_string();
            self.save_accounts(&accounts)?;
        } else {
            return Err(anyhow::anyhow!("Account not found"));
        }

        Ok(())
    }

    /// Get the total number of accounts
    pub fn get_account_count(&self) -> usize {
        self.get_accounts().map(|a| a.len()).unwrap_or(0)
    }

    /// Get permission state for a specific account
    pub fn get_account_permission(&self, account_id: &str, asked_key: &str) -> (bool, bool) {
        if let Ok(accounts) = self.get_accounts() {
            if let Some(account) = accounts.iter().find(|a| a.id == account_id) {
                return match asked_key {
                    "notification" => (account.notification_permission_asked, account.notification_permission_granted),
                    "microphone" => (account.microphone_permission_asked, account.microphone_permission_granted),
                    "camera" => (account.camera_permission_asked, account.camera_permission_granted),
                    _ => (false, false),
                };
            }
        }
        (false, false)
    }

    /// Set permission state for a specific account
    pub fn set_account_permission(&self, account_id: &str, perm_key: &str, asked: bool, granted: bool) -> anyhow::Result<()> {
        let mut accounts = self.get_accounts()?;

        if let Some(account) = accounts.iter_mut().find(|a| a.id == account_id) {
            match perm_key {
                "notification" => {
                    account.notification_permission_asked = asked;
                    account.notification_permission_granted = granted;
                }
                "microphone" => {
                    account.microphone_permission_asked = asked;
                    account.microphone_permission_granted = granted;
                }
                "camera" => {
                    account.camera_permission_asked = asked;
                    account.camera_permission_granted = granted;
                }
                _ => return Err(anyhow::anyhow!("Unknown permission key")),
            }
            self.save_accounts(&accounts)?;
        } else {
            return Err(anyhow::anyhow!("Account not found"));
        }

        Ok(())
    }

    /// Get zoom level for a specific account
    pub fn get_account_zoom(&self, account_id: &str) -> f64 {
        if let Ok(accounts) = self.get_accounts() {
            if let Some(account) = accounts.iter().find(|a| a.id == account_id) {
                return account.zoom_level;
            }
        }
        1.0
    }

    /// Set zoom level for a specific account
    pub fn set_account_zoom(&self, account_id: &str, zoom: f64) -> anyhow::Result<()> {
        let mut accounts = self.get_accounts()?;

        if let Some(account) = accounts.iter_mut().find(|a| a.id == account_id) {
            account.zoom_level = zoom;
            self.save_accounts(&accounts)?;
        } else {
            return Err(anyhow::anyhow!("Account not found"));
        }

        Ok(())
    }

    /// Set unread status for a specific account
    pub fn set_account_unread(&self, account_id: &str, unread: bool) -> anyhow::Result<()> {
        let mut accounts = self.get_accounts()?;

        if let Some(account) = accounts.iter_mut().find(|a| a.id == account_id) {
            account.has_unread = unread;
            self.save_accounts(&accounts)?;
        } else {
            return Err(anyhow::anyhow!("Account not found"));
        }

        Ok(())
    }

    /// Get all accounts sorted by their order field
    pub fn get_accounts_sorted(&self) -> anyhow::Result<Vec<Account>> {
        let mut accounts = self.get_accounts()?;
        accounts.sort_by_key(|a| a.order);
        Ok(accounts)
    }

    /// Reorder an account by swapping it with an adjacent account.
    /// The default/primary account is always pinned first and cannot be reordered.
    /// direction: -1 for up (earlier), +1 for down (later)
    pub fn reorder_account(&self, account_id: &str, direction: i32) -> anyhow::Result<()> {
        // The default account is pinned and cannot be reordered
        if account_id == DEFAULT_ACCOUNT_ID {
            return Ok(());
        }

        let mut accounts = self.get_accounts()?;
        accounts.sort_by_key(|a| a.order);

        let pos = accounts.iter().position(|a| a.id == account_id);
        if let Some(idx) = pos {
            let swap_idx = if direction < 0 {
                if idx == 0 { return Ok(()); }
                // Don't swap with the default account
                if accounts[idx - 1].id == DEFAULT_ACCOUNT_ID { return Ok(()); }
                idx - 1
            } else {
                if idx >= accounts.len() - 1 { return Ok(()); }
                idx + 1
            };

            // Swap order values
            let a_order = accounts[idx].order;
            let b_order = accounts[swap_idx].order;
            accounts[idx].order = b_order;
            accounts[swap_idx].order = a_order;

            self.save_accounts(&accounts)?;
        }

        Ok(())
    }

    /// Get a summary of all accounts for tray integration:
    /// Returns Vec<(id, name, emoji, has_unread)>
    pub fn get_accounts_summary(&self) -> Vec<(String, String, String, bool)> {
        self.get_accounts_sorted()
            .unwrap_or_default()
            .iter()
            .map(|a| (a.id.clone(), a.name.clone(), a.emoji.clone(), a.has_unread))
            .collect()
    }

    /// Save accounts to file
    fn save_accounts(&self, accounts: &[Account]) -> anyhow::Result<()> {
        let content = serde_json::to_string_pretty(accounts)?;
        std::fs::write(&self.accounts_file, content)?;
        Ok(())
    }

    /// Root directory for per-account sessions
    pub fn session_root_dir(&self) -> PathBuf {
        self.config_dir.join("sessions")
    }

    /// Directory for a specific account's session snapshot
    pub fn session_dir_for(&self, account_id: &str) -> PathBuf {
        self.session_root_dir().join(account_id)
    }

    /// WebKit data directory for a specific account
    pub fn data_dir_for(&self, account_id: &str) -> PathBuf {
        self.session_dir_for(account_id).join("data")
    }

    /// WebKit cache directory for a specific account
    pub fn cache_dir_for(&self, account_id: &str) -> PathBuf {
        self.session_dir_for(account_id).join("cache")
    }

    /// Get legacy WebKit directories from before multi-account system
    fn get_legacy_webkit_dirs(&self) -> (PathBuf, PathBuf) {
        let data_dir = glib::user_data_dir().join("karere").join("webkit");
        let cache_dir = glib::user_cache_dir().join("karere").join("webkit");
        (data_dir, cache_dir)
    }

    /// Check if a directory contains WebKit session data
    /// WebKit stores data in subdirectories like storage/, serviceworkers/,
    /// deviceidhashsalts/, mediakeys/, etc.
    pub fn check_session_exists(&self, data_dir: &PathBuf) -> bool {
        if !data_dir.exists() {
            return false;
        }

        // Known WebKit data subdirectories that indicate session data
        let session_indicators = [
            "storage",
            "serviceworkers",
            "deviceidhashsalts",
            "mediakeys",
            "resourcemonitorthrottler",
        ];

        for indicator in &session_indicators {
            let dir = data_dir.join(indicator);
            if dir.exists() && dir.read_dir().map(|mut d| d.next().is_some()).unwrap_or(false) {
                return true;
            }
        }

        // Also check for any non-empty subdirectory as a fallback
        if let Ok(entries) = std::fs::read_dir(data_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() && path.read_dir().map(|mut d| d.next().is_some()).unwrap_or(false) {
                    return true;
                }
            }
        }

        false
    }

    /// Ensure a default account always exists.
    /// If no accounts exist, create the default "Primary Account".
    /// If legacy WebKit data directories are found, migrate them to the default account.
    /// Returns the default account.
    pub fn ensure_default_account(&self) -> anyhow::Result<Account> {
        let existing_accounts = self.get_accounts()?;

        // If accounts already exist, return the active one (or the first)
        if !existing_accounts.is_empty() {
            let active = existing_accounts.iter().find(|a| a.is_active).cloned();
            return Ok(active.unwrap_or_else(|| existing_accounts[0].clone()));
        }

        println!("Karere: No accounts found, creating default account...");

        // Check for legacy directories to migrate
        let (legacy_data, legacy_cache) = self.get_legacy_webkit_dirs();
        let has_legacy_data = legacy_data.exists() && self.check_session_exists(&legacy_data);

        // Create default account
        let mut default_account = Account::new(
            DEFAULT_ACCOUNT_ID.to_string(),
            "Primary Account".to_string(),
            DEFAULT_COLOR.to_string(),
            DEFAULT_EMOJI.to_string(),
        );
        default_account.is_active = true;
        default_account.has_session = has_legacy_data;

        // Get new account directories
        let new_data = self.data_dir_for(&default_account.id);
        let new_cache = self.cache_dir_for(&default_account.id);

        // Migrate legacy data if it exists
        if has_legacy_data {
            println!("Karere: Migrating legacy session to multi-account system...");

            // Ensure parent directories exist
            if let Some(parent) = new_data.parent() {
                std::fs::create_dir_all(parent)?;
            }
            if let Some(parent) = new_cache.parent() {
                std::fs::create_dir_all(parent)?;
            }

            // Move the legacy data directory
            if legacy_data.exists() {
                if let Err(e) = std::fs::rename(&legacy_data, &new_data) {
                    eprintln!("Karere: Failed to move legacy data, copying instead: {}", e);
                    // Fallback: copy if rename fails (e.g., cross-device)
                    copy_dir_recursive(&legacy_data, &new_data)?;
                    let _ = std::fs::remove_dir_all(&legacy_data);
                }
                println!("Karere: Moved session data to {:?}", new_data);
            }

            // Move the legacy cache directory
            if legacy_cache.exists() {
                if let Err(e) = std::fs::rename(&legacy_cache, &new_cache) {
                    eprintln!("Karere: Failed to move legacy cache, copying instead: {}", e);
                    copy_dir_recursive(&legacy_cache, &new_cache)?;
                    let _ = std::fs::remove_dir_all(&legacy_cache);
                }
                println!("Karere: Moved session cache to {:?}", new_cache);
            }

            println!("Karere: Migration complete - created 'Primary Account' with existing session");
        } else {
            println!("Karere: Created new 'Primary Account' (no existing session found)");
        }

        // Save the account
        self.save_accounts(&[default_account.clone()])?;

        Ok(default_account)
    }

    /// Update session state for an account by checking if session files exist
    pub fn update_session_state(&self, account_id: &str) -> anyhow::Result<()> {
        let mut accounts = self.get_accounts()?;
        
        if let Some(account) = accounts.iter_mut().find(|a| a.id == account_id) {
            let data_dir = self.data_dir_for(account_id);
            account.has_session = self.check_session_exists(&data_dir);
            self.save_accounts(&accounts)?;
        }

        Ok(())
    }
}

impl Default for AccountManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Generate a PNG icon with a colored circle and emoji for an account.
/// Uses Cairo + Pango for proper color emoji rendering.
/// Returns raw PNG bytes suitable for `Icon::Bytes` in portal notifications.
pub fn create_account_icon_bytes(color: &str, emoji: &str, size: i32) -> Option<Vec<u8>> {
    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, size, size).ok()?;
    let cr = cairo::Context::new(&surface).ok()?;

    // Parse hex color
    let hex = color.trim_start_matches('#');
    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(53) as f64 / 255.0;
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(132) as f64 / 255.0;
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(228) as f64 / 255.0;

    // Draw colored circle
    cr.arc(
        size as f64 / 2.0,
        size as f64 / 2.0,
        size as f64 / 2.0,
        0.0,
        2.0 * std::f64::consts::PI,
    );
    cr.set_source_rgb(r, g, b);
    let _ = cr.fill();

    // Render emoji using Pango (handles color emoji fonts properly)
    let layout = pangocairo::functions::create_layout(&cr);
    layout.set_text(emoji);
    let mut font_desc = pango::FontDescription::new();
    let font_size = (28.0 * size as f64 / 64.0) as i32 * pango::SCALE;
    font_desc.set_size(font_size);
    layout.set_font_description(Some(&font_desc));

    // Center the emoji on the circle
    let (_, logical) = layout.pixel_extents();
    let x = (size - logical.width()) as f64 / 2.0 - logical.x() as f64;
    let y = (size - logical.height()) as f64 / 2.0 - logical.y() as f64;
    cr.move_to(x, y);
    pangocairo::functions::show_layout(&cr, &layout);

    // Write PNG to in-memory buffer
    drop(cr);
    let mut buf = Vec::new();
    surface.write_to_png(&mut buf).ok()?;
    Some(buf)
}

/// Create a `gdk::Texture` from the colored-circle+emoji PNG.
pub fn create_account_texture(color: &str, emoji: &str, size: i32) -> Option<gdk::Texture> {
    let bytes = create_account_icon_bytes(color, emoji, size)?;
    let glib_bytes = glib::Bytes::from(&bytes);
    gdk::Texture::from_bytes(&glib_bytes).ok()
}

/// Apply a rendered PNG texture to an `adw::Avatar`, replacing any CSS color hack.
pub fn apply_avatar_texture(avatar: &adw::Avatar, color: &str, emoji: &str) {
    let pixel_size = avatar.size() * 2; // 2x for HiDPI
    if let Some(texture) = create_account_texture(color, emoji, pixel_size) {
        avatar.set_custom_image(Some(&texture));
    }
}

/// Create a ListBoxRow for an account with edit, delete, and reorder buttons.
/// Returns (row, edit_button, delete_button, up_button, down_button) so the caller can connect signals.
/// The row's widget_name is set to the account ID for identification.
pub fn build_account_row(account: &Account, is_first: bool, is_last: bool) -> (gtk::ListBoxRow, gtk::Button, gtk::Button, gtk::Button, gtk::Button) {
    let row = gtk::ListBoxRow::new();
    row.set_activatable(true);
    row.set_widget_name(&account.id);

    let box_container = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    box_container.set_margin_start(8);
    box_container.set_margin_end(8);
    box_container.set_margin_top(8);
    box_container.set_margin_bottom(8);

    // Colored emoji avatar (rendered as PNG texture)
    let avatar = adw::Avatar::builder().size(32).build();
    apply_avatar_texture(&avatar, &account.color, &account.emoji);

    // Unread indicator + name
    let label_text = if account.has_unread {
        format!("● {}", account.name)
    } else {
        account.name.clone()
    };
    let label = gtk::Label::new(Some(&label_text));
    label.set_halign(gtk::Align::Start);
    label.set_hexpand(true);
    label.set_wrap(true);
    label.set_max_width_chars(20);
    if account.has_unread {
        label.add_css_class("accent");
    }

    let content_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    content_box.append(&avatar);
    content_box.append(&label);
    content_box.set_hexpand(true);

    // Reorder buttons: hidden for the default/primary account (always pinned first)
    // and hidden when there's nothing to reorder (fewer than 2 non-default accounts)
    let show_reorder = account.id != DEFAULT_ACCOUNT_ID;
    let up_btn = gtk::Button::builder()
        .icon_name("go-up-symbolic")
        .css_classes(["flat"])
        .halign(gtk::Align::End)
        .valign(gtk::Align::Center)
        .tooltip_text("Move up")
        .visible(show_reorder && !is_first)
        .build();

    let down_btn = gtk::Button::builder()
        .icon_name("go-down-symbolic")
        .css_classes(["flat"])
        .halign(gtk::Align::End)
        .valign(gtk::Align::Center)
        .tooltip_text("Move down")
        .visible(show_reorder && !is_last)
        .build();

    let edit_btn = gtk::Button::builder()
        .icon_name("document-edit-symbolic")
        .css_classes(["flat"])
        .halign(gtk::Align::End)
        .valign(gtk::Align::Center)
        .build();

    let delete_btn = gtk::Button::builder()
        .icon_name("user-trash-symbolic")
        .css_classes(["destructive-action", "flat"])
        .halign(gtk::Align::End)
        .build();

    box_container.append(&content_box);
    box_container.append(&up_btn);
    box_container.append(&down_btn);
    box_container.append(&edit_btn);
    box_container.append(&delete_btn);
    row.set_child(Some(&box_container));

    // Hide delete button for the default (primary) account — it can never be removed
    if account.id == DEFAULT_ACCOUNT_ID {
        delete_btn.set_visible(false);
    }

    if account.is_active {
        row.add_css_class("active");
        avatar.add_css_class("active-account");
    }

    (row, edit_btn, delete_btn, up_btn, down_btn)
}
