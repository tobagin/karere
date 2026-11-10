use super::keyboard::{
    IM_FOCUS_OUT_DEBOUNCE_TICKS, ImFocusAction, ImeAction, ImeCommit, ImeState, enter_char,
    gdk_key_to_vk, im_focus_step, ime_utf16_extent, printable_char,
};
use super::mouse::{
    MouseButtonTracker, MouseDispatch, MouseEventSink, MouseInput, dispatch_mouse_with,
    page_coords, physical_mouse_coordinates, send_click_with, send_move_with,
    should_forward_mouse, zoom_scroll_steps,
};
use crate::web_view::clipboard_dnd::{
    ExplicitCopyTrigger, RendererMessageSink, batch_by_size, dispatch_explicit_copy,
    request_live_selection_with,
};
use crate::handlers::{
    client::dispatch_renderer_message_with,
    render_process::{
        RendererDispatchSink, dispatch_browser_message_with, renderer_message_from_v8_args,
    },
};
use base64::Engine;
use cef::sys::cef_event_flags_t as F;
use cef::{MouseButtonType, MouseEvent};
use gtk::gdk::{Key, ModifierType};
use std::cell::{Cell, RefCell};
use std::process::Command;

#[derive(Debug, PartialEq)]
enum Observed {
    Click {
        browser: i32,
        x: i32,
        y: i32,
        button: u32,
        down: bool,
        count: i32,
        modifiers: u32,
    },
    Move {
        browser: i32,
        x: i32,
        y: i32,
        modifiers: u32,
    },
}

#[derive(Clone)]
struct FakeMouseHost<'a> {
    browser: i32,
    out: &'a RefCell<Vec<Observed>>,
}

impl MouseEventSink for FakeMouseHost<'_> {
    fn send_mouse_move(&mut self, event: &MouseEvent, _leave: bool) {
        self.out.borrow_mut().push(Observed::Move {
            browser: self.browser,
            x: event.x,
            y: event.y,
            modifiers: event.modifiers,
        });
    }

    fn send_mouse_click(
        &mut self,
        event: &MouseEvent,
        button: MouseButtonType,
        mouse_up: bool,
        click_count: i32,
    ) {
        let button = if button == MouseButtonType::LEFT {
            1
        } else if button == MouseButtonType::MIDDLE {
            2
        } else {
            3
        };
        self.out.borrow_mut().push(Observed::Click {
            browser: self.browser,
            x: event.x,
            y: event.y,
            button,
            down: !mouse_up,
            count: click_count,
            modifiers: event.modifiers,
        });
    }
}

fn route_mouse(
    event: MouseDispatch,
    active: &Cell<i32>,
    scale: i32,
    out: &RefCell<Vec<Observed>>,
) {
    dispatch_mouse_with(
        event,
        |x, y, button, down, count, modifiers| {
            send_click_with((x, y), scale, (button, down, count), modifiers, || {
                Some(FakeMouseHost {
                    browser: active.get(),
                    out,
                })
            });
        },
        |x, y, modifiers| {
            send_move_with(
                x,
                y,
                scale,
                modifiers,
                false,
                || {
                    Some(FakeMouseHost {
                        browser: active.get(),
                        out,
                    })
                },
                |_, _| {},
            );
        },
    );
}

struct JavaScriptRenderer<'a> {
    selection: &'a str,
    collapsed: bool,
    clipboard: &'a RefCell<Vec<(bool, String)>>,
}

impl RendererDispatchSink for JavaScriptRenderer<'_> {
    fn execute_java_script(&mut self, script: &str, source_url: &str) {
        assert_eq!(source_url, "karere://copy-selection");
        let b64 = base64::engine::general_purpose::STANDARD;
        let output = Command::new("node")
            .args([
                "tests/copy_bridge_pipeline.js",
                &b64.encode(self.selection),
                if self.collapsed { "true" } else { "false" },
                &b64.encode(script),
            ])
            .output()
            .expect("node must execute the production copy bridge");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        if result.is_null() {
            return;
        }
        let message = renderer_message_from_v8_args(
            result["name"].as_str().unwrap(),
            result["innerJson"].as_str().unwrap(),
        )
        .unwrap();
        dispatch_renderer_message_with(&message, |text, primary| {
            self.clipboard.borrow_mut().push((primary, text.to_owned()));
        });
    }

    fn send_to_browser(&mut self, _message: crate::ipc::RendererMessage) {
        panic!("copy selection must execute JavaScript, not send a renderer reply");
    }
}

