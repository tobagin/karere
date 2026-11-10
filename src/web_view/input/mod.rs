mod drag_out;
mod gestures;
pub(super) mod keyboard;
pub(super) mod mouse;
mod touch;

use cef::{CefString, ImplBrowserHost, TouchEventType, sys};
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use std::cell::RefCell;

use crate::web_view::clipboard_dnd::{
    DROP_TEXT_MIMES, ExplicitCopyTrigger, PORTAL_FILE_TRANSFER, dispatch_explicit_copy,
    drop_may_carry_files, drop_offers_text, read_primary_clipboard_paste, read_text_drop,
    read_uri_list_drop, request_live_selection, send_drag_hover, send_portal_files,
    try_intercept_paste,
};
use crate::web_view::imp::with_host;
use gestures::{KineticScroll, PinchSteps, activate_zoom_steps};
use keyboard::{
    ImeCommit, apply_ime_action, enter_char, ime_invalid_range, is_accelerator_key, printable_char,
    reset_ime, send_char, send_key_raw, set_focus, update_ime_cursor_location,
};
use mouse::{
    MouseButtonTracker, MouseInput, ZOOM_SCROLL_SURFACE_STEP, dispatch_mouse, is_pointer_emulated,
    release_stuck_mouse_buttons, send_move, send_wheel, should_forward_mouse, surface_to_widget,
    widget_to_page, zoom_scroll_steps,
};
use touch::send_touch;

