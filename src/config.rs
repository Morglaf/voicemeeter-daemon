use directories::ProjectDirs;
use crate::mini_keyboard::mapping::{DeviceMode, MappingTable, DEFAULT_PID, DEFAULT_VID};
use crate::scenes::{
    migrate_toggle_mode_to_cycle, ScenesConfig, Scene, ZOOM_SCENE_ID, VIDEO_SCENE_ID,
    DEFAULT_SCENE_ID,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::PathBuf;

/// Nombre de bus affichés dans le menu tray (Banana = 5, Potato = 8).
pub const MENU_BUS_COUNT: usize = 5;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeyConfig {
    pub switch: String,
    pub volume_up: String,
    pub volume_down: String,
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        Self {
            switch: "AudioVolumeMute".into(),
            volume_up: "AudioVolumeUp".into(),
            volume_down: "AudioVolumeDown".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Bus participant au switch (ex. [0, 2] = A1 et A3).
    #[serde(default = "default_switch_buses")]
    pub switch_buses: Vec<usize>,
    pub active_bus: usize,
    pub volume_increment: f32,
    pub hotkeys: HotkeyConfig,
    #[serde(default)]
    pub scenes: ScenesConfig,
    #[serde(default)]
    pub mini_keyboard: MiniKeyboardConfig,
    /// Champs legacy — migrés vers switch_buses au chargement.
    #[serde(default, skip_serializing)]
    bus_a: Option<usize>,
    #[serde(default, skip_serializing)]
    bus_b: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MiniKeyboardDevice {
    pub vid: u16,
    pub pid: u16,
    #[serde(default)]
    pub path: Option<String>,
}

impl Default for MiniKeyboardDevice {
    fn default() -> Self {
        Self {
            vid: DEFAULT_VID,
            pid: DEFAULT_PID,
            path: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MiniKeyboardConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub device: MiniKeyboardDevice,
    #[serde(default = "default_device_mode")]
    pub mode: DeviceMode,
    #[serde(default = "default_mappings")]
    pub mappings: MappingTable,
}

fn default_device_mode() -> DeviceMode {
    DeviceMode::Zoom
}

fn default_mappings() -> MappingTable {
    MappingTable::defaults()
}

impl Default for MiniKeyboardConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            device: MiniKeyboardDevice::default(),
            mode: DeviceMode::Zoom,
            mappings: MappingTable::defaults(),
        }
    }
}

impl MiniKeyboardConfig {
    #[allow(dead_code)]
    pub fn reset_mappings(&mut self) {
        self.mappings = MappingTable::defaults();
    }
}

fn default_switch_buses() -> Vec<usize> {
    vec![0, 2]
}

impl Default for Config {
    fn default() -> Self {
        Self {
            switch_buses: default_switch_buses(),
            active_bus: 0,
            volume_increment: 2.0,
            hotkeys: HotkeyConfig::default(),
            scenes: ScenesConfig::default(),
            mini_keyboard: MiniKeyboardConfig::default(),
            bus_a: None,
            bus_b: None,
        }
    }
}

impl Config {
    pub fn config_dir() -> Option<PathBuf> {
        ProjectDirs::from("com", "voicemeeter-controller", "voicemeeter-controller")
            .map(|dirs| dirs.config_dir().to_path_buf())
    }

    pub fn config_path() -> Option<PathBuf> {
        Self::config_dir().map(|dir| dir.join("config.json"))
    }

    pub fn load() -> io::Result<Self> {
        if let Some(path) = Self::config_path() {
            if path.exists() {
                let data = fs::read_to_string(&path)?;
                let mut config: Config = serde_json::from_str(&data)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
                config.migrate_legacy();
                config.migrate_scenes();
                config.clamp_values();
                return Ok(config);
            }
        }

        let mut config = Self::default();
        if let Some(active) = Self::read_legacy_vm_target() {
            config.active_bus = active;
        }
        config.migrate_scenes();
        config.clamp_values();
        config.save()?;
        Ok(config)
    }

    pub fn save(&self) -> io::Result<()> {
        let path = Self::config_path()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "config dir introuvable"))?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let data = serde_json::to_string_pretty(self)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        fs::write(path, data)
    }

    fn read_legacy_vm_target() -> Option<usize> {
        let exe_dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
        let legacy = exe_dir.join("vm_target.txt");
        if !legacy.exists() {
            return None;
        }
        let content = fs::read_to_string(legacy).ok()?;
        content.trim().parse().ok()
    }

    fn migrate_legacy(&mut self) {
        if self.switch_buses.is_empty() {
            match (self.bus_a, self.bus_b) {
                (Some(a), Some(b)) => self.switch_buses = vec![a, b],
                _ => self.switch_buses = default_switch_buses(),
            }
        }
        self.bus_a = None;
        self.bus_b = None;
    }

    /// Migre les mappings Zoom/Video legacy vers le système de scènes.
    fn migrate_scenes(&mut self) {
        let needs_migration = self.scenes.items.is_empty()
            || (self.scenes.items.len() == 1
                && self.scenes.items[0].id == DEFAULT_SCENE_ID
                && self.scenes.items[0].triggers.is_empty());

        if !needs_migration {
            self.scenes.ensure_valid();
            return;
        }

        let mut zoom_mappings = self.mini_keyboard.mappings.mode_to_scene_mappings(DeviceMode::Zoom);
        let mut video_mappings =
            self.mini_keyboard.mappings.mode_to_scene_mappings(DeviceMode::Video);
        migrate_toggle_mode_to_cycle(&mut zoom_mappings);
        migrate_toggle_mode_to_cycle(&mut video_mappings);

        self.scenes = ScenesConfig {
            active_id: match self.mini_keyboard.mode {
                DeviceMode::Video => VIDEO_SCENE_ID.to_string(),
                DeviceMode::Zoom => ZOOM_SCENE_ID.to_string(),
            },
            default_scene_id: ZOOM_SCENE_ID.to_string(),
            auto_switch: true,
            items: vec![
                Scene::with_id(ZOOM_SCENE_ID, "Zoom", zoom_mappings),
                Scene::with_id(VIDEO_SCENE_ID, "Video", video_mappings),
            ],
        };
        self.scenes.ensure_valid();
    }

    fn clamp_values(&mut self) {
        if self.volume_increment <= 0.0 {
            self.volume_increment = 2.0;
        }

        self.switch_buses.sort_unstable();
        self.switch_buses.dedup();
        self.switch_buses.retain(|&b| b < MENU_BUS_COUNT);

        if self.switch_buses.len() < 2 {
            self.switch_buses = default_switch_buses();
        }

        if !self.switch_buses.contains(&self.active_bus) {
            self.active_bus = self.switch_buses[0];
        }

        self.scenes.ensure_valid();
    }

    pub fn bus_label(index: usize) -> String {
        format!("A{}", index + 1)
    }

    pub fn is_switch_bus(&self, bus: usize) -> bool {
        self.switch_buses.contains(&bus)
    }

    pub fn set_active_bus(&mut self, bus: usize) {
        if self.switch_buses.contains(&bus) {
            self.active_bus = bus;
        }
    }

    /// Ajoute ou retire un bus de la liste switchable (minimum 2 bus requis).
    pub fn toggle_switch_bus(&mut self, bus: usize) -> bool {
        if bus >= MENU_BUS_COUNT {
            return false;
        }

        if let Some(pos) = self.switch_buses.iter().position(|&b| b == bus) {
            if self.switch_buses.len() <= 2 {
                return false;
            }
            self.switch_buses.remove(pos);
            if self.active_bus == bus {
                self.active_bus = self.switch_buses[0];
            }
        } else {
            self.switch_buses.push(bus);
            self.switch_buses.sort_unstable();
        }
        true
    }

    pub fn next_switch_bus(&self) -> usize {
        let idx = self
            .switch_buses
            .iter()
            .position(|&b| b == self.active_bus)
            .unwrap_or(0);
        let next = (idx + 1) % self.switch_buses.len();
        self.switch_buses[next]
    }
}

pub const INCREMENT_OPTIONS: [f32; 4] = [0.5, 1.0, 2.0, 5.0];

#[cfg(test)]
mod tests {
    use super::Config;
    use crate::mini_keyboard::mapping::{DeviceMode, MappingTable};
    use crate::scenes::{ScenesConfig, VIDEO_SCENE_ID};

    #[test]
    fn next_switch_bus_cycles() {
        let mut config = Config::default();
        config.switch_buses = vec![0, 2];
        config.active_bus = 0;
        assert_eq!(config.next_switch_bus(), 2);
        config.active_bus = 2;
        assert_eq!(config.next_switch_bus(), 0);
    }

    #[test]
    fn migrate_legacy_zoom_video_to_scenes() {
        let mut config = Config::default();
        config.scenes = ScenesConfig::default();
        config.mini_keyboard.mappings = MappingTable::defaults();
        config.mini_keyboard.mode = DeviceMode::Video;
        config.migrate_scenes();
        assert_eq!(config.scenes.items.len(), 2);
        assert_eq!(config.scenes.active_id, VIDEO_SCENE_ID);
    }
}