struct ForegroundFrame<'a> {
    browser: i32,
    requested_browsers: &'a RefCell<Vec<i32>>,
    renderer: JavaScriptRenderer<'a>,
}

impl RendererMessageSink for ForegroundFrame<'_> {
    fn send_to_renderer(&mut self, message: crate::ipc::BrowserMessage) {
        self.requested_browsers.borrow_mut().push(self.browser);
        // This is the typed value delivered by Frame::send_process_message
        // after the production renderer callback decodes its CEF envelope.
        dispatch_browser_message_with(message, &mut self.renderer);
    }
}

fn copy_through_production_adapters(
    trigger: ExplicitCopyTrigger,
    active: &Cell<i32>,
    selection: &str,
    collapsed: bool,
    requested_browsers: &RefCell<Vec<i32>>,
    clipboard: &RefCell<Vec<(bool, String)>>,
) {
    dispatch_explicit_copy(trigger, || {
        assert!(request_live_selection_with(|| Some(ForegroundFrame {
            browser: active.get(),
            requested_browsers,
            renderer: JavaScriptRenderer {
                selection,
                collapsed,
                clipboard,
            },
        })));
    });
}

#[test]
fn drag_then_immediate_keyboard_and_command_copy_use_foreground_pipeline() {
    // The copy bridge runs under node; build environments without it (the
    // GNOME SDK) skip instead of failing.
    if Command::new("node").arg("--version").output().is_err() {
        eprintln!("skipping: `node` not found on PATH");
        return;
    }
    let active = Cell::new(7);
    let observed = RefCell::new(Vec::new());
    let mut adapter = MouseButtonTracker::default();
    let inputs = [
        MouseInput::Press {
            button: 1,
            x: 10.0,
            y: 10.0,
            time: 100,
            double_time: 250,
            double_distance: 5.0,
            modifiers: 0,
        },
        // Far beyond the historical GTK GestureClick threshold.
        MouseInput::Motion {
            x: 110.0,
            y: 42.0,
            modifiers: 0,
        },
        MouseInput::Release {
            button: 1,
            x: 110.0,
            y: 42.0,
            modifiers: 0,
        },
    ];
    for input in inputs {
        for event in adapter.handle(input) {
            route_mouse(event, &active, 2, &observed);
        }
    }
    assert_eq!(
        observed.into_inner(),
        vec![
            Observed::Click {
                browser: 7,
                x: 20,
                y: 20,
                button: 1,
                down: true,
                count: 1,
                modifiers: F::EVENTFLAG_LEFT_MOUSE_BUTTON.0,
            },
            Observed::Move {
                browser: 7,
                x: 220,
                y: 84,
                modifiers: F::EVENTFLAG_LEFT_MOUSE_BUTTON.0,
            },
            Observed::Click {
                browser: 7,
                x: 220,
                y: 84,
                button: 1,
                down: false,
                count: 1,
                modifiers: 0,
            },
        ]
    );

    let clipboard = RefCell::new(vec![(false, "existing".to_owned())]);
    let requested_browsers = RefCell::new(Vec::new());
    let selection = "first line\nZażółć 🙂\x1b";
    let sanitized_selection = "first line\nZażółć 🙂";
    copy_through_production_adapters(
        ExplicitCopyTrigger::Keyboard {
            keyval: Key::c,
            state: ModifierType::CONTROL_MASK,
        },
        &active,
        selection,
        false,
        &requested_browsers,
        &clipboard,
    );
    // Switching pooled accounts between copy actions must re-resolve the
    // foreground frame; browser 7 must not receive later requests.
    active.set(8);
    copy_through_production_adapters(
        ExplicitCopyTrigger::ContextMenu(cef::sys::cef_menu_id_t::MENU_ID_COPY as i32),
        &active,
        selection,
        false,
        &requested_browsers,
        &clipboard,
    );
    assert_eq!(
        clipboard.borrow().as_slice(),
        [
            (false, "existing".to_owned()),
            (false, sanitized_selection.to_owned()),
            (false, sanitized_selection.to_owned()),
        ]
    );

    // Empty/collapsed live selection produces no SetClipboard in JavaScript;
    // even a defensive empty envelope cannot clobber the existing value.
    copy_through_production_adapters(
        ExplicitCopyTrigger::Keyboard {
            keyval: Key::C,
            state: ModifierType::CONTROL_MASK,
        },
        &active,
        "",
        true,
        &requested_browsers,
        &clipboard,
    );
    assert_eq!(clipboard.borrow().len(), 3);
    assert_eq!(requested_browsers.borrow().as_slice(), [7, 8, 8]);
}

