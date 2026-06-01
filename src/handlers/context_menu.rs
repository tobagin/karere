use std::os::raw::c_int;

use cef::{
    self, Browser, CefString, ContextMenuHandler, ContextMenuParams, Frame, ImplBrowser,
    ImplContextMenuParams, ImplContextMenuHandler, ImplMenuModel, ImplRunContextMenuCallback,
    MenuItemType, MenuModel, RunContextMenuCallback, WrapContextMenuHandler, rc::Rc,
    wrap_context_menu_handler,
};

/// A plain, owned snapshot of one entry in a CEF `MenuModel`, decoupled from the
/// non-`Send`/refcounted CEF objects so the GTK layer can render it on the main
/// thread. Built by [`snapshot_model`].
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

/// Convert a CEF `CefStringUserfree` (UTF-16, owned) into a Rust `String`.
fn userfree_to_string(s: cef::CefStringUserfree) -> String {
    CefString::from(&s).to_string()
}

/// Strip Chromium's Windows-style `&` mnemonic markers from a menu label so the
/// GTK menu shows "Undo"/"Cut" instead of "&Undo"/"Cu&t". A lone `&` flags the
/// next character as the accelerator and is dropped; `&&` is a literal `&`.
fn strip_mnemonics(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '&' {
            if chars.peek() == Some(&'&') {
                out.push('&');
                chars.next();
            }
            // Otherwise the lone `&` is a mnemonic marker — drop it.
        } else {
            out.push(c);
        }
    }
    out
}

/// Walk a CEF `MenuModel` into a plain [`MenuEntry`] tree (labels, command ids,
/// enabled state, separators, nested submenus). Recurses into submenus — the
/// spellcheck suggestion list arrives as one.
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

// Chromium content context-menu command ids (chrome/app/chrome_command_ids.h).
// cef-rs 148 does not surface these as named `cef_menu_id_t` constants — that
// enum stops at MENU_ID_VIEW_SOURCE — so the link-open ids are pinned here by
// value. Re-verify against the live menu (see verification step 6.2) whenever
// the bundled CEF/Chromium version changes.
const IDC_CONTENT_CONTEXT_OPENLINKNEWTAB: i32 = 50100;
const IDC_CONTENT_CONTEXT_OPENLINKNEWWINDOW: i32 = 50101;
const IDC_CONTENT_CONTEXT_OPENLINKOFFTHERECORD: i32 = 50102;

const FORBIDDEN: [i32; 3] = [
    IDC_CONTENT_CONTEXT_OPENLINKNEWWINDOW,
    IDC_CONTENT_CONTEXT_OPENLINKNEWTAB,
    IDC_CONTENT_CONTEXT_OPENLINKOFFTHERECORD,
];

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
            _params: Option<&mut ContextMenuParams>,
            model: Option<&mut MenuModel>,
        ) {
            let Some(model) = model else { return };
            strip_open_link_entries(model);
        }

        // OSR has no platform window for CEF to draw the menu into, so the host
        // must render it. Snapshot the (already link-stripped) model, then hand
        // it plus the cursor position and the non-`Send` callback to the GTK
        // widget keyed by browser id. Returning 1 takes ownership of display;
        // the widget guarantees exactly one `cont`/`cancel`.
        fn run_context_menu(
            &self,
            browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            params: Option<&mut ContextMenuParams>,
            model: Option<&mut MenuModel>,
            callback: Option<&mut RunContextMenuCallback>,
        ) -> c_int {
            let (Some(model), Some(callback)) = (model, callback) else {
                // No model/callback to manage → let CEF keep default ownership.
                return 0;
            };
            let callback = callback.clone();

            let items = snapshot_model(model);
            if items.is_empty() {
                callback.cancel();
                return 1;
            }

            // Cursor in view (device-pixel) coordinates — the GTK layer divides
            // by the scale factor for placement.
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

/// Remove the "Open Link in New Window/Tab/Incognito" entries and any separator
/// they leave orphaned (leading, trailing, or doubled).
fn strip_open_link_entries(model: &MenuModel) {
    let count = model.count();
    if count == 0 {
        return;
    }

    let types: Vec<MenuItemType> = (0..count).map(|i| model.type_at(i)).collect();
    let mut remove = vec![false; count];
    for (i, slot) in remove.iter_mut().enumerate() {
        if FORBIDDEN.contains(&model.command_id_at(i)) {
            *slot = true;
        }
    }

    // Leave unrelated menus (no link-open entries) completely untouched.
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
    // Treat the menu start as a separator boundary so a leading separator drops.
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
