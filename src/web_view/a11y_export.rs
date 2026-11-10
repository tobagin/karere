//! AT-SPI mirror of the foreground page's accessibility tree (#204).
//!
//! CEF renders offscreen, so assistive technology only sees the GLArea. When
//! the page tree is on (`handlers::accessibility::wanted`), the view exports
//! the page's meaningful nodes as `GtkAccessible` objects under itself, so a
//! screen reader can walk headings, links, buttons and chat rows, read their
//! bounds and follow page focus.
//!
//! Proxies are created lazily: a node's children exist only once an AT has
//! asked for them (or focus passed through them). After that, each batch of
//! CEF updates (one per main-loop idle, via `handlers::wake_view`) re-lists
//! only the materialized parents whose children may have changed and emits
//! children-changed for real differences. Generic containers are flattened
//! away by `AxTree::exported_children`.
//!
//! GTK has no public call to announce a new or removed child of a custom
//! accessible, but it does announce a `hidden` state change as one. A new
//! proxy is therefore created hidden, realized, then shown; a removed one is
//! hidden before it is dropped.
//!
//! Locking: the tree mutex is never held across a GTK call. GTK calls back
//! into the proxies (index lookups, child listing) synchronously, and those
//! callbacks lock the tree themselves.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::{Rc, Weak};
use std::sync::Arc;

use gtk::accessible::{Property, State};
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use parking_lot::Mutex;

use crate::handlers::accessibility::{AxProps, AxTree, tree_for};

/// Proxy key of the document root (the tree's root node, whatever its id).
const ROOT: i64 = i64::MIN;

mod imp {
    use super::*;
    use std::sync::LazyLock;

    pub struct AxProxy {
        /// Node id, or `ROOT`.
        pub key: Cell<i64>,
        /// Key of the exposed parent (`ROOT` for the root's children).
        pub parent: Cell<i64>,
        pub export: RefCell<Weak<super::Export>>,
        pub ctx: RefCell<Option<gtk::ATContext>>,
        pub role: Cell<gtk::AccessibleRole>,
        /// Last properties pushed to GTK, to skip no-op updates.
        pub props: RefCell<Option<AxProps>>,
    }

    impl Default for AxProxy {
        fn default() -> Self {
            Self {
                key: Cell::new(ROOT),
                parent: Cell::new(ROOT),
                export: RefCell::default(),
                ctx: RefCell::default(),
                role: Cell::new(gtk::AccessibleRole::Generic),
                props: RefCell::default(),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AxProxy {
        const NAME: &'static str = "KarereAxNode";
        type Type = super::AxProxy;
        type ParentType = glib::Object;
        type Interfaces = (gtk::Accessible,);
    }

    impl ObjectImpl for AxProxy {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPS: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
                vec![glib::ParamSpecOverride::for_interface::<gtk::Accessible>(
                    "accessible-role",
                )]
            });
            PROPS.as_ref()
        }

        fn property(&self, _id: usize, _pspec: &glib::ParamSpec) -> glib::Value {
            self.role.get().to_value()
        }

        // The role is fixed when the context is created.
        fn set_property(&self, _id: usize, _value: &glib::Value, _pspec: &glib::ParamSpec) {}

        fn dispose(&self) {
            // Unrealize (AT-SPI "defunct" + unregister) while we are alive.
            self.ctx.take();
        }
    }

    impl AccessibleImpl for AxProxy {
        fn at_context(&self) -> Option<gtk::ATContext> {
            self.ctx.borrow().clone()
        }

        fn platform_state(&self, state: gtk::AccessiblePlatformState) -> bool {
            self.export
                .borrow()
                .upgrade()
                .is_some_and(|e| e.platform_state(self.key.get(), state))
        }

        fn bounds(&self) -> Option<(i32, i32, i32, i32)> {
            let e = self.export.borrow().upgrade()?;
            Some(e.bounds(self.key.get(), self.parent.get()))
        }

        fn accessible_parent(&self) -> Option<gtk::Accessible> {
            let e = self.export.borrow().upgrade()?;
            e.parent_of(self.key.get(), self.parent.get())
        }

        fn first_accessible_child(&self) -> Option<gtk::Accessible> {
            let e = self.export.borrow().upgrade()?;
            e.first_child(self.key.get())
        }

