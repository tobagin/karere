use glib;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use gtk::prelude::*;
use libadwaita as adw;

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
}

impl Account {
    pub fn new(id: String, name: String) -> Self {
        Self {
            id,
            name,
            profile_picture: None,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64,
            is_active: false,
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

    /// Remove an account
    pub fn remove_account(&self, account_id: &str) -> anyhow::Result<()> {
        let mut accounts = self.get_accounts()?;
        accounts.retain(|a| a.id != account_id);

        // If we removed the active account, make the first one active
        if accounts.iter().all(|a| !a.is_active) && !accounts.is_empty() {
            accounts[0].is_active = true;
        }

        // Also remove any saved session data for this account
        let session_dir = self.session_dir_for(account_id);
        if session_dir.exists() {
            let _ = std::fs::remove_dir_all(&session_dir);
        }

        self.save_accounts(&accounts)?;
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
}

impl Default for AccountManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a ListBoxRow for an account with a delete button.
/// Returns (row, delete_button) so the caller can connect signals.
/// The row's widget_name is set to the account ID for identification.
pub fn build_account_row(account: &Account) -> (gtk::ListBoxRow, gtk::Button) {
    let row = gtk::ListBoxRow::new();
    row.set_activatable(true);
    row.set_widget_name(&account.id);

    let box_container = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    box_container.set_margin_start(8);
    box_container.set_margin_end(8);
    box_container.set_margin_top(8);
    box_container.set_margin_bottom(8);

    let avatar = adw::Avatar::builder()
        .size(32)
        .text(&account.name)
        .build();

    let label = gtk::Label::new(Some(&account.name));
    label.set_halign(gtk::Align::Start);
    label.set_hexpand(true);
    label.set_wrap(true);
    label.set_max_width_chars(20);

    let content_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    content_box.append(&avatar);
    content_box.append(&label);
    content_box.set_hexpand(true);

    let delete_btn = gtk::Button::builder()
        .icon_name("edit-delete-symbolic")
        .css_classes(["destructive-action", "flat"])
        .halign(gtk::Align::End)
        .build();

    box_container.append(&content_box);
    box_container.append(&delete_btn);
    row.set_child(Some(&box_container));

    if account.is_active {
        row.add_css_class("active");
        avatar.add_css_class("active-account");
    }

    (row, delete_btn)
}
