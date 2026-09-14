use crate::config::{Config, INCREMENT_OPTIONS, MENU_BUS_COUNT};
use crate::vm::VmController;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tray_icon::menu::{CheckMenuItem, Menu, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};
use winit::event_loop::EventLoopProxy;

pub mod ids {
    use tray_icon::menu::MenuId;

    pub fn quit() -> MenuId {
        MenuId::new("quit")
    }
    pub fn reconnect() -> MenuId {
        MenuId::new("reconnect")
    }
    pub fn active_bus(bus: usize) -> MenuId {
        MenuId::new(&format!("active_bus_{bus}"))
    }
    pub fn switch_bus(bus: usize) -> MenuId {
        MenuId::new(&format!("switch_bus_{bus}"))
    }
    pub fn increment(value: f32) -> MenuId {
        MenuId::new(&format!("increment_{value}"))
    }
}

pub enum TrayCommand {
    Quit,
    Reconnect,
    SetActiveBus(usize),
    ToggleSwitchBus(usize),
    SetIncrement(f32),
}

pub struct TrayHandle {
    pub tray: TrayIcon,
}

pub fn build_tray(
    config: &Config,
    vm: &Arc<VmController>,
    proxy: EventLoopProxy<crate::UserEvent>,
) -> Result<TrayHandle, Box<dyn std::error::Error>> {
    let menu = build_menu(config);
    let icon = create_icon()?;
    let tooltip = vm.tooltip(config);

    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip(tooltip)
        .with_icon(icon)
        .build()?;

    let proxy_tray = proxy.clone();
    tray_icon::TrayIconEvent::set_event_handler(Some(move |event| {
        let _ = proxy_tray.send_event(crate::UserEvent::Tray(event));
    }));

    let proxy_menu = proxy;
    tray_icon::menu::MenuEvent::set_event_handler(Some(move |event| {
        let _ = proxy_menu.send_event(crate::UserEvent::Menu(event));
    }));

    Ok(TrayHandle { tray })
}

pub fn update_tooltip(tray: &TrayIcon, vm: &Arc<VmController>, config: &Config) {
    let _ = tray.set_tooltip(Some(vm.tooltip(config)));
}

pub fn refresh_menu(tray: &TrayIcon, config: &Config) {
    let menu = build_menu(config);
    let _ = tray.set_menu(Some(Box::new(menu)));
}

pub fn command_from_menu_id(id: &MenuId, _config: &Config) -> Option<TrayCommand> {
    if ids::quit() == id {
        return Some(TrayCommand::Quit);
    }
    if ids::reconnect() == id {
        return Some(TrayCommand::Reconnect);
    }

    for bus in 0..MENU_BUS_COUNT {
        if ids::active_bus(bus) == id {
            return Some(TrayCommand::SetActiveBus(bus));
        }
        if ids::switch_bus(bus) == id {
            return Some(TrayCommand::ToggleSwitchBus(bus));
        }
    }

    for &inc in INCREMENT_OPTIONS.iter() {
        if ids::increment(inc) == id {
            return Some(TrayCommand::SetIncrement(inc));
        }
    }
    None
}

fn build_menu(config: &Config) -> Menu {
    let menu = Menu::new();

    let status = MenuItem::with_id(
        MenuId::new("status"),
        format!("Volume sur : {}", Config::bus_label(config.active_bus)),
        false,
        None,
    );
    let _ = menu.append(&status);
    let _ = menu.append(&PredefinedMenuItem::separator());

    let switch_header = MenuItem::with_id(
        MenuId::new("switch_header"),
        "Canaux switchables",
        false,
        None,
    );
    let _ = menu.append(&switch_header);

    for bus in 0..MENU_BUS_COUNT {
        let item = CheckMenuItem::with_id(
            ids::switch_bus(bus),
            format!("Inclure {}", Config::bus_label(bus)),
            true,
            config.is_switch_bus(bus),
            None,
        );
        let _ = menu.append(&item);
    }

    let _ = menu.append(&PredefinedMenuItem::separator());

    let active_header = MenuItem::with_id(
        MenuId::new("active_header"),
        "Bus actif (volume)",
        false,
        None,
    );
    let _ = menu.append(&active_header);

    for &bus in &config.switch_buses {
        let item = CheckMenuItem::with_id(
            ids::active_bus(bus),
            Config::bus_label(bus),
            true,
            config.active_bus == bus,
            None,
        );
        let _ = menu.append(&item);
    }

    let _ = menu.append(&PredefinedMenuItem::separator());

    for &inc in INCREMENT_OPTIONS.iter() {
        let item = CheckMenuItem::with_id(
            ids::increment(inc),
            format!("Incrément {inc} dB"),
            true,
            (config.volume_increment - inc).abs() < f32::EPSILON,
            None,
        );
        let _ = menu.append(&item);
    }

    let _ = menu.append(&PredefinedMenuItem::separator());
    let reconnect = MenuItem::with_id(ids::reconnect(), "Reconnecter Voicemeeter", true, None);
    let quit = MenuItem::with_id(ids::quit(), "Quitter", true, None);
    let _ = menu.append(&reconnect);
    let _ = menu.append(&quit);

    menu
}

/// Noms cherchés dans le dossier de l'exe (dans cet ordre).
const ICON_FILENAMES: &[&str] = &["icon.ico", "app.ico"];

fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
}

fn find_custom_icon_path() -> Option<PathBuf> {
    let dir = exe_dir()?;
    ICON_FILENAMES
        .iter()
        .map(|name| dir.join(name))
        .find(|path| path.is_file())
}

fn load_ico(path: &Path) -> Result<Icon, Box<dyn std::error::Error>> {
    let file = std::fs::File::open(path)?;
    let dir = ico::IconDir::read(BufReader::new(file))?;
    let entry = dir
        .entries()
        .iter()
        .max_by_key(|e| e.width())
        .ok_or("fichier .ico vide")?;
    let image = entry.decode()?;
    Ok(Icon::from_rgba(
        image.rgba_data().to_vec(),
        image.width(),
        image.height(),
    )?)
}

fn create_default_icon() -> Result<Icon, Box<dyn std::error::Error>> {
    let size = 32u32;
    let mut rgba = vec![0u8; (size * size * 4) as usize];
    let cx = 16.0f32;
    let cy = 16.0f32;
    let radius = 12.0f32;

    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            let inside = (dx * dx + dy * dy) <= radius * radius;
            let i = ((y * size + x) * 4) as usize;
            if inside {
                rgba[i] = 70;
                rgba[i + 1] = 130;
                rgba[i + 2] = 220;
                rgba[i + 3] = 255;
            }
        }
    }

    Ok(Icon::from_rgba(rgba, size, size)?)
}

fn create_icon() -> Result<Icon, Box<dyn std::error::Error>> {
    if let Some(path) = find_custom_icon_path() {
        match load_ico(&path) {
            Ok(icon) => return Ok(icon),
            Err(e) => eprintln!(
                "icône personnalisée ignorée ({}): {e}",
                path.display()
            ),
        }
    }
    create_default_icon()
}
