use glib;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use gtk::prelude::*;
use gtk::gdk;
use libadwaita as adw;

/// Predefined color palette for account avatars (GNOME-style)
pub const ACCOUNT_COLORS: &[&str] = &[
    "#3584e4", // Blue
    "#33d17a", // Green
    "#ff7800", // Orange
    "#e01b24", // Red
    "#9141ac", // Purple
    "#f6d32d", // Yellow
    "#26a269", // Teal
    "#c061cb", // Pink
    "#1c71d8", // Dark Blue
    "#a51d2d", // Dark Red
];

/// Predefined emoji palette for account avatars
pub const ACCOUNT_EMOJIS: &[&str] = &[
    "💬", "🏠", "💼", "🎓", "👤", "👥", "🌟", "🔔",
    "📱", "💻", "🎯", "🚀", "🎨", "🎵", "📷", "✈️",
    "🌍", "❤️", "🔥", "⚡", "🌈", "🎮", "📚", "🏆",
    "🍀", "🌸", "🦋", "🐱", "🐶", "🦊",
];

pub const DEFAULT_COLOR: &str = "#3584e4";
pub const DEFAULT_EMOJI: &str = "💬";

/// Convert a hex color string (e.g. "#3584e4") to a gdk::RGBA
pub fn hex_to_rgba(hex: &str) -> gdk::RGBA {
    let hex = hex.trim_start_matches('#');
    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(53) as f32 / 255.0;
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(132) as f32 / 255.0;
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(228) as f32 / 255.0;
    gdk::RGBA::new(r, g, b, 1.0)
}

/// Convert a gdk::RGBA to a hex color string (e.g. "#3584e4")
pub fn rgba_to_hex(rgba: &gdk::RGBA) -> String {
    let r = (rgba.red() * 255.0).round() as u8;
    let g = (rgba.green() * 255.0).round() as u8;
    let b = (rgba.blue() * 255.0).round() as u8;
    format!("#{:02x}{:02x}{:02x}", r, g, b)
}

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
    /// Path to profile picture (base64 encoded or file path)
    pub profile_picture: Option<String>,
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
    /// Whether this account has unread notifications
    #[serde(default)]
    pub has_unread: bool,
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

impl Account {
    pub fn new(id: String, name: String, color: String, emoji: String) -> Self {
        Self {
            id,
            name,
            profile_picture: None,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64,
            is_active: false,
            has_session: false,
            color,
            emoji,
            has_unread: false,
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

        accounts.push(account);
        self.save_accounts(&accounts)?;
        Ok(())
    }

    /// Remove an account (the default account can never be removed)
    pub fn remove_account(&self, account_id: &str) -> anyhow::Result<()> {
        // Protect the default account from removal
        if account_id == "default" {
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

    /// Update account profile picture
    pub fn update_profile_picture(&self, account_id: &str, picture_data: String) -> anyhow::Result<()> {
        let mut accounts = self.get_accounts()?;

        if let Some(account) = accounts.iter_mut().find(|a| a.id == account_id) {
            account.profile_picture = Some(picture_data);
            self.save_accounts(&accounts)?;
        } else {
            return Err(anyhow::anyhow!("Account not found"));
        }

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

    /// Update account name and emoji
    pub fn update_account_identity(&self, account_id: &str, name: &str, emoji: &str) -> anyhow::Result<()> {
        let mut accounts = self.get_accounts()?;

        if let Some(account) = accounts.iter_mut().find(|a| a.id == account_id) {
            account.name = name.to_string();
            account.emoji = emoji.to_string();
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

    /// Set unread status for a specific account
    pub fn set_account_unread(&self, account_id: &str, unread: bool) -> anyhow::Result<()> {
        let mut accounts = self.get_accounts()?;

        if let Some(account) = accounts.iter_mut().find(|a| a.id == account_id) {
            account.has_unread = unread;
            self.save_accounts(&accounts)?;
        }

        Ok(())
    }

    /// Get a summary of all accounts for tray integration:
    /// Returns Vec<(id, name, emoji, has_unread)>
    pub fn get_accounts_summary(&self) -> Vec<(String, String, String, bool)> {
        self.get_accounts()
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

    /// Get the path where profile pictures are stored
    pub fn get_profile_picture_dir(&self) -> PathBuf {
        self.config_dir.join("profiles")
    }

    /// Save a profile picture file and return the path
    pub fn save_profile_picture(&self, account_id: &str, image_data: &[u8]) -> anyhow::Result<PathBuf> {
        let pic_dir = self.get_profile_picture_dir();
        if !pic_dir.exists() {
            std::fs::create_dir_all(&pic_dir)?;
        }

        let filename = format!("{}.jpg", account_id);
        let path = pic_dir.join(&filename);
        std::fs::write(&path, image_data)?;
        Ok(path)
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
            "default".to_string(),
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

/// Apply account color to an `Adw.Avatar` via a unique CSS class
pub fn apply_avatar_color(avatar: &adw::Avatar, color: &str) {
    // Generate a unique class name from the color hex (strip #)
    let class_name = format!("account-color-{}", color.trim_start_matches('#'));
    let css = format!(
        "avatar.{} {{ background-color: {}; color: white; }}",
        class_name, color
    );
    let provider = gtk::CssProvider::new();
    provider.load_from_string(&css);
    if let Some(display) = gdk::Display::default() {
        gtk::style_context_add_provider_for_display(&display, &provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 10);
    }
    avatar.add_css_class(&class_name);
}

/// Create a ListBoxRow for an account with edit and delete buttons.
/// Returns (row, edit_button, delete_button) so the caller can connect signals.
/// The row's widget_name is set to the account ID for identification.
pub fn build_account_row(account: &Account) -> (gtk::ListBoxRow, gtk::Button, gtk::Button) {
    let row = gtk::ListBoxRow::new();
    row.set_activatable(true);
    row.set_widget_name(&account.id);

    let box_container = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    box_container.set_margin_start(8);
    box_container.set_margin_end(8);
    box_container.set_margin_top(8);
    box_container.set_margin_bottom(8);

    // Colored emoji avatar
    let avatar = adw::Avatar::builder()
        .size(32)
        .text(&account.emoji)
        .show_initials(true)
        .build();
    apply_avatar_color(&avatar, &account.color);

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
    box_container.append(&edit_btn);
    box_container.append(&delete_btn);
    row.set_child(Some(&box_container));

    // Hide delete button for the default (primary) account — it can never be removed
    if account.id == "default" {
        delete_btn.set_visible(false);
    }

    if account.is_active {
        row.add_css_class("active");
        avatar.add_css_class("active-account");
    }

    (row, edit_btn, delete_btn)
}