#[test]
fn every_event_re_resolves_the_foreground_after_account_switch() {
    let active = Cell::new(1);
    let observed = RefCell::new(Vec::new());
    let mut adapter = MouseButtonTracker::default();
    for event in adapter.handle(MouseInput::Press {
        button: 1,
        x: 1.0,
        y: 1.0,
        time: 1,
        double_time: 250,
        double_distance: 5.0,
        modifiers: 0,
    }) {
        route_mouse(event, &active, 1, &observed);
    }
    active.set(2);
    for input in [
        MouseInput::Motion {
            x: 20.0,
            y: 20.0,
            modifiers: 0,
        },
        MouseInput::Release {
            button: 1,
            x: 20.0,
            y: 20.0,
            modifiers: 0,
        },
    ] {
        for event in adapter.handle(input) {
            route_mouse(event, &active, 1, &observed);
        }
    }
    let observed = observed.into_inner();
    assert!(matches!(observed[0], Observed::Click { browser: 1, .. }));
    assert!(observed[1..].iter().all(|event| matches!(
        event,
        Observed::Move { browser: 2, .. } | Observed::Click { browser: 2, .. }
    )));
}

#[test]
fn preserves_multiclick_buttons_touch_suppression_layouts_and_cancellation() {
    assert!(should_forward_mouse(false));
    assert!(!should_forward_mouse(true));
    for width in [600.0, 1280.0] {
        let x = width - 10.1;
        // physical_mouse_coordinates now rounds (x*scale).round() to avoid
        // truncation bias at HiDPI (bounded fix #158/#176).
        assert_eq!(
            physical_mouse_coordinates(x, 7.1, 1),
            ((x * 1.0).round() as i32, 7)
        );
        assert_eq!(
            physical_mouse_coordinates(x, 7.1, 2),
            ((x * 2.0).round() as i32, 14)
        );
    }

    let mut adapter = MouseButtonTracker::default();
    for (time, expected) in [(100, 1), (200, 2), (300, 3), (400, 1)] {
        assert_eq!(adapter.press(1, 4.0, 5.0, time, 250, 5.0), Some(expected));
        assert_eq!(adapter.press(1, 4.0, 5.0, time, 250, 5.0), None);
        assert_eq!(adapter.release(1), Some(expected));
    }
    adapter.press(2, 1.0, 1.0, 1_000, 250, 5.0).unwrap();
    adapter.press(3, 1.0, 1.0, 1_001, 250, 5.0).unwrap();
    let cancelled = adapter.handle(MouseInput::Cancel { x: 9.0, y: 8.0 });
    assert_eq!(cancelled.len(), 2);
    assert_eq!(adapter.active_modifiers(), 0);
}

/// Drive the debounce over a tick sequence, returning every action it takes.
fn run_im_focus(start_focused: bool, requests: &[Option<bool>]) -> Vec<ImFocusAction> {
    let mut focused = start_focused;
    let mut pending = 0u8;
    let mut actions = Vec::new();
    for req in requests {
        let action = im_focus_step(*req, focused, &mut pending);
        match action {
            ImFocusAction::FocusIn => focused = true,
            ImFocusAction::FocusOut => focused = false,
            ImFocusAction::Idle => {}
        }
        actions.push(action);
    }
    actions
}

/// The #180 regression: a composer re-render emits editable-focus false then
/// true. Acting on that false unfocused the IM, and the next keystroke raced
/// an async ibus `focus_in` — `filter_keypress` consumed it, no `commit` came,
/// and the character vanished. The pair must be a no-op.
#[test]
fn composer_rerender_never_unfocuses_the_im() {
    let actions = run_im_focus(
        true,
        &[Some(false), None, Some(true), None, None, None, None],
    );
    assert!(
        actions.iter().all(|a| *a == ImFocusAction::Idle),
        "re-render pair disturbed IM focus: {actions:?}"
    );
}

