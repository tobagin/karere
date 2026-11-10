//! Persistent multi-account store at `$XDG_DATA_HOME/karere/accounts/accounts.json`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use gtk::glib;
use gtk::glib::subclass::Signal;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use serde::{Deserialize, Serialize};

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
    pub created_at: i64,
    pub last_used_at: i64,
    #[serde(default)]
    pub is_active: bool,
    #[serde(default = "default_zoom")]
    pub zoom_level: f64,
    /// When true, suppress this account's notification banners + tray bump (the
    /// unread badge still shows). Per-account, persisted.
    #[serde(default)]
    pub muted: bool,
    /// User-chosen order in the switcher (and Alt+N). Absent in older files;
    /// [`load_from`] derives it from the `created_at` order.
    #[serde(default)]
    pub position: i64,
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
            created_at: ts,
            last_used_at: ts,
            is_active: false,
            zoom_level: default_zoom(),
            muted: false,
            position: 0,
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

/// Name suffix of a session dir whose deletion has started.
const REMOVED_SUFFIX: &str = ".removed";

/// Best-effort delete of an account's CEF session dir (device stays linked on the phone).
pub fn delete_session_dir(id: &str) {
    // Rename first: if the wipe stops halfway, what is left no longer has an
    // account-id name, so startup deletes it instead of adopting it.
    let live = session_cache_path(id);
    let dir = sessions_root().join(format!("{id}{REMOVED_SUFFIX}"));
    let dir = if std::fs::rename(&live, &dir).is_ok() { dir } else { live };
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
            .map(|mut list| {
                normalize_positions(&mut list);
                list
            })
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
    let mut out = Vec::new();
    adopt_unlisted_sessions(sessions, &mut out);
    out
}

/// Add an account to `list` for every session dir under `sessions` that is not
/// listed and carries no removal tombstone. Such dirs are live sessions whose
/// entry was lost (an old release replaced a corrupt accounts.json, or a save
/// failed), never something to delete. Returns the ids adopted.
pub fn adopt_unlisted_sessions(sessions: &std::path::Path, list: &mut Vec<Account>) -> Vec<String> {
    let mut adopted = Vec::new();
    let start = list.len();
    for (id, meta) in session_dir_ids(sessions) {
        if list.iter().any(|a| a.id == id) || is_tombstoned(&sessions.join(&id)) {
            continue;
        }
        let ts = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or_else(now);
        adopted.push(id.clone());
        let position = next_position(list);
        list.push(Account {
            id,
            created_at: ts,
            last_used_at: ts,
            position,
            ..Account::new()
        });
    }
    // Adopted accounts follow the listed ones, oldest first.
    let base = next_position(&list[..start]);
    list[start..].sort_by(|a, b| a.created_at.cmp(&b.created_at).then_with(|| a.id.cmp(&b.id)));
    for (i, a) in list[start..].iter_mut().enumerate() {
        a.position = base + i as i64;
    }
    adopted
}

/// Empty marker written into `sessions/<id>/` when the user removes an account,
/// before the delayed directory wipe. Its presence is the only thing that makes
/// a session dir eligible for deletion at startup.
const REMOVED_MARKER: &str = ".karere-removed";

fn is_tombstoned(dir: &std::path::Path) -> bool {
    std::fs::symlink_metadata(dir.join(REMOVED_MARKER)).is_ok()
}

/// Write the removal tombstone into `sessions/<id>/`. A missing dir needs none.
pub fn write_tombstone(sessions: &std::path::Path, id: &str) {
    if !is_account_id(id) {
        return;
    }
    let dir = sessions.join(id);
    if !std::fs::symlink_metadata(&dir).is_ok_and(|m| m.is_dir()) {
        return;
    }
    if let Err(e) = std::fs::write(dir.join(REMOVED_MARKER), b"") {
        log::warn!("accounts: failed to mark {} as removed: {e}", dir.display());
    }
}

