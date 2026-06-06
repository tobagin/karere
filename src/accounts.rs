//! Persistent multi-account store at `$XDG_DATA_HOME/karere/accounts/accounts.json`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use gtk::glib;
use gtk::glib::subclass::Signal;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

use crate::permissions_store::State;

/// Per-account permission overrides; empty defers to global.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct AccountPermissions {
    #[serde(default)]
    pub overrides: HashMap<String, HashMap<u32, State>>,
}

/// One WhatsApp Web account.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Account {
    pub id: String,
    #[serde(default)]
    pub wid: Option<String>,
    #[serde(default)]
    pub pushname: Option<String>,
    #[serde(default)]
    pub user_label: Option<String>,
    #[serde(default, with = "avatar_b64")]
    pub avatar_png: Option<Vec<u8>>,
    #[serde(default)]
    pub avatar_url: Option<String>,
    pub created_at: i64,
    pub last_used_at: i64,
    #[serde(default)]
    pub is_active: bool,
    #[serde(default)]
    pub has_session: bool,
    #[serde(default = "default_zoom")]
    pub zoom_level: f64,
    #[serde(default)]
    pub permissions: AccountPermissions,
}

fn default_zoom() -> f64 {
    1.0
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

impl Account {
    /// A fresh account with a new UUID.
    pub fn new() -> Self {
        let ts = now();
        Account {
            id: uuid::Uuid::new_v4().to_string(),
            wid: None,
            pushname: None,
            user_label: None,
            avatar_png: None,
            avatar_url: None,
            created_at: ts,
            last_used_at: ts,
            is_active: false,
            has_session: false,
            zoom_level: default_zoom(),
            permissions: AccountPermissions::default(),
        }
    }
}

impl Default for Account {
    fn default() -> Self {
        Account::new()
    }
}

/// serde adapter: `Option<Vec<u8>>` ⇆ base64 string on disk.
mod avatar_b64 {
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD as B64;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(
        bytes: &Option<Vec<u8>>,
        ser: S,
    ) -> Result<S::Ok, S::Error> {
        match bytes {
            Some(b) => ser.serialize_some(&B64.encode(b)),
            None => ser.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        de: D,
    ) -> Result<Option<Vec<u8>>, D::Error> {
        let opt = Option::<String>::deserialize(de)?;
        match opt {
            Some(s) => B64
                .decode(s.as_bytes())
                .map(Some)
                .map_err(serde::de::Error::custom),
            None => Ok(None),
        }
    }
}

#[derive(Debug)]
pub enum AccountsError {
    Parse(String),
    Io(String),
}

impl std::fmt::Display for AccountsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AccountsError::Parse(m) => write!(f, "accounts.json parse error: {m}"),
            AccountsError::Io(m) => write!(f, "accounts store I/O error: {m}"),
        }
    }
}

impl std::error::Error for AccountsError {}

pub fn accounts_root() -> PathBuf {
    glib::user_data_dir().join("karere").join("accounts")
}

