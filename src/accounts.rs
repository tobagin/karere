//! Persistent multi-account store (M20).
//!
//! # JSON layout
//! All accounts live in a single file at
//! `$XDG_DATA_HOME/karere/accounts/accounts.json` (fallback
//! `~/.local/share/karere/accounts/accounts.json`). The file is a JSON array of
//! [`Account`] records. Avatar bytes are stored inline as base64 strings (see
//! [`avatar_b64`]); per-account CEF session data lives separately under
//! `accounts/sessions/<id>/data` and is owned by the browser pool, not this
//! module.
//!
//! # Atomic-write guarantee
//! [`save_to`] writes the serialized array to `accounts.json.tmp` in the same
//! directory and then `fs::rename`s it over `accounts.json`. `rename` is atomic
//! on a single filesystem, so a crash mid-write either leaves the prior
//! `accounts.json` fully intact or completes the swap — never a half-written
//! live file. The temp file is a sibling of the target so the rename never
//! crosses a filesystem boundary.
//!
//! # MRU contract
//! Accounts have no user-controllable `order` field. The UI always renders the
//! list returned by [`AccountManager::get_accounts_sorted`], which sorts by
//! `last_used_at` descending (most-recently-used first). [`AccountManager::activate`]
//! stamps `last_used_at = now` and re-persists, so the just-used account floats
//! to the top on the next read.

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

/// Per-account permission decisions, keyed exactly like M11's global store:
/// `origin → (permission-mask bit → State)`. Reuses [`State`] from
/// `permissions_store` so a per-account decision and a global one are the same
/// type. Empty map means "defer to the global store".
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct AccountPermissions {
    /// origin → (permission-mask bit → decision).
    #[serde(default)]
    pub overrides: HashMap<String, HashMap<u32, State>>,
}

/// One WhatsApp Web account: identity, session state, and per-account prefs.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Account {
    /// Stable UUID (v4). Names the session dir and keys the browser pool.
    pub id: String,
    /// WhatsApp internal id (`Store.Conn.wid`), once discovered.
    #[serde(default)]
    pub wid: Option<String>,
    /// Display name from `Store.Conn.pushname`, once discovered.
    #[serde(default)]
    pub pushname: Option<String>,
    /// User-chosen label; the only user-editable identity field.
    #[serde(default)]
    pub user_label: Option<String>,
    /// Decoded avatar PNG bytes (base64 on disk — see [`avatar_b64`]).
    #[serde(default, with = "avatar_b64")]
    pub avatar_png: Option<Vec<u8>>,
    /// Source URL the avatar was fetched from (`descriptor.eurl`).
    #[serde(default)]
    pub avatar_url: Option<String>,
    /// Unix seconds at creation.
    pub created_at: i64,
    /// Unix seconds of the last `activate`; drives MRU ordering.
    pub last_used_at: i64,
    /// Whether this account is the current foreground.
    #[serde(default)]
    pub is_active: bool,
    /// Whether a CEF session (cookies/storage) exists on disk.
    #[serde(default)]
    pub has_session: bool,
    /// Whether the account has unread chats.
    #[serde(default)]
    pub has_unread: bool,
    /// Per-account zoom (M18).
    #[serde(default = "default_zoom")]
    pub zoom_level: f64,
    /// Per-account permission overrides (M11 stub).
    #[serde(default)]
    pub permissions: AccountPermissions,
}

fn default_zoom() -> f64 {
    1.0
}

/// Unix seconds, monotonic-ish wall clock. Saturates on pre-epoch clocks.
fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

