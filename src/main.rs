#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod actions;
mod actions_exec;
mod app_watcher;
mod config;
mod gui;
mod hotkeys;
mod mini_keyboard;
mod scene_manager;
mod scenes;
mod tray;
mod tray_mini_keyboard;
mod tray_scenes;
mod vm;

use app_watcher::{AppWatcher, ForegroundInfo};
use config::Config;
use global_hotkey::GlobalHotKeyEvent;
use gui::{create_config_window, ConfigGui};
use hotkeys::{HotkeyAction, HotkeyRegistry};
use mini_keyboard::mapping::MappedAction;
use mini_keyboard::notify::{show_error_toast, show_info_toast, show_scene_toast};
use mini_keyboard::{inject, MiniKeyboardEvent, MiniKeyboardListener};
use scene_manager::SceneManager;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use tray::{
    command_from_menu_id, refresh_menu, refresh_tooltip_only, TrayCommand, TrayHandle,
};
use tray_mini_keyboard::MiniKeyboardTrayCommand;
use tray_scenes::SceneTrayCommand;
use vm::VmController;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::window::{Window, WindowId};

#[derive(Debug)]
pub enum UserEvent {
    Tray(tray_icon::TrayIconEvent),
    Menu(tray_icon::menu::MenuEvent),
    Hotkey(GlobalHotKeyEvent),
    MiniKeyboard(MiniKeyboardEvent),
    ForegroundApp(ForegroundInfo),
    /// Connexion VM OK (travail déjà fait hors thread UI).
    VmReady,
    VmError(String),
    /// Config mutée par un worker VM (hotkey / menu).
    ConfigSynced {
        config: Config,
        rebuild_menu: bool,
    },
    /// Tooltip cache rafraîchi, pas de changement de config.
    VmUiRefresh,
    /// Liste HID mise à jour — reconstruire le menu tray.
    TrayMenuRefresh,
    /// Injection clavier/souris depuis le thread UI (évite les soucis avec SendInput hors UI).
    MiniKeyboardInject(MappedAction),
}

const SWITCH_DEBOUNCE: Duration = Duration::from_millis(300);

struct App {
    config: Config,
    vm: Arc<VmController>,
    hotkeys: HotkeyRegistry,
    tray: Option<TrayHandle>,
    window: Option<Arc<Window>>,
    gui: Option<ConfigGui>,
    event_proxy: EventLoopProxy<UserEvent>,
    last_switch: Option<Instant>,
    scene_manager: SceneManager,
    mini_keyboard: Option<MiniKeyboardListener>,
    app_watcher: Option<AppWatcher>,
}

impl App {
    fn restart_mini_keyboard(&mut self) {
        if let Some(mut listener) = self.mini_keyboard.take() {
            listener.stop();
        }
        // Pas de WH_KEYBOARD_LL pour le mini-clavier : la capture HID exclusive suffit,
        // et le hook mangait les SendInput (touches injectées → aucun effet).
        if !self.config.mini_keyboard.enabled {
            return;
        }

        match MiniKeyboardListener::start(
            self.config.mini_keyboard.clone(),
            self.scene_manager.shared(),
            self.event_proxy.clone(),
        ) {
            Ok(listener) => self.mini_keyboard = Some(listener),
            Err(e) => eprintln!("mini-clavier: {e}"),
        }
    }

    fn restart_app_watcher(&mut self) {
        if let Some(mut watcher) = self.app_watcher.take() {
            watcher.stop();
        }
        let proxy = self.event_proxy.clone();
        self.app_watcher = Some(AppWatcher::start(
            self.config.scenes.auto_switch,
            move |info| {
                let _ = proxy.send_event(UserEvent::ForegroundApp(info));
            },
        ));
    }

    fn sync_scenes_to_manager(&mut self) {
        self.scene_manager.sync_from_config(&self.config.scenes);
    }

    fn activate_scene(&mut self, scene_id: &str, show_toast: bool) {
        if !self.config.scenes.activate(scene_id) {
            return;
        }
        self.sync_scenes_to_manager();
        let _ = self.config.save();
        if show_toast {
            show_scene_toast(self.config.scenes.active_scene_name());
        }
        self.refresh_tray_menu();
    }

    fn apply_auto_scene(&mut self, info: &ForegroundInfo) {
        if !self.config.scenes.auto_switch {
            return;
        }
        let matched = self
            .config
            .scenes
            .match_foreground(&info.process_exe, &info.window_title);
        let target = matched.unwrap_or_else(|| self.config.scenes.default_scene_id.clone());
        if target != self.config.scenes.active_id {
            self.activate_scene(&target, true);
        }
    }

