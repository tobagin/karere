use std::os::raw::c_int;

use cef::{
    self, Browser, CefString, ContextMenuHandler, ContextMenuParams, Frame, ImplBrowser,
    ImplContextMenuHandler, ImplContextMenuParams, ImplMenuModel, ImplRunContextMenuCallback,
    MenuItemType, MenuModel, RunContextMenuCallback, WrapContextMenuHandler, rc::Rc,
    wrap_context_menu_handler,
};

/// Owned snapshot of a CEF `MenuModel` entry, decoupled from the non-`Send`
/// refcounted CEF objects so the GTK layer can render it on the main thread.
#[derive(Clone)]
pub enum MenuEntry {
    Separator,
    Item {
        label: String,
        command_id: i32,
        enabled: bool,
    },
    Submenu {
        label: String,
        items: Vec<MenuEntry>,
    },
}

/// Convert a CEF `CefStringUserfree` into a Rust `String`.
fn userfree_to_string(s: cef::CefStringUserfree) -> String {
    CefString::from(&s).to_string()
}

/// Strip Chromium's `&` mnemonic markers from a menu label. A lone `&` is a
/// dropped accelerator marker; `&&` is a literal `&`.
fn strip_mnemonics(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '&' {
            if chars.peek() == Some(&'&') {
                out.push('&');
                chars.next();
            }
            // Lone `&` is a mnemonic marker — drop it.
        } else {
            out.push(c);
        }
    }
    out
}

/// Walk a CEF `MenuModel` into a [`MenuEntry`] tree. Recurses into submenus
/// (the spellcheck suggestion list arrives as one).
fn snapshot_model(model: &MenuModel) -> Vec<MenuEntry> {
    let count = model.count();
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let ty = model.type_at(i);
        if ty == MenuItemType::SEPARATOR {
            out.push(MenuEntry::Separator);
            continue;
        }
        let label = strip_mnemonics(&userfree_to_string(model.label_at(i)));
        if ty == MenuItemType::SUBMENU
            && let Some(sub) = model.sub_menu_at(i)
        {
            out.push(MenuEntry::Submenu {
                label,
                items: snapshot_model(&sub),
            });
            continue;
        }
        out.push(MenuEntry::Item {
            label,
            command_id: model.command_id_at(i),
            enabled: model.is_enabled_at(i) != 0,
        });
    }
    out
}

/// Karere's own "Copy Link Address" entry. CEF's menu has no link items, so
/// links were uncopyable (#205).
pub(crate) const MENU_ID_COPY_LINK: i32 = cef::sys::cef_menu_id_t::MENU_ID_USER_FIRST as i32;

/// Commands the host menu may run: editing, spellcheck and Karere's own Copy
/// Link Address. Everything else CEF offers (navigation, print, view source,
/// open link in new tab/window) is removed from the menu and refused.
pub(crate) fn is_allowed_command(command_id: i32) -> bool {
    use cef::sys::cef_menu_id_t as M;
    let id = command_id;
    (M::MENU_ID_UNDO as i32..=M::MENU_ID_SELECT_ALL as i32).contains(&id)
        || (M::MENU_ID_SPELLCHECK_SUGGESTION_0 as i32..=M::MENU_ID_ADD_TO_DICTIONARY as i32)
            .contains(&id)
        || id == MENU_ID_COPY_LINK
}

/// Classify Chromium's Copy command by stable CEF command id, never by its
/// localized display label.
pub(crate) fn is_copy_command(command_id: i32) -> bool {
    command_id == cef::sys::cef_menu_id_t::MENU_ID_COPY as i32
}

/// Classify Chromium's Paste command by stable CEF command id.
pub(crate) fn is_paste_command(command_id: i32) -> bool {
    command_id == cef::sys::cef_menu_id_t::MENU_ID_PASTE as i32
}

#[derive(Clone, Default)]
pub struct ShellContextMenuHandler;

wrap_context_menu_handler! {
    pub struct ShellContextMenuHandlerBuilder {
        handler: ShellContextMenuHandler,
    }

    impl ContextMenuHandler {
        fn on_before_context_menu(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            params: Option<&mut ContextMenuParams>,
            model: Option<&mut MenuModel>,
        ) {
            let Some(model) = model else { return };
            strip_disallowed_entries(model);
            let has_link = params.is_some_and(|p| !userfree_to_string(p.link_url()).is_empty());
            if has_link {
                let label = CefString::from(gettextrs::gettext("Copy Link Address").as_str());
                let had_items = model.count() > 0;
                model.insert_item_at(0, MENU_ID_COPY_LINK, Some(&label));
                if had_items {
                    model.insert_separator_at(1);
                }
            }
        }

        fn on_context_menu_command(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            params: Option<&mut ContextMenuParams>,
            command_id: c_int,
            _event_flags: cef::EventFlags,
        ) -> c_int {
            if !is_allowed_command(command_id) {
                log::debug!("context menu: refused command {command_id}");
                return 1;
            }
            if command_id != MENU_ID_COPY_LINK {
                return 0;
            }
            if let Some(p) = params {
                super::client::write_regular_clipboard(&userfree_to_string(p.link_url()));
            }
            1
        }

        // OSR has no platform window for CEF to draw the menu, so the host
        // renders it: snapshot the (link-stripped) model and hand it + cursor
        // pos + non-`Send` callback to the GTK widget keyed by browser id.
        // Returning 1 takes ownership; the widget guarantees one `cont`/`cancel`.
        fn run_context_menu(
            &self,
            browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            params: Option<&mut ContextMenuParams>,
            model: Option<&mut MenuModel>,
            callback: Option<&mut RunContextMenuCallback>,
        ) -> c_int {
            let (Some(model), Some(callback)) = (model, callback) else {
                // Nothing to manage → let CEF keep default ownership.
                return 0;
            };
            let callback = callback.clone();

            let items = snapshot_model(model);
            if items.is_empty() {
                callback.cancel();
                return 1;
            }

            // Cursor in view coords — DIP, matching the mouse events we feed CEF.
            let (x, y) = match &params {
                Some(p) => {
                    if log::log_enabled!(log::Level::Debug) {
                        let word = userfree_to_string(p.misspelled_word());
                        if !word.is_empty() {
                            let mut sugg = cef::CefStringList::new();
                            p.dictionary_suggestions(Some(&mut sugg));
                            log::debug!(
                                "context menu: misspelled {word:?} suggestions {:?}",
                                sugg.into_iter().collect::<Vec<_>>()
                            );
                        }
                    }
                    (p.xcoord(), p.ycoord())
                }
                None => (0, 0),
            };

            let browser_id = browser.as_ref().map(|b| b.identifier()).unwrap_or(-1);
            crate::web_view::dispatch_context_menu(browser_id, items, x, y, callback);
            1
        }
    }
}