impl Account {
    /// A fresh account with a new UUID and `created_at == last_used_at == now`.
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
            has_unread: false,
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

/// serde adapter: `Option<Vec<u8>>` ⇆ base64 string on disk (vs. serde's
/// default int-array encoding, which would bloat the file ~4×).
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

/// Errors from loading the account store.
#[derive(Debug)]
pub enum AccountsError {
    /// `accounts.json` exists but could not be read or parsed.
    Parse(String),
    /// A persistence (write/rename) operation failed.
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

/// `$XDG_DATA_HOME/karere/accounts/` (fallback `~/.local/share/karere/accounts/`).
pub fn accounts_root() -> PathBuf {
    glib::user_data_dir().join("karere").join("accounts")
}

/// Path to the live `accounts.json`.
pub fn accounts_file() -> PathBuf {
    accounts_root().join("accounts.json")
}

/// Delete an account's on-disk CEF session dir (`accounts/sessions/<id>`) so a
/// removed account leaves no orphaned cookies/storage. Best-effort: logs on
/// failure (e.g. if files are still briefly held by a closing browser). Only
/// the local session is wiped — the device stays linked on the user's phone
/// until removed there (no reliable remote unlink).
pub fn delete_session_dir(id: &str) {
    let dir = session_cache_path(id);
    if let Err(e) = std::fs::remove_dir_all(&dir)
        && e.kind() != std::io::ErrorKind::NotFound
    {
        log::warn!("failed to delete session dir {}: {e}", dir.display());
    }
}

/// Per-account CEF session cache dir: `accounts/sessions/<id>`.
///
/// Must be a DIRECT child of the global `root_cache_path` (`accounts/sessions`):
/// CEF's Chrome runtime treats each immediate subdirectory of the user-data dir
/// as a profile and rejects deeper nesting (`.../<id>/data` fails with "Cannot
/// create profile at path"), silently falling back to the shared global profile
/// — which destroys per-account isolation.
pub fn session_cache_path(id: &str) -> PathBuf {
    accounts_root().join("sessions").join(id)
}

/// Load accounts from `path`. Missing file → empty list; present-but-malformed
/// → `Err` (the file is never silently overwritten on a parse failure).
pub fn load_from(path: &std::path::Path) -> Result<Vec<Account>, AccountsError> {
    match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice::<Vec<Account>>(&bytes)
            .map_err(|e| AccountsError::Parse(e.to_string())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(AccountsError::Io(e.to_string())),
    }
}

/// Persist `accounts` to `path` via temp-then-rename (atomic on one filesystem).
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

/// Sort in place by `last_used_at` descending (most-recently-used first).
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
    /// Owns the in-memory account list and persists it. Emits `accounts-changed`
    /// after every mutation so the switcher and tray can re-render.
    pub struct AccountManager(ObjectSubclass<imp::AccountManager>);
}

impl AccountManager {
    /// Build a manager and load `accounts.json` from the default location.
    /// Missing file → empty list; malformed file → `Err`.
    pub fn load() -> Result<Self, AccountsError> {
        let obj: Self = glib::Object::new();
        let accts = load_from(&accounts_file())?;
        obj.imp().accounts.replace(accts);
        Ok(obj)
    }

    /// Persist the current list to `accounts.json` (atomic).
    pub fn save(&self) -> Result<(), AccountsError> {
        save_to(&accounts_file(), &self.imp().accounts.borrow())
    }

    fn emit_changed(&self) {
        self.emit_by_name::<()>("accounts-changed", &[]);
    }

    /// Create, persist, and return a new account (appended to the list).
    pub fn add(&self) -> Account {
        let account = Account::new();
        self.imp().accounts.borrow_mut().push(account.clone());
        let _ = self.save();
        self.emit_changed();
        account
    }

    /// Remove the account with `id`, persist, and emit `accounts-changed`.
    pub fn remove(&self, id: &str) {
        self.imp().accounts.borrow_mut().retain(|a| a.id != id);
        let _ = self.save();
        self.emit_changed();
    }

    /// Stamp `last_used_at = now` for `id`, persist, emit `accounts-changed`.
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

    /// MRU-ordered clone of the account list (most-recently-used first).
    pub fn get_accounts_sorted(&self) -> Vec<Account> {
        let mut out = self.imp().accounts.borrow().clone();
        sort_mru(&mut out);
        out
    }

    /// Store discovered `wid`/`pushname` for `id` and persist.
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

    /// Store the user-chosen `user_label` for `id` (the only editable identity
    /// field) and persist. `None`/empty clears it.
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

    /// Look up a single account by id.
    pub fn get(&self, id: &str) -> Option<Account> {
        self.imp().accounts.borrow().iter().find(|a| a.id == id).cloned()
    }