    fn refresh_tooltip(&self) {
        if let Some(tray) = &self.tray {
            refresh_tooltip_only(&tray.tray, &self.vm, &self.config);
        }
    }

    fn refresh_tray_menu(&self) {
        if let Some(tray) = &self.tray {
            refresh_tooltip_only(&tray.tray, &self.vm, &self.config);
            refresh_menu(&tray.tray, &self.config);
        }
    }

    fn save_config_from_gui(&mut self) {
        self.sync_scenes_to_manager();
        let _ = self.config.save();
        self.restart_app_watcher();
        self.restart_mini_keyboard();
        self.refresh_tray_menu();
    }

    fn ensure_gui(&mut self) -> bool {
        if self.gui.is_some() {
            return true;
        }
        let Some(window) = self.window.as_ref() else {
            return false;
        };
        match ConfigGui::new(window, &self.config) {
            Ok(gui) => {
                self.gui = Some(gui);
                true
            }
            Err(e) => {
                show_error_toast(&format!("Fenêtre de config : {e}"));
                false
            }
        }
    }

    fn on_vm_ready(&mut self) {
        show_info_toast("Voicemeeter connecté");
        self.refresh_tooltip();
    }

    fn spawn_device_list_refresh(&self) {
        let vid = self.config.mini_keyboard.device.vid;
        let pid = self.config.mini_keyboard.device.pid;
        let proxy = self.event_proxy.clone();
        tray_mini_keyboard::refresh_devices_async(vid, pid, move || {
            let _ = proxy.send_event(UserEvent::TrayMenuRefresh);
        });
    }

    fn spawn_vm_hotkey(&self, action: HotkeyAction) {
        let vm = self.vm.clone();
        let mut config = self.config.clone();
        let proxy = self.event_proxy.clone();
        thread::spawn(move || {
            let result = match action {
                HotkeyAction::Switch => actions::switch_next(&vm, &mut config),
                HotkeyAction::VolumeUp => actions::adjust_volume(&vm, &config, 1.0),
                HotkeyAction::VolumeDown => actions::adjust_volume(&vm, &config, -1.0),
            };
            match result {
                Ok(()) => {
                    vm.refresh_tooltip_cache(&config);
                    if action == HotkeyAction::Switch {
                        let _ = proxy.send_event(UserEvent::ConfigSynced {
                            config,
                            rebuild_menu: true,
                        });
                    } else {
                        let _ = proxy.send_event(UserEvent::VmUiRefresh);
                    }
                }
                Err(e) => {
                    let _ = proxy.send_event(UserEvent::VmError(format!("action hotkey: {e}")));
                }
            }
        });
    }

    fn spawn_vm_menu_bus(&self, kind: BusMenuOp) {
        let vm = self.vm.clone();
        let mut config = self.config.clone();
        let proxy = self.event_proxy.clone();
        thread::spawn(move || {
            let result = match kind {
                BusMenuOp::Reconnect => vm
                    .reconnect()
                    .and_then(|_| actions::apply_active_bus(&vm, &config))
                    .map(|_| true),
                BusMenuOp::SetActive(bus) => {
                    actions::set_active_bus(&vm, &mut config, bus).map(|_| true)
                }
                BusMenuOp::ToggleSwitch(bus) => actions::toggle_switch_bus(&vm, &mut config, bus),
            };
            match result {
                Ok(true) => {
                    vm.refresh_tooltip_cache(&config);
                    if matches!(kind, BusMenuOp::Reconnect) {
                        show_info_toast("Voicemeeter reconnecté");
                    }
                    let _ = proxy.send_event(UserEvent::ConfigSynced {
                        config,
                        rebuild_menu: true,
                    });
                }
                Ok(false) => {
                    let _ = proxy.send_event(UserEvent::VmError(
                        "impossible de retirer le bus : minimum 2 canaux requis".into(),
                    ));
                }
                Err(e) => {
                    let _ = proxy.send_event(UserEvent::VmError(format!("{e}")));
                }
            }
        });
    }
}

#[derive(Clone, Copy)]
enum BusMenuOp {
    Reconnect,
    SetActive(usize),
    ToggleSwitch(usize),
}

