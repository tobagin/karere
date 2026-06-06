//! Spellcheck-language dropdown: `SpellLang` item type, model, sorter, and row
//! factories. The window owns the `Gtk.DropDown` and GSettings/webview wiring.
//!
//! Sorting: starred favorites float to the top, then alphabetical. Toggling a
//! star updates the item's `favorite`; the caller persists
//! `favorite-spell-check-languages` and calls `sorter.changed(...)` to re-sort.

use std::rc::Rc;

use gtk::glib;
use gtk::prelude::*;

use crate::spellcheck::{KNOWN_LANGUAGES, display_name};

glib::wrapper! {
    /// One spellcheck language row: BCP-47 `code`, friendly `name`, `favorite`.
    pub struct SpellLang(ObjectSubclass<imp::SpellLang>);
}

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
    for (code, _name) in KNOWN_LANGUAGES {
        let fav = favorites.iter().any(|f| f == code);
        // display_name keeps friendly names in one source of truth.
        store.append(&SpellLang::new(code, &display_name(code), fav));
    }
    store
}

/// Sorter: favorites first, then case-insensitive by display name.
pub fn build_sorter() -> gtk::CustomSorter {
    gtk::CustomSorter::new(move |a, b| {
        let a = a.downcast_ref::<SpellLang>().expect("SpellLang");
        let b = b.downcast_ref::<SpellLang>().expect("SpellLang");
        match (a.favorite(), b.favorite()) {
            (true, false) => gtk::Ordering::Smaller,
            (false, true) => gtk::Ordering::Larger,
            _ => a
                .name()
                .to_lowercase()
                .cmp(&b.name().to_lowercase())
                .into(),
        }
    })
}

/// Factory for the popup rows: friendly name + a trailing star `ToggleButton`.
/// `on_toggle(lang, now_favorite)` fires when the user clicks a star.
pub fn build_list_factory(
    on_toggle: Rc<dyn Fn(&SpellLang, bool)>,
) -> gtk::SignalListItemFactory {
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
        star.set_tooltip_text(Some("Pin to top"));

        row.append(&label);
        row.append(&star);
        item.set_child(Some(&row));
    });

    factory.connect_bind(move |_, item| {
        let item = item.downcast_ref::<gtk::ListItem>().expect("ListItem");
        let Some(lang) = item.item().and_downcast::<SpellLang>() else {
            return;
        };
        let Some(row) = item.child().and_downcast::<gtk::Box>() else {
            return;
        };
        let label = row.first_child().and_downcast::<gtk::Label>().unwrap();
        let star = row.last_child().and_downcast::<gtk::ToggleButton>().unwrap();

        label.set_text(&lang.name());
        update_star_visual(&star, lang.favorite());
        star.set_active(lang.favorite());

        // Reconnect cleanly each bind: stash the handler id on the button.
        if let Some(id) = unsafe { star.steal_data::<glib::SignalHandlerId>("toggle-id") } {
            star.disconnect(id);
        }
        let on_toggle = on_toggle.clone();
        let lang_weak = lang.downgrade();
        let id = star.connect_toggled(move |btn| {
            let Some(lang) = lang_weak.upgrade() else { return };
            let now = btn.is_active();
            update_star_visual(btn, now);
            on_toggle(&lang, now);
        });
        unsafe { star.set_data("toggle-id", id) };
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