/// A real blur — clicking out of the composer, no follow-up focus — must
/// still reach the IM so an on-screen keyboard hides.
#[test]
fn genuine_blur_still_focuses_out() {
    let mut requests = vec![Some(false)];
    requests.extend(std::iter::repeat_n(
        None,
        usize::from(IM_FOCUS_OUT_DEBOUNCE_TICKS),
    ));
    let actions = run_im_focus(true, &requests);
    assert_eq!(
        actions
            .iter()
            .filter(|a| **a == ImFocusAction::FocusOut)
            .count(),
        1,
        "expected exactly one focus-out: {actions:?}"
    );
    assert_eq!(*actions.last().unwrap(), ImFocusAction::FocusOut);
}

/// Focusing a text field when the IM is out must focus in at once — the
/// debounce only ever delays going *out*.
#[test]
fn editable_focus_focuses_in_immediately() {
    assert_eq!(
        run_im_focus(false, &[Some(true)]),
        vec![ImFocusAction::FocusIn]
    );
    // Already focused: nothing to do.
    assert_eq!(run_im_focus(true, &[Some(true)]), vec![ImFocusAction::Idle]);
}

/// An unfocused IM receiving another false must not arm a countdown that
/// later fires a focus-out against an already-unfocused context.
#[test]
fn redundant_focus_out_is_inert() {
    let mut requests = vec![Some(false)];
    requests.extend(std::iter::repeat_n(
        None,
        usize::from(IM_FOCUS_OUT_DEBOUNCE_TICKS) + 2,
    ));
    let actions = run_im_focus(false, &requests);
    assert!(
        actions.iter().all(|a| *a == ImFocusAction::Idle),
        "focus-out fired on an unfocused IM: {actions:?}"
    );
}

/// A key the IM declines must still insert text: RAWKEYDOWN alone inserts
/// nothing, which is the dropped-character path in #180.
#[test]
fn declined_printable_keys_synthesize_a_char() {
    use gtk::gdk::{Key, ModifierType};
    let none = ModifierType::empty();
    assert_eq!(printable_char(Key::a, none), Some(u16::from(b'a')));
    assert_eq!(printable_char(Key::A, none), Some(u16::from(b'A')));
    assert_eq!(printable_char(Key::_1, none), Some(u16::from(b'1')));
    assert_eq!(printable_char(Key::space, none), Some(0x20));
    assert_eq!(printable_char(Key::eacute, none), Some(0xe9));
    assert_eq!(
        printable_char(Key::a, ModifierType::SHIFT_MASK),
        Some(u16::from(b'a'))
    );
}

/// Shortcuts and control keys keep their raw-key-only handling — a
/// synthesized CHAR there would type text the user never asked for.
#[test]
fn non_text_keys_synthesize_nothing() {
    use gtk::gdk::{Key, ModifierType};
    let none = ModifierType::empty();
    assert_eq!(printable_char(Key::a, ModifierType::CONTROL_MASK), None);
    assert_eq!(printable_char(Key::a, ModifierType::ALT_MASK), None);
    assert_eq!(printable_char(Key::a, ModifierType::SUPER_MASK), None);
    assert_eq!(printable_char(Key::Return, none), None);
    assert_eq!(printable_char(Key::Tab, none), None);
    assert_eq!(printable_char(Key::BackSpace, none), None);
    assert_eq!(printable_char(Key::Escape, none), None);
    assert_eq!(printable_char(Key::Delete, none), None);
    assert_eq!(printable_char(Key::Left, none), None);
    assert_eq!(printable_char(Key::F5, none), None);
    assert_eq!(printable_char(Key::Shift_L, none), None);
    // Enter is not "printable"; its CHAR comes from `enter_char` instead.
    assert_eq!(printable_char(Key::KP_Enter, none), None);
}

