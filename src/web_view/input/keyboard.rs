use cef::{self, CefString, ImplBrowserHost, KeyEvent, KeyEventType, sys};
use gtk::prelude::*;
use gtk::subclass::prelude::*;

use crate::web_view::imp::with_host;
use crate::web_view::input::modifiers_from_state;

/// Interval of the IM focus-out debounce timer (`arm_im_focus_timer`).
pub(in crate::web_view) const IM_FOCUS_TICK: std::time::Duration = std::time::Duration::from_millis(16);

/// Ticks a focus-out waits before it is applied. The debounce timer ticks
/// every `IM_FOCUS_TICK`, so this is ~64 ms — longer than the gap between the editable-focus false
/// and true a composer re-render emits, and short enough that a real blur
/// still hides an on-screen keyboard promptly. (#180)
pub(super) const IM_FOCUS_OUT_DEBOUNCE_TICKS: u8 = 4;

/// What one drain tick should do to the IM context's focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::web_view) enum ImFocusAction {
    /// Leave the IM as it is.
    Idle,
    /// Call `focus_in` and mark the IM focused.
    FocusIn,
    /// Call `focus_out` and mark the IM unfocused.
    FocusOut,
}

/// Decide one tick of the IM focus debounce.
///
/// `request` is the editable-focus value drained from CEF this tick (`None`
/// when the page said nothing), `focused` mirrors `im_focused`, and `pending`
/// is the focus-out countdown, updated in place.
///
/// A `true` wins immediately and cancels any queued focus-out — that pairing
/// is what a composer re-render produces, and acting on its `false` is what
/// dropped keys in #180. A `false` only arms the countdown; the focus-out
/// lands on a later tick if nothing re-focuses first.
pub(in crate::web_view) fn im_focus_step(
    request: Option<bool>,
    focused: bool,
    pending: &mut u8,
) -> ImFocusAction {
    match request {
        Some(true) => {
            *pending = 0;
            if focused {
                ImFocusAction::Idle
            } else {
                ImFocusAction::FocusIn
            }
        }
        Some(false) => {
            // Nothing to defer if the IM is already out.
            if focused && *pending == 0 {
                *pending = IM_FOCUS_OUT_DEBOUNCE_TICKS;
            }
            ImFocusAction::Idle
        }
        None => {
            if *pending == 0 {
                return ImFocusAction::Idle;
            }
            *pending -= 1;
            if *pending == 0 && focused {
                ImFocusAction::FocusOut
            } else {
                ImFocusAction::Idle
            }
        }
    }
}

/// UTF-16 unit to synthesize as a CEF `CHAR` for a key the IM declined, or
/// `None` when the key is not text. Ctrl/Alt/Super combos are shortcuts, and
/// C0/DEL keyvals (Enter, Tab, Backspace, Escape) keep their raw-key-only
/// handling — the page acts on those from the key-down. (#180)
pub(super) fn printable_char(
    keyval: gtk::gdk::Key,
    state: gtk::gdk::ModifierType,
) -> Option<u16> {
    use gtk::gdk::ModifierType;
    if state.intersects(
        ModifierType::CONTROL_MASK | ModifierType::ALT_MASK | ModifierType::SUPER_MASK,
    ) {
        return None;
    }
    let ch = keyval.to_unicode()?;
    if (ch as u32) < 0x20 || ch == '\u{7f}' {
        return None;
    }
    let mut buf = [0u16; 2];
    match ch.encode_utf16(&mut buf) {
        [unit] => Some(*unit),
        // Astral-plane keyvals can't be expressed as one UTF-16 unit; the IM
        // commit path handles those (no physical key produces them directly).
        _ => None,
    }
}

/// CHAR (`\r`) to send after Enter's key-down. Blink inserts a newline in a
/// `<textarea>` and submits an `<input>` from this CHAR, not the key-down,
/// and no IM commits one. Ctrl/Alt/Super+Enter are shortcuts. (I2)
pub(super) fn enter_char(keyval: gtk::gdk::Key, state: gtk::gdk::ModifierType) -> Option<u16> {
    use gtk::gdk::{Key, ModifierType};
    if state.intersects(
        ModifierType::CONTROL_MASK | ModifierType::ALT_MASK | ModifierType::SUPER_MASK,
    ) {
        return None;
    }
    matches!(keyval, Key::Return | Key::KP_Enter | Key::ISO_Enter).then_some(0x0D)
}

