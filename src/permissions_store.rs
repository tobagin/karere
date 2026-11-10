//! Durable per-origin permission decisions in a GSettings dict.
//! Permissions are app/origin-scoped, NOT per-account (asked once per computer).
//! Keyed by `(origin, single mask bit)` so partial overlaps re-prompt.
//!
//! CEF has two unrelated bit enums (media-access types and permission-prompt
//! types) whose values overlap (1, 2, 4, 8). Stored keys therefore carry a
//! kind flag in a high bit: `MEDIA_FLAG | bit` or `PROMPT_FLAG | bit`. Keys
//! with neither flag are legacy (written before the split) and are migrated by
//! `migrate` on load.

use std::collections::HashMap;

use gtk::gio;
use gtk::glib::prelude::*;
use gtk::prelude::*;

use crate::application::APP_ID;

const KEY: &str = "permission-decisions";

/// Stored per-bit state; integer values match the GSettings `i` schema type.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum State {
    Ask = 0,
    Allow = 1,
    Deny = 2,
}

impl State {
    pub fn from_i32(v: i32) -> State {
        match v {
            1 => State::Allow,
            2 => State::Deny,
            _ => State::Ask,
        }
    }
}

/// Resolution of a requested mask against the stored per-bit states.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Decision {
    Allow,
    Deny,
    AskAll,
    AskMixed,
}

/// Which CEF request enum a mask belongs to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// `cef_media_access_permission_types_t` (mic, camera, screen share).
    Media,
    /// `cef_permission_request_types_t` (notifications, clipboard, ...).
    Prompt,
}

pub const MEDIA_FLAG: u32 = 1 << 31;
pub const PROMPT_FLAG: u32 = 1 << 30;
/// Bits that carry the request value itself (prompt types use bits 0..=28).
const VALUE_MASK: u32 = (1 << 29) - 1;

/// Stored keys for each bit of `mask` under `kind`.
fn keys(kind: Kind, mask: u32) -> Vec<u32> {
    let flag = match kind {
        Kind::Media => MEDIA_FLAG,
        Kind::Prompt => PROMPT_FLAG,
    };
    bits(mask & VALUE_MASK).map(|b| flag | b).collect()
}

/// Upgrade legacy (unflagged) keys. Returns the new store and whether anything
/// changed. Legacy entries came from both paths and cannot be told apart:
/// - bits >= 16 can only be prompt types, so they move to the prompt kind;
/// - bits 1 and 2 were overwhelmingly mic/camera from the media path (the
///   dialog called them microphone/camera), so they move to the media kind;
/// - bits 4 and 8 are ambiguous (screen-share audio/video vs camera stream /
///   surface control), so only a Deny is kept (as media); an Allow is dropped
///   and the user is asked again;
/// - anything else is dropped.
fn migrate(store: Store) -> (Store, bool) {
    let mut changed = false;
    let mut out = Store::new();
    for (origin, inner) in store {
        let mut new_inner = HashMap::new();
        for (key, state) in inner {
            if key & (MEDIA_FLAG | PROMPT_FLAG) != 0 {
                new_inner.insert(key, state);
                continue;
            }
            changed = true;
            let migrated = match key {
                1 | 2 => Some(MEDIA_FLAG | key),
                4 | 8 if state == State::Deny as i32 => Some(MEDIA_FLAG | key),
                k if k >= 16 && k <= VALUE_MASK => Some(PROMPT_FLAG | k),
                _ => None,
            };
            if let Some(nk) = migrated {
                new_inner.entry(nk).or_insert(state);
            }
        }
        if !new_inner.is_empty() {
            out.insert(origin, new_inner);
        }
    }
    (out, changed)
}

/// GSettings `a{sa{ui}}`: origin → (bit → state).
type Store = HashMap<String, HashMap<u32, i32>>;

fn settings() -> gio::Settings {
    gio::Settings::new(APP_ID)
}

fn load() -> Store {
    let raw = settings().value(KEY).get::<Store>().unwrap_or_default();
    let (store, changed) = migrate(raw);
    if changed {
        save(&store);
    }
    store
}