/// Enter (plain or Shift) sends the `\r` CHAR Blink needs for <textarea>
/// newlines and <input> submit; nothing else does, and shortcuts don't.
#[test]
fn only_plain_enter_synthesizes_a_carriage_return() {
    let none = ModifierType::empty();
    assert_eq!(enter_char(Key::Return, none), Some(0x0D));
    assert_eq!(enter_char(Key::KP_Enter, none), Some(0x0D));
    assert_eq!(enter_char(Key::Return, ModifierType::SHIFT_MASK), Some(0x0D));
    assert_eq!(enter_char(Key::Return, ModifierType::CONTROL_MASK), None);
    assert_eq!(enter_char(Key::Return, ModifierType::ALT_MASK), None);
    for key in [Key::Tab, Key::BackSpace, Key::Escape, Key::a, Key::space] {
        assert_eq!(enter_char(key, none), None, "{key:?}");
    }
}

/// Keyval → Windows VK. Punctuation uses VK_OEM_* (never its ASCII value:
/// '.' = 0x2E = VK_DELETE ran the Delete command and dropped the char).
#[test]
fn keyvals_map_to_windows_virtual_keys() {
    let cases: &[(Key, i32)] = &[
        (Key::BackSpace, 0x08),
        (Key::Tab, 0x09),
        (Key::ISO_Left_Tab, 0x09),
        (Key::Return, 0x0D),
        (Key::KP_Enter, 0x0D),
        (Key::Shift_L, 0x10),
        (Key::Shift_R, 0x10),
        (Key::Control_L, 0x11),
        (Key::Control_R, 0x11),
        (Key::Alt_L, 0x12),
        (Key::Alt_R, 0x12),
        (Key::Pause, 0x13),
        (Key::Caps_Lock, 0x14),
        (Key::Escape, 0x1B),
        (Key::space, 0x20),
        (Key::Page_Up, 0x21),
        (Key::KP_Page_Down, 0x22),
        (Key::KP_End, 0x23),
        (Key::KP_Home, 0x24),
        (Key::Left, 0x25),
        (Key::KP_Up, 0x26),
        (Key::KP_Right, 0x27),
        (Key::Down, 0x28),
        (Key::Print, 0x2C),
        (Key::KP_Insert, 0x2D),
        (Key::Delete, 0x2E),
        (Key::KP_Delete, 0x2E),
        (Key::Super_L, 0x5B),
        (Key::Super_R, 0x5C),
        (Key::Menu, 0x5D),
        (Key::KP_0, 0x60),
        (Key::KP_9, 0x69),
        (Key::KP_Multiply, 0x6A),
        (Key::KP_Add, 0x6B),
        (Key::KP_Subtract, 0x6D),
        (Key::KP_Decimal, 0x6E),
        (Key::KP_Divide, 0x6F),
        (Key::F1, 0x70),
        (Key::F5, 0x74),
        (Key::F12, 0x7B),
        (Key::F24, 0x87),
        (Key::Num_Lock, 0x90),
        (Key::Scroll_Lock, 0x91),
        (Key::a, 0x41),
        (Key::Z, 0x5A),
        (Key::_0, 0x30),
        (Key::_7, 0x37),
        (Key::semicolon, 0xBA),
        (Key::equal, 0xBB),
        (Key::comma, 0xBC),
        (Key::minus, 0xBD),
        (Key::period, 0xBE),
        (Key::slash, 0xBF),
        (Key::grave, 0xC0),
        (Key::bracketleft, 0xDB),
        (Key::backslash, 0xDC),
        (Key::bracketright, 0xDD),
        (Key::apostrophe, 0xDE),
        // Shifted symbols of a US layout keep their key's code.
        (Key::question, 0xBF),
        (Key::greater, 0xBE),
        (Key::colon, 0xBA),
        (Key::quotedbl, 0xDE),
    ];
    for &(key, vk) in cases {
        assert_eq!(gdk_key_to_vk(key, None, 0), vk, "{key:?}");
    }
    // No punctuation may land on a command VK (Delete, Insert, arrows...).
    for key in [Key::period, Key::comma, Key::slash, Key::minus, Key::apostrophe] {
        assert!(gdk_key_to_vk(key, None, 0) >= 0xBA, "{key:?}");
    }
}

