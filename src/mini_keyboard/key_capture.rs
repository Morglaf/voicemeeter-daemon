use crate::mini_keyboard::hook::{self};
use egui::Key;
use winit::event::ElementState;
use winit::keyboard::{KeyCode, PhysicalKey};

pub use crate::mini_keyboard::hook::CapturedKeys;

pub fn begin_capture() {
    hook::begin_key_capture();
}

pub fn cancel_capture() {
    hook::cancel_key_capture();
}

pub fn poll_capture() -> Option<CapturedKeys> {
    hook::poll_key_capture()
}

pub fn format_captured(keys: &CapturedKeys) -> String {
    let mut parts: Vec<String> = Vec::new();
    if keys.ctrl {
        parts.push("Ctrl".into());
    }
    if keys.alt {
        parts.push("Alt".into());
    }
    if keys.shift {
        parts.push("Shift".into());
    }
    parts.push(crate::mini_keyboard::mapping::vk_label(keys.vk));
    parts.join("+")
}

pub fn try_winit_key(event: &winit::event::KeyEvent) -> Option<CapturedKeys> {
    if event.state != ElementState::Pressed || event.repeat {
        return None;
    }
    let vk = physical_key_to_vk(event.physical_key)?;
    if hook::is_modifier_vk(vk) {
        return None;
    }
    Some(CapturedKeys {
        vk,
        ctrl: modifier_down_ctrl(),
        alt: modifier_down_alt(),
        shift: modifier_down_shift(),
    })
}

pub fn try_egui_key(key: Key, modifiers: egui::Modifiers) -> Option<CapturedKeys> {
    let vk = egui_key_to_vk(key)?;
    if hook::is_modifier_vk(vk) {
        return None;
    }
    Some(CapturedKeys {
        vk,
        ctrl: modifiers.ctrl,
        alt: modifiers.alt,
        shift: modifiers.shift,
    })
}

fn modifier_down_ctrl() -> bool {
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_CONTROL};
    unsafe { GetAsyncKeyState(VK_CONTROL.0 as i32) as u16 & 0x8000 != 0 }
}

fn modifier_down_alt() -> bool {
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_MENU};
    unsafe { GetAsyncKeyState(VK_MENU.0 as i32) as u16 & 0x8000 != 0 }
}

fn modifier_down_shift() -> bool {
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_SHIFT};
    unsafe { GetAsyncKeyState(VK_SHIFT.0 as i32) as u16 & 0x8000 != 0 }
}

fn physical_key_to_vk(key: PhysicalKey) -> Option<u16> {
    match key {
        PhysicalKey::Code(code) => key_code_to_vk(code),
        PhysicalKey::Unidentified(_) => None,
    }
}

fn key_code_to_vk(code: KeyCode) -> Option<u16> {
    use KeyCode::*;
    Some(match code {
        KeyA => 0x41,
        KeyB => 0x42,
        KeyC => 0x43,
        KeyD => 0x44,
        KeyE => 0x45,
        KeyF => 0x46,
        KeyG => 0x47,
        KeyH => 0x48,
        KeyI => 0x49,
        KeyJ => 0x4A,
        KeyK => 0x4B,
        KeyL => 0x4C,
        KeyM => 0x4D,
        KeyN => 0x4E,
        KeyO => 0x4F,
        KeyP => 0x50,
        KeyQ => 0x51,
        KeyR => 0x52,
        KeyS => 0x53,
        KeyT => 0x54,
        KeyU => 0x55,
        KeyV => 0x56,
        KeyW => 0x57,
        KeyX => 0x58,
        KeyY => 0x59,
        KeyZ => 0x5A,
        Digit0 => 0x30,
        Digit1 => 0x31,
        Digit2 => 0x32,
        Digit3 => 0x33,
        Digit4 => 0x34,
        Digit5 => 0x35,
        Digit6 => 0x36,
        Digit7 => 0x37,
        Digit8 => 0x38,
        Digit9 => 0x39,
        F1 => 0x70,
        F2 => 0x71,
        F3 => 0x72,
        F4 => 0x73,
        F5 => 0x74,
        F6 => 0x75,
        F7 => 0x76,
        F8 => 0x77,
        F9 => 0x78,
        F10 => 0x79,
        F11 => 0x7A,
        F12 => 0x7B,
        ArrowLeft => 0x25,
        ArrowRight => 0x27,
        ArrowUp => 0x26,
        ArrowDown => 0x28,
        PageUp => 0x21,
        PageDown => 0x22,
        Home => 0x24,
        End => 0x23,
        Insert => 0x2D,
        Delete => 0x2E,
        Space => 0x20,
        Enter => 0x0D,
        Escape => 0x1B,
        Tab => 0x09,
        Backspace => 0x08,
        Minus => 0xBD,
        Equal => 0xBB,
        BracketLeft => 0xDB,
        BracketRight => 0xDD,
        Backslash => 0xDC,
        Semicolon => 0xBA,
        Quote => 0xDE,
        Comma => 0xBC,
        Period => 0xBE,
        Slash => 0xBF,
        Numpad0 => 0x60,
        Numpad1 => 0x61,
        Numpad2 => 0x62,
        Numpad3 => 0x63,
        Numpad4 => 0x64,
        Numpad5 => 0x65,
        Numpad6 => 0x66,
        Numpad7 => 0x67,
        Numpad8 => 0x68,
        Numpad9 => 0x69,
        _ => return None,
    })
}