        fn next_accessible_sibling(&self) -> Option<gtk::Accessible> {
            let e = self.export.borrow().upgrade()?;
            e.next_sibling(self.key.get(), self.parent.get())
        }
    }
}

glib::wrapper! {
    /// One exposed page node.
    pub struct AxProxy(ObjectSubclass<imp::AxProxy>) @implements gtk::Accessible;
}

// gtk4-rs binds this only with the v4_18 feature; the Flatpak runtime ships
// GTK 4.24. It is how a non-widget accessible reports a focus change.
unsafe extern "C" {
    fn gtk_accessible_update_platform_state(
        accessible: *mut gtk::ffi::GtkAccessible,
        state: gtk::ffi::GtkAccessiblePlatformState,
    );
}

fn update_platform_state(a: &impl IsA<gtk::Accessible>, state: gtk::AccessiblePlatformState) {
    use glib::translate::{IntoGlib, ToGlibPtr};
    unsafe {
        gtk_accessible_update_platform_state(a.as_ref().to_glib_none().0, state.into_glib());
    }
}

/// Push `p` into the proxy's GTK accessible state.
fn apply_props(obj: &AxProxy, p: &AxProps) {
    obj.update_property(&[
        Property::Label(&p.label),
        Property::ReadOnly(p.read_only),
        Property::Required(p.required),
        Property::MultiLine(p.multi_line),
    ]);
    for (text, prop, reset) in [
        (
            &p.description,
            Property::Description(&p.description),
            gtk::AccessibleProperty::Description,
        ),
        (
            &p.placeholder,
            Property::Placeholder(&p.placeholder),
            gtk::AccessibleProperty::Placeholder,
        ),
    ] {
        if text.is_empty() {
            obj.reset_property(reset);
        } else {
            obj.update_property(&[prop]);
        }
    }
    if p.level > 0 {
        obj.update_property(&[Property::Level(p.level)]);
    }
    obj.update_state(&[
        State::Disabled(p.disabled),
        State::Expanded(p.expanded),
        State::Selected(p.selected),
    ]);
    match p.checked {
        Some(c) if p.pressed => obj.update_state(&[State::Pressed(c)]),
        Some(c) => obj.update_state(&[State::Checked(c)]),
        None => {
            obj.reset_state(gtk::AccessibleState::Checked);
            obj.reset_state(gtk::AccessibleState::Pressed);
        }
    }
}

/// Make GTK export `obj` now: `set_accessible_parent` realizes the context
/// of a non-widget accessible. The link is cleared again right away, so the
/// parent and sibling queries keep coming from the export's own lists.
fn realize(obj: &AxProxy, parent: &gtk::Accessible) {
    obj.set_accessible_parent(Some(parent), None::<&gtk::Accessible>);
    obj.set_accessible_parent(None::<&gtk::Accessible>, None::<&gtk::Accessible>);
}

/// One materialized parent's new child list, gathered under the tree lock.
type Listing = (i64, Vec<(i64, AxProps)>);

/// What one sync must apply, gathered under the tree lock.
#[derive(Default)]
struct Plan {
    props: Vec<(i64, AxProps)>,
    listings: Vec<Listing>,
    /// Exposed path from the root to the focused node.
    focus_path: Vec<i64>,
}

pub struct Export {
    view: glib::WeakRef<super::KarereWebView>,
    root: AxProxy,
    /// Browser whose tree is mirrored (0 = none yet).
    browser: Cell<i32>,
    tree: RefCell<Option<Arc<Mutex<AxTree>>>>,
    /// Tree root node id the `ROOT` proxy currently stands for.
    root_id: Cell<Option<i64>>,
    nodes: RefCell<HashMap<i64, AxProxy>>,
    /// Materialized child lists by proxy key, in document order.
    children: RefCell<HashMap<i64, Vec<i64>>>,
    focused: Cell<Option<i64>>,
    /// Removed proxies, released from a later idle. GTK can be iterating a
    /// snapshot of exported contexts (AT-SPI `Cache.GetItems`) when it calls
    /// back into us, so a context must never be freed synchronously.
    graveyard: Rc<RefCell<Vec<AxProxy>>>,
}