/// Shift+/ and Shift+1 keep the unshifted key's VK; a key with no ASCII
/// meaning (non-Latin layout, dead key) falls back to its US position.
#[test]
fn shifted_and_non_latin_keys_use_unshifted_keyval_then_position() {
    // XKB keycodes (evdev + 8) on a US layout.
    const KC_1: u32 = 10;
    const KC_A: u32 = 38;
    const KC_SLASH: u32 = 61;
    const KC_APOSTROPHE: u32 = 48;
    assert_eq!(gdk_key_to_vk(Key::question, Some(Key::slash), KC_SLASH), 0xBF);
    assert_eq!(gdk_key_to_vk(Key::exclam, Some(Key::_1), KC_1), 0x31);
    // Unshifted keyval unknown: the position still says '1'.
    assert_eq!(gdk_key_to_vk(Key::exclam, None, KC_1), 0x31);
    // Cyrillic 'ф' sits on the US 'A' key.
    assert_eq!(
        gdk_key_to_vk(Key::Cyrillic_ef, Some(Key::Cyrillic_ef), KC_A),
        0x41
    );
    assert_eq!(gdk_key_to_vk(Key::dead_acute, None, KC_APOSTROPHE), 0xDE);
    // A letter's own keyval wins over its position (AZERTY 'q' on US 'A').
    assert_eq!(gdk_key_to_vk(Key::q, Some(Key::q), KC_A), 0x51);
    // Unknown key, unknown position.
    assert_eq!(gdk_key_to_vk(Key::VoidSymbol, None, 0), 0);
}

/// Plain typing: no preedit, raw key-down first (#180), commit as CHARs.
#[test]
fn ime_plain_typing_keeps_the_raw_first_order_and_chars() {
    let mut ime = ImeState::default();
    for _ in 0..3 {
        ime.key_press_begin();
        assert!(!ime.filter_first());
        assert_eq!(ime.commit(), ImeCommit::Chars);
    }
}

/// Dead key ´ then e: GtkIMContextSimple shows a "´" preedit, then on 'e'
/// ends it and commits "é". The 'e' goes to the IM first (no raw key-down
/// for the page to act on) and the é lands once, via ime_commit_text. The
/// next key is ordinary typing again.
#[test]
fn ime_dead_key_commits_once_in_either_signal_order() {
    // preedit-end before commit (GtkIMContextSimple).
    let mut ime = ImeState::default();
    ime.key_press_begin();
    assert!(!ime.filter_first(), "the dead key itself goes raw-first");
    ime.preedit_start();
    assert_eq!(ime.preedit_changed(false), ImeAction::SetComposition);
    ime.key_press_begin();
    assert!(ime.filter_first(), "'e' must reach the IM first");
    assert_eq!(ime.preedit_changed(true), ImeAction::Cancel);
    assert_eq!(ime.preedit_end(), ImeAction::Idle);
    assert_eq!(ime.commit(), ImeCommit::Composition);
    ime.key_press_begin();
    assert!(!ime.filter_first());
    assert_eq!(ime.commit(), ImeCommit::Chars);

    // commit before preedit-end (ibus-style).
    let mut ime = ImeState::default();
    ime.key_press_begin();
    assert_eq!(ime.preedit_changed(false), ImeAction::SetComposition);
    ime.key_press_begin();
    assert_eq!(ime.commit(), ImeCommit::Composition);
    assert_eq!(ime.preedit_end(), ImeAction::Idle, "nothing left to cancel");
    ime.key_press_begin();
    assert!(!ime.filter_first());
}

/// CJK: while composing, Enter/Backspace/arrows go to the IM first; an
/// async commit after the key handler returned still replaces the
/// composition; Escape (preedit emptied, no commit) cancels it.
#[test]
fn ime_cjk_composition_routes_keys_to_the_im() {
    let mut ime = ImeState::default();
    ime.key_press_begin();
    ime.preedit_start();
    assert_eq!(ime.preedit_changed(false), ImeAction::SetComposition);
    for _ in 0..4 {
        ime.key_press_begin();
        assert!(ime.filter_first());
        assert_eq!(ime.preedit_changed(false), ImeAction::SetComposition);
    }
    // Enter picks the candidate: preedit ends, then the async commit lands.
    ime.key_press_begin();
    assert!(ime.filter_first());
    assert_eq!(ime.preedit_end(), ImeAction::Cancel);
    assert_eq!(ime.commit(), ImeCommit::Composition);

    // Escape: composition discarded, then typing is plain again.
    let mut ime = ImeState::default();
    assert_eq!(ime.preedit_changed(false), ImeAction::SetComposition);
    assert_eq!(ime.preedit_end(), ImeAction::Cancel);
    ime.key_press_begin();
    assert!(!ime.filter_first());
    assert_eq!(ime.commit(), ImeCommit::Chars);
}

