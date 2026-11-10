//! Screen-reader bridge for the page (#204).
//!
//! CEF renders offscreen, so Chromium never publishes the page to AT-SPI and
//! Orca sees only the GTK shell. When a screen reader is running (or the
//! `screen-reader-opts` preference is on) CEF streams its accessibility tree
//! here. The tree backs the page's accessible objects on AT-SPI
//! (`web_view::a11y_export`): the queries below decide which nodes are worth
//! exposing (generic containers are flattened away), with what role and
//! states, where they sit on screen, and which one has focus.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, OnceLock, Weak};

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

/// Node rectangle `(x, y, width, height)` in CEF view (physical) pixels.
pub type AxRect = (f64, f64, f64, f64);

#[derive(Default, Clone)]
struct AxNode {
    role: String,
    name: String,
    value: String,
    description: String,
    placeholder: String,
    children: Vec<i64>,
    /// Chromium `ax::mojom::State` names (`focusable`, `expanded`, ...).
    states: Vec<String>,
    /// Remaining attributes the export maps: `checkedState`, `restriction`,
    /// `selected`, `hierarchicalLevel`.
    checked: Option<String>,
    restriction: Option<String>,
    selected: Option<bool>,
    level: i32,
    /// Bounds relative to `offset_container` (or to the root when unset),
    /// and this node's own scroll offset, which its contents move against.
    location: Option<AxRect>,
    offset_container: Option<i64>,
    scroll: (f64, f64),
}

impl AxNode {
    fn has_state(&self, state: &str) -> bool {
        self.states.iter().any(|s| s == state)
    }
}

/// The parts of one browser's accessibility tree needed to describe focus
/// and to mirror the page on AT-SPI.
#[derive(Default)]
pub struct AxTree {
    nodes: HashMap<i64, AxNode>,
    /// child id → parent id, to tell a re-parented node from a removed one.
    parents: HashMap<i64, i64>,
    focus: Option<i64>,
    root: Option<i64>,
    /// Nodes (re)sent or cleared since the export last synced.
    changed: HashSet<i64>,
    /// The export must rebuild from scratch (new document, or updates were
    /// discarded while this browser was in the background).
    resync: bool,
}

impl AxTree {
    /// Forget the whole tree (main-frame navigation: a new document).
    pub fn reset(&mut self) {
        *self = Self {
            resync: true,
            ..Self::default()
        };
    }

    /// Remove `id` and everything below it.
    fn remove_subtree(&mut self, id: i64) {
        let mut stack = vec![id];
        while let Some(id) = stack.pop() {
            self.parents.remove(&id);
            if self.focus == Some(id) {
                self.focus = None;
            }
            if self.root == Some(id) {
                self.root = None;
            }
            if let Some(n) = self.nodes.remove(&id) {
                stack.extend(
                    n.children
                        .into_iter()
                        .filter(|c| self.parents.get(c) == Some(&id)),
                );
            }
        }
    }

