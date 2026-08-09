//! Forward GPUI input events into the CEF host (when Design Mode is off).

use extension_cef::{AsyncCefHost, BrowserId, KeyEventPayload, MouseButtonKind};
use gpui::{
    KeyDownEvent, KeyUpEvent, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    Point, ScrollWheelEvent,
};
use util::ResultExt as _;

/// CEF `cef_event_flags_t` bits used for input forwarding.
const EVENTFLAG_SHIFT_DOWN: u32 = 1 << 1;
const EVENTFLAG_CONTROL_DOWN: u32 = 1 << 2;
const EVENTFLAG_ALT_DOWN: u32 = 1 << 3;
const EVENTFLAG_COMMAND_DOWN: u32 = 1 << 7;
const EVENTFLAG_LEFT_MOUSE_BUTTON: u32 = 1 << 4;
const EVENTFLAG_MIDDLE_MOUSE_BUTTON: u32 = 1 << 5;
const EVENTFLAG_RIGHT_MOUSE_BUTTON: u32 = 1 << 6;

pub fn forward_mouse_down(
    host: &AsyncCefHost,
    id: BrowserId,
    event: &MouseDownEvent,
    view_origin: Point<gpui::Pixels>,
) {
    let (x, y) = view_relative(event.position, view_origin);
    let button = map_button(event.button);
    let modifiers = modifiers_bits(&event.modifiers) | mouse_button_flag(button);
    // CEF expects the cursor position to be updated before click events.
    host.send_mouse_move(id, x, y, false, modifiers).log_err();
    host.send_mouse_click(id, x, y, button, false, event.click_count as u32, modifiers)
        .log_err();
}

pub fn forward_mouse_up(
    host: &AsyncCefHost,
    id: BrowserId,
    event: &MouseUpEvent,
    view_origin: Point<gpui::Pixels>,
) {
    let (x, y) = view_relative(event.position, view_origin);
    let button = map_button(event.button);
    let modifiers = modifiers_bits(&event.modifiers) | mouse_button_flag(button);
    host.send_mouse_move(id, x, y, false, modifiers).log_err();
    host.send_mouse_click(id, x, y, button, true, event.click_count as u32, modifiers)
        .log_err();
}

pub fn forward_mouse_move(
    host: &AsyncCefHost,
    id: BrowserId,
    event: &MouseMoveEvent,
    view_origin: Point<gpui::Pixels>,
) {
    let (x, y) = view_relative(event.position, view_origin);
    let mut modifiers = modifiers_bits(&event.modifiers);
    if event.pressed_button == Some(MouseButton::Left) {
        modifiers |= EVENTFLAG_LEFT_MOUSE_BUTTON;
    }
    host.send_mouse_move(id, x, y, false, modifiers).log_err();
}

pub fn forward_scroll(
    host: &AsyncCefHost,
    id: BrowserId,
    event: &ScrollWheelEvent,
    view_origin: Point<gpui::Pixels>,
) {
    let (x, y) = view_relative(event.position, view_origin);
    let delta = event.delta.pixel_delta(gpui::px(16.));
    host.send_mouse_wheel(
        id,
        x,
        y,
        delta.x.into(),
        delta.y.into(),
        modifiers_bits(&event.modifiers),
    )
    .log_err();
}

pub fn forward_key_down(host: &AsyncCefHost, id: BrowserId, event: &KeyDownEvent) {
    let characters = event
        .keystroke
        .key_char
        .clone()
        .unwrap_or_else(|| event.keystroke.key.clone());
    host.send_key_event(
        id,
        KeyEventPayload {
            key_down: true,
            characters,
            keycode: 0,
            modifiers: modifiers_bits(&event.keystroke.modifiers),
        },
    )
    .log_err();
}

pub fn forward_key_up(host: &AsyncCefHost, id: BrowserId, event: &KeyUpEvent) {
    host.send_key_event(
        id,
        KeyEventPayload {
            key_down: false,
            characters: event.keystroke.key.clone(),
            keycode: 0,
            modifiers: modifiers_bits(&event.keystroke.modifiers),
        },
    )
    .log_err();
}

fn view_relative(position: Point<gpui::Pixels>, origin: Point<gpui::Pixels>) -> (f32, f32) {
    let x: f32 = (position.x - origin.x).into();
    let y: f32 = (position.y - origin.y).into();
    (x, y)
}

fn map_button(button: MouseButton) -> MouseButtonKind {
    match button {
        MouseButton::Left => MouseButtonKind::Left,
        MouseButton::Middle => MouseButtonKind::Middle,
        MouseButton::Right => MouseButtonKind::Right,
        _ => MouseButtonKind::Left,
    }
}

fn mouse_button_flag(button: MouseButtonKind) -> u32 {
    match button {
        MouseButtonKind::Left => EVENTFLAG_LEFT_MOUSE_BUTTON,
        MouseButtonKind::Middle => EVENTFLAG_MIDDLE_MOUSE_BUTTON,
        MouseButtonKind::Right => EVENTFLAG_RIGHT_MOUSE_BUTTON,
    }
}

fn modifiers_bits(modifiers: &Modifiers) -> u32 {
    let mut bits = 0u32;
    if modifiers.shift {
        bits |= EVENTFLAG_SHIFT_DOWN;
    }
    if modifiers.control {
        bits |= EVENTFLAG_CONTROL_DOWN;
    }
    if modifiers.alt {
        bits |= EVENTFLAG_ALT_DOWN;
    }
    if modifiers.platform {
        bits |= EVENTFLAG_COMMAND_DOWN;
    }
    bits
}