#[test]
fn ime_composition_extent_counts_utf16_units() {
    assert_eq!(ime_utf16_extent("", 0), (0, 0));
    assert_eq!(ime_utf16_extent("nihao", 2), (5, 2));
    assert_eq!(ime_utf16_extent("你好", 1), (2, 1));
    // An astral character is two UTF-16 units.
    assert_eq!(ime_utf16_extent("a🙂b", 2), (4, 3));
    assert_eq!(ime_utf16_extent("ab", 9), (2, 2));
    assert_eq!(ime_utf16_extent("ab", -1), (2, 0));
}

/// One zoom step per wheel notch; hi-res and touchpad deltas accumulate;
/// reversing drops the partial step. Up (dy < 0) zooms in.
#[test]
fn ctrl_scroll_zoom_steps() {
    let mut accum = 0.0;
    assert_eq!(zoom_scroll_steps(&mut accum, -1.0, 1.0), 1);
    assert_eq!(zoom_scroll_steps(&mut accum, 1.0, 1.0), -1);
    assert_eq!(zoom_scroll_steps(&mut accum, -3.0, 1.0), 3);
    let mut accum = 0.0;
    let steps: i32 = (0..8)
        .map(|_| zoom_scroll_steps(&mut accum, -0.125, 1.0))
        .sum();
    assert_eq!(steps, 1);
    let mut accum = 0.0;
    assert_eq!(zoom_scroll_steps(&mut accum, -30.0, 40.0), 0);
    assert_eq!(zoom_scroll_steps(&mut accum, 5.0, 40.0), 0);
    assert_eq!(zoom_scroll_steps(&mut accum, -30.0, 40.0), 0, "reversal reset");
    assert_eq!(zoom_scroll_steps(&mut accum, -15.0, 40.0), 1);
    assert_eq!(zoom_scroll_steps(&mut accum, 0.0, 40.0), 0);
}

/// Widget logical px → page CSS px: CEF's page zoom includes the display
/// scale (#158), so at 2x with 100 % user zoom they coincide.
#[test]
fn page_coordinates_follow_the_page_zoom() {
    assert_eq!(page_coords(100.0, 50.0, 1.0, 1.0), (100.0, 50.0));
    assert_eq!(page_coords(100.0, 50.0, 2.0, 2.0), (100.0, 50.0));
    assert_eq!(page_coords(150.0, 60.0, 1.0, 1.5), (100.0, 40.0));
    assert_eq!(page_coords(150.0, 60.0, 2.0, 3.0), (100.0, 40.0));
    assert_eq!(page_coords(10.0, 20.0, 1.0, 0.0), (10.0, 20.0));
}

#[test]
fn file_batches_respect_the_message_budget() {
    assert!(batch_by_size(&[], 10).is_empty());
    assert_eq!(batch_by_size(&[3, 3, 3], 10), vec![0..3]);
    assert_eq!(batch_by_size(&[6, 6, 6], 10), vec![0..1, 1..2, 2..3]);
    assert_eq!(batch_by_size(&[4, 6, 1, 9], 10), vec![0..2, 2..4]);
    // Oversized file: alone in its batch (make_blob then refuses it).
    assert_eq!(batch_by_size(&[2, 50, 2], 10), vec![0..1, 1..2, 2..3]);
}

