//! Spellcheck-language dropdown shared by the header bar and Preferences:
//! `SpellLang` item type, model, sorter, row factories and the GSettings sync
//! (`SpellBinding`). Callers own the widget and the live-browser switch.
//!
//! Sorting: the "Automatic" entry (empty code) first, then starred favorites,
//! then alphabetical. Choosing "Automatic" clears `spell-checking-languages` so
//! auto-detection applies again.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use gettextrs::gettext;
use gtk::gio;
use gtk::gio::prelude::SettingsExtManual;
use gtk::glib;
use gtk::prelude::*;

use crate::spellcheck::{KNOWN_LANGUAGES, display_name, resolve_languages};

glib::wrapper! {
    /// One spellcheck language row: BCP-47 `code`, friendly `name`, `favorite`.
    pub struct SpellLang(ObjectSubclass<imp::SpellLang>);
}

pub type SpellToggleCallback = Rc<dyn Fn(&SpellLang, bool)>;

impl SpellLang {
    pub fn new(code: &str, name: &str, favorite: bool) -> Self {
        glib::Object::builder()
            .property("code", code)
            .property("name", name)
            .property("favorite", favorite)
            .build()
    }
}

/// Build the backing store from `KNOWN_LANGUAGES`, marking any code present in
/// `favorites` as starred.
pub fn build_store(favorites: &[String]) -> gtk::gio::ListStore {
    let store = gtk::gio::ListStore::new::<SpellLang>();
    store.append(&SpellLang::new("", &gettext("Automatic"), false));
    for (code, _name) in KNOWN_LANGUAGES {
        let fav = favorites.iter().any(|f| f == code);
        // display_name keeps friendly names in one source of truth.
        store.append(&SpellLang::new(code, &display_name(code), fav));
    }
    store
}

/// Sorter: "Automatic" first, then favorites, then case-insensitive by name.
pub fn build_sorter() -> gtk::CustomSorter {
    gtk::CustomSorter::new(move |a, b| {
        let a = a.downcast_ref::<SpellLang>().expect("SpellLang");
        let b = b.downcast_ref::<SpellLang>().expect("SpellLang");
        match (a.code().is_empty(), b.code().is_empty()) {
            (true, false) => return gtk::Ordering::Smaller,
            (false, true) => return gtk::Ordering::Larger,
            _ => {}
        }
        match (a.favorite(), b.favorite()) {
            (true, false) => gtk::Ordering::Smaller,
            (false, true) => gtk::Ordering::Larger,
            _ => a.name().to_lowercase().cmp(&b.name().to_lowercase()).into(),
        }
    })
}