pub(in crate::web_view) fn install_input_controllers(widget: &crate::web_view::KarereWebView) {
    use gtk::gdk;

    let im = gtk::IMMulticontext::new();
    im.set_client_widget(Some(widget));
    im.connect_commit(glib::clone!(
        #[weak]
        widget,
        move |_im, text| {
            let imp = widget.imp();
            let mut state = imp.ime_state.get();
            let route = state.commit();
            imp.ime_state.set(state);
            match route {
                // Text that ends a composition replaces the page's composing
                // range (or inserts at the caret if it was already cleared).
                ImeCommit::Composition => {
                    let text = CefString::from(text);
                    with_host(&widget, |host| {
                        host.ime_commit_text(Some(&text), Some(&ime_invalid_range()), 0)
                    });
                }
                ImeCommit::Chars => {
                    // CEF's `character` field is UTF-16. `ch as u16` truncated
                    // anything above the BMP — an emoji committed by the IM
                    // (U+1F600) arrived as U+F600 — so encode properly and send
                    // each surrogate in turn.
                    let mut buf = [0u16; 2];
                    for ch in text.chars() {
                        for unit in ch.encode_utf16(&mut buf) {
                            send_char(&widget, *unit, 0);
                        }
                    }
                }
            }
        }
    ));
    // Composition (ibus/fcitx CJK, dead keys): mirror the IM's preedit into
    // the page as a CEF IME composition so it renders inline, and so the
    // key handler knows to hand keys to the IM first while it is active. (I1)
    im.connect_preedit_start(glib::clone!(
        #[weak]
        widget,
        move |_im| {
            let imp = widget.imp();
            let mut state = imp.ime_state.get();
            state.preedit_start();
            imp.ime_state.set(state);
        }
    ));
    im.connect_preedit_changed(glib::clone!(
        #[weak]
        widget,
        move |im| {
            let (text, _attrs, cursor) = im.preedit_string();
            let imp = widget.imp();
            let mut state = imp.ime_state.get();
            let action = state.preedit_changed(text.is_empty());
            imp.ime_state.set(state);
            apply_ime_action(&widget, action, &text, cursor);
            update_ime_cursor_location(&widget);
        }
    ));
    im.connect_preedit_end(glib::clone!(
        #[weak]
        widget,
        move |_im| {
            let imp = widget.imp();
            let mut state = imp.ime_state.get();
            let action = state.preedit_end();
            imp.ime_state.set(state);
            apply_ime_action(&widget, action, "", 0);
        }
    ));
    // Stored so the widget timer can focus it in/out from CEF's editable-focus
    // signal — see the keyboard_request drain.
    widget.imp().im_context.replace(Some(im.clone()));

    // Touch --------------------------------------------------------------
    // Touchscreens (Phosh) need native touch events for scrolling — emulated
    // pointer events don't drive WhatsApp's touch scroll. A touch-only
    // GestureDrag gives clean widget-relative single-finger press/move/release
    // (covers scroll and tap); the mouse controllers below skip the emulated
    // pointer stream so the page isn't double-fed. Pinch is the zoom gesture
    // below. (#162)
    let drag = gtk::GestureDrag::new();
    drag.set_touch_only(true);
    // Whether CEF holds a pressed touch point: a cancelled sequence is
    // followed by GTK's drag-end, which must not also send RELEASED. (I7)
    let touch_down = std::rc::Rc::new(std::cell::Cell::new(false));
    // Touchpad fling in progress (see the scroll controller); any new input
    // stops it.
    let kinetic = std::rc::Rc::new(KineticScroll::default());
    drag.connect_drag_begin(glib::clone!(
        #[weak]
        widget,
        #[strong]
        touch_down,
        #[strong]
        kinetic,
        move |_g, x, y| {
            kinetic.stop();
            crate::web_view::note_user_input();
            widget.grab_focus();
            set_focus(&widget, true);
            touch_down.set(true);
            send_touch(&widget, 0, x, y, TouchEventType::PRESSED);
        }
    ));
    drag.connect_drag_update(glib::clone!(
        #[weak]
        widget,
        #[strong]
        touch_down,
        move |g, ox, oy| {
            if !touch_down.get() {
                return;
            }
            crate::web_view::note_user_input();
            if let Some((sx, sy)) = g.start_point() {
                send_touch(&widget, 0, sx + ox, sy + oy, TouchEventType::MOVED);
            }
        }
    ));
    drag.connect_drag_end(glib::clone!(
        #[weak]
        widget,
        #[strong]
        touch_down,
        move |g, ox, oy| {
            if !touch_down.replace(false) {
                return;
            }
            if let Some((sx, sy)) = g.start_point() {
                send_touch(&widget, 0, sx + ox, sy + oy, TouchEventType::RELEASED);
            }
        }
    ));
    // A sequence GTK takes back (another gesture claimed it, the grab
    // broke) must be cancelled in CEF too, or the page keeps a stuck
    // pressed touch. (I7)
    drag.connect_cancel(glib::clone!(
        #[weak]
        widget,
        #[strong]
        touch_down,
        move |g, _sequence| {
            if !touch_down.replace(false) {
                return;
            }
            let (sx, sy) = g.start_point().unwrap_or((0.0, 0.0));
            let (ox, oy) = g.offset().unwrap_or((0.0, 0.0));
            send_touch(&widget, 0, sx + ox, sy + oy, TouchEventType::CANCELLED);
        }
    ));
    widget.add_controller(drag);

    // Pinch (touchscreen two-finger, touchpad pinch) ---------------------
    // Steps the per-account zoom like Ctrl+scroll: one step per ×1.1 of
    // scale since the last step. Once a step fires the gesture claims its
    // touch points, which denies them to the touch drag above, so its cancel
    // path sends CANCELLED instead of the first finger also scrolling the
    // page. A two-finger pan that never changes scale claims nothing and
    // keeps scrolling through the drag.
    let zoom = gtk::GestureZoom::new();
    let pinch = std::rc::Rc::new(std::cell::Cell::new(PinchSteps::default()));
    zoom.connect_begin(glib::clone!(
        #[strong]
        pinch,
        move |_g, _sequence| pinch.set(PinchSteps::default())
    ));
    zoom.connect_scale_changed(glib::clone!(
        #[weak]
        widget,
        #[strong]
        pinch,
        #[strong]
        kinetic,
        move |g, scale| {
            let mut steps = pinch.get();
            let n = steps.update(scale);
            pinch.set(steps);
            if n == 0 {
                return;
            }
            kinetic.stop();
            g.set_state(gtk::EventSequenceState::Claimed);
            activate_zoom_steps(&widget, n);
        }
    ));
    zoom.connect_end(glib::clone!(
        #[strong]
        pinch,
        move |_g, _sequence| pinch.set(PinchSteps::default())
    ));
    zoom.connect_cancel(glib::clone!(
        #[strong]
        pinch,
        move |_g, _sequence| pinch.set(PinchSteps::default())
    ));
    widget.add_controller(zoom);

    // Mouse --------------------------------------------------------------
    // Keep raw button lifecycle separate from GTK gestures. GestureClick's
    // `released` signal is cancelled after a drag threshold crossing, which
    // used to leave CEF logically button-down and made text unselectable.
    let mouse_buttons = std::rc::Rc::new(RefCell::new(MouseButtonTracker::default()));
    // A page drag-out ends with the button released over the drop target,
    // never here: forget it, or the next press of that button is ignored.
    widget
        .imp()
        .forget_mouse_buttons
        .replace(Some(Box::new(glib::clone!(
            #[strong]
            mouse_buttons,
            move || mouse_buttons.borrow_mut().forget_all()
        ))));

    let motion = gtk::EventControllerMotion::new();
    motion.connect_motion(glib::clone!(
        #[weak]
        widget,
        #[strong]
        mouse_buttons,
        move |ctrl, x, y| {
            if !should_forward_mouse(is_pointer_emulated(ctrl)) {
                return; // touch handled above; don't double-feed as mouse
            }
            let modifiers = modifiers_from_state(ctrl.current_event_state());
            // A drag-select is user input for as long as it lasts: the PRIMARY
            // mirror it triggers must pass the clipboard gesture gate. (S9)
            if mouse_buttons.borrow().active_modifiers() != 0 {
                crate::web_view::note_user_input();
            }
            for event in
                mouse_buttons
                    .borrow_mut()
                    .handle(MouseInput::Motion { x, y, modifiers })
            {
                dispatch_mouse(&widget, event);
            }
        }
    ));
    motion.connect_leave(glib::clone!(
        #[weak]
        widget,
        #[strong]
        mouse_buttons,
        move |ctrl| {
            let modifiers = modifiers_from_state(ctrl.current_event_state())
                | mouse_buttons.borrow().active_modifiers();
            send_move(&widget, 0.0, 0.0, modifiers, true);
        }
    ));
    widget.add_controller(motion);

    let buttons = gtk::EventControllerLegacy::new();
    buttons.connect_event(glib::clone!(
        #[weak]
        widget,
        #[strong]
        im,
        #[strong]
        mouse_buttons,
        #[strong]
        kinetic,
        #[upgrade_or]
        glib::Propagation::Proceed,
        move |_ctrl, event| {
            use gtk::gdk::{ButtonEvent, EventType};
            if !should_forward_mouse(event.is_pointer_emulated()) {
                return glib::Propagation::Proceed;
            }
            if event.event_type() == EventType::GrabBroken {
                release_stuck_mouse_buttons(&widget, &mouse_buttons);
                return glib::Propagation::Proceed;
            }
            let Some(button_event) = event.downcast_ref::<ButtonEvent>() else {
                return glib::Propagation::Proceed;
            };
            let button = button_event.button();
            // EventControllerLegacy hands out raw surface coords; CEF needs
            // widget coords (GestureClick used to do this mapping; KARE-002 regression).
            let Some((x, y)) = event
                .position()
                .and_then(|(sx, sy)| surface_to_widget(&widget, sx, sy))
            else {
                return glib::Propagation::Proceed;
            };
            let modifiers = modifiers_from_state(event.modifier_state());
            match event.event_type() {
                EventType::ButtonPress => {
                    kinetic.stop();
                    crate::web_view::note_user_input();
                    let settings = widget.settings();
                    widget.grab_focus();
                    set_focus(&widget, true);
                    // Clicking moves the caret away from an open composition;
                    // end it instead of leaving the IM composing elsewhere. (I1)
                    if widget.imp().ime_state.get().filter_first() {
                        reset_ime(&widget);
                    }
                    im.set_cursor_location(&gtk::gdk::Rectangle::new(x as i32, y as i32, 1, 1));
                    let events = mouse_buttons.borrow_mut().handle(MouseInput::Press {
                        button,
                        x,
                        y,
                        time: event.time(),
                        double_time: settings.gtk_double_click_time().max(0) as u32,
                        double_distance: settings.gtk_double_click_distance().max(0) as f64,
                        modifiers,
                    });
                    if !events.is_empty() && button == 2 {
                        // The page hit-tests in CSS px, not widget px. (I8)
                        let (px, py) = widget_to_page(&widget, x, y);
                        read_primary_clipboard_paste(&widget, px, py);
                    }
                    for event in events {
                        dispatch_mouse(&widget, event);
                    }
                }
                EventType::ButtonRelease => {
                    crate::web_view::note_user_input();
                    for event in mouse_buttons.borrow_mut().handle(MouseInput::Release {
                        button,
                        x,
                        y,
                        modifiers,
                    }) {
                        dispatch_mouse(&widget, event);
                    }
                }
                _ => {}
            }
            glib::Propagation::Proceed
        }
    ));
    widget.add_controller(buttons);

    // A cancelled device grab or widget teardown must not strand CEF in a
    // button-down state. Normal releases have already removed their bit.
    widget.connect_unrealize(glib::clone!(
        #[strong]
        mouse_buttons,
        move |widget| release_stuck_mouse_buttons(widget, &mouse_buttons)
    ));

    // Scroll -------------------------------------------------------------
    // KINETIC: GTK reports the release velocity of a touchpad scroll via
    // `decelerate`; CEF's windowless wheel events carry no momentum phase,
    // so Chromium never flings one itself. Touchscreen flings are Chromium's
    // own (they arrive as touch events), and mouse wheels never decelerate.
    let scroll = gtk::EventControllerScroll::new(
        gtk::EventControllerScrollFlags::BOTH_AXES | gtk::EventControllerScrollFlags::KINETIC,
    );
    scroll.connect_scroll_begin(glib::clone!(
        #[strong]
        kinetic,
        move |_| kinetic.stop()
    ));
    scroll.connect_decelerate(glib::clone!(
        #[weak]
        widget,
        #[strong]
        kinetic,
        move |ctrl, vx, vy| {
            if ctrl.unit() != gtk::gdk::ScrollUnit::Surface
                || ctrl
                    .current_event_state()
                    .contains(gtk::gdk::ModifierType::CONTROL_MASK)
            {
                return;
            }
            kinetic.start(&widget, vx, vy);
        }
    ));
    let zoom_accum = std::cell::Cell::new(0.0_f64);
    scroll.connect_scroll(glib::clone!(
        #[weak]
        widget,
        #[strong]
        kinetic,
        #[upgrade_or]
        glib::Propagation::Proceed,
        move |ctrl, dx, dy| {
            kinetic.stop();
            let state = ctrl.current_event_state();
            // Surface unit = pixel deltas (touchpad); Wheel unit = notch
            // clicks (mouse). Scaled differently in send_wheel. (#161)
            let precise = ctrl.unit() == gtk::gdk::ScrollUnit::Surface;
            // Ctrl+wheel is the window's zoom, not the page's: CEF would zoom
            // the browser behind the per-account level. One action per notch;
            // smooth deltas accumulate to a notch's worth first. (I10)
            if state.contains(gtk::gdk::ModifierType::CONTROL_MASK) {
                let threshold = if precise { ZOOM_SCROLL_SURFACE_STEP } else { 1.0 };
                let mut accum = zoom_accum.get();
                let steps = zoom_scroll_steps(&mut accum, dy, threshold);
                zoom_accum.set(accum);
                activate_zoom_steps(&widget, steps);
                return glib::Propagation::Stop;
            }
            zoom_accum.set(0.0);
            send_wheel(&widget, dx, dy, modifiers_from_state(state), precise);
            glib::Propagation::Stop
        }
    ));
    widget.add_controller(scroll);

    // Keyboard -----------------------------------------------------------
    // Every press sends its raw key-down first, then goes through the IM,
    // whose `commit` is the only source of text for a key it consumes. (#154)
    // While a composition is open the IM gets the key first instead, and a
    // key it consumes never reaches the page (Enter picks a candidate, it
    // does not send the message). (I1)
    let keys = gtk::EventControllerKey::new();
    // Hardware keycodes whose press the IM swallowed during composition;
    // their release is not forwarded either, so CEF sees no orphan key-up.
    let swallowed_keys = std::rc::Rc::new(RefCell::new(Vec::<u32>::new()));
    keys.connect_key_pressed(glib::clone!(
        #[weak]
        widget,
        #[strong]
        im,
        #[strong]
        swallowed_keys,
        #[strong]
        kinetic,
        #[upgrade_or]
        glib::Propagation::Proceed,
        move |ctrl, keyval, keycode, state| {
            use gtk::gdk::{Key, ModifierType};
            kinetic.stop();
            crate::web_view::note_user_input();
            // M17: intercept Ctrl+V before CEF. If GDK clipboard holds an image
            // or files, synthesize the paste and swallow the key so CEF's
            // GDK-blind native paste doesn't also fire. Text-only/empty falls through.
            if state.contains(ModifierType::CONTROL_MASK)
                && !state.intersects(ModifierType::ALT_MASK | ModifierType::SUPER_MASK)
                && matches!(keyval, Key::v | Key::V)
                && try_intercept_paste(&widget)
            {
                return glib::Propagation::Stop;
            }
            // OSR's native copy has no platform clipboard. Ask the renderer
            // for the current selection now, without racing the 50 ms PRIMARY
            // mirror. The key still forwards so page copy behavior is preserved.
            dispatch_explicit_copy(ExplicitCopyTrigger::Keyboard { keyval, state }, || {
                request_live_selection(&widget)
            });
            // A physical keypress means the user is typing — if the page's
            // editable-focus signal left the IM focused-out (post-send input
            // re-render), dead keys silently stop composing. Re-focus first. (#154)
            let imp = widget.imp();
            // A keystroke means the field is live: drop any focus-out the page
            // queued, so the debounce window can't expire mid-word (#180).
            imp.im_focus_out_pending.set(0);
            if !imp.im_focused.get() {
                im.focus_in();
                imp.im_focused.set(true);
            }
            let layout = ctrl.group();
            let mut ime = imp.ime_state.get();
            ime.key_press_begin();
            imp.ime_state.set(ime);
            if ime.filter_first() {
                // Composing: the IM drives the candidate window with this
                // key (Enter, Backspace, arrows, Space). Only a key it
                // declines is the page's.
                update_ime_cursor_location(&widget);
                let consumed = ctrl
                    .current_event()
                    .map(|e| im.filter_keypress(&e))
                    .unwrap_or(false);
                if consumed {
                    let mut keys = swallowed_keys.borrow_mut();
                    if !keys.contains(&keycode) {
                        keys.push(keycode);
                    }
                    drop(keys);
                    return glib::Propagation::Stop;
                }
                send_key_raw(&widget, keyval, keycode, layout, state, true);
            } else {
                // The key-down goes first, for text keys too. Chromium drops
                // the CHAR that follows a key-down the page handled, and only
                // the next key-down lifts that; a letter sent as a bare CHAR
                // while Backspace or an arrow was still held was therefore
                // swallowed. It carries no character, so it inserts nothing
                // by itself. (#180)
                send_key_raw(&widget, keyval, keycode, layout, state, true);
                // A consumed key is text (a letter, or a dead-key composition
                // starting) — the `commit` handler emits the CHAR, which runs
                // inside this call, so nothing else is sent.
                let consumed = ctrl
                    .current_event()
                    .map(|e| im.filter_keypress(&e))
                    .unwrap_or(false);
                if consumed {
                    return glib::Propagation::Stop;
                }
            }
            // A declined printable key means no `commit` is coming, and a
            // RAWKEYDOWN alone inserts nothing — the character is silently
            // dropped. Happens when the IM is momentarily not focused: the
            // page's editable-focus signal focuses it out on composer
            // re-renders, and re-focusing an async IM (ibus) can land after
            // this key. Synthesize the CHAR ourselves. (#180)
            if let Some(unit) = printable_char(keyval, state) {
                send_char(&widget, unit, modifiers_from_state(state));
            } else if let Some(unit) = enter_char(keyval, state) {
                // Enter's CHAR is what inserts a newline in a <textarea> and
                // submits an <input>; no IM commits one. If the page handled
                // the key-down (the composer's send), Chromium drops it.
                send_char(&widget, unit, modifiers_from_state(state));
            }
            // Let accelerator combos (Ctrl/Alt/Super+key, F5/F11) bubble to
            // window/app shortcuts; consume the rest so typing stays in the webview.
            if is_accelerator_key(keyval, state) {
                glib::Propagation::Proceed
            } else {
                glib::Propagation::Stop
            }
        }
    ));
    keys.connect_key_released(glib::clone!(
        #[weak]
        widget,
        #[strong]
        im,
        #[strong]
        swallowed_keys,
        move |ctrl, keyval, keycode, state| {
            // Mirror the press path: if the IM consumes the release, it
            // belongs to composition — don't also send a raw key-up.
            let consumed = ctrl
                .current_event()
                .map(|e| im.filter_keypress(&e))
                .unwrap_or(false);
            let swallowed = {
                let mut keys = swallowed_keys.borrow_mut();
                match keys.iter().position(|k| *k == keycode) {
                    Some(i) => {
                        keys.swap_remove(i);
                        true
                    }
                    None => false,
                }
            };
            if !consumed && !swallowed {
                send_key_raw(&widget, keyval, keycode, ctrl.group(), state, false);
            }
        }
    ));
    widget.add_controller(keys);

    // Focus --------------------------------------------------------------
    let focus = gtk::EventControllerFocus::new();
    focus.connect_enter(glib::clone!(
        #[weak]
        widget,
        move |_| {
            // Note: no im.focus_in() here. The GLArea holds GTK focus almost
            // always, so focusing the IM on widget-focus kept Phosh's on-screen
            // keyboard up permanently. IM focus-in is driven by the page's
            // editable-focus signal instead (keyboard_request). focus_out stays
            // on leave so the keyboard hides when leaving the webview entirely.
            set_focus(&widget, true);
        }
    ));
    focus.connect_leave(glib::clone!(
        #[weak]
        widget,
        #[strong]
        im,
        #[strong]
        swallowed_keys,
        move |_| {
            // Releases of keys pressed here now go elsewhere.
            swallowed_keys.borrow_mut().clear();
            reset_ime(&widget);
            im.focus_out();
            widget.imp().im_focused.set(false);
            set_focus(&widget, false);
        }
    ));
    widget.add_controller(focus);

    // Drag-drop ----------------------------------------------------------
    // M17: accept file drops; surface each as a synthetic `drop` on the element
    // under the cursor (paste_bridge.js targets it via the envelope's coords).
    // Text and web-link drops are inserted into the editable under the cursor.
    // Async target: the drop is read by hand so a portal transfer can be
    // resolved here rather than by GTK's own conversion. (#192)
    let drop_formats = DROP_TEXT_MIMES
        .iter()
        .fold(
            gdk::ContentFormatsBuilder::new()
                .add_mime_type(PORTAL_FILE_TRANSFER)
                // Matched against the offer as mime types, so the plain-path
                // form has to be named: the file list GType alone accepts nothing.
                .add_mime_type("text/uri-list")
                .add_type(gdk::FileList::static_type())
                .add_type(glib::Type::STRING),
            |builder, mime| builder.add_mime_type(*mime),
        )
        .build();
    let drop_target = gtk::DropTargetAsync::new(Some(drop_formats), gdk::DragAction::COPY);
    // Decide on the offered types alone. The default check also wants the
    // drop's current action to be one of ours, but on entry that is whatever
    // the compositor picked for the source (Hyprland + Nautilus: MOVE), so a
    // COPY-only target refused every drag before it could ask for COPY.
    drop_target.connect_accept(|_t, drop| {
        let formats = drop.formats();
        log::debug!(
            "drag accept? actions={:?} offered {formats}",
            drop.actions()
        );
        drop_may_carry_files(&formats) || drop_offers_text(&formats)
    });
    // Forward hover (enter/motion/leave) so the page's dropzone overlay mounts
    // DURING the hover — CEF only delivers the drop on release, too late to mount.
    drop_target.connect_drag_enter(glib::clone!(
        #[weak]
        widget,
        #[upgrade_or]
        gdk::DragAction::COPY,
        move |_t, drop, x, y| {
            log::debug!("drag enter: offered {}", drop.formats());
            if drop_may_carry_files(&drop.formats()) {
                send_drag_hover(&widget, "enter", x, y);
            }
            gdk::DragAction::COPY
        }
    ));
    let last_motion = std::cell::Cell::new(std::time::Instant::now());
    drop_target.connect_drag_motion(glib::clone!(
        #[weak]
        widget,
        #[upgrade_or]
        gdk::DragAction::COPY,
        move |_t, drop, x, y| {
            // Throttle to ~10/s: keeps the dropzone alive without flooding IPC.
            let now = std::time::Instant::now();
            if drop_may_carry_files(&drop.formats())
                && now.duration_since(last_motion.get()) >= std::time::Duration::from_millis(100)
            {
                last_motion.set(now);
                send_drag_hover(&widget, "over", x, y);
            }
            gdk::DragAction::COPY
        }
    ));
    drop_target.connect_drag_leave(glib::clone!(
        #[weak]
        widget,
        move |_t, _drop| send_drag_hover(&widget, "leave", 0.0, 0.0)
    ));
    drop_target.connect_drop(glib::clone!(
        #[weak]
        widget,
        #[upgrade_or]
        false,
        move |_target, drop, x, y| {
            // Taken now, at release, and in page CSS px for the bridge. (I8)
            let coords = Some(widget_to_page(&widget, x, y));
            if drop.formats().contain_mime_type(PORTAL_FILE_TRANSFER) {
                drop.read_async(
                    &[PORTAL_FILE_TRANSFER],
                    glib::Priority::DEFAULT,
                    gtk::gio::Cancellable::NONE,
                    glib::clone!(
                        #[weak]
                        widget,
                        #[strong]
                        drop,
                        move |res| {
                            match res {
                                Ok((stream, _mime)) => {
                                    send_portal_files(&widget, stream, "drop", coords)
                                }
                                Err(err) => log::warn!("drop transfer read failed: {err}"),
                            }
                            drop.finish(gdk::DragAction::COPY);
                        }
                    ),
                );
            } else if drop.formats().contain_mime_type("text/uri-list") {
                // Local files (attached) or web links (inserted as text).
                read_uri_list_drop(&widget, drop, coords);
            } else {
                read_text_drop(&widget, drop, coords);
            }
            true
        }
    ));
    widget.add_controller(drop_target);
}