impl Export {
    /// Build the export for `view` and publish its root. The caller stores
    /// the returned value on the view before calling [`Self::attach`].
    pub fn new(view: &super::KarereWebView) -> Rc<Self> {
        Rc::new_cyclic(|weak: &Weak<Self>| {
            let root: AxProxy = glib::Object::new();
            let r = root.imp();
            r.key.set(ROOT);
            r.role.set(gtk::AccessibleRole::Document);
            *r.export.borrow_mut() = weak.clone();
            *r.ctx.borrow_mut() =
                gtk::ATContext::create(gtk::AccessibleRole::Document, &root, &view.display());
            root.update_state(&[State::Hidden(true)]);
            Self {
                view: view.downgrade(),
                root,
                browser: Cell::new(0),
                tree: RefCell::default(),
                root_id: Cell::new(None),
                nodes: RefCell::default(),
                children: RefCell::default(),
                focused: Cell::new(None),
                graveyard: Rc::default(),
            }
        })
    }

    /// Announce the root under the view (the view now lists it as its first
    /// accessible child) and follow the view's own focus.
    pub fn attach(self: &Rc<Self>) {
        let Some(view) = self.view.upgrade() else {
            return;
        };
        realize(&self.root, view.upcast_ref());
        self.root.update_state(&[State::Hidden(false)]);
        let weak = Rc::downgrade(self);
        // Window (re)activated onto the page: say where focus is again.
        view.connect_has_focus_notify(move |v| {
            if v.has_focus()
                && let Some(e) = weak.upgrade()
            {
                e.refocus(None);
            }
        });
    }

    pub fn root(&self) -> &AxProxy {
        &self.root
    }

    /// A page node holds focus, so the view itself should not report it.
    pub fn page_focused(&self) -> bool {
        self.focused.get().is_some()
    }

    fn tree(&self) -> Option<Arc<Mutex<AxTree>>> {
        self.tree.borrow().clone()
    }

    fn node_id(&self, key: i64) -> Option<i64> {
        if key == ROOT {
            self.root_id.get()
        } else {
            Some(key)
        }
    }

    fn key_of(&self, id: i64) -> i64 {
        if Some(id) == self.root_id.get() {
            ROOT
        } else {
            id
        }
    }

    fn proxy(&self, key: i64) -> Option<AxProxy> {
        if key == ROOT {
            Some(self.root.clone())
        } else {
            self.nodes.borrow().get(&key).cloned()
        }
    }

    // ---- GtkAccessible queries ---------------------------------------

    fn platform_state(&self, key: i64, state: gtk::AccessiblePlatformState) -> bool {
        match state {
            // Like Chromium, page focus is kept while the window is
            // inactive; it only needs the view to be the window's focus.
            gtk::AccessiblePlatformState::Focused => {
                self.focused.get() == Some(key) && self.view.upgrade().is_some_and(|v| v.is_focus())
            }
            gtk::AccessiblePlatformState::Focusable => self
                .proxy(key)
                .and_then(|p| p.imp().props.borrow().as_ref().map(|p| p.focusable))
                .unwrap_or(false),
            _ => false,
        }
    }

    fn bounds(&self, key: i64, parent: i64) -> (i32, i32, i32, i32) {
        let Some(view) = self.view.upgrade() else {
            return (0, 0, 0, 0);
        };
        if key == ROOT {
            return (0, 0, view.width(), view.height());
        }
        let Some(tree) = self.tree() else {
            return (0, 0, 0, 0);
        };
        let rel = tree.lock().relative_rect(key, self.node_id(parent));
        to_logical(rel, view.scale_factor())
    }

    fn parent_of(&self, key: i64, parent: i64) -> Option<gtk::Accessible> {
        if key == ROOT {
            return self.view.upgrade().map(|v| v.upcast());
        }
        self.proxy(parent).map(|p| p.upcast())
    }

    fn first_child(self: &Rc<Self>, key: i64) -> Option<gtk::Accessible> {
        self.materialize(key);
        let first = self.children.borrow().get(&key)?.first().copied()?;
        self.proxy(first).map(|p| p.upcast())
    }