    /// Apply one tree update with Chromium's `AXTreeUpdate` semantics:
    /// `node_id_to_clear` drops that node's descendants first, every node
    /// entry is the node's full state, and a child that left its parent's
    /// `child_ids` (and was not re-parented in the same update) is deleted
    /// along with its subtree.
    fn apply_update(&mut self, update: &Json) {
        if let Some(root) = update["root_id"].as_i64().filter(|&id| id != 0) {
            self.root = Some(root);
        }
        if let Some(clear) = update["node_id_to_clear"].as_i64().filter(|&id| id != 0) {
            self.changed.insert(clear);
            let children = self
                .nodes
                .get_mut(&clear)
                .map(|n| std::mem::take(&mut n.children))
                .unwrap_or_default();
            for c in children {
                if self.parents.get(&c) == Some(&clear) {
                    self.remove_subtree(c);
                }
            }
        }
        let mut dropped = Vec::new();
        for n in update["nodes"].as_array().into_iter().flatten() {
            let Some(id) = n["id"].as_i64() else { continue };
            let attr = |k: &str| n["attributes"][k].as_str().unwrap_or("").trim().to_owned();
            let children: Vec<i64> = n["child_ids"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Json::as_i64)
                .collect();
            if let Some(old) = self.nodes.get(&id) {
                dropped.extend(
                    old.children
                        .iter()
                        .filter(|c| !children.contains(c))
                        .map(|&c| (id, c)),
                );
            }
            for &c in &children {
                self.parents.insert(c, id);
            }
            let a = &n["attributes"];
            let role = n["role"].as_str().unwrap_or("").to_owned();
            if self.root.is_none() && role == "rootWebArea" {
                self.root = Some(id);
            }
            self.changed.insert(id);
            self.nodes.insert(
                id,
                AxNode {
                    role,
                    name: attr("name"),
                    value: attr("value"),
                    description: attr("description"),
                    placeholder: attr("placeholder"),
                    children,
                    states: n["state"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|s| s.as_str().map(str::to_owned))
                        .collect(),
                    checked: a["checkedState"].as_str().map(str::to_owned),
                    restriction: a["restriction"].as_str().map(str::to_owned),
                    selected: a["selected"].as_bool(),
                    level: a["hierarchicalLevel"].as_i64().unwrap_or(0) as i32,
                    location: rect(&n["location"]),
                    offset_container: n["offset_container_id"].as_i64().filter(|&c| c >= 0),
                    scroll: (
                        a["scrollX"].as_f64().unwrap_or(0.0),
                        a["scrollY"].as_f64().unwrap_or(0.0),
                    ),
                },
            );
        }
        for (parent, child) in dropped {
            if self.parents.get(&child) == Some(&parent) {
                self.remove_subtree(child);
            }
        }
    }