impl ApplicationHandler<UserEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.tray.is_some() {
            return;
        }

        let window = create_config_window(event_loop);

        let proxy = self.event_proxy.clone();
        GlobalHotKeyEvent::set_event_handler(Some(move |event| {
            let _ = proxy.send_event(UserEvent::Hotkey(event));
        }));

        // Tooltip depuis cache uniquement — aucun appel DLL ici.
        let tray = tray::build_tray(&self.config, &self.vm, self.event_proxy.clone())
            .expect("icône tray");

        self.tray = Some(tray);
        self.window = Some(window);
        // Pas de hook clavier au boot : WH_KEYBOARD_LL sur le thread UI + charge VM = frappe lag.
        self.sync_scenes_to_manager();
        self.restart_mini_keyboard();
        self.restart_app_watcher();
        self.spawn_device_list_refresh();
        spawn_vm_connect_retry(self.vm.clone(), self.config.clone(), self.event_proxy.clone());
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::Hotkey(hotkey_event) => {
                if let Some(action) = self.hotkeys.action_for_event(&hotkey_event) {
                    if action == HotkeyAction::Switch {
                        let now = Instant::now();
                        if let Some(last) = self.last_switch {
                            if now.duration_since(last) < SWITCH_DEBOUNCE {
                                return;
                            }
                        }
                        self.last_switch = Some(now);
                    }
                    self.spawn_vm_hotkey(action);
                }
            }
            UserEvent::Menu(menu_event) => {
                if let Some(cmd) = command_from_menu_id(&menu_event.id, &self.config) {
                    match cmd {
                        TrayCommand::Quit => {
                            let _ = self.config.save();
                            if let Some(mut listener) = self.mini_keyboard.take() {
                                listener.stop();
                            }
                            if let Some(mut watcher) = self.app_watcher.take() {
                                watcher.stop();
                            }
                            self.vm.shutdown();
                            event_loop.exit();
                        }
                        TrayCommand::Reconnect => {
                            self.spawn_vm_menu_bus(BusMenuOp::Reconnect);
                        }
                        TrayCommand::SetActiveBus(bus) => {
                            self.spawn_vm_menu_bus(BusMenuOp::SetActive(bus));
                        }
                        TrayCommand::ToggleSwitchBus(bus) => {
                            self.spawn_vm_menu_bus(BusMenuOp::ToggleSwitch(bus));
                        }
                        TrayCommand::SetIncrement(inc) => {
                            self.config.volume_increment = inc;
                            let _ = self.config.save();
                            self.refresh_tray_menu();
                        }
                        TrayCommand::MiniKeyboard(mk_cmd) => {
                            handle_mini_keyboard_tray(self, mk_cmd);
                        }
                        TrayCommand::Scene(scene_cmd) => {
                            handle_scene_tray(self, scene_cmd);
                        }
                    }
                }
            }
            UserEvent::MiniKeyboard(mk_event) => match mk_event {
                MiniKeyboardEvent::SceneChanged {
                    scene_id,
                    scene_name: _,
                } => {
                    self.config.scenes.active_id = scene_id;
                    let _ = self.config.save();
                    self.refresh_tray_menu();
                }
                MiniKeyboardEvent::ActionError(msg) => {
                    eprintln!("mini-clavier action: {msg}");
                }
                MiniKeyboardEvent::DeviceConnected(label) => {
                    eprintln!("mini-clavier connecté: {label}");
                    self.refresh_tooltip();
                }
                MiniKeyboardEvent::DeviceDisconnected => {
                    eprintln!("mini-clavier déconnecté, retry...");
                }
                MiniKeyboardEvent::Error(msg) => {
                    eprintln!("mini-clavier: {msg}");
                }
            },
            UserEvent::ForegroundApp(info) => {
                self.apply_auto_scene(&info);
            }
            UserEvent::VmReady => {
                self.on_vm_ready();
            }
            UserEvent::VmError(msg) => {
                eprintln!("{msg}");
                show_error_toast(&msg);
            }
            UserEvent::ConfigSynced {
                config,
                rebuild_menu,
            } => {
                self.config = config;
                self.sync_scenes_to_manager();
                if rebuild_menu {
                    self.refresh_tray_menu();
                } else {
                    self.refresh_tooltip();
                }
            }
            UserEvent::VmUiRefresh => {
                self.refresh_tooltip();
            }
            UserEvent::TrayMenuRefresh => {
                self.refresh_tray_menu();
            }
            UserEvent::MiniKeyboardInject(action) => {
                if let Err(e) = inject::execute(&action) {
                    show_error_toast(&format!("Injection: {e}"));
                }
            }
            UserEvent::Tray(_) => {}
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(window) = self.window.as_ref() else {
            return;
        };
        if window.id() != window_id {
            return;
        }

        if let Some(gui) = self.gui.as_mut() {
            if let WindowEvent::KeyboardInput { event, .. } = &event {
                if gui.on_winit_key(event) {
                    return;
                }
            }
            let response = gui.handle_window_event(window, &event);
            if response.consumed {
                return;
            }
        }

        match event {
            WindowEvent::CloseRequested => {
                if let Some(gui) = self.gui.as_mut() {
                    gui.hide(window);
                }
            }
            WindowEvent::RedrawRequested => {
                if let Some(gui) = self.gui.as_mut() {
                    gui.render(window, &mut self.config);
                    if gui.take_pending_save() {
                        self.save_config_from_gui();
                    }
                }
            }
            WindowEvent::Resized(size) => {
                if let Some(gui) = self.gui.as_mut() {
                    gui.on_resize(size.width, size.height);
                }
            }
            _ => {}
        }

        let _ = event_loop;
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {}
}

