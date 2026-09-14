use crate::config::Config;
use global_hotkey::hotkey::{Code, HotKey};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HotkeyAction {
    Switch,
    VolumeUp,
    VolumeDown,
}

#[derive(Error, Debug)]
pub enum HotkeyError {
    #[error("hotkey: {0}")]
    Global(#[from] global_hotkey::Error),
    #[error("raccourci invalide '{value}': {message}")]
    Parse { value: String, message: String },
}

pub struct HotkeyRegistry {
    _manager: GlobalHotKeyManager,
    action_by_id: HashMap<u32, HotkeyAction>,
}

impl HotkeyRegistry {
    pub fn register(config: &Config) -> Result<Self, HotkeyError> {
        let manager = GlobalHotKeyManager::new()?;
        let mut action_by_id = HashMap::new();

        let switch = register_hotkey(&manager, &config.hotkeys.switch, HotkeyAction::Switch)?;
        let volume_up =
            register_hotkey(&manager, &config.hotkeys.volume_up, HotkeyAction::VolumeUp)?;
        let volume_down = register_hotkey(
            &manager,
            &config.hotkeys.volume_down,
            HotkeyAction::VolumeDown,
        )?;

        action_by_id.insert(switch.0, switch.1);
        action_by_id.insert(volume_up.0, volume_up.1);
        action_by_id.insert(volume_down.0, volume_down.1);

        Ok(Self {
            _manager: manager,
            action_by_id,
        })
    }

    pub fn action_for_event(&self, event: &GlobalHotKeyEvent) -> Option<HotkeyAction> {
        // Ignorer le relâchement : sinon un appui Mute = 2 switches (retour au 1er canal).
        if event.state != HotKeyState::Pressed {
            return None;
        }
        self.action_by_id.get(&event.id).copied()
    }
}

fn register_hotkey(
    manager: &GlobalHotKeyManager,
    value: &str,
    action: HotkeyAction,
) -> Result<(u32, HotkeyAction), HotkeyError> {
    let hotkey = parse_hotkey(value)?;
    manager.register(hotkey)?;
    Ok((hotkey.id(), action))
}

pub fn parse_hotkey(value: &str) -> Result<HotKey, HotkeyError> {
    let trimmed = value.trim();
    if let Ok(hotkey) = trimmed.parse::<HotKey>() {
        return Ok(hotkey);
    }

    if let Some(code) = media_code(trimmed) {
        return Ok(HotKey::new(None, code));
    }

    Err(HotkeyError::Parse {
        value: value.into(),
        message: "format attendu: AudioVolumeUp ou Ctrl+Alt+KeyS".into(),
    })
}

fn media_code(name: &str) -> Option<Code> {
    match name {
        "AudioVolumeUp" => Some(Code::AudioVolumeUp),
        "AudioVolumeDown" => Some(Code::AudioVolumeDown),
        "AudioVolumeMute" => Some(Code::AudioVolumeMute),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_media_keys() {
        assert!(parse_hotkey("AudioVolumeMute").is_ok());
        assert!(parse_hotkey("AudioVolumeUp").is_ok());
    }
}
