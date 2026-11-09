//! Screen-reader bridge for the page (#204).
//!
//! CEF renders offscreen, so Chromium never publishes the page to AT-SPI and
//! Orca sees only the GTK shell. When a screen reader is running (or the
//! `screen-reader-opts` preference is on) CEF streams its accessibility tree
//! here. We keep each node's role, name and value, and when page focus moves
//! we queue a spoken description; the widget tick hands it to
//! `gtk::Accessible::announce`. Keyboard navigation in WhatsApp (Tab, arrow
//! keys through chats and messages) is then read aloud.
// ponytail: focus announcements only. Browse mode / flat review need a full
// GtkAccessible mirror of the tree; build that if focus reading falls short.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use cef::{
    self, AccessibilityHandler, CefString, CefStringList, ImplAccessibilityHandler,
    ImplDictionaryValue, ImplListValue, ImplValue, Value, ValueType, WrapAccessibilityHandler,
    rc::Rc, wrap_accessibility_handler,
};
use parking_lot::Mutex;
use serde_json::{Map, Value as Json};

use super::SharedRef;

/// Whether to turn on CEF's accessibility tree for new browsers. Building the
/// tree costs CPU on a page as large as WhatsApp, so it's only on when a screen
/// reader is running or the user asked for it.
pub fn wanted() -> bool {
    static WANTED: OnceLock<bool> = OnceLock::new();
    *WANTED.get_or_init(|| {
        use gtk::gio;
        use gtk::prelude::SettingsExt;
        let opted_in = gio::Settings::new(crate::application::APP_ID).boolean("screen-reader-opts");
        let running = screen_reader_running();
        log::info!(
            "a11y: page tree {} (pref={opted_in}, screen reader={running})",
            if opted_in || running { "on" } else { "off" }
        );
        opted_in || running
    })
}

/// `org.a11y.Status.ScreenReaderEnabled` from the a11y bus launcher. Flatpak
/// allows this property read by default.
fn screen_reader_running() -> bool {
    use gtk::gio;
    use gtk::glib::{self, prelude::*};
    let Ok(bus) = gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE) else {
        return false;
    };
    bus.call_sync(
        Some("org.a11y.Bus"),
        "/org/a11y/bus",
        "org.freedesktop.DBus.Properties",
        "Get",
        Some(&("org.a11y.Status", "ScreenReaderEnabled").to_variant()),
        Some(glib::VariantTy::new("(v)").unwrap()),
        gio::DBusCallFlags::NONE,
        500,
        gio::Cancellable::NONE,
    )
    .ok()
    .and_then(|v| v.child_value(0).as_variant())
    .and_then(|v| v.get::<bool>())
    .unwrap_or(false)
}

#[derive(Default, Clone)]
struct AxNode {
    role: String,
    name: String,
    value: String,
}

/// The parts of one browser's accessibility tree needed to describe focus.
#[derive(Default)]
pub struct AxTree {
    nodes: HashMap<i64, AxNode>,
    focus: Option<i64>,
}

impl AxTree {
    /// Apply one `on_accessibility_tree_change` payload (CEF's JSON-shaped
    /// dictionary: `updates[]` and `events[]`, each update carrying `nodes[]`
    /// and optional `tree_data.focus_id`). Returns what to announce when focus
    /// landed on a different, describable node.
    pub fn apply(&mut self, msg: &Json) -> Option<String> {
        let mut focus = None;
        let updates = msg["updates"].as_array().into_iter().flatten();
        let event_updates = msg["events"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|e| &e["update"]);
        for update in updates.chain(event_updates) {
            for n in update["nodes"].as_array().into_iter().flatten() {
                let Some(id) = n["id"].as_i64() else { continue };
                let attr = |k: &str| n["attributes"][k].as_str().unwrap_or("").trim().to_owned();
                self.nodes.insert(
                    id,
                    AxNode {
                        role: n["role"].as_str().unwrap_or("").to_owned(),
                        name: attr("name"),
                        value: attr("value"),
                    },
                );
            }
            if let Some(id) = update["tree_data"]["focus_id"].as_i64() {
                focus = Some(id);
            }
        }
        for e in msg["events"].as_array().into_iter().flatten() {
            if e["event_type"] == "focus"
                && let Some(id) = e["id"].as_i64()
            {
                focus = Some(id);
            }
        }
        let id = focus?;
        if self.focus == Some(id) {
            return None;
        }
        self.focus = Some(id);
        self.nodes.get(&id).and_then(describe)
    }
}

/// Spoken text for a focused node: name, role, then any field value.
fn describe(n: &AxNode) -> Option<String> {
    let role = match n.role.as_str() {
        // The document itself takes focus on every click outside a control.
        "rootWebArea" | "webArea" | "genericContainer" | "none" => return None,
        "button" | "popUpButton" | "toggleButton" => "button",
        "link" => "link",
        "textField" | "searchBox" | "textFieldWithComboBox" => "edit text",
        "checkBox" => "check box",
        "radioButton" => "radio button",
        "switch" => "switch",
        "menuItem" | "menuItemCheckBox" | "menuItemRadio" => "menu item",
        "tab" => "tab",
        "heading" => "heading",
        "image" => "image",
        "dialog" | "alertDialog" => "dialog",
        _ => "",
    };
    let parts: Vec<&str> = [n.name.as_str(), role, n.value.as_str()]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect();
    // A bare role with no name says nothing useful.
    (!n.name.is_empty() || !n.value.is_empty()).then(|| parts.join(", "))
}