/// Input-method composition state, kept on the widget. `preedit`: the IM
/// shows a non-empty preedit. `cef_composing`: the page holds a composition
/// we set. `session`: a composition ran since the last key that started
/// outside one, so its commit must replace that composition. (I1)
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ImeState {
    preedit: bool,
    cef_composing: bool,
    session: bool,
}

/// What a preedit change asks of CEF.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ImeAction {
    Idle,
    /// `ime_set_composition` with the current preedit.
    SetComposition,
    /// `ime_cancel_composition`: the preedit emptied without a commit.
    Cancel,
}

/// How committed IM text reaches the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ImeCommit {
    /// `ime_commit_text`: ends (replaces) a composition.
    Composition,
    /// CHAR events: ordinary typing through the IM, as before. (#154/#180)
    Chars,
}

impl ImeState {
    /// Whether a key press goes to the IM before CEF sees its raw key-down.
    /// Only while composing; otherwise the #180 order (raw key-down first)
    /// stays.
    pub(super) fn filter_first(&self) -> bool {
        self.preedit || self.cef_composing
    }

    /// A key press outside any composition closes the previous session, so
    /// ordinary typing commits as CHARs again. An async IM (ibus) may commit
    /// after its key's handler returned; the session survives until here.
    pub(super) fn key_press_begin(&mut self) {
        if !self.filter_first() {
            self.session = false;
        }
    }

    pub(super) fn preedit_start(&mut self) {
        self.session = true;
    }

    pub(super) fn preedit_changed(&mut self, empty: bool) -> ImeAction {
        self.preedit = !empty;
        if !empty {
            self.session = true;
            self.cef_composing = true;
            ImeAction::SetComposition
        } else if std::mem::take(&mut self.cef_composing) {
            ImeAction::Cancel
        } else {
            ImeAction::Idle
        }
    }

    pub(super) fn preedit_end(&mut self) -> ImeAction {
        self.preedit_changed(true)
    }

    /// GtkIMContextSimple ends a dead-key sequence with preedit-end then
    /// commit; ibus may commit first. Either order lands the text exactly
    /// once: through `ime_commit_text` while in a session.
    pub(super) fn commit(&mut self) -> ImeCommit {
        let route = if self.session || self.cef_composing {
            ImeCommit::Composition
        } else {
            ImeCommit::Chars
        };
        // CEF ends the composition on commit; a preedit that continues
        // (partial commit) opens a new one on its next change.
        self.cef_composing = false;
        route
    }
}

/// `CefRange::InvalidRange()`: "no replacement range", i.e. the current
/// composition or selection.
pub(super) fn ime_invalid_range() -> cef::Range {
    cef::Range {
        from: u32::MAX,
        to: u32::MAX,
    }
}

pub(super) fn apply_ime_action(widget: &crate::web_view::KarereWebView, action: ImeAction, text: &str, cursor: i32) {
    match action {
        ImeAction::Idle => {}
        ImeAction::SetComposition => send_ime_composition(widget, text, cursor),
        ImeAction::Cancel => with_host(widget, |host| host.ime_cancel_composition()),
    }
}

/// Show `text` as the page's composition, underlined, with the caret at
/// `cursor_chars` (GTK counts characters; CEF counts UTF-16 units).
fn send_ime_composition(widget: &crate::web_view::KarereWebView, text: &str, cursor_chars: i32) {
    let (len, caret) = ime_utf16_extent(text, cursor_chars);
    let underline = cef::CompositionUnderline {
        range: cef::Range { from: 0, to: len },
        // Transparent = drawn in the text colour, so it shows in dark mode.
        color: 0,
        background_color: 0,
        thick: 0,
        style: cef::CompositionUnderlineStyle::SOLID,
        ..Default::default()
    };
    let text = CefString::from(text);
    with_host(widget, |host| {
        host.ime_set_composition(
            Some(&text),
            Some(std::slice::from_ref(&underline)),
            Some(&ime_invalid_range()),
            Some(&cef::Range {
                from: caret,
                to: caret,
            }),
        )
    });
}

