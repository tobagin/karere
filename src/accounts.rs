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
    /// When true, suppress this account's notification banners + tray bump (the
    /// unread badge still shows). Per-account, persisted.
    #[serde(default)]
    pub muted: bool,
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
            muted: false,
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

    pub fn serialize<S: Serializer>(bytes: &Option<Vec<u8>>, ser: S) -> Result<S::Ok, S::Error> {
        match bytes {
            Some(b) => ser.serialize_some(&B64.encode(b)),
            None => ser.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(de: D) -> Result<Option<Vec<u8>>, D::Error> {
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
    sessions_root().join(id)
}

/// Load accounts from `path`. Missing file → empty list; malformed → `Err`. Callers
/// must not save over a malformed file; [`load_or_recover`] moves it aside first.
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
    use std::io::Write;
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
    if let Some(dir) = path.parent() {
        std::fs::DirBuilder::new()
            .mode(0o700)
            .recursive(true)
            .create(dir)
            .map_err(|e| AccountsError::Io(e.to_string()))?;
    }
    let json = serde_json::to_vec_pretty(accounts).map_err(|e| AccountsError::Io(e.to_string()))?;
    let tmp = path.with_extension("json.tmp");
    let _ = std::fs::remove_file(&tmp);
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&tmp)
        .map_err(|e| AccountsError::Io(e.to_string()))?;
    f.write_all(&json)
        .map_err(|e| AccountsError::Io(e.to_string()))?;
    f.flush().map_err(|e| AccountsError::Io(e.to_string()))?;
    // Data must be on disk before the rename publishes it, or a power cut can
    // leave a renamed-but-empty accounts.json.
    f.sync_all().map_err(|e| AccountsError::Io(e.to_string()))?;
    drop(f);
    std::fs::rename(&tmp, path).map_err(|e| AccountsError::Io(e.to_string()))?;
    Ok(())
}

/// Root holding one CEF profile dir per account (`sessions/<account id>`).
pub fn sessions_root() -> PathBuf {
    accounts_root().join("sessions")
}

/// `name` is a canonical (lowercase, hyphenated) UUID, the only form
/// [`Account::new`] produces. Anything else under `sessions/` is not ours.
fn is_account_id(name: &str) -> bool {
    uuid::Uuid::parse_str(name).is_ok_and(|u| u.hyphenated().to_string() == name)
}

/// Account ids of the real (non-symlink) directories directly under `sessions`.
fn session_dir_ids(sessions: &std::path::Path) -> Vec<(String, std::fs::Metadata)> {
    let Ok(entries) = std::fs::read_dir(sessions) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|e| {
            // DirEntry::file_type does not follow symlinks.
            if !e.file_type().ok()?.is_dir() {
                return None;
            }
            let name = e.file_name().into_string().ok()?;
            if !is_account_id(&name) {
                return None;
            }
            Some((name, e.metadata().ok()?))
        })
        .collect()
}

/// Rebuild a minimal account list from the session dirs under `sessions`, so a
/// lost/corrupt accounts.json does not orphan logged-in sessions. Labels, mute
/// and zoom are gone; identity re-fills from the Store hook on load.
pub fn recover_from_sessions(sessions: &std::path::Path) -> Vec<Account> {
    let mut out: Vec<Account> = session_dir_ids(sessions)
        .into_iter()
        .map(|(id, meta)| {
            let ts = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or_else(now);
            Account {
                id,
                created_at: ts,
                last_used_at: ts,
                has_session: true,
                ..Account::new()
            }
        })
        .collect();
    sort_stable(&mut out);
    out
}

/// How [`load_or_recover`] produced its list.
#[derive(Debug, PartialEq, Eq)]
pub enum LoadOutcome {
    /// accounts.json existed and parsed: the list is authoritative.
    Loaded,
    /// accounts.json was missing, unreadable or corrupt; the list was rebuilt
    /// from session dirs (possibly empty). Not authoritative for cleanup.
    Recovered,
}

