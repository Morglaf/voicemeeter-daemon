use super::mapping::MappedAction;
use std::thread;
use std::time::Duration;
use thiserror::Error;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT, KEYEVENTF_KEYUP, KEYEVENTF_EXTENDEDKEY,
    MOUSEEVENTF_WHEEL, MOUSEINPUT, VIRTUAL_KEY, VK_CONTROL, VK_LMENU, VK_LEFT, VK_RIGHT, VK_SHIFT,
};

const STEP_DELAY: Duration = Duration::from_millis(10);

#[derive(Error, Debug)]
pub enum InjectError {
    #[error("SendInput a échoué")]
    SendFailed,
}

pub fn execute(action: &MappedAction) -> Result<(), InjectError> {
    match action {
        MappedAction::None | MappedAction::ToggleMode | MappedAction::CycleScene
        | MappedAction::SwitchScene { .. } | MappedAction::LaunchProgram { .. }
        | MappedAction::RunCommand { .. } => Ok(()),
        MappedAction::KeyCombo {
            vk,
            ctrl,
            alt,
            shift,
        } => send_key_combo(*vk, *ctrl, *alt, *shift),
        MappedAction::MouseWheel { delta, ctrl } => send_mouse_wheel(*delta, *ctrl),
        MappedAction::ArrowLeft => send_single_key(VK_LEFT),
        MappedAction::ArrowRight => send_single_key(VK_RIGHT),
        MappedAction::SimpleKey { vk } => send_single_key(VIRTUAL_KEY(*vk)),
    }
}

pub fn build_key_combo_inputs(vk: u16, ctrl: bool, alt: bool, shift: bool) -> Vec<INPUT> {
    let mut inputs = Vec::new();
    if ctrl {
        inputs.push(key_input(VK_CONTROL, false));
    }
    if alt {
        inputs.push(key_input(VK_LMENU, false));
    }
    if shift {
        inputs.push(key_input(VK_SHIFT, false));
    }
    inputs.push(key_input(VIRTUAL_KEY(vk), false));
    inputs.push(key_input(VIRTUAL_KEY(vk), true));
    if shift {
        inputs.push(key_input(VK_SHIFT, true));
    }
    if alt {
        inputs.push(key_input(VK_LMENU, true));
    }
    if ctrl {
        inputs.push(key_input(VK_CONTROL, true));
    }
    inputs
}

fn send_key_combo(vk: u16, ctrl: bool, alt: bool, shift: bool) -> Result<(), InjectError> {
    let inputs = build_key_combo_inputs(vk, ctrl, alt, shift);
    send_inputs(&inputs)
}

fn send_single_key(vk: VIRTUAL_KEY) -> Result<(), InjectError> {
    let inputs = vec![key_input(vk, false), key_input(vk, true)];
    send_inputs(&inputs)
}

fn send_mouse_wheel(delta: i16, ctrl: bool) -> Result<(), InjectError> {
    let mut batch = Vec::new();
    if ctrl {
        batch.push(key_input(VK_CONTROL, false));
    }
    batch.push(wheel_input(delta));
    if ctrl {
        batch.push(key_input(VK_CONTROL, true));
    }
    send_inputs_fast(&batch)
}

fn key_input(vk: VIRTUAL_KEY, key_up: bool) -> INPUT {
    let mut flags = KEYBDINPUT {
        wVk: vk,
        wScan: 0,
        dwFlags: if key_up { KEYEVENTF_KEYUP } else { Default::default() },
        time: 0,
        dwExtraInfo: 0,
    };
    if matches!(vk, VK_LEFT | VK_RIGHT) {
        flags.dwFlags |= KEYEVENTF_EXTENDEDKEY;
    }
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
            ki: flags,
        },
    }
}

fn wheel_input(delta: i16) -> INPUT {
    INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
            mi: MOUSEINPUT {
                dx: 0,
                dy: 0,
                mouseData: delta as u32,
                dwFlags: MOUSEEVENTF_WHEEL,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn send_inputs(inputs: &[INPUT]) -> Result<(), InjectError> {
    for (i, input) in inputs.iter().enumerate() {
        if i > 0 {
            thread::sleep(STEP_DELAY);
        }
        let sent = unsafe { SendInput(&[*input], std::mem::size_of::<INPUT>() as i32) };
        if sent != 1 {
            return Err(InjectError::SendFailed);
        }
    }
    Ok(())
}

fn send_inputs_fast(inputs: &[INPUT]) -> Result<(), InjectError> {
    if inputs.is_empty() {
        return Ok(());
    }
    let sent = unsafe { SendInput(inputs, std::mem::size_of::<INPUT>() as i32) };
    if sent != inputs.len() as u32 {
        return Err(InjectError::SendFailed);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combo_input_count() {
        let inputs = build_key_combo_inputs(0x6A, true, true, false);
        assert_eq!(inputs.len(), 6);
    }

    #[test]
    fn combo_with_shift_has_more_inputs() {
        let base = build_key_combo_inputs(0x30, true, true, false);
        let shifted = build_key_combo_inputs(0x30, true, true, true);
        assert!(shifted.len() > base.len());
    }
}