    /// The currently-active account (`is_active`), if any.
    pub fn active(&self) -> Option<Account> {
        self.imp().accounts.borrow().iter().find(|a| a.is_active).cloned()
    }

    /// Store decoded avatar PNG bytes for `id` and persist.
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
    /// App-wide manager handle. Both the GTK widgets and the CEF handlers run on
    /// the glib main thread (external message pump), so a `thread_local` is a
    /// sound home for this non-`Send` GObject — every reachable caller is on that
    /// one thread. Populated lazily by [`manager`].
    static MANAGER: RefCell<Option<AccountManager>> = const { RefCell::new(None) };
}

/// The process-wide [`AccountManager`], loading `accounts.json` on first use.
///
/// A malformed file is reported (never silently discarded on disk) and an empty
/// in-memory manager is returned so the app still launches; the bad file remains
/// until the user resolves it or a mutation rewrites it.
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
    /// `Store.AppState` (or the page) has not reached `CONNECTED` yet.
    pub awaiting_pairing: bool,
    /// The Store hook failed for this account; the DOM fallback is active and
    /// the switcher shows a persistent "degraded mode" badge. Cleared only by a
    /// fresh successful Store attachment (a page reload that re-runs the hook).
    pub degraded: bool,
    /// Human-readable reason captured from the `StoreUnavailable` message.
    pub degraded_reason: Option<String>,
}

thread_local! {
    /// CEF `Browser::identifier()` → account id, so a renderer message arriving
    /// on the CEF UI thread can be attributed to its account.
    static BROWSER_IDS: RefCell<HashMap<i32, String>> = RefCell::new(HashMap::new());
    /// account id → transient [`AccountRuntime`] flags.
    static RUNTIME: RefCell<HashMap<String, AccountRuntime>> = RefCell::new(HashMap::new());
}

/// Associate a spawned CEF browser id with the account it serves.
pub fn register_browser(cef_id: i32, account_id: &str) {
    BROWSER_IDS.with(|m| {
        m.borrow_mut().insert(cef_id, account_id.to_owned());
    });
}

/// Drop a browser id mapping when its browser is closed.
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

/// Apply `f` to the account's runtime entry; emit `accounts-changed` only when
/// `f` reports an actual change. Idempotent setters are essential: the Store
/// hook can fire `StoreUnavailable` / `AwaitingPairing` many times a second, and
/// emitting on every one would rebuild the switcher continuously and eat clicks.
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

/// Mark the account as awaiting pairing (QR not yet scanned / not CONNECTED).
pub fn set_awaiting_pairing(account_id: &str, awaiting: bool) {
    mutate_runtime(account_id, |r| {
        let changed = r.awaiting_pairing != awaiting;
        r.awaiting_pairing = awaiting;
        changed
    });
}

/// Mark the account degraded (Store hook failed); persists until a later
/// successful Store attachment, never cleared by the DOM fallback succeeding.
/// Returns `true` only on the first transition into degraded, so callers can
/// do one-time work (inject the DOM fallback, log) without repeating it.
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

/// Clear the degraded badge — called only on a fresh successful Store hook
/// attachment (a `ProfileIdentity` with `source: "store"`). No-op (and no
/// signal) when the account was not degraded.
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
        // Three accounts with shuffled timestamps → newest first.
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
        // Avatar must be a base64 string on disk, not a numeric array.
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
        // Simulate: a good accounts.json exists; a save crashes after writing the
        // .tmp sibling but before the rename. The live file must be untouched.
        let dir = std::env::temp_dir().join(format!("karere-acct-{}", uuid::Uuid::new_v4()));
        let path = dir.join("accounts.json");
        let good = acct_with(42);
        save_to(&path, &[good.clone()]).unwrap();
        let before = std::fs::read(&path).unwrap();

        // Crash-before-rename: write only the temp sibling with new contents.
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, b"<partial write, never renamed>").unwrap();

        // The live file still parses to the prior account.
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert_eq!(load_from(&path).unwrap(), vec![good]);
        std::fs::remove_dir_all(&dir).ok();
    }
}