/// UTF-16 length of `text` and the UTF-16 offset of character `cursor_chars`.
pub(super) fn ime_utf16_extent(text: &str, cursor_chars: i32) -> (u32, u32) {
    let len = text.encode_utf16().count() as u32;
    let caret = text
        .chars()
        .take(cursor_chars.max(0) as usize)
        .map(char::len_utf16)
        .sum::<usize>() as u32;
    (len, caret.min(len))
}

/// Drop any open composition: the IM's (it may commit it, which then
/// replaces the page's composition) and then the page's. Focus-out,
/// account switch, browser change, and a click while composing. (I1)
pub(in crate::web_view) fn reset_ime(widget: &crate::web_view::KarereWebView) {
    let imp = widget.imp();
    let im = imp.im_context.borrow().clone();
    if let Some(im) = im {
        im.reset();
    }
    if imp.ime_state.replace(ImeState::default()).cef_composing {
        with_host(widget, |host| host.ime_cancel_composition());
    }
    // The next composition reports its own range; until then the click
    // position is the better guess.
    if let Some(shared) = imp.shared.lock().as_ref() {
        shared.lock().ime_caret_rect = None;
    }
}

/// Place the IM candidate window at the page's composition, from the last
/// range CEF reported (`on_ime_composition_range_changed`, view = physical
/// px). Without one, the location set on the last click stays. (I1)
pub(super) fn update_ime_cursor_location(widget: &crate::web_view::KarereWebView) {
    let imp = widget.imp();
    let rect = imp
        .shared
        .lock()
        .as_ref()
        .and_then(|shared| shared.lock().ime_caret_rect);
    let Some((x, y, w, h)) = rect else {
        return;
    };
    let s = widget.scale_factor().max(1);
    let im = imp.im_context.borrow().clone();
    if let Some(im) = im {
        im.set_cursor_location(&gtk::gdk::Rectangle::new(
            x / s,
            y / s,
            (w / s).max(1),
            (h / s).max(1),
        ));
    }
}

pub(super) fn is_accelerator_key(keyval: gtk::gdk::Key, state: gtk::gdk::ModifierType) -> bool {
    use gtk::gdk::{Key, ModifierType};
    if state.intersects(
        ModifierType::CONTROL_MASK | ModifierType::ALT_MASK | ModifierType::SUPER_MASK,
    ) {
        return true;
    }
    matches!(keyval, Key::F5 | Key::F11)
}

/// Send only the raw key-down/up (RAWKEYDOWN / KEYUP) — no CHAR. Character
/// insertion is driven separately by the IM `commit` (`send_char`), so dead
/// keys compose and there's no double-insert on the first key. (#154)
pub(super) fn send_key_raw(
    widget: &crate::web_view::KarereWebView,
    keyval: gtk::gdk::Key,
    keycode: u32,
    layout: u32,
    state: gtk::gdk::ModifierType,
    down: bool,
) {
    let evt = KeyEvent {
        size: std::mem::size_of::<sys::_cef_key_event_t>(),
        type_: if down {
            KeyEventType::RAWKEYDOWN
        } else {
            KeyEventType::KEYUP
        },
        modifiers: modifiers_from_state(state),
        windows_key_code: gdk_key_to_vk(
            keyval,
            unshifted_keyval(widget, keycode, layout),
            keycode,
        ),
        native_key_code: keycode as i32,
        is_system_key: 0,
        character: 0,
        unmodified_character: 0,
        focus_on_editable_field: 0,
    };
    with_host(widget, |host| {
        host.send_key_event(Some(&evt));
    });
}

/// Send a single composed UTF-16 unit as a CEF CHAR event. Used for IM
/// `commit` text (dead-key / IME composition). (#154)
pub(super) fn send_char(widget: &crate::web_view::KarereWebView, code: u16, modifiers: u32) {
    if code == 0 {
        return;
    }
    let evt = KeyEvent {
        size: std::mem::size_of::<sys::_cef_key_event_t>(),
        type_: KeyEventType::CHAR,
        modifiers,
        windows_key_code: code as i32,
        native_key_code: 0,
        is_system_key: 0,
        character: code,
        unmodified_character: code,
        focus_on_editable_field: 0,
    };
    with_host(widget, |host| {
        host.send_key_event(Some(&evt));
    });
}

pub(in crate::web_view) fn set_focus(widget: &crate::web_view::KarereWebView, focused: bool) {
    with_host(widget, |host| host.set_focus(focused as i32));
}