/// Factory for the popup rows: friendly name + a trailing star `ToggleButton`.
/// `on_toggle(lang, now_favorite)` fires when the user clicks a star.
pub fn build_list_factory(on_toggle: SpellToggleCallback) -> gtk::SignalListItemFactory {
    let factory = gtk::SignalListItemFactory::new();

    factory.connect_setup(|_, item| {
        let item = item.downcast_ref::<gtk::ListItem>().expect("ListItem");
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);

        let label = gtk::Label::new(None);
        label.set_xalign(0.0);
        label.set_hexpand(true);

        let star = gtk::ToggleButton::new();
        star.set_icon_name("non-starred-symbolic");
        star.add_css_class("flat");
        star.set_valign(gtk::Align::Center);
        star.set_tooltip_text(Some(&gettext("Pin to top")));

        row.append(&label);
        row.append(&star);
        item.set_child(Some(&row));
    });

    // Handler ids live here, not on the button (main thread only).
    let toggle_ids: Rc<RefCell<HashMap<usize, glib::SignalHandlerId>>> =
        Rc::new(RefCell::new(HashMap::new()));
    let bind_ids = toggle_ids.clone();
    factory.connect_bind(move |_, item| {
        let item = item.downcast_ref::<gtk::ListItem>().expect("ListItem");
        let Some(lang) = item.item().and_downcast::<SpellLang>() else {
            return;
        };
        let Some(row) = item.child().and_downcast::<gtk::Box>() else {
            return;
        };
        let label = row.first_child().and_downcast::<gtk::Label>().unwrap();
        let star = row
            .last_child()
            .and_downcast::<gtk::ToggleButton>()
            .unwrap();

        label.set_text(&lang.name());
        // "Automatic" cannot be pinned.
        star.set_visible(!lang.code().is_empty());
        update_star_visual(&star, lang.favorite());

        // Reconnect cleanly each bind; disconnect before set_active so a
        // recycled row never fires the previous item's handler.
        let key = star.as_ptr() as usize;
        if let Some(old) = bind_ids.borrow_mut().remove(&key) {
            star.disconnect(old);
        }
        star.set_active(lang.favorite());
        let on_toggle = on_toggle.clone();
        let lang_weak = lang.downgrade();
        let id = star.connect_toggled(move |btn| {
            let Some(lang) = lang_weak.upgrade() else {
                return;
            };
            let now = btn.is_active();
            update_star_visual(btn, now);
            on_toggle(&lang, now);
        });
        bind_ids.borrow_mut().insert(key, id);
    });

    // Forget the handler on unbind so recycled rows don't leak.
    let unbind_ids = toggle_ids.clone();
    factory.connect_unbind(move |_, item| {
        let Some(item) = item.downcast_ref::<gtk::ListItem>() else {
            return;
        };
        let Some(row) = item.child().and_downcast::<gtk::Box>() else {
            return;
        };
        let Some(star) = row.last_child().and_downcast::<gtk::ToggleButton>() else {
            return;
        };
        let key = star.as_ptr() as usize;
        if let Some(old) = unbind_ids.borrow_mut().remove(&key) {
            star.disconnect(old);
        }
    });

    factory
}

/// Plain factory for the collapsed button face: name only, no star.
pub fn build_button_factory() -> gtk::SignalListItemFactory {
    let factory = gtk::SignalListItemFactory::new();
    factory.connect_setup(|_, item| {
        let item = item.downcast_ref::<gtk::ListItem>().expect("ListItem");
        let label = gtk::Label::new(None);
        label.set_xalign(0.0);
        item.set_child(Some(&label));
    });
    factory.connect_bind(|_, item| {
        let item = item.downcast_ref::<gtk::ListItem>().expect("ListItem");
        let Some(lang) = item.item().and_downcast::<SpellLang>() else {
            return;
        };
        if let Some(label) = item.child().and_downcast::<gtk::Label>() {
            label.set_text(&lang.name());
        }
    });
    factory
}

/// Shared model + settings sync for every spellcheck-language widget. Cheap to
/// clone; all clones share the same model and re-entrancy guard.
#[derive(Clone)]
pub struct SpellBinding {
    settings: gio::Settings,
    model: gtk::SortListModel,
    on_toggle: SpellToggleCallback,
    syncing: Rc<std::cell::Cell<bool>>,
}

impl SpellBinding {
    /// Build the sorted model (favorites persisted through `settings`).
    pub fn new(settings: &gio::Settings) -> Self {
        let favorites = strv_vec(settings, "favorite-spell-check-languages");
        let sorter = build_sorter();
        let model = gtk::SortListModel::new(Some(build_store(&favorites)), Some(sorter.clone()));
        let on_toggle: SpellToggleCallback = Rc::new({
            let settings = settings.clone();
            move |lang: &SpellLang, now: bool| {
                lang.set_favorite(now);
                let mut favs = strv_vec(&settings, "favorite-spell-check-languages");
                let code = lang.code();
                if now {
                    if !favs.contains(&code) {
                        favs.push(code);
                    }
                } else {
                    favs.retain(|c| c != &code);
                }
                let refs: Vec<&str> = favs.iter().map(String::as_str).collect();
                let _ = settings.set_strv("favorite-spell-check-languages", refs);
                sorter.changed(gtk::SorterChange::Different);
            }
        });
        Self {
            settings: settings.clone(),
            model,
            on_toggle,
            syncing: Rc::new(std::cell::Cell::new(false)),
        }
    }