    fn next_sibling(&self, key: i64, parent: i64) -> Option<gtk::Accessible> {
        if key == ROOT {
            // The view's own children (popovers) follow the page.
            let view = self.view.upgrade()?;
            return view.imp().widget_first_accessible_child();
        }
        let next = {
            let children = self.children.borrow();
            let list = children.get(&parent)?;
            let i = list.iter().position(|&c| c == key)?;
            list.get(i + 1).copied()?
        };
        self.proxy(next).map(|p| p.upcast())
    }

    // ---- proxy lifecycle ---------------------------------------------

    fn create(self: &Rc<Self>, id: i64, parent: i64, props: &AxProps, hidden: bool) -> AxProxy {
        let obj: AxProxy = glib::Object::new();
        let imp = obj.imp();
        imp.key.set(id);
        imp.parent.set(parent);
        imp.role.set(props.role);
        *imp.export.borrow_mut() = Rc::downgrade(self);
        if let Some(view) = self.view.upgrade() {
            *imp.ctx.borrow_mut() = gtk::ATContext::create(props.role, &obj, &view.display());
        }
        if hidden {
            obj.update_state(&[State::Hidden(true)]);
        }
        apply_props(&obj, props);
        *imp.props.borrow_mut() = Some(props.clone());
        obj
    }

    /// List `key`'s children for the first time. No events: nobody has
    /// seen them yet.
    fn materialize(self: &Rc<Self>, key: i64) {
        // Already listed, or a removed proxy GTK still holds on to.
        if self.children.borrow().contains_key(&key)
            || (key != ROOT && !self.nodes.borrow().contains_key(&key))
        {
            return;
        }
        let (Some(tree), Some(id)) = (self.tree(), self.node_id(key)) else {
            return;
        };
        let listing: Vec<(i64, AxProps)> = {
            let t = tree.lock();
            t.exported_children(id)
                .into_iter()
                .filter_map(|c| t.props(c).map(|p| (c, p)))
                .collect()
        };
        self.place(key, listing, false);
    }

    /// Make `key`'s child list `listing`: drop children that left (or
    /// changed role), create the new ones, and with `notify` announce both.
    fn place(self: &Rc<Self>, key: i64, listing: Vec<(i64, AxProps)>, notify: bool) {
        let Some(parent) = self.proxy(key) else {
            return;
        };
        let old = self
            .children
            .borrow()
            .get(&key)
            .cloned()
            .unwrap_or_default();
        let old_roles: Vec<(i64, gtk::AccessibleRole)> = old
            .iter()
            .filter_map(|&id| self.proxy(id).map(|x| (id, x.imp().role.get())))
            .collect();
        let new_roles: Vec<(i64, gtk::AccessibleRole)> =
            listing.iter().map(|(id, p)| (*id, p.role)).collect();
        let kept = kept_children(&old_roles, &new_roles);
        for id in old.iter().filter(|id| !kept.contains(id)) {
            self.remove(*id, notify);
        }
        // A node listed elsewhere (moved without its old parent re-listing)
        // leaves its old place first.
        let mut created = Vec::new();
        for (id, props) in &listing {
            if kept.contains(id) {
                self.update(*id, props);
                continue;
            }
            if let Some(stale) = self.proxy(*id) {
                let from = stale.imp().parent.get();
                if let Some(list) = self.children.borrow_mut().get_mut(&from) {
                    list.retain(|c| c != id);
                }
                self.remove(*id, notify);
            }
            let obj = self.create(*id, key, props, notify);
            self.nodes.borrow_mut().insert(*id, obj.clone());
            created.push(obj);
        }
        self.children
            .borrow_mut()
            .insert(key, listing.iter().map(|(id, _)| *id).collect());
        // Announce new children: realize them (still hidden), then show
        // them. Only from `sync`, never inside a GTK query: GTK serializes an
        // object while realizing it and asks for its children then, and
        // realizing a child from there re-realizes that half-realized parent
        // under a second AT-SPI path, which dangles once it is freed. Listed
        // children nobody is told about are realized by GTK when it hands
        // them out.
        if notify {
            let parent: gtk::Accessible = parent.upcast();
            for obj in &created {
                realize(obj, &parent);
            }
            for obj in &created {
                obj.update_state(&[State::Hidden(false)]);
            }
        }
    }