fn egui_key_to_vk(key: Key) -> Option<u16> {
    match key {
        Key::A => Some(0x41),
        Key::B => Some(0x42),
        Key::C => Some(0x43),
        Key::D => Some(0x44),
        Key::E => Some(0x45),
        Key::F => Some(0x46),
        Key::G => Some(0x47),
        Key::H => Some(0x48),
        Key::I => Some(0x49),
        Key::J => Some(0x4A),
        Key::K => Some(0x4B),
        Key::L => Some(0x4C),
        Key::M => Some(0x4D),
        Key::N => Some(0x4E),
        Key::O => Some(0x4F),
        Key::P => Some(0x50),
        Key::Q => Some(0x51),
        Key::R => Some(0x52),
        Key::S => Some(0x53),
        Key::T => Some(0x54),
        Key::U => Some(0x55),
        Key::V => Some(0x56),
        Key::W => Some(0x57),
        Key::X => Some(0x58),
        Key::Y => Some(0x59),
        Key::Z => Some(0x5A),
        Key::Num0 => Some(0x30),
        Key::Num1 => Some(0x31),
        Key::Num2 => Some(0x32),
        Key::Num3 => Some(0x33),
        Key::Num4 => Some(0x34),
        Key::Num5 => Some(0x35),
        Key::Num6 => Some(0x36),
        Key::Num7 => Some(0x37),
        Key::Num8 => Some(0x38),
        Key::Num9 => Some(0x39),
        Key::F1 => Some(0x70),
        Key::F2 => Some(0x71),
        Key::F3 => Some(0x72),
        Key::F4 => Some(0x73),
        Key::F5 => Some(0x74),
        Key::F6 => Some(0x75),
        Key::F7 => Some(0x76),
        Key::F8 => Some(0x77),
        Key::F9 => Some(0x78),
        Key::F10 => Some(0x79),
        Key::F11 => Some(0x7A),
        Key::F12 => Some(0x7B),
        Key::ArrowLeft => Some(0x25),
        Key::ArrowRight => Some(0x27),
        Key::ArrowUp => Some(0x26),
        Key::ArrowDown => Some(0x28),
        Key::PageUp => Some(0x21),
        Key::PageDown => Some(0x22),
        Key::Home => Some(0x24),
        Key::End => Some(0x23),
        Key::Insert => Some(0x2D),
        Key::Delete => Some(0x2E),
        Key::Space => Some(0x20),
        Key::Enter => Some(0x0D),
        Key::Escape => Some(0x1B),
        Key::Tab => Some(0x09),
        Key::Backspace => Some(0x08),
        Key::Minus => Some(0xBD),
        Key::Plus | Key::Equals => Some(0xBB),
        Key::OpenBracket => Some(0xDB),
        Key::CloseBracket => Some(0xDD),
        Key::Backslash => Some(0xDC),
        Key::Semicolon => Some(0xBA),
        Key::Quote => Some(0xDE),
        Key::Comma => Some(0xBC),
        Key::Period => Some(0xBE),
        Key::Slash => Some(0xBF),
        _ => None,
    }
}
