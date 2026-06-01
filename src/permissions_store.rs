//! Durable per-origin permission decisions backed by a GSettings dict.
//!
//! Decisions are keyed by `(origin, single permission-mask bit)` so partial
//! overlaps work: a stored microphone=Allow plus a later mic+camera request is
//! recognised as mixed and re-prompted. Every concrete Allow/Deny is persisted
//! automatically (browser-style), so a granted permission is never re-asked.
//! The store is deliberately decoupled from the permission handler so M20 can
//! swap the backend to per-account JSON.

use std::collections::HashMap;

use gtk::gio;
use gtk::glib::prelude::*;
use gtk::prelude::*;

use crate::application::APP_ID;

const KEY: &str = "permission-decisions";

/// Stored per-bit state. Matches the integer values persisted in GSettings
/// (the schema value type is `i`, so these are stored as `i32`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum State {
    Ask = 0,
    Allow = 1,
    Deny = 2,
}

impl State {
    /// Decode a persisted `i32` (GSettings / account JSON) into a `State`.
    pub fn from_i32(v: i32) -> State {
        match v {
            1 => State::Allow,
            2 => State::Deny,
            _ => State::Ask,
        }
    }
}

/// Resolution of a whole requested mask against the stored per-bit states.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Decision {
    Allow,
    Deny,
    /// Every requested bit is unset — show a fresh prompt.
    AskAll,
    /// A mix of stored and unset (or conflicting) bits — re-prompt for the
    /// full mask, ignoring the stored bits.
    AskMixed,
}

/// Matches the GSettings `a{sa{ui}}` shape: origin → (bit → state).
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

/// Iterate the individual set bits of `mask`.
fn bits(mask: u32) -> impl Iterator<Item = u32> {
    (0..32).map(|i| 1u32 << i).filter(move |b| mask & b != 0)
}

/// Reduce the per-bit states for a requested mask into a single `Decision`.
/// Pure (no GSettings) so the reduction is unit-testable.
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
        // No bits at all, or any mix/conflict.
        (false, false, false) => Decision::AskAll,
        _ => Decision::AskMixed,
    }
}

/// Resolve the stored decision for `origin` and the requested permission `mask`.
pub fn get(origin: &str, mask: u32) -> Decision {
    let map = load();
    resolve(map.get(origin), mask)
}

/// Every stored decision flattened to `(origin, single-bit, state)` triples.
/// Only concrete Allow/Deny are ever persisted, so each entry is one of those.
/// Used by the Preferences Privacy page to render the registry (M22 2.8).
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

/// Remove a single `(origin, bit)` decision. Drops the origin row when its last
/// bit is removed so the dict never retains empty entries (M22 2.8).
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

/// Empty the entire permission registry (Privacy page Clear-all, M22 2.8).
pub fn clear() {
    save(&Store::new());
}

/// Persist the user's `decision` for every bit in `mask`. Allow/Deny are stored
/// (so the prompt never re-fires for that origin+mask); Ask states are not
/// written, keeping the dict free of empty rows.
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

    /// CEF's notifications permission bit, mirrored so the store tests don't
    /// pull in the cef crate. Value matches `CEF_PERMISSION_TYPE_NOTIFICATIONS`.
    const NOTIFICATIONS: u32 = 1 << 5;

    /// M14 1.2: on first visit a notifications request must surface a prompt,
    /// never auto-allow or auto-deny. An origin with no stored notifications
    /// decision resolves to `AskAll` (the dialog branch).
    #[test]
    fn notifications_first_visit_prompts() {
        assert_eq!(resolve(None, NOTIFICATIONS), Decision::AskAll);
        let empty = map(&[]);
        assert_eq!(resolve(Some(&empty), NOTIFICATIONS), Decision::AskAll);
    }

    /// M14 1.3: decisions are keyed per origin in the reduction layer — a stored
    /// decision is consulted via the per-origin inner map, so an origin without
    /// its own entry still prompts even when another origin allowed.
    #[test]
    fn notifications_decision_is_per_origin() {
        // The remembered origin allows; resolving its inner map yields Allow.
        let allowed = map(&[(NOTIFICATIONS, State::Allow)]);
        assert_eq!(resolve(Some(&allowed), NOTIFICATIONS), Decision::Allow);
        // A different origin (no inner map) is unaffected and still prompts.
        assert_eq!(resolve(None, NOTIFICATIONS), Decision::AskAll);
    }
}