/// Load `path`; when it is missing, unreadable or corrupt, rebuild the list from
/// the session dirs under `sessions` instead of starting empty. A corrupt file
/// is moved aside to `<path>.corrupt-<unix ts>` (never overwritten) and the
/// recovered list is written in its place.
pub fn load_or_recover(
    path: &std::path::Path,
    sessions: &std::path::Path,
) -> (Vec<Account>, LoadOutcome) {
    let err = match std::fs::metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        _ => match load_from(path) {
            Ok(list) => return (list, LoadOutcome::Loaded),
            Err(e) => Some(e),
        },
    };
    if let Some(e) = &err {
        log::error!("accounts: {e}; rebuilding the account list from session dirs");
    }
    if matches!(err, Some(AccountsError::Parse(_))) {
        let mut aside = path.as_os_str().to_owned();
        aside.push(format!(".corrupt-{}", now()));
        let aside = PathBuf::from(aside);
        match std::fs::rename(path, &aside) {
            Ok(()) => log::warn!("accounts: moved corrupt file to {}", aside.display()),
            Err(e) => {
                log::error!("accounts: could not move corrupt file aside: {e}");
                if let Err(e) = std::fs::copy(path, &aside) {
                    log::error!("accounts: could not back up corrupt file: {e}");
                }
            }
        }
    }
    let list = recover_from_sessions(sessions);
    if !list.is_empty() {
        log::warn!("accounts: recovered {} account(s) from session dirs", list.len());
        if let Err(e) = save_to(path, &list) {
            log::error!("accounts: failed to save recovered list: {e}");
        }
    }
    (list, LoadOutcome::Recovered)
}

/// Delete session dirs directly under `sessions` whose name is a canonical
/// account id not in `known` (left behind when removal's delayed wipe did not
/// run). Only call with an authoritative list ([`LoadOutcome::Loaded`]) and
/// before any CEF browser exists. Returns the ids removed.
pub fn sweep_orphan_sessions(sessions: &std::path::Path, known: &[Account]) -> Vec<String> {
    let mut removed = Vec::new();
    for (id, _) in session_dir_ids(sessions) {
        if known.iter().any(|a| a.id == id) {
            continue;
        }
        let dir = sessions.join(&id);
        match std::fs::remove_dir_all(&dir) {
            Ok(()) => {
                log::info!("accounts: removed orphan session dir {}", dir.display());
                removed.push(id);
            }
            Err(e) => log::warn!("accounts: failed to remove orphan {}: {e}", dir.display()),
        }
    }
    removed
}

/// Lowest unused default label `Account N` (N >= 1) among `accounts`.
pub fn lowest_unused_label(accounts: &[Account]) -> String {
    let used: std::collections::HashSet<u32> = accounts
        .iter()
        .filter_map(|a| a.user_label.as_deref()?.trim().strip_prefix("Account ")?.parse().ok())
        .collect();
    let n = (1..).find(|n| !used.contains(n)).unwrap_or(1);
    format!("Account {n}")
}