/// Delete the session dirs directly under `sessions` that carry a removal
/// tombstone (the user removed the account but the delayed wipe did not run).
/// Unlisted dirs without a tombstone are kept and adopted instead. Call before
/// any CEF browser exists. Returns the ids removed.
pub fn sweep_removed_sessions(sessions: &std::path::Path) -> Vec<String> {
    let mut removed = Vec::new();
    // Leftovers of a wipe that did not finish (see `delete_session_dir`).
    for e in std::fs::read_dir(sessions).into_iter().flatten().flatten() {
        let is_dir = e.file_type().is_ok_and(|t| t.is_dir());
        let name = e.file_name();
        let stale = name
            .to_str()
            .and_then(|n| n.strip_suffix(REMOVED_SUFFIX))
            .is_some_and(is_account_id);
        if is_dir && stale {
            let _ = std::fs::remove_dir_all(e.path());
        }
    }
    for (id, _) in session_dir_ids(sessions) {
        let dir = sessions.join(&id);
        if !is_tombstoned(&dir) {
            continue;
        }
        match std::fs::remove_dir_all(&dir) {
            Ok(()) => {
                log::info!("accounts: removed session dir of deleted account {}", dir.display());
                removed.push(id);
            }
            Err(e) => log::warn!("accounts: failed to remove {}: {e}", dir.display()),
        }
    }
    removed
}

/// How [`load_or_recover`] produced its list.
#[derive(Debug, PartialEq, Eq)]
pub enum LoadOutcome {
    /// accounts.json existed and parsed: the list is authoritative.
    Loaded,
    /// accounts.json was missing, unreadable or corrupt; the list was rebuilt
    /// from session dirs (possibly empty). Not authoritative for cleanup.
    Recovered,
    /// As `Recovered`, but the existing file could be neither read nor backed
    /// up: it must not be saved over for the rest of the session.
    RecoveredUnsaved,
}

/// Load `path`; when it is missing, unreadable or corrupt, rebuild the list from
/// the session dirs under `sessions` instead of starting empty. An unreadable or
/// corrupt file is moved (or copied) aside to `<path>.corrupt-<unix ts>` and the
/// recovered list is written in its place; when neither works the file is left
/// alone and [`LoadOutcome::RecoveredUnsaved`] is returned.
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
    // A file that exists but could not be read or parsed is never overwritten:
    // move it aside (or copy it) first, and if neither works keep it untouched.
    let mut preserved = true;
    if err.is_some() {
        let mut aside = path.as_os_str().to_owned();
        aside.push(format!(".corrupt-{}", now()));
        let aside = PathBuf::from(aside);
        match std::fs::rename(path, &aside) {
            Ok(()) => log::warn!("accounts: moved unusable file to {}", aside.display()),
            Err(e) => {
                log::error!("accounts: could not move unusable file aside: {e}");
                if let Err(e) = std::fs::copy(path, &aside) {
                    log::error!(
                        "accounts: could not back up {} ({e}); running with the recovered \
                         list in memory only, changes this session will not be saved",
                        path.display()
                    );
                    preserved = false;
                }
            }
        }
    }
    let list = recover_from_sessions(sessions);
    if !preserved {
        return (list, LoadOutcome::RecoveredUnsaved);
    }
    if !list.is_empty() {
        log::warn!("accounts: recovered {} account(s) from session dirs", list.len());
        if let Err(e) = save_to(path, &list) {
            log::error!("accounts: failed to save recovered list: {e}");
        }
    }
    (list, LoadOutcome::Recovered)
}

