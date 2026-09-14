#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod actions;
mod config;
mod hotkeys;
mod tray;
mod vm;

use config::Config;
use global_hotkey::GlobalHotKeyEvent;
use hotkeys::{HotkeyAction, HotkeyRegistry};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tray::{command_from_menu_id, refresh_menu, update_tooltip, TrayCommand, TrayHandle};
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
}

const SWITCH_DEBOUNCE: Duration = Duration::from_millis(300);

struct App {
    config: Config,
    vm: Arc<VmController>,
    hotkeys: HotkeyRegistry,
    tray: Option<TrayHandle>,
    _window: Option<Window>,
    event_proxy: EventLoopProxy<UserEvent>,
    last_switch: Option<Instant>,
}

impl ApplicationHandler<UserEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.tray.is_some() {
            return;
        }

        let window_attrs = Window::default_attributes()
            .with_visible(false)
            .with_title("voicemeeter-controller");
        let window = event_loop.create_window(window_attrs).expect("fenêtre winit");

        let proxy = self.event_proxy.clone();
        GlobalHotKeyEvent::set_event_handler(Some(move |event| {
            let _ = proxy.send_event(UserEvent::Hotkey(event));
        }));

        let tray = tray::build_tray(&self.config, &self.vm, self.event_proxy.clone())
            .expect("icône tray");
        self.tray = Some(tray);
        self._window = Some(window);
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

                    let result = match action {
                        HotkeyAction::Switch => actions::switch_next(&self.vm, &mut self.config),
                        HotkeyAction::VolumeUp => {
                            actions::adjust_volume(&self.vm, &self.config, 1.0)
                        }
                        HotkeyAction::VolumeDown => {
                            actions::adjust_volume(&self.vm, &self.config, -1.0)
                        }
                    };
                    if let Err(e) = result {
                        eprintln!("action hotkey: {e}");
                    }
                    if let Some(tray) = &self.tray {
                        refresh_tray(tray, &self.vm, &self.config);
                    }
                }
            }
            UserEvent::Menu(menu_event) => {
                if let Some(cmd) = command_from_menu_id(&menu_event.id, &self.config) {
                    match cmd {
                        TrayCommand::Quit => {
                            let _ = self.config.save();
                            event_loop.exit();
                        }
                        TrayCommand::Reconnect => {
                            if let Err(e) = self.vm.reconnect() {
                                eprintln!("reconnexion: {e}");
                            } else if let Err(e) = actions::apply_active_bus(&self.vm, &self.config) {
                                eprintln!("sync bus: {e}");
                            }
                            if let Some(tray) = &self.tray {
                                refresh_tray(tray, &self.vm, &self.config);
                            }
                        }
                        TrayCommand::SetActiveBus(bus) => {
                            if let Err(e) =
                                actions::set_active_bus(&self.vm, &mut self.config, bus)
                            {
                                eprintln!("bus actif: {e}");
                            }
                            if let Some(tray) = &self.tray {
                                refresh_tray(tray, &self.vm, &self.config);
                            }
                        }
                        TrayCommand::ToggleSwitchBus(bus) => {
                            match actions::toggle_switch_bus(&self.vm, &mut self.config, bus) {
                                Ok(false) => eprintln!(
                                    "impossible de retirer {} : minimum 2 canaux requis",
                                    Config::bus_label(bus)
                                ),
                                Err(e) => eprintln!("toggle bus: {e}"),
                                Ok(true) => {}
                            }
                            if let Some(tray) = &self.tray {
                                refresh_tray(tray, &self.vm, &self.config);
                            }
                        }
                        TrayCommand::SetIncrement(inc) => {
                            self.config.volume_increment = inc;
                            let _ = self.config.save();
                            if let Some(tray) = &self.tray {
                                refresh_tray(tray, &self.vm, &self.config);
                            }
                        }
                    }
                }
            }
            UserEvent::Tray(_) => {}
        }
    }

    fn window_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        _event: WindowEvent,
    ) {
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {}
}

fn refresh_tray(tray: &TrayHandle, vm: &Arc<VmController>, config: &Config) {
    update_tooltip(&tray.tray, vm, config);
    refresh_menu(&tray.tray, config);
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::load()?;
    let vm = VmController::new()?;
    actions::apply_active_bus(&vm, &config)?;
    let hotkeys = HotkeyRegistry::register(&config)?;

    let event_loop = EventLoop::<UserEvent>::with_user_event().build()?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let event_proxy = event_loop.create_proxy();

    let mut app = App {
        config,
        vm,
        hotkeys,
        tray: None,
        _window: None,
        event_proxy,
        last_switch: None,
    };

    event_loop.run_app(&mut app)?;

    Ok(())
}