pub(super) fn modifiers_from_state(state: gtk::gdk::ModifierType) -> u32 {
    use gtk::gdk::ModifierType;
    use sys::cef_event_flags_t as F;
    let mut m = 0u32;
    if state.contains(ModifierType::SHIFT_MASK) {
        m |= F::EVENTFLAG_SHIFT_DOWN.0;
    }
    if state.contains(ModifierType::CONTROL_MASK) {
        m |= F::EVENTFLAG_CONTROL_DOWN.0;
    }
    if state.contains(ModifierType::ALT_MASK) {
        m |= F::EVENTFLAG_ALT_DOWN.0;
    }
    if state.contains(ModifierType::SUPER_MASK) {
        m |= F::EVENTFLAG_COMMAND_DOWN.0;
    }
    if state.contains(ModifierType::BUTTON1_MASK) {
        m |= F::EVENTFLAG_LEFT_MOUSE_BUTTON.0;
    }
    if state.contains(ModifierType::BUTTON2_MASK) {
        m |= F::EVENTFLAG_MIDDLE_MOUSE_BUTTON.0;
    }
    if state.contains(ModifierType::BUTTON3_MASK) {
        m |= F::EVENTFLAG_RIGHT_MOUSE_BUTTON.0;
    }
    m
}

#[cfg(test)]
mod tests;