/// Sort in place by `created_at` ascending (id tiebreak): a stable order that
/// never reshuffles on switch, so the switcher list and Alt+N stay put (#166).
pub fn sort_stable(accounts: &mut [Account]) {
    accounts.sort_by(|a, b| {
        a.created_at
            .cmp(&b.created_at)
            .then_with(|| a.id.cmp(&b.id))
    });
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
    /// Persist the current list to `accounts.json`.
    pub fn save(&self) -> Result<(), AccountsError> {
        save_to(&accounts_file(), &self.imp().accounts.borrow())
    }

    /// [`save`](Self::save), logging instead of returning a failure.
    fn save_logged(&self) {
        if let Err(e) = self.save() {
            log::error!("accounts: failed to save: {e}");
        }
    }

    fn emit_changed(&self) {
        self.emit_by_name::<()>("accounts-changed", &[]);
    }

    /// Create, persist, and return a new account.
    pub fn add(&self) -> Account {
        let account = Account::new();
        self.imp().accounts.borrow_mut().push(account.clone());
        self.save_logged();
        self.emit_changed();
        account
    }

    /// Create a new account labelled with the lowest unused `Account N` and make
    /// it the active one, with a single save and a single `accounts-changed`.
    pub fn add_active_with_default_label(&self) -> Account {
        let mut account = Account::new();
        {
            let mut list = self.imp().accounts.borrow_mut();
            account.user_label = Some(lowest_unused_label(&list));
            account.is_active = true;
            for a in list.iter_mut() {
                a.is_active = false;
            }
            list.push(account.clone());
        }
        self.save_logged();
        self.emit_changed();
        account
    }

    /// Give every label-less account a default `Account N` label, numbered by
    /// creation order (oldest → "Account 1"). The bootstrap/default account is
    /// created without a label; this backfills it (and migrates older installs)
    /// so it reads "Account 1" alongside added accounts. No-op when all labelled.
    pub fn backfill_labels(&self) {
        let mut changed = false;
        {
            let mut list = self.imp().accounts.borrow_mut();
            let mut order: Vec<usize> = (0..list.len()).collect();
            order.sort_by_key(|&i| list[i].created_at);
            for (pos, &i) in order.iter().enumerate() {
                let blank = list[i]
                    .user_label
                    .as_deref()
                    .map(str::trim)
                    .unwrap_or("")
                    .is_empty();
                if blank {
                    list[i].user_label = Some(format!("Account {}", pos + 1));
                    changed = true;
                }
            }
        }
        if changed {
            self.save_logged();
            self.emit_changed();
        }
    }

    /// Remove the account with `id`.
    pub fn remove(&self, id: &str) {
        self.imp().accounts.borrow_mut().retain(|a| a.id != id);
        self.save_logged();
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
        self.save_logged();
        self.emit_changed();
    }

    /// Stable (creation-ordered) clone of the account list — the order the
    /// switcher shows and Alt+1..9 index into (#166).
    pub fn get_accounts_sorted(&self) -> Vec<Account> {
        let mut out = self.imp().accounts.borrow().clone();
        sort_stable(&mut out);
        out
    }

    /// Most-recently-used account: startup activation + promoting a survivor
    /// after the active account is removed.
    pub fn mru_first(&self) -> Option<Account> {
        self.imp()
            .accounts
            .borrow()
            .iter()
            .max_by_key(|a| a.last_used_at)
            .cloned()
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
        self.save_logged();
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
        self.save_logged();
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
        self.save_logged();
    }

    /// Set this account's notification mute flag and persist it. Deliberately
    /// does NOT emit accounts-changed: the only UI reflection is the row's own
    /// mute toggle (updated in place by its handler), and emitting here would
    /// rebuild the switcher synchronously from within that toggle's callback —
    /// destroying the live button mid-handler (mirrors set_zoom's reasoning).
    pub fn set_muted(&self, id: &str, muted: bool) {
        let mut list = self.imp().accounts.borrow_mut();
        match list.iter_mut().find(|a| a.id == id) {
            Some(a) if a.muted != muted => a.muted = muted,
            _ => return,
        }
        drop(list);
        self.save_logged();
    }

    /// Whether `id`'s notifications are muted (unknown id → not muted).
    pub fn is_muted(&self, id: &str) -> bool {
        self.imp()
            .accounts
            .borrow()
            .iter()
            .find(|a| a.id == id)
            .map(|a| a.muted)
            .unwrap_or(false)
    }

    /// Look up an account by id.
    pub fn get(&self, id: &str) -> Option<Account> {
        self.imp()
            .accounts
            .borrow()
            .iter()
            .find(|a| a.id == id)
            .cloned()
    }

    /// The currently-active account, if any.
    pub fn active(&self) -> Option<Account> {
        self.imp()
            .accounts
            .borrow()
            .iter()
            .find(|a| a.is_active)
            .cloned()
    }

    /// Store decoded avatar PNG bytes for `id`.
    pub fn update_avatar(&self, id: &str, png: Vec<u8>) {
        {
            let mut list = self.imp().accounts.borrow_mut();
            if let Some(a) = list.iter_mut().find(|a| a.id == id) {
                a.avatar_png = Some(png);
            }
        }
        self.save_logged();
        self.emit_changed();
    }

    /// Drop discovered identity (wid/pushname/avatar) for `id` — called on
    /// logout so the switcher stops showing a stale name/avatar. `user_label`
    /// (user-chosen) is left intact.
    pub fn clear_identity(&self, id: &str) {
        {
            let mut list = self.imp().accounts.borrow_mut();
            if let Some(a) = list.iter_mut().find(|a| a.id == id) {
                a.wid = None;
                a.pushname = None;
                a.avatar_png = None;
            }
        }
        self.save_logged();
        self.emit_changed();
    }
}

thread_local! {
    // Sound: all GTK/CEF handlers run on the glib main thread, so this non-Send GObject never crosses threads.
    static MANAGER: RefCell<Option<AccountManager>> = const { RefCell::new(None) };
}

