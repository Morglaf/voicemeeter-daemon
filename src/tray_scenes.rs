use crate::config::Config;
use tray_icon::menu::{CheckMenuItem, Menu, MenuId, MenuItem, Submenu};

pub mod ids {
    use tray_icon::menu::MenuId;

    pub fn scene(id: &str) -> MenuId {
        MenuId::new(&format!("scene:{id}"))
    }

    pub fn open_config() -> MenuId {
        MenuId::new("open_config")
    }

    pub fn toggle_auto_switch() -> MenuId {
        MenuId::new("scene_auto_switch")
    }

    pub fn parse_scene(id: &str) -> Option<String> {
        id.strip_prefix("scene:").map(str::to_string)
    }
}

#[derive(Debug, Clone)]
pub enum SceneTrayCommand {
    ActivateScene(String),
    OpenConfig,
    ToggleAutoSwitch,
}

pub fn parse_command(id: &MenuId) -> Option<SceneTrayCommand> {
    let id_str = id.0.as_str();
    if ids::open_config() == id {
        return Some(SceneTrayCommand::OpenConfig);
    }
    if ids::toggle_auto_switch() == id {
        return Some(SceneTrayCommand::ToggleAutoSwitch);
    }
    if let Some(scene_id) = ids::parse_scene(id_str) {
        return Some(SceneTrayCommand::ActivateScene(scene_id));
    }
    None
}

pub fn append_scenes_menu(menu: &Menu, config: &Config) {
    let submenu = Submenu::with_id(MenuId::new("scenes_root"), "Scènes", true);

    let active_name = config.scenes.active_scene_name();
    let status = MenuItem::with_id(
        MenuId::new("scene_status"),
        format!("Active : {active_name}"),
        false,
        None,
    );
    let _ = submenu.append(&status);

    for scene in &config.scenes.items {
        let item = CheckMenuItem::with_id(
            ids::scene(&scene.id),
            &scene.name,
            true,
            config.scenes.active_id == scene.id,
            None,
        );
        let _ = submenu.append(&item);
    }

    let auto = CheckMenuItem::with_id(
        ids::toggle_auto_switch(),
        "Bascule auto par application",
        true,
        config.scenes.auto_switch,
        None,
    );
    let _ = submenu.append(&auto);

    let config_item = MenuItem::with_id(ids::open_config(), "Configurer…", true, None);
    let _ = submenu.append(&config_item);

    let _ = menu.append(&submenu);
}