    /// Apply one `on_accessibility_tree_change` payload (CEF's JSON-shaped
    /// dictionary: `updates[]` and `events[]`, each update carrying `nodes[]`,
    /// optional `node_id_to_clear` and optional `tree_data.focus_id`).
    pub fn apply(&mut self, msg: &Json) {
        let mut focus = None;
        let updates = msg["updates"].as_array().into_iter().flatten();
        let event_updates = msg["events"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|e| &e["update"]);
        for update in updates.chain(event_updates) {
            self.apply_update(update);
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
        if focus.is_some() {
            self.focus = focus;
        }
    }
}

/// `{x, y, width, height}` → rect; `None` when absent.
fn rect(v: &Json) -> Option<AxRect> {
    Some((
        v["x"].as_f64()?,
        v["y"].as_f64()?,
        v["width"].as_f64()?,
        v["height"].as_f64()?,
    ))
}

/// What the AT-SPI export does with a node when listing its parent.
#[derive(Debug, PartialEq)]
enum Disposition {
    /// Expose it as an accessible object.
    Export,
    /// Skip it but expose its children in its place.
    Flatten,
    /// Skip it and everything below it.
    Drop,
}

/// Roles whose descendants a screen reader never needs as separate objects:
/// the node's name already carries their text (ARIA "children
/// presentational"), or they are editors whose content is the value.
const LEAF_ROLES: &[&str] = &[
    "button",
    "toggleButton",
    "popUpButton",
    "checkBox",
    "radioButton",
    "switch",
    "image",
    "menuItem",
    "menuItemCheckBox",
    "menuItemRadio",
    "menuListOption",
    "tab",
    "listBoxOption",
    "slider",
    "progressIndicator",
    "meter",
    "scrollBar",
    "splitter",
    "textField",
    "searchBox",
    "textFieldWithComboBox",
    "comboBoxSelect",
    "staticText",
];

/// Layout-only roles: exposed only when the page gave them a name.
const TRANSPARENT_ROLES: &[&str] = &[
    "genericContainer",
    "none",
    "group",
    "section",
    "unknown",
    "div",
    "presentational",
    "ignored",
    "figure",
    "application",
];

/// Roles that never carry content of their own.
const DROP_ROLES: &[&str] = &[
    "inlineTextBox",
    "lineBreak",
    "listMarker",
    "iframe",
    "iframePresentational",
];

fn disposition(n: &AxNode, ancestor_name: &str) -> Disposition {
    if n.has_state("invisible") || DROP_ROLES.contains(&n.role.as_str()) {
        return Disposition::Drop;
    }
    if n.role == "staticText" {
        // Text runs repeat their container's name (a heading, a link, a
        // chat row); keep only text nobody else announces.
        return if n.name.is_empty() || ancestor_name.contains(n.name.as_str()) {
            Disposition::Drop
        } else {
            Disposition::Export
        };
    }
    if n.has_state("ignored") || (TRANSPARENT_ROLES.contains(&n.role.as_str()) && n.name.is_empty())
    {
        return Disposition::Flatten;
    }
    Disposition::Export
}

/// GTK role for a Chromium role name. Unknown roles become `Generic`.
pub fn gtk_role(role: &str) -> gtk::AccessibleRole {
    use gtk::AccessibleRole as R;
    match role {
        "rootWebArea" | "webArea" => R::Document,
        "button" => R::Button,
        "toggleButton" => R::ToggleButton,
        "popUpButton"
        | "comboBoxSelect"
        | "comboBoxMenuButton"
        | "comboBoxGrouping"
        | "textFieldWithComboBox" => R::ComboBox,
        "link" => R::Link,
        "heading" => R::Heading,
        "staticText" | "labelText" => R::Label,
        "legend" => R::Legend,
        "paragraph" => R::Paragraph,
        "image" | "svgRoot" | "graphicsDocument" | "canvas" | "video" | "audio" => R::Img,
        "textField" => R::TextBox,
        "searchBox" => R::SearchBox,
        "checkBox" => R::Checkbox,
        "radioButton" => R::Radio,
        "radioGroup" => R::RadioGroup,
        "switch" => R::Switch,
        "slider" => R::Slider,
        "spinButton" => R::SpinButton,
        "progressIndicator" => R::ProgressBar,
        "meter" => R::Meter,
        "scrollBar" => R::Scrollbar,
        "splitter" => R::Separator,
        "list" | "descriptionList" => R::List,
        "listItem" | "term" | "definition" => R::ListItem,
        "listBox" => R::ListBox,
        "listBoxOption" | "menuListOption" => R::Option,
        "menu" | "menuListPopup" => R::Menu,
        "menuBar" => R::MenuBar,
        "menuItem" => R::MenuItem,
        "menuItemCheckBox" => R::MenuItemCheckbox,
        "menuItemRadio" => R::MenuItemRadio,
        "tab" => R::Tab,
        "tabList" => R::TabList,
        "tabPanel" => R::TabPanel,
        "table" | "layoutTable" => R::Table,
        "grid" => R::Grid,
        "treeGrid" => R::TreeGrid,
        "row" | "layoutTableRow" => R::Row,
        "rowGroup" => R::RowGroup,
        "cell" | "layoutTableCell" => R::Cell,
        "gridCell" => R::GridCell,
        "columnHeader" => R::ColumnHeader,
        "rowHeader" => R::RowHeader,
        "tree" => R::Tree,
        "treeItem" => R::TreeItem,
        "dialog" => R::Dialog,
        "alertDialog" => R::AlertDialog,
        "alert" => R::Alert,
        "status" => R::Status,
        "log" => R::Log,
        "timer" => R::Timer,
        "marquee" => R::Marquee,
        "tooltip" => R::Tooltip,
        "toolbar" => R::Toolbar,
        "navigation" => R::Navigation,
        "main" | "banner" | "contentInfo" | "complementary" => R::Landmark,
        "region" => R::Region,
        "form" => R::Form,
        "search" => R::Search,
        "article" => R::Article,
        "blockquote" => R::BlockQuote,
        "note" => R::Note,
        "math" | "mathMLMath" => R::Math,
        "caption" => R::Caption,
        "time" => R::Time,
        "comment" => R::Comment,
        "group" | "figure" | "section" | "application" => R::Group,
        _ => R::Generic,
    }
}

/// Everything the export pushes into one node's GTK accessible.
#[derive(Debug, Clone, PartialEq)]
pub struct AxProps {
    pub role: gtk::AccessibleRole,
    pub label: String,
    pub description: String,
    pub placeholder: String,
    /// Heading level, 0 when not a heading.
    pub level: i32,
    pub checked: Option<gtk::AccessibleTristate>,
    /// `checked` belongs in the pressed state (toggle buttons).
    pub pressed: bool,
    pub selected: Option<bool>,
    pub expanded: Option<bool>,
    pub disabled: bool,
    pub read_only: bool,
    pub required: bool,
    pub multi_line: bool,
    pub focusable: bool,
}

impl AxTree {
    pub fn root(&self) -> Option<i64> {
        self.root
    }