/// One-time, marker-guarded purge of pre-v4 data; call before CEF init.
pub fn purge_legacy_v3_data() {
    let data = glib::user_data_dir();
    let marker = data.join("karere").join(".v3-data-purged");
    if marker.exists() {
        return;
    }
    let cache = glib::user_cache_dir();
    let config = glib::user_config_dir();

    let legacy = [
        data.join("webkitgtk-6.0"),
        cache.join("webkitgtk-6.0"),
        cache.join("cef_user_data"),
        config.join("cef_user_data"),
        cache.join("karere"),
    ];
    for dir in &legacy {
        match std::fs::remove_dir_all(dir) {
            Ok(()) => log::info!("purged legacy v3 data: {}", dir.display()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => log::warn!("failed to purge legacy dir {}: {e}", dir.display()),
        }
    }

    if let Some(parent) = marker.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Err(e) = std::fs::write(&marker, b"v3 data purged on first v4 launch\n") {
        log::warn!("failed to write purge marker {}: {e}", marker.display());
    }
}

pub fn accounts_file() -> PathBuf {
    accounts_root().join("accounts.json")
}

/// Best-effort delete of an account's CEF session dir (device stays linked on the phone).
pub fn delete_session_dir(id: &str) {
    let dir = session_cache_path(id);
    if let Err(e) = std::fs::remove_dir_all(&dir)
        && e.kind() != std::io::ErrorKind::NotFound
    {
        log::warn!("failed to delete session dir {}: {e}", dir.display());
    }
}

/// Per-account CEF session cache dir. Must be a DIRECT child of `root_cache_path`: CEF treats each immediate subdir as a profile and rejects deeper nesting, silently falling back to the shared global profile.
pub fn session_cache_path(id: &str) -> PathBuf {
    accounts_root().join("sessions").join(id)
}

/// Load accounts from `path`. Missing file → empty list; malformed → `Err` (never silently overwritten).
pub fn load_from(path: &std::path::Path) -> Result<Vec<Account>, AccountsError> {
    match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice::<Vec<Account>>(&bytes)
            .map_err(|e| AccountsError::Parse(e.to_string())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(AccountsError::Io(e.to_string())),
    }
}

/// Persist `accounts` to `path` via temp-then-rename (atomic).
pub fn save_to(path: &std::path::Path, accounts: &[Account]) -> Result<(), AccountsError> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| AccountsError::Io(e.to_string()))?;
    }
    let json = serde_json::to_vec_pretty(accounts)
        .map_err(|e| AccountsError::Io(e.to_string()))?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, &json).map_err(|e| AccountsError::Io(e.to_string()))?;
    std::fs::rename(&tmp, path).map_err(|e| AccountsError::Io(e.to_string()))?;
    Ok(())
}

/// Sort in place by `last_used_at` descending.
pub fn sort_mru(accounts: &mut [Account]) {
    accounts.sort_by(|a, b| b.last_used_at.cmp(&a.last_used_at));
}

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct AccountManager {
        pub accounts: RefCell<Vec<Account>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AccountManager {
        const NAME: &'static str = "KarereAccountManager";
        type Type = super::AccountManager;
    }

    impl ObjectImpl for AccountManager {
        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> =
                Lazy::new(|| vec![Signal::builder("accounts-changed").build()]);
            SIGNALS.as_ref()
        }
    }
}

glib::wrapper! {
    /// Owns and persists the account list; emits `accounts-changed` after every mutation.
    pub struct AccountManager(ObjectSubclass<imp::AccountManager>);
}

impl AccountManager {
    /// Build a manager and load `accounts.json`.
    pub fn load() -> Result<Self, AccountsError> {
        let obj: Self = glib::Object::new();
        let accts = load_from(&accounts_file())?;
        obj.imp().accounts.replace(accts);
        Ok(obj)
    }

    /// Persist the current list to `accounts.json`.
    pub fn save(&self) -> Result<(), AccountsError> {
        save_to(&accounts_file(), &self.imp().accounts.borrow())
    }

    fn emit_changed(&self) {
        self.emit_by_name::<()>("accounts-changed", &[]);
    }

    /// Create, persist, and return a new account.
    pub fn add(&self) -> Account {
        let account = Account::new();
        self.imp().accounts.borrow_mut().push(account.clone());
        let _ = self.save();
        self.emit_changed();
        account
    }

    /// Remove the account with `id`.
    pub fn remove(&self, id: &str) {
        self.imp().accounts.borrow_mut().retain(|a| a.id != id);
        let _ = self.save();
        self.emit_changed();
    }

    /// Make `id` the active account and stamp `last_used_at = now`.
    pub fn activate(&self, id: &str) {
        {
            let mut list = self.imp().accounts.borrow_mut();
            for a in list.iter_mut() {
                a.is_active = a.id == id;
                if a.id == id {
                    a.last_used_at = now();
                }
            }
        }
        let _ = self.save();
        self.emit_changed();
    }

    /// MRU-ordered clone of the account list.
    pub fn get_accounts_sorted(&self) -> Vec<Account> {
        let mut out = self.imp().accounts.borrow().clone();
        sort_mru(&mut out);
        out
    }