/// Deep-copy a CEF `Value` into JSON. Depth-capped: the payload is a tree of
/// nodes, never more than a few levels of containers.
fn to_json(v: &Value, depth: u8) -> Json {
    if depth > 16 {
        return Json::Null;
    }
    let t = v.get_type();
    if t == ValueType::BOOL {
        Json::Bool(v.bool() != 0)
    } else if t == ValueType::INT {
        Json::from(v.int())
    } else if t == ValueType::DOUBLE {
        Json::from(v.double())
    } else if t == ValueType::STRING {
        Json::String(CefString::from(&v.string()).to_string())
    } else if t == ValueType::DICTIONARY {
        let Some(d) = v.dictionary() else {
            return Json::Null;
        };
        let mut keys = CefStringList::new();
        d.keys(Some(&mut keys));
        let mut map = Map::new();
        for k in keys {
            if let Some(child) = d.value(Some(&CefString::from(k.as_str()))) {
                map.insert(k, to_json(&child, depth + 1));
            }
        }
        Json::Object(map)
    } else if t == ValueType::LIST {
        let Some(l) = v.list() else { return Json::Null };
        Json::Array(
            (0..l.size())
                .filter_map(|i| l.value(i))
                .map(|child| to_json(&child, depth + 1))
                .collect(),
        )
    } else {
        Json::Null
    }
}

#[derive(Clone)]
pub struct ShellAccessibilityHandler {
    shared: SharedRef,
    tree: Arc<Mutex<AxTree>>,
}

impl ShellAccessibilityHandler {
    pub fn new(shared: SharedRef) -> Self {
        Self {
            shared,
            tree: Arc::default(),
        }
    }
}

wrap_accessibility_handler! {
    pub struct ShellAccessibilityHandlerBuilder {
        handler: ShellAccessibilityHandler,
    }

    impl AccessibilityHandler {
        fn on_accessibility_tree_change(&self, value: Option<&mut Value>) {
            let Some(value) = value else { return };
            let json = to_json(value, 0);
            log::trace!("a11y tree change: {json}");
            if let Some(text) = self.handler.tree.lock().apply(&json) {
                log::trace!("a11y: announce {text:?}");
                self.handler.shared.lock().a11y_announce = Some(text);
            }
        }
    }
}

impl ShellAccessibilityHandlerBuilder {
    pub fn build(handler: ShellAccessibilityHandler) -> AccessibilityHandler {
        Self::new(handler)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn node(id: i64, role: &str, name: &str) -> Json {
        json!({ "id": id, "role": role, "attributes": { "name": name } })
    }

    #[test]
    fn focus_event_announces_name_and_role() {
        let mut t = AxTree::default();
        let msg = json!({
            "updates": [{ "nodes": [node(1, "rootWebArea", "WhatsApp"), node(7, "button", "New chat")] }],
            "events": [{ "event_type": "focus", "id": 7 }]
        });
        assert_eq!(t.apply(&msg).as_deref(), Some("New chat, button"));
        // Same focus again: silent.
        assert_eq!(t.apply(&msg), None);
    }

    #[test]
    fn tree_data_focus_and_field_value() {
        let mut t = AxTree::default();
        let msg = json!({
            "events": [{ "update": {
                "nodes": [{ "id": 3, "role": "textField",
                            "attributes": { "name": "Type a message", "value": "hi" } }],
                "tree_data": { "focus_id": 3 }
            }}]
        });
        assert_eq!(
            t.apply(&msg).as_deref(),
            Some("Type a message, edit text, hi")
        );
    }

    #[test]
    fn document_and_unnamed_nodes_stay_silent() {
        let mut t = AxTree::default();
        let doc = json!({ "updates": [{ "nodes": [node(1, "rootWebArea", "WhatsApp")],
                                        "tree_data": { "focus_id": 1 } }] });
        assert_eq!(t.apply(&doc), None);
        let bare = json!({ "updates": [{ "nodes": [node(2, "button", "")] }],
                           "events": [{ "event_type": "focus", "id": 2 }] });
        assert_eq!(t.apply(&bare), None);
    }

    #[test]
    fn row_reads_its_name() {
        let mut t = AxTree::default();
        let msg = json!({ "updates": [{ "nodes": [node(9, "row", "Ana, Lunch at 1?, 12:30")] }],
                          "events": [{ "event_type": "focus", "id": 9 }] });
        assert_eq!(t.apply(&msg).as_deref(), Some("Ana, Lunch at 1?, 12:30"));
    }
}