fn save(map: &Store) {
    if let Err(err) = settings().set_value(KEY, &map.to_variant()) {
        log::warn!("permission-decisions write failed: {err}");
    }
}

fn bits(mask: u32) -> impl Iterator<Item = u32> {
    (0..32).map(|i| 1u32 << i).filter(move |b| mask & b != 0)
}

fn resolve(inner: Option<&HashMap<u32, i32>>, keys: &[u32]) -> Decision {
    let (mut allow, mut deny, mut ask) = (false, false, false);
    for b in keys {
        let state = inner
            .and_then(|m| m.get(b))
            .copied()
            .map(State::from_i32)
            .unwrap_or(State::Ask);
        match state {
            State::Allow => allow = true,
            State::Deny => deny = true,
            State::Ask => ask = true,
        }
    }
    match (allow, deny, ask) {
        (true, false, false) => Decision::Allow,
        (false, true, false) => Decision::Deny,
        (false, false, true) => Decision::AskAll,
        (false, false, false) => Decision::AskAll,
        _ => Decision::AskMixed,
    }
}

/// Stored decision for `origin` and the requested permission `mask`.
pub fn get(origin: &str, kind: Kind, mask: u32) -> Decision {
    let map = load();
    resolve(map.get(origin), &keys(kind, mask))
}

/// Stored decisions flattened to `(origin, key, state)` triples. `key` carries
/// the kind flag (`MEDIA_FLAG` / `PROMPT_FLAG`); pass it back to `remove` as is.
pub fn entries() -> Vec<(String, u32, State)> {
    let map = load();
    let mut out = Vec::new();
    for (origin, inner) in &map {
        for (bit, state) in inner {
            out.push((origin.clone(), *bit, State::from_i32(*state)));
        }
    }
    out
}

/// Remove a single `(origin, key)` decision (key as returned by `entries`), dropping the origin when empty.
pub fn remove(origin: &str, bit: u32) {
    let mut map = load();
    if let Some(inner) = map.get_mut(origin) {
        inner.remove(&bit);
        if inner.is_empty() {
            map.remove(origin);
        }
        save(&map);
    }
}

/// Empty the entire permission registry.
pub fn clear() {
    save(&Store::new());
}

