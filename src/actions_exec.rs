use crate::mini_keyboard::inject;
use crate::mini_keyboard::mapping::MappedAction;
use crate::scenes::ScenesConfig;
use std::process::Command;

#[derive(Debug)]
pub enum ActionResult {
    Ok,
    SceneChanged {
        scene_id: String,
        scene_name: String,
    },
    Error(String),
}

pub fn execute(action: &MappedAction, scenes: &mut ScenesConfig) -> ActionResult {
    match action {
        MappedAction::None => ActionResult::Ok,
        MappedAction::ToggleMode | MappedAction::CycleScene => {
            if let Some(scene) = scenes.cycle_next() {
                ActionResult::SceneChanged {
                    scene_id: scene.id.clone(),
                    scene_name: scene.name.clone(),
                }
            } else {
                ActionResult::Error("Aucune scène disponible".into())
            }
        }
        MappedAction::SwitchScene { scene_id } => {
            if scenes.activate(scene_id) {
                let name = scenes
                    .active_scene()
                    .map(|s| s.name.clone())
                    .unwrap_or_else(|| scene_id.clone());
                ActionResult::SceneChanged {
                    scene_id: scene_id.clone(),
                    scene_name: name,
                }
            } else {
                ActionResult::Error(format!("Scène introuvable: {scene_id}"))
            }
        }
        MappedAction::LaunchProgram { path, args } => match launch_program(path, args) {
            Ok(()) => ActionResult::Ok,
            Err(e) => ActionResult::Error(e),
        },
        MappedAction::RunCommand { line, use_shell } => match run_command(line, *use_shell) {
            Ok(()) => ActionResult::Ok,
            Err(e) => ActionResult::Error(e),
        },
        other if other.is_injectable() => match inject::execute(other) {
            Ok(()) => ActionResult::Ok,
            Err(e) => ActionResult::Error(e.to_string()),
        },
        _ => ActionResult::Ok,
    }
}

fn launch_program(path: &str, args: &[String]) -> Result<(), String> {
    Command::new(path)
        .args(args)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Lancement échoué ({path}): {e}"))
}

fn run_command(line: &str, use_shell: bool) -> Result<(), String> {
    if use_shell {
        Command::new("cmd")
            .args(["/C", line])
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("Commande échouée: {e}"))
    } else {
        Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", line])
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("PowerShell échoué: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cycle_scene_changes_active() {
        let mut scenes = ScenesConfig::default();
        scenes.items.push(crate::scenes::Scene::new("B"));
        let first = scenes.active_id.clone();
        let result = execute(&MappedAction::CycleScene, &mut scenes);
        assert!(matches!(result, ActionResult::SceneChanged { .. }));
        assert_ne!(scenes.active_id, first);
    }
}
