use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const DEFAULT_VID: u16 = 0x1189;
pub const DEFAULT_PID: u16 = 0x8890;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum DeviceMode {
    Zoom,
    Video,
}

#[allow(dead_code)]
impl DeviceMode {
    pub fn label(self) -> &'static str {
        match self {
            DeviceMode::Zoom => "Zoom",
            DeviceMode::Video => "Video",
        }
    }

    pub fn toggle(self) -> Self {
        match self {
            DeviceMode::Zoom => DeviceMode::Video,
            DeviceMode::Video => DeviceMode::Zoom,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum PhysicalKey {
    Prev,
    Next,
    Play,
    KnobClick,
    KnobLeft,
    KnobRight,
}

impl PhysicalKey {
    pub const ALL: [PhysicalKey; 6] = [
        PhysicalKey::Prev,
        PhysicalKey::Next,
        PhysicalKey::Play,
        PhysicalKey::KnobClick,
        PhysicalKey::KnobLeft,
        PhysicalKey::KnobRight,
    ];

    pub fn label(self) -> &'static str {
        match self {
            PhysicalKey::Prev => "Bouton 1 (Prev)",
            PhysicalKey::Next => "Bouton 2 (Next)",
            PhysicalKey::Play => "Bouton 3 (Play)",
            PhysicalKey::KnobClick => "Knob clic (Mute)",
            PhysicalKey::KnobLeft => "Knob gauche (Vol-)",
            PhysicalKey::KnobRight => "Knob droite (Vol+)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionPreset {
    None,
    ToggleMode,
    CtrlAltStar,
    CtrlAltQuote,
    CtrlAltParen,
    CtrlScrollUp,
    CtrlScrollDown,
    ArrowLeft,
    ArrowRight,
    /// Touches simples pour tester les mappings (sans modificateur).
    KeyF1,
    KeyF2,
    KeyF3,
    KeyF4,
    KeyF5,
    KeyF6,
    KeyA,
    KeyB,
    KeyC,
    PageUp,
    PageDown,
}

#[allow(dead_code)]
impl ActionPreset {
    pub const TRAY_OPTIONS: [ActionPreset; 20] = [
        ActionPreset::None,
        ActionPreset::ToggleMode,
        ActionPreset::KeyF1,
        ActionPreset::KeyF2,
        ActionPreset::KeyF3,
        ActionPreset::KeyF4,
        ActionPreset::KeyF5,
        ActionPreset::KeyF6,
        ActionPreset::KeyA,
        ActionPreset::KeyB,
        ActionPreset::KeyC,
        ActionPreset::PageUp,
        ActionPreset::PageDown,
        ActionPreset::CtrlAltStar,
        ActionPreset::CtrlAltQuote,
        ActionPreset::CtrlAltParen,
        ActionPreset::CtrlScrollUp,
        ActionPreset::CtrlScrollDown,
        ActionPreset::ArrowLeft,
        ActionPreset::ArrowRight,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ActionPreset::None => "Aucune",
            ActionPreset::ToggleMode => "Basculer mode Zoom/Video",
            ActionPreset::CtrlAltStar => "Ctrl + Alt + *",
            ActionPreset::CtrlAltQuote => "Ctrl + Alt + \"",
            ActionPreset::CtrlAltParen => "Ctrl + Alt + )",
            ActionPreset::CtrlScrollUp => "Ctrl + Molette haut",
            ActionPreset::CtrlScrollDown => "Ctrl + Molette bas",
            ActionPreset::ArrowLeft => "Flèche gauche",
            ActionPreset::ArrowRight => "Flèche droite",
            ActionPreset::KeyF1 => "Touche F1",
            ActionPreset::KeyF2 => "Touche F2",
            ActionPreset::KeyF3 => "Touche F3",
            ActionPreset::KeyF4 => "Touche F4",
            ActionPreset::KeyF5 => "Touche F5",
            ActionPreset::KeyF6 => "Touche F6",
            ActionPreset::KeyA => "Touche A",
            ActionPreset::KeyB => "Touche B",
            ActionPreset::KeyC => "Touche C",
            ActionPreset::PageUp => "Page Up",
            ActionPreset::PageDown => "Page Down",
        }
    }

    fn simple_key(vk: u16) -> MappedAction {
        MappedAction::SimpleKey { vk }
    }

    pub fn to_mapped_action(self) -> MappedAction {
        match self {
            ActionPreset::None => MappedAction::None,
            ActionPreset::ToggleMode => MappedAction::ToggleMode,
            ActionPreset::KeyF1 => Self::simple_key(0x70),
            ActionPreset::KeyF2 => Self::simple_key(0x71),
            ActionPreset::KeyF3 => Self::simple_key(0x72),
            ActionPreset::KeyF4 => Self::simple_key(0x73),
            ActionPreset::KeyF5 => Self::simple_key(0x74),
            ActionPreset::KeyF6 => Self::simple_key(0x75),
            ActionPreset::KeyA => Self::simple_key(0x41),
            ActionPreset::KeyB => Self::simple_key(0x42),
            ActionPreset::KeyC => Self::simple_key(0x43),
            ActionPreset::PageUp => Self::simple_key(0x21),
            ActionPreset::PageDown => Self::simple_key(0x22),
            ActionPreset::CtrlAltStar => MappedAction::KeyCombo {
                vk: 0x6A, // VK_MULTIPLY
                ctrl: true,
                alt: true,
                shift: false,
            },
            ActionPreset::CtrlAltQuote => MappedAction::KeyCombo {
                vk: 0xDE, // VK_OEM_7 (US ")
                ctrl: true,
                alt: true,
                shift: false,
            },
            ActionPreset::CtrlAltParen => MappedAction::KeyCombo {
                vk: 0x30, // '0' + shift = )
                ctrl: true,
                alt: true,
                shift: true,
            },
            ActionPreset::CtrlScrollUp => MappedAction::MouseWheel {
                delta: 120,
                ctrl: true,
            },
            ActionPreset::CtrlScrollDown => MappedAction::MouseWheel {
                delta: -120,
                ctrl: true,
            },
            ActionPreset::ArrowLeft => MappedAction::ArrowLeft,
            ActionPreset::ArrowRight => MappedAction::ArrowRight,
        }
    }

    pub fn from_mapped_action(action: &MappedAction) -> Option<Self> {
        match action {
            MappedAction::None => Some(ActionPreset::None),
            MappedAction::ToggleMode => Some(ActionPreset::ToggleMode),
            MappedAction::KeyCombo {
                vk: 0x6A,
                ctrl: true,
                alt: true,
                shift: false,
            } => Some(ActionPreset::CtrlAltStar),
            MappedAction::KeyCombo {
                vk: 0xDE,
                ctrl: true,
                alt: true,
                shift: false,
            } => Some(ActionPreset::CtrlAltQuote),
            MappedAction::KeyCombo {
                vk: 0x30,
                ctrl: true,
                alt: true,
                shift: true,
            } => Some(ActionPreset::CtrlAltParen),
            MappedAction::MouseWheel {
                delta: 120,
                ctrl: true,
            } => Some(ActionPreset::CtrlScrollUp),
            MappedAction::MouseWheel {
                delta: -120,
                ctrl: true,
            } => Some(ActionPreset::CtrlScrollDown),
            MappedAction::ArrowLeft => Some(ActionPreset::ArrowLeft),
            MappedAction::ArrowRight => Some(ActionPreset::ArrowRight),
            MappedAction::SimpleKey { vk: 0x70 } => Some(ActionPreset::KeyF1),
            MappedAction::SimpleKey { vk: 0x71 } => Some(ActionPreset::KeyF2),
            MappedAction::SimpleKey { vk: 0x72 } => Some(ActionPreset::KeyF3),
            MappedAction::SimpleKey { vk: 0x73 } => Some(ActionPreset::KeyF4),
            MappedAction::SimpleKey { vk: 0x74 } => Some(ActionPreset::KeyF5),
            MappedAction::SimpleKey { vk: 0x75 } => Some(ActionPreset::KeyF6),
            MappedAction::SimpleKey { vk: 0x41 } => Some(ActionPreset::KeyA),
            MappedAction::SimpleKey { vk: 0x42 } => Some(ActionPreset::KeyB),
            MappedAction::SimpleKey { vk: 0x43 } => Some(ActionPreset::KeyC),
            MappedAction::SimpleKey { vk: 0x21 } => Some(ActionPreset::PageUp),
            MappedAction::SimpleKey { vk: 0x22 } => Some(ActionPreset::PageDown),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MappedAction {
    None,
    /// Legacy — traité comme CycleScene.
    ToggleMode,
    KeyCombo {
        vk: u16,
        ctrl: bool,
        alt: bool,
        shift: bool,
    },
    MouseWheel {
        delta: i16,
        ctrl: bool,
    },
    ArrowLeft,
    ArrowRight,
    SimpleKey {
        vk: u16,
    },
    LaunchProgram {
        path: String,
        #[serde(default)]
        args: Vec<String>,
    },
    RunCommand {
        line: String,
        #[serde(default = "default_use_shell")]
        use_shell: bool,
    },
    SwitchScene {
        scene_id: String,
    },
    CycleScene,
}

fn default_use_shell() -> bool {
    true
}

pub type SceneMappings = HashMap<PhysicalKey, MappedAction>;

impl MappedAction {
    pub fn label(&self) -> String {
        match self {
            MappedAction::None => "Aucune".into(),
            MappedAction::ToggleMode | MappedAction::CycleScene => "Scène suivante".into(),
            MappedAction::KeyCombo {
                vk,
                ctrl,
                alt,
                shift,
            } => format!(
                "{}{}{}{}",
                if *ctrl { "Ctrl+" } else { "" },
                if *alt { "Alt+" } else { "" },
                if *shift { "Shift+" } else { "" },
                vk_label(*vk)
            ),
            MappedAction::SimpleKey { vk } => vk_label(*vk),
            MappedAction::MouseWheel { delta, ctrl } => {
                let dir = if *delta > 0 { "haut" } else { "bas" };
                if *ctrl {
                    format!("Ctrl + Molette {dir}")
                } else {
                    format!("Molette {dir}")
                }
            }
            MappedAction::ArrowLeft => "Flèche gauche".into(),
            MappedAction::ArrowRight => "Flèche droite".into(),
            MappedAction::LaunchProgram { path, args } => {
                if args.is_empty() {
                    format!("Lancer {}", path)
                } else {
                    format!("Lancer {} {:?}", path, args)
                }
            }
            MappedAction::RunCommand { line, .. } => {
                let preview: String = line.chars().take(40).collect();
                if line.len() > 40 {
                    format!("Commande: {preview}…")
                } else {
                    format!("Commande: {preview}")
                }
            }
            MappedAction::SwitchScene { scene_id } => format!("Scène: {scene_id}"),
        }
    }

    pub fn is_injectable(&self) -> bool {
        matches!(
            self,
            MappedAction::KeyCombo { .. }
                | MappedAction::MouseWheel { .. }
                | MappedAction::ArrowLeft
                | MappedAction::ArrowRight
                | MappedAction::SimpleKey { .. }
        )
    }
}

pub fn vk_label(vk: u16) -> String {
    match vk {
        0x70..=0x7B => format!("F{}", vk - 0x70 + 1),
        0x21 => "Page Up".into(),
        0x22 => "Page Down".into(),
        0x25 => "Gauche".into(),
        0x26 => "Haut".into(),
        0x27 => "Droite".into(),
        0x28 => "Bas".into(),
        0x6A => "*".into(),
        0xDE => "\"".into(),
        0x30..=0x39 => ((vk as u8) as char).to_string(),
        0x41..=0x5A => ((vk as u8) as char).to_string(),
        other => format!("VK_{other:02X}"),
    }
}

pub type ModeMappings = HashMap<PhysicalKey, MappedAction>;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MappingTable {
    #[serde(default)]
    pub zoom: ModeMappings,
    #[serde(default)]
    pub video: ModeMappings,
}

impl MappingTable {
    pub fn defaults() -> Self {
        let mut zoom = ModeMappings::new();
        zoom.insert(PhysicalKey::Prev, ActionPreset::CtrlAltStar.to_mapped_action());
        zoom.insert(PhysicalKey::Next, ActionPreset::CtrlAltQuote.to_mapped_action());
        zoom.insert(PhysicalKey::Play, ActionPreset::CtrlAltParen.to_mapped_action());
        zoom.insert(PhysicalKey::KnobClick, ActionPreset::ToggleMode.to_mapped_action());
        zoom.insert(PhysicalKey::KnobLeft, ActionPreset::CtrlScrollDown.to_mapped_action());
        zoom.insert(PhysicalKey::KnobRight, ActionPreset::CtrlScrollUp.to_mapped_action());

        let mut video = ModeMappings::new();
        video.insert(PhysicalKey::Prev, ActionPreset::CtrlAltStar.to_mapped_action());
        video.insert(PhysicalKey::Next, ActionPreset::CtrlAltQuote.to_mapped_action());
        video.insert(PhysicalKey::Play, ActionPreset::CtrlAltParen.to_mapped_action());
        video.insert(PhysicalKey::KnobClick, ActionPreset::ToggleMode.to_mapped_action());
        video.insert(PhysicalKey::KnobLeft, ActionPreset::ArrowLeft.to_mapped_action());
        video.insert(PhysicalKey::KnobRight, ActionPreset::ArrowRight.to_mapped_action());

        Self { zoom, video }
    }

    pub fn get(&self, mode: DeviceMode, key: PhysicalKey) -> MappedAction {
        let table = match mode {
            DeviceMode::Zoom => &self.zoom,
            DeviceMode::Video => &self.video,
        };
        table
            .get(&key)
            .cloned()
            .unwrap_or(MappedAction::None)
    }

    #[allow(dead_code)]
    pub fn set(&mut self, mode: DeviceMode, key: PhysicalKey, preset: ActionPreset) {
        let table = match mode {
            DeviceMode::Zoom => &mut self.zoom,
            DeviceMode::Video => &mut self.video,
        };
        table.insert(key, preset.to_mapped_action());
    }

    #[allow(dead_code)]
    pub fn preset_for(&self, mode: DeviceMode, key: PhysicalKey) -> ActionPreset {
        let action = self.get(mode, key);
        ActionPreset::from_mapped_action(&action).unwrap_or(ActionPreset::None)
    }

    pub fn mode_to_scene_mappings(&self, mode: DeviceMode) -> SceneMappings {
        let table = match mode {
            DeviceMode::Zoom => &self.zoom,
            DeviceMode::Video => &self.video,
        };
        PhysicalKey::ALL
            .iter()
            .map(|&key| {
                (
                    key,
                    table
                        .get(&key)
                        .cloned()
                        .unwrap_or(MappedAction::None),
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_have_all_keys() {
        let table = MappingTable::defaults();
        for key in PhysicalKey::ALL {
            assert_ne!(table.get(DeviceMode::Zoom, key), MappedAction::None);
            assert_ne!(table.get(DeviceMode::Video, key), MappedAction::None);
        }
    }
}
