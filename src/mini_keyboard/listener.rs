use super::hid::{self, parse_report, DeviceReader, HidError};
use super::hook;
use super::mapping::{MappedAction, PhysicalKey};
use super::notify;
use crate::actions_exec::{self, ActionResult};
use crate::scene_manager::SharedScenes;
use crate::config::MiniKeyboardConfig;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use winit::event_loop::EventLoopProxy;

#[derive(Debug, Clone)]
pub enum MiniKeyboardEvent {
    SceneChanged {
        scene_id: String,
        scene_name: String,
    },
    ActionError(String),
    DeviceConnected(String),
    DeviceDisconnected,
    Error(String),
}

pub struct MiniKeyboardListener {
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl MiniKeyboardListener {
    pub fn start(
        config: MiniKeyboardConfig,
        scenes: SharedScenes,
        proxy: EventLoopProxy<crate::UserEvent>,
    ) -> Result<Self, String> {
        let stop = Arc::new(AtomicBool::new(false));
        let stop_worker = stop.clone();
        let worker_config = config.clone();
        let worker_scenes = scenes;
        let worker_proxy = proxy.clone();

        // Pas d'énum HID sur le thread UI — ça bloque et, avec un hook LL, fige la frappe.
        let worker = thread::spawn(move || {
            let targets = hid::capture_targets(
                worker_config.device.vid,
                worker_config.device.pid,
                worker_config.device.path.as_deref(),
            );
            let device_label = if worker_config.device.path.is_none() && targets.len() > 1 {
                format!(
                    "{:04X}:{:04X} ({} interfaces)",
                    worker_config.device.vid,
                    worker_config.device.pid,
                    targets.len()
                )
            } else {
                targets
                    .first()
                    .map(|d| d.label())
                    .unwrap_or_else(|| {
                        format!(
                            "{:04X}:{:04X}",
                            worker_config.device.vid, worker_config.device.pid
                        )
                    })
            };
            let _ = worker_proxy.send_event(crate::UserEvent::MiniKeyboard(
                MiniKeyboardEvent::DeviceConnected(device_label),
            ));
            reader_loop(
                worker_config,
                worker_scenes,
                worker_proxy,
                stop_worker,
            );
        });

        Ok(Self {
            stop,
            worker: Some(worker),
        })
    }

    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.worker.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for MiniKeyboardListener {
    fn drop(&mut self) {
        self.stop();
    }
}

fn reader_loop(
    config: MiniKeyboardConfig,
    scenes: SharedScenes,
    proxy: EventLoopProxy<crate::UserEvent>,
    stop: Arc<AtomicBool>,
) {
    let mut debounce = InputDebounce::new();

    while !stop.load(Ordering::SeqCst) {
        let readers = match hid::open_readers(
            config.device.vid,
            config.device.pid,
            config.device.path.as_deref(),
        ) {
            Ok(r) => r,
            Err(HidError::NotFound) => {
                let _ = proxy.send_event(crate::UserEvent::MiniKeyboard(
                    MiniKeyboardEvent::DeviceDisconnected,
                ));
                thread::sleep(hid::RETRY_INTERVAL);
                continue;
            }
            Err(e) => {
                let _ = proxy.send_event(crate::UserEvent::MiniKeyboard(
                    MiniKeyboardEvent::Error(format!("HID: {e}")),
                ));
                thread::sleep(hid::RETRY_INTERVAL);
                continue;
            }
        };

        eprintln!(
            "mini-clavier: {} interface(s) en écoute{}",
            readers.len(),
            if config.device.path.is_none() {
                " (toutes)"
            } else {
                ""
            }
        );

        let (tx, rx) = mpsc::channel();
        let mut threads = Vec::new();
        for reader in readers {
            let stop_reader = stop.clone();
            let tx = tx.clone();
            threads.push(thread::spawn(move || {
                interface_loop(reader, stop_reader, tx);
            }));
        }
        drop(tx);

        while !stop.load(Ordering::SeqCst) {
            match rx.recv_timeout(Duration::from_millis(200)) {
                Ok(event) => {
                    hook::suppress_os_media(true);
                    if let Some(key) = event.key {
                        if matches!(key, PhysicalKey::Prev | PhysicalKey::Next | PhysicalKey::Play) {
                            hook::suppress_os_media(true);
                        }
                        if debounce.should_fire(key) {
                            handle_key(key, &scenes, &proxy);
                        }
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }

        for handle in threads {
            let _ = handle.join();
        }
    }
}

struct InterfaceEvent {
    key: Option<PhysicalKey>,
}

fn interface_loop(reader: DeviceReader, stop: Arc<AtomicBool>, tx: mpsc::Sender<InterfaceEvent>) {
    let label = reader.info.label();
    let mut buf = [0u8; 64];
    while !stop.load(Ordering::SeqCst) {
        match reader.read_report(&mut buf) {
            Ok(Some(n)) => {
                if report_may_trigger_os_media(&buf[..n]) {
                    hook::suppress_os_media(true);
                }
                eprintln!("HID [{}] report: {:02X?}", label, &buf[..n.min(8)]);
                let key = parse_report(&buf[..n], &reader.info);
                let _ = tx.send(InterfaceEvent { key });
            }
            Ok(None) => thread::sleep(Duration::from_millis(2)),
            Err(e) => {
                eprintln!("HID [{}] lecture: {e}", label);
                break;
            }
        }
    }
}

fn report_may_trigger_os_media(data: &[u8]) -> bool {
    data.iter().copied().any(|b| {
        matches!(
            b,
            0xB0 | 0xB3 | 0xB5 | 0xB6 | 0xCD | 0xE2 | 0xE9 | 0xEA
        )
    })
}

struct InputDebounce {
    last: HashMap<PhysicalKey, Instant>,
}

impl InputDebounce {
    fn new() -> Self {
        Self {
            last: HashMap::new(),
        }
    }

    fn should_fire(&mut self, key: PhysicalKey) -> bool {
        let now = Instant::now();
        let min_gap = if is_knob(key) {
            Duration::from_millis(35)
        } else {
            Duration::from_millis(180)
        };

        if let Some(last) = self.last.get(&key) {
            if now.duration_since(*last) < min_gap {
                return false;
            }
        }

        if is_knob(key) {
            if let Some((_other, last)) =
                opposite_knob(key).and_then(|o| self.last.get(&o).map(|t| (o, *t)))
            {
                if now.duration_since(last) < Duration::from_millis(80) {
                    return false;
                }
            }
        }

        self.last.insert(key, now);
        true
    }
}

fn is_knob(key: PhysicalKey) -> bool {
    matches!(key, PhysicalKey::KnobLeft | PhysicalKey::KnobRight)
}

fn opposite_knob(key: PhysicalKey) -> Option<PhysicalKey> {
    match key {
        PhysicalKey::KnobLeft => Some(PhysicalKey::KnobRight),
        PhysicalKey::KnobRight => Some(PhysicalKey::KnobLeft),
        _ => None,
    }
}

fn handle_key(
    key: PhysicalKey,
    scenes: &SharedScenes,
    proxy: &EventLoopProxy<crate::UserEvent>,
) {
    let action = {
        let guard = scenes.read().expect("scenes lock");
        guard
            .active_scene()
            .map(|s| s.mapping_for(key))
            .unwrap_or(MappedAction::None)
    };

    if matches!(action, MappedAction::None) {
        return;
    }

    let result = {
        let mut guard = scenes.write().expect("scenes lock");
        actions_exec::execute(&action, &mut guard)
    };

    match result {
        ActionResult::Ok => {}
        ActionResult::SceneChanged {
            scene_id,
            scene_name,
        } => {
            notify::show_scene_toast(&scene_name);
            let _ = proxy.send_event(crate::UserEvent::MiniKeyboard(
                MiniKeyboardEvent::SceneChanged {
                    scene_id,
                    scene_name,
                },
            ));
        }
        ActionResult::Error(msg) => {
            notify::show_error_toast(&msg);
            let _ = proxy.send_event(crate::UserEvent::MiniKeyboard(
                MiniKeyboardEvent::ActionError(msg),
            ));
        }
    }
}

pub fn list_devices(vid: u16, pid: u16) -> Vec<hid::HidDeviceInfo> {
    hid::list_matching_devices(vid, pid)
}
