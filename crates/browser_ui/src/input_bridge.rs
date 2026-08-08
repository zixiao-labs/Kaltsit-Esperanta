//! Forward GPUI input events into the CEF host (when Design Mode is off).

use extension_cef::{AsyncCefHost, BrowserId, KeyEventPayload, MouseButtonKind};
use gpui::{
    KeyDownEvent, KeyUpEvent, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    ScrollWheelEvent,
};
use util::ResultExt as _;

pub fn forward_mouse_down(host: &AsyncCefHost, id: BrowserId, event: &MouseDownEvent) {
    let button = map_button(event.button);
    host.send_mouse_click_blocking(
        id,
        event.position.x.into(),
        event.position.y.into(),
        button,
        false,
        event.click_count as u32,
    )
    .log_err();
}

pub fn forward_mouse_up(host: &AsyncCefHost, id: BrowserId, event: &MouseUpEvent) {
    let button = map_button(event.button);
    host.send_mouse_click_blocking(
        id,
        event.position.x.into(),
        event.position.y.into(),
        button,
        true,
        event.click_count as u32,
    )
    .log_err();
}

pub fn forward_mouse_move(host: &AsyncCefHost, id: BrowserId, event: &MouseMoveEvent) {
    host.send_mouse_move(id, event.position.x.into(), event.position.y.into(), false)
        .log_err();
}

pub fn forward_scroll(host: &AsyncCefHost, id: BrowserId, event: &ScrollWheelEvent) {
    let delta = event.delta.pixel_delta(gpui::px(16.));
    host.send_mouse_wheel_blocking(
        id,
        event.position.x.into(),
        event.position.y.into(),
        delta.x.into(),
        delta.y.into(),
    )
    .log_err();
}

pub fn forward_key_down(host: &AsyncCefHost, id: BrowserId, event: &KeyDownEvent) {
    host.send_key_event_blocking(
        id,
        KeyEventPayload {
            key_down: true,
            characters: event.keystroke.key.clone(),
            keycode: 0,
            modifiers: modifiers_bits(&event.keystroke.modifiers),
        },
    )
    .log_err();
}

pub fn forward_key_up(host: &AsyncCefHost, id: BrowserId, event: &KeyUpEvent) {
    host.send_key_event_blocking(
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

fn map_button(button: MouseButton) -> MouseButtonKind {
    match button {
        MouseButton::Left => MouseButtonKind::Left,
        MouseButton::Middle => MouseButtonKind::Middle,
        MouseButton::Right => MouseButtonKind::Right,
        _ => MouseButtonKind::Left,
    }
}

fn modifiers_bits(modifiers: &gpui::Modifiers) -> u32 {
    let mut bits = 0u32;
    if modifiers.shift {
        bits |= 1 << 0;
    }
    if modifiers.control {
        bits |= 1 << 1;
    }
    if modifiers.alt {
        bits |= 1 << 2;
    }
    if modifiers.platform {
        bits |= 1 << 3;
    }
    bits
}