    pub fn focus(&self) -> Option<i64> {
        self.focus
    }

    /// Hand the export what changed since its last sync: whether it must
    /// rebuild, and which nodes were (re)sent or cleared.
    pub fn take_changes(&mut self) -> (bool, HashSet<i64>) {
        (
            std::mem::take(&mut self.resync),
            std::mem::take(&mut self.changed),
        )
    }

    /// Nobody is mirroring this tree right now (background browser): drop
    /// the change log and rebuild whenever it comes back to the foreground.
    pub fn discard_changes(&mut self) {
        self.changed.clear();
        self.resync = true;
    }

    /// Apply an `on_accessibility_location_change` payload: a list of
    /// `{id, new_location: {offset_container_id, bounds}}`.
    pub fn apply_locations(&mut self, msg: &Json) {
        for change in msg.as_array().into_iter().flatten() {
            let Some(n) = change["id"].as_i64().and_then(|id| self.nodes.get_mut(&id)) else {
                continue;
            };
            let loc = &change["new_location"];
            n.location = rect(&loc["bounds"]);
            n.offset_container = loc["offset_container_id"].as_i64().filter(|&c| c >= 0);
        }
    }

    /// Whether `id` is exposed as its own object. The root always is.
    pub fn is_exported(&self, id: i64) -> bool {
        if Some(id) == self.root {
            return true;
        }
        self.nodes
            .get(&id)
            .is_some_and(|n| disposition(n, "") == Disposition::Export)
    }

    /// The exposed children of `id`: its children, with flattened nodes
    /// replaced by their own exposed children, in document order.
    pub fn exported_children(&self, id: i64) -> Vec<i64> {
        let mut out = Vec::new();
        let Some(n) = self.nodes.get(&id) else {
            return out;
        };
        if LEAF_ROLES.contains(&n.role.as_str()) {
            return out;
        }
        self.collect_exported(n, &n.name, &mut out, 0);
        out
    }

    fn collect_exported(&self, n: &AxNode, ancestor_name: &str, out: &mut Vec<i64>, depth: u32) {
        if depth > 256 {
            return;
        }
        for c in &n.children {
            let Some(cn) = self.nodes.get(c) else {
                continue;
            };
            match disposition(cn, ancestor_name) {
                Disposition::Export => out.push(*c),
                Disposition::Flatten => self.collect_exported(cn, ancestor_name, out, depth + 1),
                Disposition::Drop => {}
            }
        }
    }

    /// Nearest exposed strict ancestor of `id`.
    pub fn exported_parent(&self, id: i64) -> Option<i64> {
        let mut cur = id;
        for _ in 0..1024 {
            cur = *self.parents.get(&cur)?;
            if self.is_exported(cur) {
                return Some(cur);
            }
        }
        None
    }

    /// Exposed ancestors of `id` from the root down, ending with `id` itself
    /// or, when `id` is flattened away, its nearest exposed ancestor. Empty
    /// when `id` is not under the root.
    pub fn exported_path(&self, id: i64) -> Vec<i64> {
        let mut path = Vec::new();
        let mut cur = Some(id);
        while let Some(c) = cur {
            if path.len() > 1024 {
                return Vec::new();
            }
            if self.is_exported(c) {
                path.push(c);
            }
            if Some(c) == self.root {
                path.reverse();
                return path;
            }
            cur = self.parents.get(&c).copied();
        }
        Vec::new()
    }