    /// Drop proxy `key` and everything materialized below it. With
    /// `notify`, its parent announces the removal.
    fn remove(&self, key: i64, notify: bool) {
        let Some(obj) = self.proxy(key).filter(|_| key != ROOT) else {
            return;
        };
        // Announce while still listed, so GTK can report its index.
        if notify {
            obj.update_state(&[State::Hidden(true)]);
        }
        self.nodes.borrow_mut().remove(&key);
        if let Some(list) = self.children.borrow_mut().get_mut(&obj.imp().parent.get()) {
            list.retain(|&c| c != key);
        }
        if self.focused.get() == Some(key) {
            self.focused.set(None);
        }
        let mut dead = vec![obj];
        let mut stack = self.children.borrow_mut().remove(&key).unwrap_or_default();
        while let Some(c) = stack.pop() {
            if self.focused.get() == Some(c) {
                self.focused.set(None);
            }
            dead.extend(self.nodes.borrow_mut().remove(&c));
            stack.extend(self.children.borrow_mut().remove(&c).unwrap_or_default());
        }
        self.bury(dead);
    }

    /// Release `dead` proxies from an idle callback (see `graveyard`).
    fn bury(&self, dead: Vec<AxProxy>) {
        if dead.is_empty() {
            return;
        }
        let mut grave = self.graveyard.borrow_mut();
        let first = grave.is_empty();
        grave.extend(dead);
        if first {
            let grave = Rc::downgrade(&self.graveyard);
            glib::idle_add_local_once(move || {
                if let Some(g) = grave.upgrade() {
                    let dead = std::mem::take(&mut *g.borrow_mut());
                    drop(dead);
                }
            });
        }
    }

    fn update(&self, key: i64, props: &AxProps) {
        let Some(obj) = self.proxy(key) else { return };
        let old = obj.imp().props.borrow().clone();
        if old.as_ref() == Some(props) {
            return;
        }
        apply_props(&obj, props);
        *obj.imp().props.borrow_mut() = Some(props.clone());
        if old.is_some_and(|o| o.focusable != props.focusable) {
            update_platform_state(&obj, gtk::AccessiblePlatformState::Focusable);
        }
    }

    /// Forget every proxy below the root, announcing their removal.
    fn clear(&self) {
        let top = self.children.borrow_mut().remove(&ROOT).unwrap_or_default();
        for c in top {
            self.remove(c, true);
        }
        self.children.borrow_mut().clear();
        let rest: Vec<AxProxy> = self.nodes.borrow_mut().drain().map(|(_, p)| p).collect();
        self.bury(rest);
        self.focused.set(None);
    }

    /// Move the focused state to `target` (or just re-announce the current
    /// one, for a view focus change), keeping the view's own state in step.
    fn refocus(&self, target: Option<Option<i64>>) {
        let old = self.focused.get();
        if let Some(new) = target {
            if new == old {
                return;
            }
            self.focused.set(new);
            if let Some(p) = old.and_then(|k| self.proxy(k)) {
                update_platform_state(&p, gtk::AccessiblePlatformState::Focused);
            }
        }
        let Some(view) = self.view.upgrade() else {
            return;
        };
        if old.is_some() != self.focused.get().is_some() {
            update_platform_state(&view, gtk::AccessiblePlatformState::Focused);
        }
        if let Some(p) = self.focused.get().and_then(|k| self.proxy(k)) {
            update_platform_state(&p, gtk::AccessiblePlatformState::Focused);
        }
    }

    // ---- sync ----------------------------------------------------------