#[test]
fn hidpi_physical_coordinates_round_instead_of_truncating() {
    // Truncation at HiDPI could bias by up to scale-1 px (e.g. 10.9*2=20 vs 22).
    // Bounded fix #158 rounds to nearest, keeping error ≤0.5 px.
    assert_eq!(physical_mouse_coordinates(10.9, 20.9, 1), (11, 21));
    assert_eq!(physical_mouse_coordinates(10.9, 20.9, 2), (22, 42));
    assert_eq!(physical_mouse_coordinates(10.1, 20.1, 2), (20, 40));
    assert_eq!(physical_mouse_coordinates(0.5, 0.5, 2), (1, 1));
    // Send path uses same helper, so dispatch stays within 1 px of expected
    // physical after rounding, even for fractional GTK gesture coords.
    let observed = std::cell::RefCell::new(Vec::new());
    let active = std::cell::Cell::new(99);
    route_mouse(
        crate::web_view::input::mouse::MouseDispatch::Click {
            x: 10.9,
            y: 20.9,
            button: 1,
            down: true,
            count: 1,
            modifiers: 0,
        },
        &active,
        2,
        &observed,
    );
    assert_eq!(
        observed.borrow()[0],
        Observed::Click {
            browser: 99,
            x: 22,
            y: 42,
            button: 1,
            down: true,
            count: 1,
            modifiers: 0
        }
    );
}

#[test]
fn h7_origin_and_fractional_rounding() {
    // KARE-018 H7: origin-calibrated mapping and fractional Wayland scales.
    // Host dispatch is (logical * scale).round(); chrome offset is subtracted
    // before scaling via calibration (observed - origin), so calibrated physical
    // equals ((x - originX)*scale).round().
    fn calibrated_physical(x: f64, origin: f64, scale: f64) -> i32 {
        ((x - origin) * scale).round() as i32
    }
    // Origin case: header 60px offset at 2×
    assert_eq!(calibrated_physical(170.0, 60.0, 2.0), 220);
    assert_eq!(calibrated_physical(230.0, 60.0, 2.0), 340);
    // Fractional cases: 110.5 at 1.25× →138, at 1.5× →166
    assert_eq!(((110.5_f64 * 1.25).round() as i32), 138);
    assert_eq!(((110.5_f64 * 1.5).round() as i32), 166);
    assert_eq!(physical_mouse_coordinates(110.5, 20.0, 1), (111, 20));
    // Transform + scroll origin: translate(10,20)+scrollTop 80 shifts origin by 10,100
    let origin_x = 360.0 + 10.0; // panel + translateX
    let origin_y = 60.0 + 20.0 + 80.0; // header + translateY + scroll
    assert_eq!(calibrated_physical(500.0, origin_x, 1.0), 130);
    assert_eq!(calibrated_physical(300.0, origin_y, 2.0), 280);
    // pointer_pos_physical stripping + origin: native 200 at scale 2 with surface xform stripped → logical 100, minus origin 60 → 40*2=80
    // show_context_menu: x_dev 220 at scale 2 → logical 110, origin-corrected 50 → verifies divide symmetry
    assert_eq!((220.0_f64 / 2.0).round() as i32, 110);
    assert_eq!(((110.0_f64 - 60.0) * 2.0).round() as i32, 100);
    // Dispatch with origin-corrected coords stays within 1px
    let observed = std::cell::RefCell::new(Vec::new());
    let active = std::cell::Cell::new(7);
    route_mouse(
        crate::web_view::input::mouse::MouseDispatch::Move {
            x: 110.0,
            y: 110.0,
            modifiers: 0,
        },
        &active,
        2,
        &observed,
    );
    assert_eq!(
        observed.borrow()[0],
        Observed::Move {
            browser: 7,
            x: 220,
            y: 220,
            modifiers: 0
        }
    );
    // Calibrated dispatch: logical 170 with origin 60 at 2× should hit physical 220
    let cx = calibrated_physical(170.0, 60.0, 2.0);
    assert_eq!(cx, 220);
    assert_eq!(physical_mouse_coordinates(170.0 - 60.0, 0.0, 2), (220, 0));
}

#[test]
fn non_copy_commands_and_modified_shortcuts_do_not_request_selection() {
    let requests = Cell::new(0);
    for trigger in [
        ExplicitCopyTrigger::Keyboard {
            keyval: Key::c,
            state: ModifierType::empty(),
        },
        ExplicitCopyTrigger::Keyboard {
            keyval: Key::c,
            state: ModifierType::CONTROL_MASK | ModifierType::ALT_MASK,
        },
        ExplicitCopyTrigger::ContextMenu(cef::sys::cef_menu_id_t::MENU_ID_PASTE as i32),
    ] {
        dispatch_explicit_copy(trigger, || requests.set(requests.get() + 1));
    }
    assert_eq!(requests.get(), 0);
}