/// Synthesize Ctrl+Shift+C to toggle the embedded DevTools "inspect element" picker.
pub fn send_inspect_shortcut(widget: &crate::web_view::KarereWebView) {
    use sys::cef_event_flags_t as F;
    let modifiers = F::EVENTFLAG_CONTROL_DOWN.0 | F::EVENTFLAG_SHIFT_DOWN.0;
    let down = KeyEvent {
        size: std::mem::size_of::<sys::_cef_key_event_t>(),
        type_: KeyEventType::RAWKEYDOWN,
        modifiers,
        windows_key_code: 0x43, // VK_C
        native_key_code: 0,
        is_system_key: 0,
        character: 0,
        unmodified_character: 0,
        focus_on_editable_field: 0,
    };
    let up = KeyEvent {
        type_: KeyEventType::KEYUP,
        ..down.clone()
    };
    with_host(widget, |host| {
        host.set_focus(1);
        host.send_key_event(Some(&down));
        host.send_key_event(Some(&up));
    });
}

/// The level-0 (unshifted) keyval of hardware `keycode` in keyboard
/// `layout`, so Shift+/ still maps to the '/' key's VK.
fn unshifted_keyval(
    widget: &crate::web_view::KarereWebView,
    keycode: u32,
    layout: u32,
) -> Option<gtk::gdk::Key> {
    widget
        .display()
        .map_keycode(keycode)?
        .into_iter()
        .find(|(k, _)| k.group() == layout as i32 && k.level() == 0)
        .map(|(_, key)| key)
}

/// Windows virtual-key code Chromium expects in `windows_key_code`: named
/// keys and letters/digits from the keyval; punctuation from the unshifted
/// keyval of the same key, then the keyval itself; last, the key's
/// US-layout position (hardware keycode). 0 when nothing fits; text still
/// arrives through the IM commit / CHAR path either way. (I2)
pub(super) fn gdk_key_to_vk(
    keyval: gtk::gdk::Key,
    unshifted: Option<gtk::gdk::Key>,
    keycode: u32,
) -> i32 {
    let alnum = |k: gtk::gdk::Key| {
        k.to_unicode()
            .filter(|c| c.is_ascii_alphanumeric())
            .and_then(|_| ascii_vk(k))
    };
    named_vk(keyval)
        .or_else(|| alnum(keyval))
        .or_else(|| unshifted.and_then(ascii_vk))
        .or_else(|| ascii_vk(keyval))
        .or_else(|| keycode_vk(keycode))
        .unwrap_or(0)
}

/// Keys whose VK does not depend on the layout.
fn named_vk(keyval: gtk::gdk::Key) -> Option<i32> {
    use gtk::gdk::Key;
    use gtk::glib::translate::IntoGlib;
    Some(match keyval {
        Key::Break => 0x03, // VK_CANCEL (Ctrl+Pause)
        Key::BackSpace => 0x08,
        Key::Tab | Key::ISO_Left_Tab | Key::KP_Tab => 0x09,
        Key::Clear | Key::KP_Begin => 0x0C,
        Key::Return | Key::KP_Enter | Key::ISO_Enter => 0x0D,
        Key::Shift_L | Key::Shift_R => 0x10,
        Key::Control_L | Key::Control_R => 0x11,
        Key::Alt_L | Key::Alt_R | Key::Meta_L | Key::Meta_R => 0x12, // VK_MENU
        Key::Pause => 0x13,
        Key::Caps_Lock => 0x14,
        Key::Escape => 0x1B,
        Key::space | Key::KP_Space => 0x20,
        Key::Page_Up | Key::KP_Page_Up => 0x21,
        Key::Page_Down | Key::KP_Page_Down => 0x22,
        Key::End | Key::KP_End => 0x23,
        Key::Home | Key::KP_Home => 0x24,
        Key::Left | Key::KP_Left => 0x25,
        Key::Up | Key::KP_Up => 0x26,
        Key::Right | Key::KP_Right => 0x27,
        Key::Down | Key::KP_Down => 0x28,
        Key::Select => 0x29,
        Key::Execute => 0x2B,
        Key::Print | Key::Sys_Req => 0x2C, // VK_SNAPSHOT
        Key::Insert | Key::KP_Insert => 0x2D,
        Key::Delete | Key::KP_Delete => 0x2E,
        Key::Help => 0x2F,
        Key::Super_L => 0x5B,
        Key::Super_R => 0x5C,
        Key::Menu => 0x5D, // VK_APPS
        Key::KP_0 => 0x60,
        Key::KP_1 => 0x61,
        Key::KP_2 => 0x62,
        Key::KP_3 => 0x63,
        Key::KP_4 => 0x64,
        Key::KP_5 => 0x65,
        Key::KP_6 => 0x66,
        Key::KP_7 => 0x67,
        Key::KP_8 => 0x68,
        Key::KP_9 => 0x69,
        Key::KP_Multiply => 0x6A,
        Key::KP_Add => 0x6B,
        Key::KP_Separator => 0x6C,
        Key::KP_Subtract => 0x6D,
        Key::KP_Decimal => 0x6E,
        Key::KP_Divide => 0x6F,
        Key::KP_Equal => 0xBB,
        Key::Num_Lock => 0x90,
        Key::Scroll_Lock => 0x91,
        Key::ISO_Level3_Shift => 0xE1, // VKEY_ALTGR
        k if (Key::F1.into_glib()..=Key::F24.into_glib()).contains(&k.into_glib()) => {
            0x70 + (k.into_glib() - Key::F1.into_glib()) as i32
        }
        _ => return None,
    })
}

