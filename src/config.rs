use directories::ProjectDirs;
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
    /// Champs legacy — migrés vers switch_buses au chargement.
    #[serde(default, skip_serializing)]
    bus_a: Option<usize>,
    #[serde(default, skip_serializing)]
    bus_b: Option<usize>,
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
                config.clamp_values();
                return Ok(config);
            }
        }

        let mut config = Self::default();
        if let Some(active) = Self::read_legacy_vm_target() {
            config.active_bus = active;
        }
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
    fn next_switch_bus_three_channels() {
        let mut config = Config::default();
        config.switch_buses = vec![0, 1, 2];
        config.active_bus = 0;
        assert_eq!(config.next_switch_bus(), 1);
        config.active_bus = 1;
        assert_eq!(config.next_switch_bus(), 2);
        config.active_bus = 2;
        assert_eq!(config.next_switch_bus(), 0);
    }
}