    /// Role, name and states for `id`.
    pub fn props(&self, id: i64) -> Option<AxProps> {
        let n = self.nodes.get(&id)?;
        let role = gtk_role(&n.role);
        // A field's value is what the user typed; say it after any hint.
        let description = [n.value.as_str(), n.description.as_str()]
            .into_iter()
            .filter(|s| !s.is_empty() && *s != n.name)
            .collect::<Vec<_>>()
            .join(", ");
        let expanded = if n.has_state("expanded") {
            Some(true)
        } else if n.has_state("collapsed") {
            Some(false)
        } else {
            None
        };
        Some(AxProps {
            role,
            label: n.name.clone(),
            description,
            placeholder: n.placeholder.clone(),
            level: if role == gtk::AccessibleRole::Heading {
                n.level
            } else {
                0
            },
            checked: n.checked.as_deref().and_then(|c| match c {
                "true" => Some(gtk::AccessibleTristate::True),
                "false" => Some(gtk::AccessibleTristate::False),
                "mixed" => Some(gtk::AccessibleTristate::Mixed),
                _ => None,
            }),
            pressed: role == gtk::AccessibleRole::ToggleButton,
            selected: n.selected,
            expanded,
            disabled: n.restriction.as_deref() == Some("disabled"),
            read_only: n.restriction.as_deref() == Some("readOnly"),
            required: n.has_state("required"),
            multi_line: n.has_state("multiline"),
            focusable: n.has_state("focusable"),
        })
    }

    /// `id`'s rectangle in view pixels, resolved through its offset
    /// containers and their scroll offsets (Chromium's
    /// `AXTree::RelativeToTreeBounds`, without transforms).
    pub fn page_rect(&self, id: i64) -> Option<AxRect> {
        let n = self.nodes.get(&id)?;
        let (mut x, mut y, w, h) = n.location?;
        let mut cur = id;
        let mut node = n;
        for _ in 0..256 {
            let container = node.offset_container.or(self.root).filter(|&c| c != cur);
            let Some(c) = container else { break };
            let Some(cn) = self.nodes.get(&c) else { break };
            if let Some((cx, cy, _, _)) = cn.location {
                x += cx;
                y += cy;
            }
            x -= cn.scroll.0;
            y -= cn.scroll.1;
            cur = c;
            node = cn;
        }
        Some((x, y, w, h))
    }

    /// `id`'s rectangle, or a zero-size one at its nearest located
    /// ancestor's origin when Chromium sent no location for it.
    fn located_rect(&self, id: i64) -> AxRect {
        let mut cur = Some(id);
        for _ in 0..1024 {
            let Some(c) = cur else { break };
            if let Some(r) = self.page_rect(c) {
                return if c == id { r } else { (r.0, r.1, 0.0, 0.0) };
            }
            cur = self.parents.get(&c).copied();
        }
        (0.0, 0.0, 0.0, 0.0)
    }

    /// `id`'s rectangle relative to `parent`'s origin (view pixels). AT-SPI
    /// adds parent offsets back up the chain, so these must telescope.
    pub fn relative_rect(&self, id: i64, parent: Option<i64>) -> AxRect {
        let (x, y, w, h) = self.located_rect(id);
        let (px, py, _, _) = parent.map_or((0.0, 0.0, 0.0, 0.0), |p| self.located_rect(p));
        (x - px, y - py, w, h)
    }
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
    /// CEF id of the browser this handler serves; 0 until [`Self::bind_browser`].
    browser_id: Arc<AtomicI32>,
}

/// Browser id → its tree, so the load handler can reset it on navigation.
/// Weak: a closed browser's tree is dropped with its handler.
fn registry() -> &'static Mutex<HashMap<i32, Weak<Mutex<AxTree>>>> {
    static REGISTRY: OnceLock<Mutex<HashMap<i32, Weak<Mutex<AxTree>>>>> = OnceLock::new();
    REGISTRY.get_or_init(Mutex::default)
}