    pub fn model(&self) -> &gtk::SortListModel {
        &self.model
    }

    /// Row-face and popup factories for a `DropDown`/`ComboRow`. Set both
    /// before the model.
    pub fn factories(&self) -> (gtk::SignalListItemFactory, gtk::SignalListItemFactory) {
        (
            build_button_factory(),
            build_list_factory(self.on_toggle.clone()),
        )
    }

    /// Index of the entry to show: the explicit language, else "Automatic"
    /// while auto-detect is on, else the fallback dictionary actually in use.
    fn effective_index(&self) -> u32 {
        let explicit = strv_vec(&self.settings, "spell-checking-languages");
        let auto = self.settings.boolean("auto-detect-language");
        let want = if explicit.iter().any(|c| !c.is_empty()) {
            resolve_languages(&explicit, auto).into_iter().next()
        } else if auto {
            Some(String::new())
        } else {
            resolve_languages(&[], false).into_iter().next()
        };
        let Some(want) = want else { return 0 };
        (0..self.model.n_items())
            .find(|i| {
                self.model
                    .item(*i)
                    .and_downcast::<SpellLang>()
                    .is_some_and(|l| l.code() == want)
            })
            .unwrap_or(0)
    }

    /// Point the widget at the effective language; `set` must not trigger a
    /// settings write (`selected` ignores notifications raised inside it).
    pub fn sync(&self, set: impl Fn(u32)) {
        self.syncing.set(true);
        set(self.effective_index());
        self.syncing.set(false);
    }

    /// Re-run `sync` whenever the language-related settings change. Pass the
    /// result to `unwatch` when the widget goes away (the handlers keep this
    /// binding, and so the settings object, alive).
    pub fn watch(&self, set: impl Fn(u32) + 'static) -> Vec<glib::SignalHandlerId> {
        let set: Rc<dyn Fn(u32)> = Rc::new(set);
        ["spell-checking-languages", "auto-detect-language"]
            .into_iter()
            .map(|key| {
                let this = self.clone();
                let set = set.clone();
                self.settings
                    .connect_changed(Some(key), move |_, _| this.sync(|i| set(i)))
            })
            .collect()
    }

    pub fn unwatch(&self, ids: Vec<glib::SignalHandlerId>) {
        for id in ids {
            self.settings.disconnect(id);
        }
    }

    /// The user picked `lang`. Persists it ("Automatic" clears the list) and
    /// returns the effective languages to push to the running browser, or
    /// `None` when nothing changed.
    pub fn selected(&self, lang: &SpellLang) -> Option<Vec<String>> {
        if self.syncing.get() {
            return None;
        }
        let code = lang.code();
        let explicit: Vec<String> = if code.is_empty() { vec![] } else { vec![code] };
        if strv_vec(&self.settings, "spell-checking-languages") == explicit {
            return None;
        }
        let refs: Vec<&str> = explicit.iter().map(String::as_str).collect();
        let _ = self.settings.set_strv("spell-checking-languages", refs);
        Some(resolve_languages(
            &explicit,
            self.settings.boolean("auto-detect-language"),
        ))
    }
}

fn strv_vec(settings: &gio::Settings, key: &str) -> Vec<String> {
    settings.strv(key).iter().map(|s| s.to_string()).collect()
}

fn update_star_visual(star: &gtk::ToggleButton, favorite: bool) {
    star.set_icon_name(if favorite {
        "starred-symbolic"
    } else {
        "non-starred-symbolic"
    });
}

mod imp {
    use std::cell::{Cell, RefCell};

    use gtk::glib;
    use gtk::prelude::*;
    use gtk::subclass::prelude::*;

    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = super::SpellLang)]
    pub struct SpellLang {
        #[property(get, set)]
        pub code: RefCell<String>,
        #[property(get, set)]
        pub name: RefCell<String>,
        #[property(get, set)]
        pub favorite: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SpellLang {
        const NAME: &'static str = "KarereSpellLang";
        type Type = super::SpellLang;
    }

    #[glib::derived_properties]
    impl ObjectImpl for SpellLang {}
}