    /// Store discovered `wid`/`pushname` for `id`.
    pub fn update_identity(&self, id: &str, wid: Option<String>, pushname: Option<String>) {
        {
            let mut list = self.imp().accounts.borrow_mut();
            if let Some(a) = list.iter_mut().find(|a| a.id == id) {
                if wid.is_some() {
                    a.wid = wid;
                }
                if pushname.is_some() {
                    a.pushname = pushname;
                }
            }
        }
        let _ = self.save();
        self.emit_changed();
    }

    /// Store the user-chosen `user_label` for `id`; `None`/empty clears it.
    pub fn update_user_label(&self, id: &str, label: Option<String>) {
        {
            let mut list = self.imp().accounts.borrow_mut();
            if let Some(a) = list.iter_mut().find(|a| a.id == id) {
                a.user_label = label.filter(|s| !s.trim().is_empty());
            }
        }
        let _ = self.save();
        self.emit_changed();
    }

    /// Persist `id`'s per-account zoom. Deliberately does NOT emit `accounts-changed` (every Ctrl+plus step would rebuild-storm the switcher).
    pub fn set_zoom(&self, id: &str, zoom: f64) {
        {
            let mut list = self.imp().accounts.borrow_mut();
            if let Some(a) = list.iter_mut().find(|a| a.id == id) {
                if (a.zoom_level - zoom).abs() < f64::EPSILON {
                    return;
                }
                a.zoom_level = zoom;
            } else {
                return;
            }
        }
        let _ = self.save();
    }

    /// Look up an account by id.
    pub fn get(&self, id: &str) -> Option<Account> {
        self.imp().accounts.borrow().iter().find(|a| a.id == id).cloned()
    }

    /// The currently-active account, if any.
    pub fn active(&self) -> Option<Account> {
        self.imp().accounts.borrow().iter().find(|a| a.is_active).cloned()
    }

    /// Store decoded avatar PNG bytes for `id`.
    pub fn update_avatar(&self, id: &str, png: Vec<u8>) {
        {
            let mut list = self.imp().accounts.borrow_mut();
            if let Some(a) = list.iter_mut().find(|a| a.id == id) {
                a.avatar_png = Some(png);
            }
        }
        let _ = self.save();
        self.emit_changed();
    }
}

thread_local! {
    // Sound: all GTK/CEF handlers run on the glib main thread, so this non-Send GObject never crosses threads.
    static MANAGER: RefCell<Option<AccountManager>> = const { RefCell::new(None) };
}

/// The process-wide [`AccountManager`], loading `accounts.json` on first use. A malformed file is reported (never discarded) and an empty manager returned so the app still launches.
pub fn manager() -> AccountManager {
    MANAGER.with(|cell| {
        if cell.borrow().is_none() {
            let m = AccountManager::load().unwrap_or_else(|e| {
                log::error!("accounts: {e}; starting with an empty account list");
                glib::Object::new()
            });
            *cell.borrow_mut() = Some(m);
        }
        cell.borrow().as_ref().unwrap().clone()
    })
}

/// Transient, non-persisted per-account runtime flags surfaced in the switcher.
#[derive(Default, Clone, Debug)]
pub struct AccountRuntime {
    pub awaiting_pairing: bool,
    pub degraded: bool,
    pub degraded_reason: Option<String>,
    pub has_unread: bool,
}

thread_local! {
    static BROWSER_IDS: RefCell<HashMap<i32, String>> = RefCell::new(HashMap::new());
    static RUNTIME: RefCell<HashMap<String, AccountRuntime>> = RefCell::new(HashMap::new());
}

/// Associate a spawned CEF browser id with the account it serves.
pub fn register_browser(cef_id: i32, account_id: &str) {
    BROWSER_IDS.with(|m| {
        m.borrow_mut().insert(cef_id, account_id.to_owned());
    });
}

/// Drop a browser id mapping when its browser closes.
pub fn unregister_browser(cef_id: i32) {
    BROWSER_IDS.with(|m| {
        m.borrow_mut().remove(&cef_id);
    });
}

/// The account that owns CEF browser `cef_id`, if any.
pub fn account_for_browser(cef_id: i32) -> Option<String> {
    BROWSER_IDS.with(|m| m.borrow().get(&cef_id).cloned())
}