/// Browser `browser_id`'s accessibility tree, if it has one.
pub fn tree_for(browser_id: i32) -> Option<Arc<Mutex<AxTree>>> {
    registry().lock().get(&browser_id).and_then(Weak::upgrade)
}

/// Clear browser `browser_id`'s accessibility tree (main-frame navigation).
/// No-op for an unknown or unbound browser.
pub fn reset_browser(browser_id: i32) {
    let tree = registry().lock().get(&browser_id).and_then(Weak::upgrade);
    if let Some(tree) = tree {
        tree.lock().reset();
    }
}

impl ShellAccessibilityHandler {
    pub fn new(shared: SharedRef) -> Self {
        Self {
            shared,
            tree: Arc::default(),
            browser_id: Arc::default(),
        }
    }

    /// Tell the handler which browser it serves (the render handler sees the
    /// browser, the accessibility callback does not). Cheap to call repeatedly.
    /// Enables the foreground gate on the AT-SPI mirror and [`reset_browser`].
    pub fn bind_browser(&self, browser_id: i32) {
        if browser_id == 0 || self.browser_id.swap(browser_id, Ordering::Relaxed) == browser_id {
            return;
        }
        let mut reg = registry().lock();
        reg.retain(|_, t| t.strong_count() > 0);
        reg.insert(browser_id, Arc::downgrade(&self.tree));
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
            // The tree is always kept current; only the foreground browser is
            // mirrored (same gate as paints; 0 = unbound or pool
            // not wired yet).
            let mut tree = self.handler.tree.lock();
            tree.apply(&json);
            let mine = self.handler.browser_id.load(Ordering::Relaxed);
            let mut s = self.handler.shared.lock();
            if mine == 0 || s.foreground_browser_id == 0 || mine == s.foreground_browser_id {
                drop(tree);
                s.a11y_tree_dirty = true;
                super::wake_view(s);
            } else {
                tree.discard_changes();
            }
        }