/// Letters and digits map to their ASCII uppercase value (VK_A..VK_Z,
/// VK_0..VK_9). Punctuation must NOT use its ASCII value: '.' is 0x2E =
/// VK_DELETE, and Chromium then runs the Delete command on the key-down
/// and drops the character. It gets the `VK_OEM_*` code of its US-layout
/// key instead, shifted symbols included ('?' is the '/' key, VK_OEM_2).
fn ascii_vk(keyval: gtk::gdk::Key) -> Option<i32> {
    let c = keyval.to_unicode()?;
    if c.is_ascii_alphanumeric() {
        return Some(c.to_ascii_uppercase() as i32);
    }
    Some(match c {
        ';' | ':' => 0xBA,  // VK_OEM_1
        '=' | '+' => 0xBB,  // VK_OEM_PLUS
        ',' | '<' => 0xBC,  // VK_OEM_COMMA
        '-' | '_' => 0xBD,  // VK_OEM_MINUS
        '.' | '>' => 0xBE,  // VK_OEM_PERIOD
        '/' | '?' => 0xBF,  // VK_OEM_2
        '`' | '~' => 0xC0,  // VK_OEM_3
        '[' | '{' => 0xDB,  // VK_OEM_4
        '\\' | '|' => 0xDC, // VK_OEM_5
        ']' | '}' => 0xDD,  // VK_OEM_6
        '\'' | '"' => 0xDE, // VK_OEM_7
        _ => return None,
    })
}

/// VK of the key at this position on a US layout (XKB keycode = evdev + 8).
/// Last resort for keyvals with no ASCII meaning: non-Latin layouts, dead
/// keys, shifted digits whose unshifted keyval is not known.
fn keycode_vk(keycode: u32) -> Option<i32> {
    const ROW_QWERTY: &[u8] = b"QWERTYUIOP";
    const ROW_ASDF: &[u8] = b"ASDFGHJKL";
    const ROW_ZXCV: &[u8] = b"ZXCVBNM";
    let evdev = keycode.checked_sub(8)?;
    Some(match evdev {
        2..=10 => 0x31 + (evdev - 2) as i32, // 1..9
        11 => 0x30,                          // 0
        12 => 0xBD,                          // minus
        13 => 0xBB,                          // equal
        16..=25 => ROW_QWERTY[(evdev - 16) as usize] as i32,
        26 => 0xDB, // left brace
        27 => 0xDD, // right brace
        30..=38 => ROW_ASDF[(evdev - 30) as usize] as i32,
        39 => 0xBA, // semicolon
        40 => 0xDE, // apostrophe
        41 => 0xC0, // grave
        43 => 0xDC, // backslash
        44..=50 => ROW_ZXCV[(evdev - 44) as usize] as i32,
        51 => 0xBC, // comma
        52 => 0xBE, // dot
        53 => 0xBF, // slash
        86 => 0xE2, // VK_OEM_102 (ISO key left of Z)
        _ => return None,
    })
}