/// Persist `decision` for every bit in `mask`. Ask is not written.
pub fn set(origin: &str, kind: Kind, mask: u32, decision: Decision) {
    let state = match decision {
        Decision::Allow => State::Allow as i32,
        Decision::Deny => State::Deny as i32,
        Decision::AskAll | Decision::AskMixed => return,
    };
    let mut map = load();
    let inner = map.entry(origin.to_string()).or_default();
    for k in keys(kind, mask) {
        inner.insert(k, state);
    }
    save(&map);
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIC: u32 = 1 << 0;
    const CAM: u32 = 1 << 1;

    fn resolve(inner: Option<&HashMap<u32, i32>>, mask: u32) -> Decision {
        super::resolve(inner, &bits(mask).collect::<Vec<_>>())
    }

    fn map(pairs: &[(u32, State)]) -> HashMap<u32, i32> {
        pairs.iter().map(|(b, s)| (*b, *s as i32)).collect()
    }

    #[test]
    fn all_allow_is_allow() {
        let inner = map(&[(MIC, State::Allow), (CAM, State::Allow)]);
        assert_eq!(resolve(Some(&inner), MIC | CAM), Decision::Allow);
    }

    #[test]
    fn all_deny_is_deny() {
        let inner = map(&[(MIC, State::Deny), (CAM, State::Deny)]);
        assert_eq!(resolve(Some(&inner), MIC | CAM), Decision::Deny);
    }

    #[test]
    fn allow_and_deny_is_mixed() {
        let inner = map(&[(MIC, State::Allow), (CAM, State::Deny)]);
        assert_eq!(resolve(Some(&inner), MIC | CAM), Decision::AskMixed);
    }

    #[test]
    fn partial_overlap_is_mixed() {
        let inner = map(&[(MIC, State::Allow)]);
        assert_eq!(resolve(Some(&inner), MIC | CAM), Decision::AskMixed);
    }

    #[test]
    fn unstored_is_ask_all() {
        assert_eq!(resolve(None, MIC | CAM), Decision::AskAll);
        let empty = map(&[]);
        assert_eq!(resolve(Some(&empty), MIC), Decision::AskAll);
    }

    #[test]
    fn single_stored_bit_resolves() {
        let inner = map(&[(MIC, State::Allow)]);
        assert_eq!(resolve(Some(&inner), MIC), Decision::Allow);
    }

    const NOTIFICATIONS: u32 = 1 << 5;

    #[test]
    fn notifications_first_visit_prompts() {
        assert_eq!(resolve(None, NOTIFICATIONS), Decision::AskAll);
        let empty = map(&[]);
        assert_eq!(resolve(Some(&empty), NOTIFICATIONS), Decision::AskAll);
    }

    #[test]
    fn notifications_decision_is_per_origin() {
        let allowed = map(&[(NOTIFICATIONS, State::Allow)]);
        assert_eq!(resolve(Some(&allowed), NOTIFICATIONS), Decision::Allow);
        assert_eq!(resolve(None, NOTIFICATIONS), Decision::AskAll);
    }

    #[test]
    fn kinds_do_not_share_keys() {
        // A stored media mic grant (bit 1) must not satisfy a prompt AR_SESSION (bit 1).
        let inner = map(&[(MEDIA_FLAG | 1, State::Allow)]);
        let media = keys(Kind::Media, 1);
        let prompt = keys(Kind::Prompt, 1);
        assert_eq!(super::resolve(Some(&inner), &media), Decision::Allow);
        assert_eq!(super::resolve(Some(&inner), &prompt), Decision::AskAll);
    }

    #[test]
    fn keys_are_flagged_per_bit() {
        assert_eq!(keys(Kind::Media, 3), vec![MEDIA_FLAG | 1, MEDIA_FLAG | 2]);
        assert_eq!(keys(Kind::Prompt, 1 << 15), vec![PROMPT_FLAG | (1 << 15)]);
        assert!(keys(Kind::Prompt, 0).is_empty());
    }

    fn store(pairs: &[(u32, State)]) -> Store {
        let mut s = Store::new();
        s.insert("https://web.whatsapp.com".into(), map(pairs));
        s
    }

    #[test]
    fn migrate_moves_unambiguous_legacy_entries() {
        let (out, changed) = migrate(store(&[
            (1, State::Allow),
            (2, State::Deny),
            (32768, State::Allow),
        ]));
        assert!(changed);
        let inner = &out["https://web.whatsapp.com"];
        assert_eq!(inner[&(MEDIA_FLAG | 1)], State::Allow as i32);
        assert_eq!(inner[&(MEDIA_FLAG | 2)], State::Deny as i32);
        assert_eq!(inner[&(PROMPT_FLAG | 32768)], State::Allow as i32);
        assert_eq!(inner.len(), 3);
    }

    #[test]
    fn migrate_drops_ambiguous_allow_keeps_deny() {
        let (out, _) = migrate(store(&[(4, State::Allow), (8, State::Deny)]));
        let inner = &out["https://web.whatsapp.com"];
        assert!(!inner.contains_key(&(MEDIA_FLAG | 4)));
        assert!(!inner.contains_key(&(PROMPT_FLAG | 4)));
        assert_eq!(inner[&(MEDIA_FLAG | 8)], State::Deny as i32);
    }

    #[test]
    fn migrate_is_idempotent_and_drops_empty_origins() {
        let (once, _) = migrate(store(&[(1, State::Allow), (4, State::Allow)]));
        let (twice, changed) = migrate(once.clone());
        assert!(!changed);
        assert_eq!(once, twice);
        let (gone, _) = migrate(store(&[(4, State::Allow)]));
        assert!(gone.is_empty());
    }
}
