use crate::config::Config;
use crate::mini_keyboard::hid::HidDeviceInfo;
use crate::mini_keyboard::list_devices;
use std::sync::{Mutex, OnceLock};
use std::thread;
use tray_icon::menu::{CheckMenuItem, Menu, MenuId, MenuItem, Submenu};

static DEVICE_CACHE: OnceLock<Mutex<DeviceCache>> = OnceLock::new();

#[derive(Default)]
struct DeviceCache {
    vid: u16,
    pid: u16,
    devices: Vec<HidDeviceInfo>,
}

fn cache() -> &'static Mutex<DeviceCache> {
    DEVICE_CACHE.get_or_init(|| Mutex::new(DeviceCache::default()))
}

/// Liste en cache uniquement — jamais d'énum HID ici (bloque le thread UI plusieurs secondes).
pub fn cached_devices(vid: u16, pid: u16) -> Vec<HidDeviceInfo> {
    let guard = cache().lock().unwrap_or_else(|e| e.into_inner());
    if guard.vid == vid && guard.pid == pid {
        guard.devices.clone()
    } else {
        Vec::new()
    }
}

/// Énum HID en arrière-plan, puis callback (ex. refresh menu).
pub fn refresh_devices_async(vid: u16, pid: u16, on_done: impl FnOnce() + Send + 'static) {
    thread::spawn(move || {
        let devices = list_devices(vid, pid);
        {
            let mut guard = cache().lock().unwrap_or_else(|e| e.into_inner());
            guard.vid = vid;
            guard.pid = pid;
            guard.devices = devices;
        }
        on_done();
    });
}

pub mod ids {
    use super::{cached_devices, MiniKeyboardTrayCommand};
    use tray_icon::menu::MenuId;

    pub fn enable() -> MenuId {
        MenuId::new("mk_enable")
    }
    pub fn device_all() -> MenuId {
        MenuId::new("mk_dev:all")
    }
    pub fn device_index(index: usize) -> MenuId {
        MenuId::new(&format!("mk_dev:{index}"))
    }
    pub fn refresh_devices() -> MenuId {
        MenuId::new("mk_dev:refresh")
    }

    pub fn parse_device_selection(id: &str, vid: u16, pid: u16) -> Option<MiniKeyboardTrayCommand> {
        let rest = id.strip_prefix("mk_dev:")?;
        if rest == "all" {
            return Some(MiniKeyboardTrayCommand::SelectDevice {
                path: None,
                vid,
                pid,
            });
        }
        if rest == "refresh" {
            return Some(MiniKeyboardTrayCommand::RefreshDevices);
        }
        let index: usize = rest.parse().ok()?;
        let dev = cached_devices(vid, pid).into_iter().nth(index)?;
        Some(MiniKeyboardTrayCommand::SelectDevice {
            path: Some(dev.path),
            vid: dev.vid,
            pid: dev.pid,
        })
    }
}

#[derive(Debug, Clone)]
pub enum MiniKeyboardTrayCommand {
    ToggleEnabled,
    RefreshDevices,
    SelectDevice {
        path: Option<String>,
        vid: u16,
        pid: u16,
    },
}

pub fn parse_command(id: &MenuId, vid: u16, pid: u16) -> Option<MiniKeyboardTrayCommand> {
    let id_str = id.0.as_str();
    if ids::enable() == id {
        return Some(MiniKeyboardTrayCommand::ToggleEnabled);
    }
    if id_str.starts_with("mk_dev:") {
        return ids::parse_device_selection(id_str, vid, pid);
    }
    None
}

pub fn append_mini_keyboard_menu(menu: &Menu, config: &Config) {
    let mk = &config.mini_keyboard;
    let submenu = Submenu::with_id(MenuId::new("mk_root"), "Mini-clavier", true);

    let enable = CheckMenuItem::with_id(ids::enable(), "Activer", true, mk.enabled, None);
    let _ = submenu.append(&enable);

    let scene_item = MenuItem::with_id(
        MenuId::new("mk_scene_display"),
        format!("Scène active : {}", config.scenes.active_scene_name()),
        false,
        None,
    );
    let _ = submenu.append(&scene_item);

    let device_sub = Submenu::with_id(MenuId::new("mk_devices"), "Périphérique", true);
    let devices = cached_devices(mk.device.vid, mk.device.pid);

    let refresh = MenuItem::with_id(ids::refresh_devices(), "Actualiser la liste…", true, None);
    let _ = device_sub.append(&refresh);

    if devices.is_empty() {
        let none = MenuItem::with_id(
            MenuId::new("mk_no_dev"),
            if mk.device.path.is_some() {
                "Liste non chargée (Actualiser)".into()
            } else {
                format!(
                    "Aucun périphérique en cache {:04X}:{:04X}",
                    mk.device.vid, mk.device.pid
                )
            },
            false,
            None,
        );
        let _ = device_sub.append(&none);
    } else {
        let all = CheckMenuItem::with_id(
            ids::device_all(),
            "Toutes les interfaces (recommandé)",
            true,
            mk.device.path.is_none(),
            None,
        );
        let _ = device_sub.append(&all);

        for (index, dev) in devices.iter().enumerate() {
            let selected = mk.device.path.as_deref() == Some(dev.path.as_str());
            let item = CheckMenuItem::with_id(
                ids::device_index(index),
                dev.label(),
                true,
                selected,
                None,
            );
            let _ = device_sub.append(&item);
        }
    }
    let _ = submenu.append(&device_sub);

    let _ = menu.append(&submenu);
}