/// The process-wide [`AccountManager`], loading `accounts.json` on first use.
/// A missing, unreadable or corrupt file never starts the app empty over live
/// sessions: a corrupt file is moved aside and the list rebuilt from the session
/// dirs ([`load_or_recover`]). Only after a clean load are session dirs of
/// unknown accounts swept; the first call happens before any browser exists.
pub fn manager() -> AccountManager {
    MANAGER.with(|cell| {
        if cell.borrow().is_none() {
            let m: AccountManager = glib::Object::new();
            // Unit tests must never touch (rename, rewrite, sweep) the real
            // user data dir.
            if cfg!(test) {
                if let Ok(list) = load_from(&accounts_file()) {
                    m.imp().accounts.replace(list);
                }
            } else {
                let sessions = sessions_root();
                let (list, outcome) = load_or_recover(&accounts_file(), &sessions);
                if outcome == LoadOutcome::Loaded {
                    sweep_orphan_sessions(&sessions, &list);
                }
                m.imp().accounts.replace(list);
            }
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

/// Mark the account as awaiting pairing. Returns `true` only on a real
/// transition, so callers can do one-time work (e.g. clear stale identity).
pub fn set_awaiting_pairing(account_id: &str, awaiting: bool) -> bool {
    mutate_runtime(account_id, |r| {
        let changed = r.awaiting_pairing != awaiting;
        r.awaiting_pairing = awaiting;
        changed
    })
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

    fn acct_with(created: i64) -> Account {
        let mut a = Account::new();
        a.created_at = created;
        a
    }

    #[test]
    fn stable_sort_orders_by_created_asc() {
        let b = acct_with(200);
        let a = acct_with(100);
        let c = acct_with(300);
        let (bid, aid, cid) = (b.id.clone(), a.id.clone(), c.id.clone());
        let mut list = vec![b, a, c];
        sort_stable(&mut list);
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
        save_to(&path, std::slice::from_ref(&good)).unwrap();
        let before = std::fs::read(&path).unwrap();

        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, b"<partial write, never renamed>").unwrap();

        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert_eq!(load_from(&path).unwrap(), vec![good]);
        std::fs::remove_dir_all(&dir).ok();
    }

    fn temp_store() -> (PathBuf, PathBuf, PathBuf) {
        let dir = std::env::temp_dir().join(format!("karere-acct-{}", uuid::Uuid::new_v4()));
        let sessions = dir.join("sessions");
        std::fs::create_dir_all(&sessions).unwrap();
        let path = dir.join("accounts.json");
        (dir, path, sessions)
    }

    #[test]
    fn corrupt_file_is_moved_aside_and_sessions_adopted() {
        let (dir, path, sessions) = temp_store();
        std::fs::write(&path, b"{ not valid json ]").unwrap();
        let a = Account::new().id;
        let b = Account::new().id;
        std::fs::create_dir(sessions.join(&a)).unwrap();
        std::fs::create_dir(sessions.join(&b)).unwrap();
        std::fs::create_dir(sessions.join("not-an-account")).unwrap();
        std::fs::write(sessions.join(Account::new().id), b"a file, not a dir").unwrap();

        let (list, outcome) = load_or_recover(&path, &sessions);
        assert_eq!(outcome, LoadOutcome::Recovered);
        let mut ids: Vec<_> = list.iter().map(|x| x.id.clone()).collect();
        ids.sort();
        let mut want = vec![a, b];
        want.sort();
        assert_eq!(ids, want);

        // The bad bytes survive under a .corrupt-<ts> name; accounts.json now
        // holds the recovered list.
        let aside: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with("accounts.json.corrupt-")
            })
            .collect();
        assert_eq!(aside.len(), 1);
        assert_eq!(std::fs::read(aside[0].path()).unwrap(), b"{ not valid json ]");
        assert_eq!(load_from(&path).unwrap(), list);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn missing_file_adopts_sessions() {
        let (dir, path, sessions) = temp_store();
        let a = Account::new().id;
        std::fs::create_dir(sessions.join(&a)).unwrap();
        let (list, outcome) = load_or_recover(&path, &sessions);
        assert_eq!(outcome, LoadOutcome::Recovered);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, a);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn valid_file_loads_and_sweep_removes_only_orphan_account_dirs() {
        let (dir, path, sessions) = temp_store();
        let kept = Account::new();
        save_to(&path, std::slice::from_ref(&kept)).unwrap();
        let orphan = Account::new().id;
        std::fs::create_dir(sessions.join(&kept.id)).unwrap();
        std::fs::create_dir(sessions.join(&orphan)).unwrap();
        std::fs::create_dir(sessions.join("Default")).unwrap();
        std::fs::create_dir(sessions.join(orphan.to_uppercase())).unwrap();

        let (list, outcome) = load_or_recover(&path, &sessions);
        assert_eq!(outcome, LoadOutcome::Loaded);
        assert_eq!(list, vec![kept.clone()]);
        assert_eq!(sweep_orphan_sessions(&sessions, &list), vec![orphan.clone()]);
        assert!(sessions.join(&kept.id).is_dir());
        assert!(!sessions.join(&orphan).exists());
        assert!(sessions.join("Default").is_dir());
        assert!(sessions.join(orphan.to_uppercase()).is_dir());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn default_label_is_lowest_unused() {
        let mut a = Account::new();
        a.user_label = Some("Account 1".into());
        let mut b = Account::new();
        b.user_label = Some("Account 3".into());
        let mut c = Account::new();
        c.user_label = Some("Work".into());
        assert_eq!(lowest_unused_label(&[]), "Account 1");
        assert_eq!(lowest_unused_label(&[a.clone(), b.clone(), c]), "Account 2");
        let mut d = Account::new();
        d.user_label = Some("Account 2".into());
        assert_eq!(lowest_unused_label(&[a, b, d]), "Account 4");
    }
}
