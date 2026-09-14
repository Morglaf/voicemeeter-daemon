use crate::mini_keyboard::mapping::{MappedAction, PhysicalKey, SceneMappings};
use regex::Regex;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const ZOOM_SCENE_ID: &str = "scene-zoom";
pub const VIDEO_SCENE_ID: &str = "scene-video";
pub const DEFAULT_SCENE_ID: &str = "scene-default";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TitleMatcher {
    Contains(String),
    Regex(String),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SceneTrigger {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_title: Option<TitleMatcher>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scene {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub mappings: SceneMappings,
    #[serde(default)]
    pub triggers: Vec<SceneTrigger>,
    #[serde(default)]
    pub priority: i32,
}

impl Scene {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            mappings: default_scene_mappings(),
            triggers: Vec::new(),
            priority: 0,
        }
    }

    pub fn with_id(id: impl Into<String>, name: impl Into<String>, mappings: SceneMappings) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            mappings,
            triggers: Vec::new(),
            priority: 0,
        }
    }

    pub fn mapping_for(&self, key: PhysicalKey) -> MappedAction {
        self.mappings
            .get(&key)
            .cloned()
            .unwrap_or(MappedAction::None)
    }

    pub fn set_mapping(&mut self, key: PhysicalKey, action: MappedAction) {
        self.mappings.insert(key, action);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenesConfig {
    pub active_id: String,
    #[serde(default = "default_scene_id_field")]
    pub default_scene_id: String,
    #[serde(default = "default_auto_switch")]
    pub auto_switch: bool,
    #[serde(default)]
    pub items: Vec<Scene>,
}

fn default_scene_id_field() -> String {
    DEFAULT_SCENE_ID.to_string()
}

fn default_auto_switch() -> bool {
    true
}

impl Default for ScenesConfig {
    fn default() -> Self {
        Self {
            active_id: DEFAULT_SCENE_ID.to_string(),
            default_scene_id: DEFAULT_SCENE_ID.to_string(),
            auto_switch: true,
            items: vec![Scene::with_id(
                DEFAULT_SCENE_ID,
                "Défaut",
                default_scene_mappings(),
            )],
        }
    }
}

impl ScenesConfig {
    pub fn active_scene(&self) -> Option<&Scene> {
        self.items
            .iter()
            .find(|s| s.id == self.active_id)
            .or_else(|| self.items.first())
    }

    pub fn active_scene_name(&self) -> &str {
        self.active_scene()
            .map(|s| s.name.as_str())
            .unwrap_or("?")
    }

    pub fn scene_index(&self, id: &str) -> Option<usize> {
        self.items.iter().position(|s| s.id == id)
    }

    pub fn scene_by_id(&self, id: &str) -> Option<&Scene> {
        self.items.iter().find(|s| s.id == id)
    }

    #[allow(dead_code)]
    pub fn scene_by_id_mut(&mut self, id: &str) -> Option<&mut Scene> {
        self.items.iter_mut().find(|s| s.id == id)
    }

    pub fn activate(&mut self, id: &str) -> bool {
        if self.items.iter().any(|s| s.id == id) {
            self.active_id = id.to_string();
            true
        } else {
            false
        }
    }

    pub fn cycle_next(&mut self) -> Option<&Scene> {
        if self.items.is_empty() {
            return None;
        }
        let idx = self
            .scene_index(&self.active_id)
            .unwrap_or(0);
        let next = (idx + 1) % self.items.len();
        self.active_id = self.items[next].id.clone();
        self.items.get(next)
    }

    #[allow(dead_code)]
    pub fn active_mappings(&self) -> SceneMappings {
        self.active_scene()
            .map(|s| s.mappings.clone())
            .unwrap_or_default()
    }

    pub fn match_foreground(&self, process_exe: &str, window_title: &str) -> Option<String> {
        let process_lower = process_exe.to_ascii_lowercase();
        let title_lower = window_title.to_ascii_lowercase();

        let mut best: Option<(i32, &str)> = None;
        for scene in &self.items {
            if scene.triggers.is_empty() {
                continue;
            }
            for trigger in &scene.triggers {
                if !trigger_matches(trigger, &process_lower, &title_lower, window_title) {
                    continue;
                }
                match best {
                    Some((prio, _)) if scene.priority <= prio => {}
                    _ => best = Some((scene.priority, scene.id.as_str())),
                }
            }
        }
        best.map(|(_, id)| id.to_string())
    }

    pub fn ensure_valid(&mut self) {
        if self.items.is_empty() {
            self.items.push(Scene::with_id(
                DEFAULT_SCENE_ID,
                "Défaut",
                default_scene_mappings(),
            ));
        }
        if !self.items.iter().any(|s| s.id == self.default_scene_id) {
            self.default_scene_id = self.items[0].id.clone();
        }
        if !self.items.iter().any(|s| s.id == self.active_id) {
            self.active_id = self.default_scene_id.clone();
        }
    }
}

pub fn default_scene_mappings() -> SceneMappings {
    use crate::mini_keyboard::mapping::MappingTable;
    MappingTable::defaults().mode_to_scene_mappings(crate::mini_keyboard::mapping::DeviceMode::Zoom)
}

pub fn migrate_toggle_mode_to_cycle(mappings: &mut SceneMappings) {
    for action in mappings.values_mut() {
        if matches!(action, MappedAction::ToggleMode) {
            *action = MappedAction::CycleScene;
        }
    }
}

fn trigger_matches(
    trigger: &SceneTrigger,
    process_lower: &str,
    title_lower: &str,
    title_raw: &str,
) -> bool {
    let process_ok = trigger
        .process
        .as_ref()
        .map(|p| {
            let want = p.to_ascii_lowercase();
            process_lower.ends_with(&want) || process_lower == want
        })
        .unwrap_or(true);

    let title_ok = trigger.window_title.as_ref().map_or(true, |matcher| {
        match matcher {
            TitleMatcher::Contains(sub) => {
                title_lower.contains(&sub.to_ascii_lowercase())
            }
            TitleMatcher::Regex(pattern) => Regex::new(pattern)
                .map(|re| re.is_match(title_raw))
                .unwrap_or(false),
        }
    });

    let has_process = trigger.process.is_some();
    let has_title = trigger.window_title.is_some();
    if !has_process && !has_title {
        return false;
    }

    if has_process && has_title {
        process_ok && title_ok
    } else if has_process {
        process_ok
    } else {
        title_ok
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn match_process_name() {
        let mut scenes = ScenesConfig::default();
        scenes.items[0].triggers.push(SceneTrigger {
            process: Some("photoshop.exe".into()),
            window_title: None,
        });
        scenes.items[0].priority = 10;

        let matched = scenes.match_foreground("C:\\Adobe\\photoshop.exe", "Untitled");
        assert_eq!(matched, Some(scenes.items[0].id.clone()));
    }

    #[test]
    fn match_title_contains() {
        let mut scenes = ScenesConfig::default();
        scenes.items[0].triggers.push(SceneTrigger {
            process: None,
            window_title: Some(TitleMatcher::Contains("Adobe Photoshop".into())),
        });

        let matched = scenes.match_foreground("other.exe", "Adobe Photoshop 2024");
        assert_eq!(matched, Some(scenes.items[0].id.clone()));
    }

    #[test]
    fn cycle_next_wraps() {
        let mut scenes = ScenesConfig::default();
        scenes.items.push(Scene::new("B"));
        scenes.active_id = scenes.items[0].id.clone();
        scenes.cycle_next();
        assert_eq!(scenes.active_id, scenes.items[1].id);
        scenes.cycle_next();
        assert_eq!(scenes.active_id, scenes.items[0].id);
    }
}