/// `N` of a stored default label `Account N` (the legacy English text that is
/// still what gets persisted), `None` for any user-chosen label.
pub fn default_label_number(label: &str) -> Option<u32> {
    let digits = label.trim().strip_prefix("Account ")?;
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

/// Fill the `{n}` placeholder of a (translated) default-label template.
fn fill_default_label(template: &str, n: u32) -> String {
    template.replace("{n}", &n.to_string())
}

/// The label to show for `account`: a stored default `Account N` is rendered
/// through gettext, a user-chosen label verbatim, `None` when it has no label.
/// Every place that displays an account label goes through this.
pub fn display_label(account: &Account) -> Option<String> {
    let label = account.user_label.as_deref().filter(|l| !l.trim().is_empty())?;
    Some(match default_label_number(label) {
        Some(n) => fill_default_label(&gettextrs::gettext("Account {n}"), n),
        None => label.to_owned(),
    })
}

/// Lowest unused default label `Account N` (N >= 1) among `accounts`. The
/// stored text stays the English legacy form; it is translated at display time.
pub fn lowest_unused_label(accounts: &[Account]) -> String {
    let used: std::collections::HashSet<u32> = accounts
        .iter()
        .filter_map(|a| default_label_number(a.user_label.as_deref()?))
        .collect();
    let n = (1..).find(|n| !used.contains(n)).unwrap_or(1);
    format!("Account {n}")
}

/// Sort in place by `position`, then `created_at` (id tiebreak): a stable order
/// that never reshuffles on switch, so the switcher list and Alt+N stay put (#166).
pub fn sort_stable(accounts: &mut [Account]) {
    accounts.sort_by(|a, b| {
        a.position
            .cmp(&b.position)
            .then_with(|| a.created_at.cmp(&b.created_at))
            .then_with(|| a.id.cmp(&b.id))
    });
}

/// Sort, then renumber `position` as 0..n. Legacy files (all positions 0) thus
/// keep their `created_at` order.
fn normalize_positions(accounts: &mut [Account]) {
    sort_stable(accounts);
    for (i, a) in accounts.iter_mut().enumerate() {
        a.position = i as i64;
    }
}

/// Position that places a new account after every existing one.
fn next_position(accounts: &[Account]) -> i64 {
    accounts.iter().map(|a| a.position + 1).max().unwrap_or(0)
}

/// Move `id` by `delta` (-1 up, +1 down) in display order. False when it is
/// unknown or already at that end.
fn move_account(accounts: &mut [Account], id: &str, delta: isize) -> bool {
    normalize_positions(accounts);
    let Some(i) = accounts.iter().position(|a| a.id == id) else {
        return false;
    };
    let Some(j) = i.checked_add_signed(delta).filter(|&j| j < accounts.len()) else {
        return false;
    };
    let (pi, pj) = (accounts[i].position, accounts[j].position);
    accounts[i].position = pj;
    accounts[j].position = pi;
    true
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
            static SIGNALS: std::sync::LazyLock<Vec<Signal>> =
                std::sync::LazyLock::new(|| vec![Signal::builder("accounts-changed").build()]);
            SIGNALS.as_ref()
        }
    }
}

glib::wrapper! {
    /// Owns and persists the account list; emits `accounts-changed` after every mutation.
    pub struct AccountManager(ObjectSubclass<imp::AccountManager>);
}