impl ShellContextMenuHandlerBuilder {
    pub fn build() -> ContextMenuHandler {
        Self::new(ShellContextMenuHandler)
    }
}

/// Remove every entry whose command is not [`is_allowed_command`] (recursing
/// into submenus, dropping ones left empty) and any separator that orphans
/// (leading, trailing, or doubled).
fn strip_disallowed_entries(model: &MenuModel) {
    let count = model.count();
    if count == 0 {
        return;
    }

    let types: Vec<MenuItemType> = (0..count).map(|i| model.type_at(i)).collect();
    let mut remove = vec![false; count];
    for (i, slot) in remove.iter_mut().enumerate() {
        *slot = if types[i] == MenuItemType::SEPARATOR {
            false
        } else if types[i] == MenuItemType::SUBMENU {
            match model.sub_menu_at(i) {
                Some(sub) => {
                    strip_disallowed_entries(&sub);
                    sub.count() == 0
                }
                None => true,
            }
        } else {
            !is_allowed_command(model.command_id_at(i))
        };
    }

    // Leave menus with nothing to remove untouched.
    if remove.iter().any(|&r| r) {
        normalize_separators(&types, &mut remove);
    }

    // Remove from the back so earlier indices stay valid.
    for i in (0..count).rev() {
        if remove[i] {
            model.remove_at(i);
        }
    }
}

/// Mark separators that become leading, trailing, or doubled once the forbidden
/// entries are gone.
fn normalize_separators(types: &[MenuItemType], remove: &mut [bool]) {
    // Treat menu start as a separator boundary so a leading separator drops.
    let mut prev_kept_is_separator = true;
    let mut last_kept: Option<usize> = None;

    for i in 0..types.len() {
        if remove[i] {
            continue;
        }
        if types[i] == MenuItemType::SEPARATOR {
            if prev_kept_is_separator {
                remove[i] = true;
                continue;
            }
            prev_kept_is_separator = true;
        } else {
            prev_kept_is_separator = false;
        }
        last_kept = Some(i);
    }

    if let Some(i) = last_kept
        && types[i] == MenuItemType::SEPARATOR
    {
        remove[i] = true;
    }
}

#[cfg(test)]
mod tests {
    use super::{MENU_ID_COPY_LINK, is_allowed_command, is_copy_command, is_paste_command};

    #[test]
    fn only_edit_spellcheck_and_copy_link_commands_are_allowed() {
        use cef::sys::cef_menu_id_t as M;
        for id in [
            M::MENU_ID_UNDO,
            M::MENU_ID_CUT,
            M::MENU_ID_COPY,
            M::MENU_ID_PASTE,
            M::MENU_ID_SELECT_ALL,
            M::MENU_ID_SPELLCHECK_SUGGESTION_0,
            M::MENU_ID_SPELLCHECK_SUGGESTION_4,
            M::MENU_ID_NO_SPELLING_SUGGESTIONS,
            M::MENU_ID_ADD_TO_DICTIONARY,
        ] {
            assert!(is_allowed_command(id as i32), "{id:?}");
        }
        assert!(is_allowed_command(MENU_ID_COPY_LINK));
        for id in [
            M::MENU_ID_BACK,
            M::MENU_ID_RELOAD,
            M::MENU_ID_PRINT,
            M::MENU_ID_VIEW_SOURCE,
            M::MENU_ID_FIND,
        ] {
            assert!(!is_allowed_command(id as i32), "{id:?}");
        }
        // Chromium's "Open link in new tab/window/incognito".
        for id in [50100, 50101, 50102, 0, -1] {
            assert!(!is_allowed_command(id));
        }
    }

    #[test]
    fn copy_is_classified_by_cef_command_id_not_label() {
        let copy = cef::sys::cef_menu_id_t::MENU_ID_COPY as i32;
        let paste = cef::sys::cef_menu_id_t::MENU_ID_PASTE as i32;
        assert!(is_copy_command(copy));
        assert!(!is_copy_command(paste));
        assert!(!is_copy_command(0));
        assert!(is_paste_command(paste));
        assert!(!is_paste_command(copy));
    }
}