    /// Bring the mirror up to date with the foreground browser's tree.
    pub fn sync(self: &Rc<Self>, browser_id: i32) {
        let tree = tree_for(browser_id);
        let switched = browser_id != self.browser.get()
            || match (&*self.tree.borrow(), &tree) {
                (Some(a), Some(b)) => !Arc::ptr_eq(a, b),
                (None, None) => false,
                _ => true,
            };
        if switched {
            self.browser.set(browser_id);
            *self.tree.borrow_mut() = tree.clone();
        }
        let Some(tree) = tree else {
            self.clear();
            self.root_id.set(None);
            return;
        };
        let root_was_listed = self.children.borrow().contains_key(&ROOT);
        let mut plan = Plan::default();
        let rebuild = {
            let mut t = tree.lock();
            let (resync, changed) = t.take_changes();
            let rebuild = switched || resync || t.root() != self.root_id.get();
            if rebuild {
                self.root_id.set(t.root());
                if let Some(p) = t.root().and_then(|r| t.props(r)) {
                    plan.props.push((ROOT, p));
                }
            } else {
                self.plan_changes(&t, &changed, &mut plan);
            }
            if let Some(f) = t.focus() {
                plan.focus_path = t.exported_path(f);
            }
            if rebuild
                && root_was_listed
                && let Some(r) = t.root()
            {
                let listing = t
                    .exported_children(r)
                    .into_iter()
                    .filter_map(|c| t.props(c).map(|p| (c, p)))
                    .collect();
                plan.listings.push((ROOT, listing));
            }
            rebuild
        };
        if rebuild {
            self.clear();
        }
        for (key, props) in &plan.props {
            self.update(*key, props);
        }
        for (key, listing) in std::mem::take(&mut plan.listings) {
            self.place(key, listing, true);
        }
        // Materialize the way down to the focused node so it exists to be
        // focused, then move focus.
        let path: Vec<i64> = plan.focus_path.iter().map(|&id| self.key_of(id)).collect();
        for &k in path.iter().take(path.len().saturating_sub(1)) {
            self.materialize(k);
        }
        let target = path.last().copied().filter(|&k| self.proxy(k).is_some());
        self.refocus(Some(target));
    }

    /// Fill `plan` for an incremental update under the tree lock.
    fn plan_changes(&self, t: &AxTree, changed: &HashSet<i64>, plan: &mut Plan) {
        let listed: HashSet<i64> = self.children.borrow().keys().copied().collect();
        let mut affected = HashSet::new();
        {
            let nodes = self.nodes.borrow();
            for &id in changed {
                let key = self.key_of(id);
                if (key == ROOT || nodes.contains_key(&id))
                    && let Some(p) = t.props(id)
                {
                    plan.props.push((key, p));
                }
                if t.is_exported(id) {
                    affected.insert(key);
                }
                if let Some(parent) = t.exported_parent(id) {
                    affected.insert(self.key_of(parent));
                }
            }
        }
        for key in affected.into_iter().filter(|k| listed.contains(k)) {
            let Some(id) = self.node_id(key) else {
                continue;
            };
            let listing = t
                .exported_children(id)
                .into_iter()
                .filter_map(|c| t.props(c).map(|p| (c, p)))
                .collect();
            plan.listings.push((key, listing));
        }
    }
}

/// Children present before and after with the same role keep their proxy;
/// everything else in `old` is removed and everything else in `new` created
/// (a role cannot change once a GTK accessible is realized).
pub(super) fn kept_children(
    old: &[(i64, gtk::AccessibleRole)],
    new: &[(i64, gtk::AccessibleRole)],
) -> HashSet<i64> {
    new.iter()
        .filter(|n| old.contains(n))
        .map(|(id, _)| *id)
        .collect()
}

/// View-pixel rect → widget logical pixels (the OSR view is physical).
pub(super) fn to_logical((x, y, w, h): (f64, f64, f64, f64), scale: i32) -> (i32, i32, i32, i32) {
    let s = scale.max(1) as f64;
    (
        (x / s).round() as i32,
        (y / s).round() as i32,
        (w / s).round() as i32,
        (h / s).round() as i32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use gtk::AccessibleRole as R;

    #[test]
    fn kept_children_needs_same_id_and_role() {
        let old = [(1, R::Button), (2, R::Link), (3, R::Heading)];
        let new = [(3, R::Heading), (2, R::Button), (4, R::Label)];
        let kept = kept_children(&old, &new);
        // 3 stays (reordered is fine), 2 changed role, 4 is new, 1 left.
        assert_eq!(kept, HashSet::from([3]));
        assert!(kept_children(&[], &new).is_empty());
    }

    #[test]
    fn logical_bounds_divide_by_paint_scale() {
        assert_eq!(to_logical((10.0, 20.0, 300.0, 41.0), 1), (10, 20, 300, 41));
        assert_eq!(to_logical((10.0, 20.0, 300.0, 41.0), 2), (5, 10, 150, 21));
        assert_eq!(to_logical((10.0, 20.0, 30.0, 40.0), 0), (10, 20, 30, 40));
    }
}