        fn on_accessibility_location_change(&self, value: Option<&mut Value>) {
            // Bounds are read live when an AT asks; nothing to notify.
            let Some(value) = value else { return };
            self.handler.tree.lock().apply_locations(&to_json(value, 0));
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
    fn focus_comes_from_events_and_tree_data() {
        let mut t = AxTree::default();
        t.apply(&json!({
            "updates": [{ "nodes": [node(1, "rootWebArea", "WhatsApp"), node(7, "button", "New chat")] }],
            "events": [{ "event_type": "focus", "id": 7 }]
        }));
        assert_eq!(t.focus(), Some(7));
        t.apply(&json!({ "events": [{ "update": {
            "nodes": [{ "id": 3, "role": "textField",
                        "attributes": { "name": "Type a message", "value": "hi" } }],
            "tree_data": { "focus_id": 3 }
        }}] }));
        assert_eq!(t.focus(), Some(3));
        // An update without focus information leaves it alone.
        t.apply(&json!({ "updates": [{ "nodes": [node(7, "button", "New chat")] }] }));
        assert_eq!(t.focus(), Some(3));
    }

    fn parent(id: i64, children: &[i64]) -> Json {
        json!({ "id": id, "role": "genericContainer", "child_ids": children })
    }

    #[test]
    fn node_id_to_clear_and_dropped_children_remove_subtrees() {
        let mut t = AxTree::default();
        t.apply(&json!({ "updates": [{ "nodes": [
            parent(1, &[2, 3]), parent(2, &[4]), node(3, "button", "Keep"), node(4, "button", "Gone")
        ] }] }));
        assert_eq!(t.nodes.len(), 4);

        // 1 drops child 2: 2 and its child 4 go, 3 stays.
        t.apply(&json!({ "updates": [{ "nodes": [parent(1, &[3])] }] }));
        let mut ids: Vec<i64> = t.nodes.keys().copied().collect();
        ids.sort();
        assert_eq!(ids, vec![1, 3]);

        // node_id_to_clear wipes 1's descendants before the update re-adds 5.
        t.apply(&json!({ "updates": [{ "node_id_to_clear": 1,
                                       "nodes": [parent(1, &[5]), node(5, "link", "New")] }] }));
        let mut ids: Vec<i64> = t.nodes.keys().copied().collect();
        ids.sort();
        assert_eq!(ids, vec![1, 5]);

        // A child moved to another parent in the same update survives.
        t.apply(&json!({ "updates": [{ "nodes": [parent(1, &[6]), parent(6, &[5])] }] }));
        assert!(t.nodes.contains_key(&5));

        t.reset();
        assert!(t.nodes.is_empty() && t.parents.is_empty());
    }

    #[test]
    fn removed_focus_is_forgotten() {
        let mut t = AxTree::default();
        t.apply(
            &json!({ "updates": [{ "nodes": [parent(1, &[7]), node(7, "button", "Send")] }],
                         "events": [{ "event_type": "focus", "id": 7 }] }),
        );
        assert_eq!(t.focus(), Some(7));
        t.apply(&json!({ "updates": [{ "nodes": [parent(1, &[])] }] }));
        assert!(!t.nodes.contains_key(&7));
        assert_eq!(t.focus(), None);
    }

    fn full(id: i64, role: &str, name: &str, children: &[i64], extra: Json) -> Json {
        let mut n = json!({ "id": id, "role": role, "child_ids": children,
                            "attributes": { "name": name } });
        if let (Some(n), Some(extra)) = (n.as_object_mut(), extra.as_object()) {
            for (k, v) in extra {
                n.insert(k.clone(), v.clone());
            }
        }
        n
    }

    /// root(1) > generic(2) > [heading(3) > text(4 "Welcome"),
    ///   generic(5) > [link(6 "Help") > text(7 "Help"), text(8 "Scan the code")],
    ///   generic named(9 "Panel") > button(10), invisible(11) > button(12)]
    fn landing() -> AxTree {
        let mut t = AxTree::default();
        t.apply(&json!({ "updates": [{ "root_id": 1, "nodes": [
            full(1, "rootWebArea", "WhatsApp", &[2], json!({})),
            full(2, "genericContainer", "", &[3, 5, 9, 11], json!({})),
            full(3, "heading", "Welcome", &[4], json!({ "attributes": { "name": "Welcome", "hierarchicalLevel": 1 } })),
            full(4, "staticText", "Welcome", &[], json!({})),
            full(5, "genericContainer", "", &[6, 8], json!({})),
            full(6, "link", "Help", &[7], json!({ "state": ["focusable", "linked"] })),
            full(7, "staticText", "Help", &[], json!({})),
            full(8, "staticText", "Scan the code", &[], json!({})),
            full(9, "genericContainer", "Panel", &[10], json!({})),
            full(10, "button", "Log in", &[], json!({})),
            full(11, "genericContainer", "", &[12], json!({ "state": ["invisible"] })),
            full(12, "button", "Hidden", &[], json!({})),
        ] }] }));
        t
    }

    #[test]
    fn role_mapping() {
        use gtk::AccessibleRole as R;
        assert_eq!(gtk_role("rootWebArea"), R::Document);
        assert_eq!(gtk_role("button"), R::Button);
        assert_eq!(gtk_role("link"), R::Link);
        assert_eq!(gtk_role("heading"), R::Heading);
        assert_eq!(gtk_role("staticText"), R::Label);
        assert_eq!(gtk_role("textField"), R::TextBox);
        assert_eq!(gtk_role("gridCell"), R::GridCell);
        assert_eq!(gtk_role("toggleButton"), R::ToggleButton);
        assert_eq!(gtk_role("somethingNew"), R::Generic);
    }

    #[test]
    fn generic_nodes_are_flattened_and_redundant_text_dropped() {
        let t = landing();
        // Unnamed containers vanish; their children move up. The heading's
        // own text and the invisible subtree are not exposed.
        assert_eq!(t.exported_children(1), vec![3, 6, 8, 9]);
        assert!(t.exported_children(3).is_empty());
        // Link text repeats the link name; button is a leaf.
        assert!(t.exported_children(6).is_empty());
        assert_eq!(t.exported_children(9), vec![10]);
        assert_eq!(t.exported_parent(6), Some(1));
        assert_eq!(t.exported_parent(10), Some(9));
        assert_eq!(t.exported_path(10), vec![1, 9, 10]);
        // A flattened node resolves to its nearest exposed ancestor.
        assert_eq!(t.exported_path(5), vec![1]);
        assert!(!t.is_exported(2) && t.is_exported(1) && t.is_exported(9));
    }

    #[test]
    fn props_carry_states() {
        let mut t = landing();
        let p = t.props(3).unwrap();
        assert_eq!(
            (p.role, p.label.as_str(), p.level),
            (gtk::AccessibleRole::Heading, "Welcome", 1)
        );
        assert!(t.props(6).unwrap().focusable);
        t.apply(&json!({ "updates": [{ "nodes": [
            full(13, "checkBox", "Keep me signed in", &[], json!({
                "state": ["focusable", "collapsed"],
                "attributes": { "name": "Keep me signed in", "checkedState": "true",
                                "restriction": "disabled", "selected": true } })),
            full(14, "textField", "Phone number", &[], json!({
                "attributes": { "name": "Phone number", "value": "+351", "placeholder": "Number" } })),
        ] }] }));
        let p = t.props(13).unwrap();
        assert_eq!(p.checked, Some(gtk::AccessibleTristate::True));
        assert!(!p.pressed && p.disabled && !p.read_only);
        assert_eq!((p.selected, p.expanded), (Some(true), Some(false)));
        let p = t.props(14).unwrap();
        assert_eq!(
            (p.description.as_str(), p.placeholder.as_str()),
            ("+351", "Number")
        );
    }

    #[test]
    fn changes_are_logged_until_taken() {
        let mut t = landing();
        let (resync, changed) = t.take_changes();
        assert!(!resync && changed.contains(&10) && changed.len() == 12);
        t.apply(
            &json!({ "updates": [{ "nodes": [full(10, "button", "Sign in", &[], json!({}))] }] }),
        );
        assert_eq!(t.take_changes(), (false, HashSet::from([10])));
        t.discard_changes();
        assert_eq!(t.take_changes(), (true, HashSet::new()));
        t.reset();
        assert!(t.take_changes().0);
    }

    #[test]
    fn bounds_resolve_offset_containers_and_scroll() {
        let mut t = AxTree::default();
        let loc =
            |x: f64, y: f64, w: f64, h: f64| json!({ "x": x, "y": y, "width": w, "height": h });
        t.apply(&json!({ "updates": [{ "root_id": 1, "nodes": [
            full(1, "rootWebArea", "", &[2], json!({ "location": loc(0.0, 0.0, 800.0, 600.0) })),
            // A scrolled list at (100, 50), scrolled down by 30.
            full(2, "list", "Chats", &[3, 4], json!({ "location": loc(100.0, 50.0, 300.0, 500.0),
                                                     "attributes": { "name": "Chats", "scrollY": 30 } })),
            full(3, "listItem", "Ana", &[], json!({ "location": loc(0.0, 40.0, 300.0, 60.0),
                                                   "offset_container_id": 2 })),
            // No location: sits at its parent's origin, zero size.
            full(4, "listItem", "Rui", &[], json!({})),
        ] }] }));
        assert_eq!(t.page_rect(3), Some((100.0, 60.0, 300.0, 60.0)));
        assert_eq!(t.relative_rect(3, Some(2)), (0.0, 10.0, 300.0, 60.0));
        assert_eq!(t.relative_rect(2, Some(1)), (100.0, 50.0, 300.0, 500.0));
        assert_eq!(t.relative_rect(4, Some(2)), (0.0, 0.0, 0.0, 0.0));
        // Location updates move the node.
        t.apply_locations(&json!([{ "id": 3, "new_location": {
            "offset_container_id": 2, "bounds": loc(0.0, 100.0, 300.0, 60.0) } }]));
        assert_eq!(t.relative_rect(3, Some(2)), (0.0, 70.0, 300.0, 60.0));
    }
}