fn handle_mini_keyboard_tray(app: &mut App, cmd: MiniKeyboardTrayCommand) {
    match cmd {
        MiniKeyboardTrayCommand::ToggleEnabled => {
            app.config.mini_keyboard.enabled = !app.config.mini_keyboard.enabled;
            let _ = app.config.save();
            app.restart_mini_keyboard();
            app.refresh_tray_menu();
            if app.config.mini_keyboard.enabled {
                app.spawn_device_list_refresh();
            }
        }
        MiniKeyboardTrayCommand::RefreshDevices => {
            show_info_toast("Recherche des périphériques…");
            app.spawn_device_list_refresh();
        }
        MiniKeyboardTrayCommand::SelectDevice { path, vid, pid } => {
            app.config.mini_keyboard.device.path = path;
            app.config.mini_keyboard.device.vid = vid;
            app.config.mini_keyboard.device.pid = pid;
            let _ = app.config.save();
            app.restart_mini_keyboard();
            app.refresh_tray_menu();
        }
    }
}

fn handle_scene_tray(app: &mut App, cmd: SceneTrayCommand) {
    match cmd {
        SceneTrayCommand::ActivateScene(id) => {
            app.activate_scene(&id, true);
        }
        SceneTrayCommand::OpenConfig => {
            if !app.ensure_gui() {
                return;
            }
            if let (Some(window), Some(gui)) = (&app.window, &mut app.gui) {
                gui.show(window);
            }
        }
        SceneTrayCommand::ToggleAutoSwitch => {
            app.config.scenes.auto_switch = !app.config.scenes.auto_switch;
            let _ = app.config.save();
            app.restart_app_watcher();
            app.refresh_tray_menu();
        }
    }
}

fn try_sync_vm(vm: &Arc<VmController>, config: &Config) -> Result<(), vm::VmError> {
    vm.try_connect()?;
    actions::apply_active_bus(vm, config)
}

/// Toute la connexion VM est hors thread UI (évite de bloquer le hook / la pompe winit).
fn spawn_vm_connect_retry(
    vm: Arc<VmController>,
    config: Config,
    proxy: EventLoopProxy<UserEvent>,
) {
    thread::spawn(move || {
        let mut delay = Duration::ZERO;
        let max_delay = Duration::from_secs(5);
        let mut waiting_toast = false;
        loop {
            if !delay.is_zero() {
                thread::sleep(delay);
            }
            if try_sync_vm(&vm, &config).is_ok() {
                vm.refresh_tooltip_cache(&config);
                let _ = proxy.send_event(UserEvent::VmReady);
                break;
            }
            if !waiting_toast {
                show_info_toast("En attente de Voicemeeter…");
                waiting_toast = true;
            }
            delay = if delay.is_zero() {
                Duration::from_millis(500)
            } else {
                (delay * 2).min(max_delay)
            };
        }
    });
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::load()?;
    let vm = VmController::new();
    let hotkeys = HotkeyRegistry::register(&config)?;
    let scene_manager = SceneManager::new(&config.scenes);

    let event_loop = EventLoop::<UserEvent>::with_user_event().build()?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let event_proxy = event_loop.create_proxy();

    let mut app = App {
        config,
        vm,
        hotkeys,
        tray: None,
        window: None,
        gui: None,
        event_proxy,
        last_switch: None,
        scene_manager,
        mini_keyboard: None,
        app_watcher: None,
    };

    event_loop.run_app(&mut app)?;

    Ok(())
}
