use cef::{
    self, Browser, ContextMenuHandler, ContextMenuParams, Frame, ImplContextMenuHandler,
    ImplMenuModel, MenuItemType, MenuModel, WrapContextMenuHandler, rc::Rc,
    wrap_context_menu_handler,
};

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