/// Snapshot the transient runtime flags for `account_id`.
pub fn runtime_state(account_id: &str) -> AccountRuntime {
    RUNTIME.with(|m| m.borrow().get(account_id).cloned().unwrap_or_default())
}

// Emits accounts-changed only on a real change: the Store hook fires many times/sec and emitting each time would rebuild-storm the switcher and eat clicks.
fn mutate_runtime(account_id: &str, f: impl FnOnce(&mut AccountRuntime) -> bool) -> bool {
    let changed = RUNTIME.with(|m| {
        let mut map = m.borrow_mut();
        f(map.entry(account_id.to_owned()).or_default())
    });
    if changed {
        manager().emit_by_name::<()>("accounts-changed", &[]);
    }
    changed
}

/// Mark the account as awaiting pairing.
pub fn set_awaiting_pairing(account_id: &str, awaiting: bool) {
    mutate_runtime(account_id, |r| {
        let changed = r.awaiting_pairing != awaiting;
        r.awaiting_pairing = awaiting;
        changed
    });
}

/// Mark the account degraded (Store hook failed). Returns `true` only on the first transition, so callers can do one-time work.
pub fn set_degraded(account_id: &str, reason: String) -> bool {
    mutate_runtime(account_id, |r| {
        if r.degraded {
            return false;
        }
        r.degraded = true;
        r.degraded_reason = Some(reason);
        true
    })
}

/// Clear the degraded badge; call only on a fresh successful Store hook attachment.
pub fn clear_degraded(account_id: &str) {
    mutate_runtime(account_id, |r| {
        if !r.degraded {
            return false;
        }
        r.degraded = false;
        r.degraded_reason = None;
        true
    });
}

/// Set the per-account unread flag.
pub fn set_unread(account_id: &str, unread: bool) {
    mutate_runtime(account_id, |r| {
        let changed = r.has_unread != unread;
        r.has_unread = unread;
        changed
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn acct_with(last_used: i64) -> Account {
        let mut a = Account::new();
        a.last_used_at = last_used;
        a
    }

    #[test]
    fn mru_sort_orders_by_last_used_desc() {
        let b = acct_with(200);
        let a = acct_with(300);
        let c = acct_with(100);
        let (bid, aid, cid) = (b.id.clone(), a.id.clone(), c.id.clone());
        let mut list = vec![b, a, c];
        sort_mru(&mut list);
        assert_eq!(
            list.iter().map(|x| x.id.clone()).collect::<Vec<_>>(),
            vec![aid, bid, cid],
        );
    }

    #[test]
    fn load_missing_file_is_empty() {
        let dir = std::env::temp_dir().join(format!("karere-acct-{}", uuid::Uuid::new_v4()));
        let path = dir.join("accounts.json");
        assert!(load_from(&path).unwrap().is_empty());
    }

    #[test]
    fn load_malformed_file_errs() {
        let dir = std::env::temp_dir().join(format!("karere-acct-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("accounts.json");
        std::fs::write(&path, b"{ not valid json ]").unwrap();
        assert!(matches!(load_from(&path), Err(AccountsError::Parse(_))));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn save_roundtrips_with_avatar_base64() {
        let dir = std::env::temp_dir().join(format!("karere-acct-{}", uuid::Uuid::new_v4()));
        let path = dir.join("accounts.json");
        let mut a = Account::new();
        a.avatar_png = Some(vec![0xDE, 0xAD, 0xBE, 0xEF]);
        a.pushname = Some("Alice".into());
        save_to(&path, &[a.clone()]).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains(&base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            [0xDE, 0xAD, 0xBE, 0xEF]
        )));
        let loaded = load_from(&path).unwrap();
        assert_eq!(loaded, vec![a]);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn atomic_save_leaves_prior_file_intact_on_mid_write_crash() {
        let dir = std::env::temp_dir().join(format!("karere-acct-{}", uuid::Uuid::new_v4()));
        let path = dir.join("accounts.json");
        let good = acct_with(42);
        save_to(&path, &[good.clone()]).unwrap();
        let before = std::fs::read(&path).unwrap();

        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, b"<partial write, never renamed>").unwrap();

        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert_eq!(load_from(&path).unwrap(), vec![good]);
        std::fs::remove_dir_all(&dir).ok();
    }
}