impl AccountManager {
    /// Persist the current list to `accounts.json`. A no-op when the file could
    /// be neither read nor backed up at startup ([`LoadOutcome::RecoveredUnsaved`],
    /// already logged): it is never saved over.
    pub fn save(&self) -> Result<(), AccountsError> {
        if SAVE_BLOCKED.load(std::sync::atomic::Ordering::Relaxed) {
            return Ok(());
        }
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
        let mut account = Account::new();
        {
            let mut list = self.imp().accounts.borrow_mut();
            account.position = next_position(&list);
            list.push(account.clone());
        }
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
            account.position = next_position(&list);
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

    /// Remove the account with `id`. Tombstones its session dir first, so the
    /// startup sweep deletes it if the caller's delayed wipe never runs.
    pub fn remove(&self, id: &str) {
        if !cfg!(test) {
            write_tombstone(&sessions_root(), id);
        }
        self.imp().accounts.borrow_mut().retain(|a| a.id != id);
        self.save_logged();
        self.emit_changed();
    }

    /// Swap `id` with its predecessor in list order; no-op (no save, no
    /// signal) at the top.
    pub fn move_up(&self, id: &str) {
        self.move_by(id, -1);
    }

    /// Swap `id` with its successor in list order; no-op at the bottom.
    pub fn move_down(&self, id: &str) {
        self.move_by(id, 1);
    }

    fn move_by(&self, id: &str, delta: isize) {
        let moved = move_account(&mut self.imp().accounts.borrow_mut(), id, delta);
        if moved {
            self.save_logged();
            self.emit_changed();
        }
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

    /// Stable (user-ordered) clone of the account list — the order the
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

    /// Drop the stored avatar for `id`; no-op (no signal, no save) when none is set.
    pub fn clear_avatar(&self, id: &str) {
        let changed = {
            let mut list = self.imp().accounts.borrow_mut();
            list.iter_mut()
                .find(|a| a.id == id)
                .is_some_and(|a| a.avatar_png.take().is_some())
        };
        if changed {
            self.save_logged();
            self.emit_changed();
        }
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

/// Set when accounts.json could be neither read nor backed up: never save over it.
static SAVE_BLOCKED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The process-wide [`AccountManager`], loading `accounts.json` on first use.
/// A missing, unreadable or corrupt file never starts the app empty over live
/// sessions: a corrupt file is moved aside and the list rebuilt from the session
/// dirs ([`load_or_recover`]). Session dirs are deleted only when tombstoned by
/// an explicit removal; unlisted untombstoned dirs are adopted. The first call
/// happens before any browser exists.
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
                let removed = sweep_removed_sessions(&sessions);
                let (mut list, outcome) = load_or_recover(&accounts_file(), &sessions);
                if outcome == LoadOutcome::RecoveredUnsaved {
                    SAVE_BLOCKED.store(true, std::sync::atomic::Ordering::Relaxed);
                }
                // A removal whose save failed: the tombstone is the user's intent.
                let before = list.len();
                list.retain(|a| !removed.contains(&a.id));
                let mut dirty = list.len() != before;
                if outcome == LoadOutcome::Loaded {
                    let adopted = adopt_unlisted_sessions(&sessions, &mut list);
                    if !adopted.is_empty() {
                        log::warn!(
                            "accounts: adopted {} unlisted session dir(s): {}",
                            adopted.len(),
                            adopted.join(", ")
                        );
                        dirty = true;
                    }
                }
                m.imp().accounts.replace(list);
                if dirty {
                    m.save_logged();
                }
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
    /// Unread chats as reported by the page title ("(N) WhatsApp").
    pub unread: u32,
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

/// Set `account_id`'s unread-chat count (from the page title). Emits
/// `accounts-changed` only when the count actually changes.
pub fn set_unread_count(account_id: &str, count: u32) -> bool {
    mutate_runtime(account_id, |r| {
        let changed = r.unread != count;
        r.unread = count;
        changed
    })
}

/// Sum of the unread counts of every existing, unmuted account: the tray total.
/// Muted accounts still show their own count in the switcher.
pub fn unread_total_unmuted() -> u32 {
    let mgr = manager();
    mgr.get_accounts_sorted()
        .iter()
        .filter(|a| !a.muted)
        .map(|a| runtime_state(&a.id).unread)
        .fold(0u32, u32::saturating_add)
}

/// Badge text for an unread count, capped at "99+".
pub fn unread_badge_text(count: u32) -> String {
    if count > 99 {
        "99+".to_owned()
    } else {
        count.to_string()
    }
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

    fn ids(list: &[Account]) -> Vec<String> {
        list.iter().map(|a| a.id.clone()).collect()
    }

    fn three() -> Vec<Account> {
        let mut v = vec![acct_with(100), acct_with(200), acct_with(300)];
        normalize_positions(&mut v);
        v
    }

    #[test]
    fn legacy_file_without_positions_keeps_created_order() {
        let dir = std::env::temp_dir().join(format!("karere-acct-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("accounts.json");
        let doc = r#"[
        {"id":"c","created_at":300,"last_used_at":3},
        {"id":"a","created_at":100,"last_used_at":1},
        {"id":"b","created_at":200,"last_used_at":2}]"#;
        std::fs::write(&path, doc).unwrap();
        let list = load_from(&path).unwrap();
        assert_eq!(ids(&list), vec!["a", "b", "c"]);
        assert_eq!(list.iter().map(|a| a.position).collect::<Vec<_>>(), vec![0, 1, 2]);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn move_up_and_down_swap_neighbours() {
        let mut v = three();
        let (a, b, c) = (v[0].id.clone(), v[1].id.clone(), v[2].id.clone());
        assert!(move_account(&mut v, &c, -1));
        sort_stable(&mut v);
        assert_eq!(ids(&v), vec![a.clone(), c.clone(), b.clone()]);
        assert!(move_account(&mut v, &a, 1));
        sort_stable(&mut v);
        assert_eq!(ids(&v), vec![c, a, b]);
    }

    #[test]
    fn move_at_the_ends_is_a_noop() {
        let mut v = three();
        let (a, c) = (v[0].id.clone(), v[2].id.clone());
        let before = ids(&v);
        assert!(!move_account(&mut v, &a, -1));
        assert!(!move_account(&mut v, &c, 1));
        assert!(!move_account(&mut v, "missing", 1));
        sort_stable(&mut v);
        assert_eq!(ids(&v), before);
    }

    #[test]
    fn order_survives_save_and_load() {
        let mut v = three();
        let (a, b, c) = (v[0].id.clone(), v[1].id.clone(), v[2].id.clone());
        move_account(&mut v, &c, -1);
        let dir = std::env::temp_dir().join(format!("karere-acct-{}", uuid::Uuid::new_v4()));
        let path = dir.join("accounts.json");
        save_to(&path, &v).unwrap();
        assert_eq!(ids(&load_from(&path).unwrap()), vec![a, c, b]);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn new_accounts_go_last() {
        let v = three();
        assert_eq!(next_position(&v), 3);
        assert_eq!(next_position(&[]), 0);
    }

    #[test]
    fn load_missing_file_is_empty() {
        let dir = std::env::temp_dir().join(format!("karere-acct-{}", uuid::Uuid::new_v4()));
        let path = dir.join("accounts.json");
        assert!(load_from(&path).unwrap().is_empty());
    }

    #[test]
    fn load_accepts_legacy_keys_from_older_versions() {
        let dir = std::env::temp_dir().join(format!("karere-acct-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("accounts.json");
        let doc = r#"[{"id":"abc","wid":null,"pushname":"Ada","user_label":null,
            "avatar_png":null,"avatar_url":"https://example.invalid/a.png",
            "created_at":1,"last_used_at":2,"is_active":true,"has_session":true,
            "zoom_level":1.25,"permissions":{"overrides":{"https://web.whatsapp.com":{"1":"Allow"}}},
            "muted":false}]"#;
        std::fs::write(&path, doc).unwrap();
        let list = load_from(&path).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, "abc");
        assert_eq!(list[0].pushname.as_deref(), Some("Ada"));
        assert_eq!(list[0].zoom_level, 1.25);
        std::fs::remove_dir_all(&dir).ok();
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
    fn unlisted_session_without_tombstone_is_adopted_not_deleted() {
        let (dir, path, sessions) = temp_store();
        let kept = Account::new();
        save_to(&path, std::slice::from_ref(&kept)).unwrap();
        let orphan = Account::new().id;
        std::fs::create_dir(sessions.join(&kept.id)).unwrap();
        std::fs::create_dir(sessions.join(&orphan)).unwrap();
        std::fs::create_dir(sessions.join("Default")).unwrap();
        std::fs::create_dir(sessions.join(orphan.to_uppercase())).unwrap();

        assert!(sweep_removed_sessions(&sessions).is_empty());
        let (mut list, outcome) = load_or_recover(&path, &sessions);
        assert_eq!(outcome, LoadOutcome::Loaded);
        assert_eq!(list, vec![kept.clone()]);
        assert_eq!(adopt_unlisted_sessions(&sessions, &mut list), vec![orphan.clone()]);
        let mut ids: Vec<_> = list.iter().map(|a| a.id.clone()).collect();
        ids.sort();
        let mut want = vec![kept.id.clone(), orphan.clone()];
        want.sort();
        assert_eq!(ids, want);
        for d in [kept.id.clone(), orphan.clone(), "Default".to_owned(), orphan.to_uppercase()] {
            assert!(sessions.join(&d).is_dir());
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn tombstoned_session_is_deleted_and_not_adopted() {
        let (dir, _path, sessions) = temp_store();
        let gone = Account::new().id;
        let live = Account::new().id;
        std::fs::create_dir(sessions.join(&gone)).unwrap();
        std::fs::create_dir(sessions.join(&live)).unwrap();
        write_tombstone(&sessions, &gone);
        assert!(sessions.join(&gone).join(REMOVED_MARKER).exists());

        // Recovery never resurrects a tombstoned dir, even before the sweep.
        let ids: Vec<_> = recover_from_sessions(&sessions).into_iter().map(|a| a.id).collect();
        assert_eq!(ids, vec![live.clone()]);

        assert_eq!(sweep_removed_sessions(&sessions), vec![gone.clone()]);
        assert!(!sessions.join(&gone).exists());
        assert!(sessions.join(&live).is_dir());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn sweep_leaves_non_uuid_names_and_symlinks_alone() {
        let (dir, _path, sessions) = temp_store();
        // Tombstones inside dirs that are not ours must not trigger a delete.
        let other = sessions.join("Default");
        std::fs::create_dir(&other).unwrap();
        std::fs::write(other.join(REMOVED_MARKER), b"").unwrap();
        let upper = Account::new().id.to_uppercase();
        std::fs::create_dir(sessions.join(&upper)).unwrap();
        std::fs::write(sessions.join(&upper).join(REMOVED_MARKER), b"").unwrap();
        // A uuid-named symlink to a tombstoned dir: neither followed nor adopted.
        let target = dir.join("elsewhere");
        std::fs::create_dir(&target).unwrap();
        std::fs::write(target.join(REMOVED_MARKER), b"").unwrap();
        let link = Account::new().id;
        std::os::unix::fs::symlink(&target, sessions.join(&link)).unwrap();
        let link2 = Account::new().id;
        let target2 = dir.join("elsewhere2");
        std::fs::create_dir(&target2).unwrap();
        std::os::unix::fs::symlink(&target2, sessions.join(&link2)).unwrap();

        assert!(sweep_removed_sessions(&sessions).is_empty());
        assert!(other.is_dir());
        assert!(sessions.join(&upper).is_dir());
        assert!(target.join(REMOVED_MARKER).exists());
        assert!(sessions.join(&link).symlink_metadata().unwrap().file_type().is_symlink());
        let mut list = Vec::new();
        assert!(adopt_unlisted_sessions(&sessions, &mut list).is_empty());
        // write_tombstone refuses symlinked and non-uuid targets too.
        write_tombstone(&sessions, &link2);
        assert!(!target2.join(REMOVED_MARKER).exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn unreadable_file_is_moved_aside_before_saving_recovered_list() {
        let (dir, path, sessions) = temp_store();
        // A directory in place of the file: exists, but reading it is an I/O error.
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("marker"), b"keep").unwrap();
        let a = Account::new().id;
        std::fs::create_dir(sessions.join(&a)).unwrap();

        let (list, outcome) = load_or_recover(&path, &sessions);
        assert_eq!(outcome, LoadOutcome::Recovered);
        assert_eq!(list.len(), 1);
        let aside: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().starts_with("accounts.json.corrupt-"))
            .collect();
        assert_eq!(aside.len(), 1);
        assert_eq!(std::fs::read(aside[0].path().join("marker")).unwrap(), b"keep");
        assert_eq!(load_from(&path).unwrap(), list);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn unreadable_file_that_cannot_be_backed_up_is_not_overwritten() {
        use std::os::unix::fs::PermissionsExt;
        let (dir, path, sessions) = temp_store();
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("marker"), b"keep").unwrap();
        let a = Account::new().id;
        std::fs::create_dir(sessions.join(&a)).unwrap();
        // Read-only parent: the rename aside fails, and copying a dir fails too.
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o500)).unwrap();
        let writable = std::fs::write(dir.join("probe"), b"").is_ok();
        if !writable {
            let (list, outcome) = load_or_recover(&path, &sessions);
            assert_eq!(outcome, LoadOutcome::RecoveredUnsaved);
            assert_eq!(list.len(), 1);
            assert!(path.is_dir());
            assert_eq!(std::fs::read(path.join("marker")).unwrap(), b"keep");
        }
        // Running as root ignores the mode; nothing to check in that case.
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
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

    #[test]
    fn default_label_pattern_is_exact() {
        assert_eq!(default_label_number("Account 7"), Some(7));
        assert_eq!(default_label_number(" Account 12 "), Some(12));
        assert_eq!(default_label_number("Account"), None);
        assert_eq!(default_label_number("Account "), None);
        assert_eq!(default_label_number("Account 1b"), None);
        assert_eq!(default_label_number("Account -1"), None);
        assert_eq!(default_label_number("My Account 1"), None);
        assert_eq!(default_label_number("Work"), None);
    }

    #[test]
    fn default_label_template_fills_number() {
        assert_eq!(fill_default_label("Account {n}", 3), "Account 3");
        assert_eq!(fill_default_label("Conta n.º {n}", 12), "Conta n.º 12");
    }

    #[test]
    fn display_label_translates_defaults_only() {
        let mut a = Account::new();
        assert_eq!(display_label(&a), None);
        a.user_label = Some("  ".into());
        assert_eq!(display_label(&a), None);
        a.user_label = Some("Account 4".into());
        assert_eq!(display_label(&a).as_deref(), Some("Account 4"));
        a.user_label = Some("Work".into());
        assert_eq!(display_label(&a).as_deref(), Some("Work"));
    }

    #[test]
    fn unread_badge_caps_at_99() {
        assert_eq!(unread_badge_text(7), "7");
        assert_eq!(unread_badge_text(99), "99");
        assert_eq!(unread_badge_text(100), "99+");
    }
}
