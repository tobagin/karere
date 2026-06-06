//! Durable per-origin permission decisions in a GSettings dict.
//! Permissions are app/origin-scoped, NOT per-account (asked once per computer).
//! Keyed by `(origin, single mask bit)` so partial overlaps re-prompt.

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

/// GSettings `a{sa{ui}}`: origin → (bit → state).
type Store = HashMap<String, HashMap<u32, i32>>;

fn settings() -> gio::Settings {
    gio::Settings::new(APP_ID)
}

fn load() -> Store {
    settings().value(KEY).get::<Store>().unwrap_or_default()
}

fn save(map: &Store) {
    if let Err(err) = settings().set_value(KEY, &map.to_variant()) {
        log::warn!("permission-decisions write failed: {err}");
    }
}

fn bits(mask: u32) -> impl Iterator<Item = u32> {
    (0..32).map(|i| 1u32 << i).filter(move |b| mask & b != 0)
}

fn resolve(inner: Option<&HashMap<u32, i32>>, mask: u32) -> Decision {
    let (mut allow, mut deny, mut ask) = (false, false, false);
    for b in bits(mask) {
        let state = inner
            .and_then(|m| m.get(&b))
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
pub fn get(origin: &str, mask: u32) -> Decision {
    let map = load();
    resolve(map.get(origin), mask)
}

/// Stored decisions flattened to `(origin, bit, state)` triples.
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

/// Remove a single `(origin, bit)` decision, dropping the origin when empty.
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
pub fn set(origin: &str, mask: u32, decision: Decision) {
    let state = match decision {
        Decision::Allow => State::Allow as i32,
        Decision::Deny => State::Deny as i32,
        Decision::AskAll | Decision::AskMixed => return,
    };
    let mut map = load();
    let inner = map.entry(origin.to_string()).or_default();
    for b in bits(mask) {
        inner.insert(b, state);
    }
    save(&map);
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIC: u32 = 1 << 0;
    const CAM: u32 = 1 << 1;

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
}
