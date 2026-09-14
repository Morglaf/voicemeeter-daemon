use crate::config::Config;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use thiserror::Error;
use voicemeeter::interface::InitializationError;
use voicemeeter::{get_voicemeeter_raw, VoicemeeterRemote};

const GAIN_MIN: f32 = -60.0;
const GAIN_MAX: f32 = 12.0;

#[derive(Error, Debug)]
pub enum VmError {
    #[error("Voicemeeter non disponible: {0}")]
    Init(#[from] InitializationError),
    #[error("paramètre bus {bus}: {message}")]
    Bus { bus: usize, message: String },
    #[error("Voicemeeter non connecté")]
    NotConnected,
}

pub struct VmController {
    /// Une fois obtenu, ce handle ne doit plus être droppé avant la fin du process :
    /// le crate `voicemeeter` refuse tout nouveau login après logout.
    remote: Mutex<Option<VoicemeeterRemote>>,
    /// Cache pour le thread UI — ne jamais appeler l'API VM juste pour le tooltip.
    connected: AtomicBool,
    tooltip_cache: Mutex<String>,
}

impl VmController {
    /// Démarre sans toucher à l'API Voicemeeter.
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            remote: Mutex::new(None),
            connected: AtomicBool::new(false),
            tooltip_cache: Mutex::new("VM déconnecté".into()),
        })
    }

    pub fn is_logged_in(&self) -> bool {
        self.remote
            .lock()
            .ok()
            .map(|guard| guard.is_some())
            .unwrap_or(false)
    }

    /// Lecture cache — safe depuis le thread UI (pas d'appel DLL).
    #[allow(dead_code)]
    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }

    /// Login si besoin, puis vérifie que le serveur répond. À appeler hors thread UI.
    pub fn try_connect(&self) -> Result<(), VmError> {
        self.ensure_logged_in()?;
        self.ping_server()
    }

    pub fn reconnect(self: &Arc<Self>) -> Result<(), VmError> {
        let clear_stale = !self.is_logged_in();
        let remote = Self::create_remote(clear_stale)?;
        {
            let mut guard = self
                .remote
                .lock()
                .map_err(|_| VmError::NotConnected)?;
            *guard = Some(remote);
        }
        self.ping_server()
    }

    pub fn shutdown(&self) {
        if let Ok(mut guard) = self.remote.lock() {
            *guard = None;
        }
        self.connected.store(false, Ordering::Relaxed);
        if let Ok(mut tip) = self.tooltip_cache.lock() {
            *tip = "VM déconnecté".into();
        }
    }

    /// Tooltip depuis le cache — aucun appel Voicemeeter.
    pub fn tooltip(&self, config: &Config) -> String {
        let base = self
            .tooltip_cache
            .lock()
            .ok()
            .map(|g| g.clone())
            .unwrap_or_else(|| "VM déconnecté".into());
        format!("{base} | Scène {}", config.scenes.active_scene_name())
    }

    /// Met à jour le cache tooltip (thread worker / après action VM).
    pub fn refresh_tooltip_cache(&self, config: &Config) {
        let label = if self.ping_server().is_ok() {
            let bus = config.active_bus;
            let gain = self
                .get_gain(bus)
                .map(|g| format!("{:+.1} dB", g))
                .unwrap_or_else(|_| "?".into());
            let ver = self.version_label_uncached();
            format!("{} | Bus {} | {}", ver, Config::bus_label(bus), gain)
        } else {
            "VM déconnecté".into()
        };
        if let Ok(mut tip) = self.tooltip_cache.lock() {
            *tip = label;
        }
    }

    fn ensure_logged_in(&self) -> Result<(), VmError> {
        if self.is_logged_in() {
            return Ok(());
        }
        let remote = Self::create_remote(true)?;
        let mut guard = self
            .remote
            .lock()
            .map_err(|_| VmError::NotConnected)?;
        if guard.is_none() {
            *guard = Some(remote);
        }
        Ok(())
    }

    fn create_remote(clear_stale: bool) -> Result<VoicemeeterRemote, VmError> {
        if clear_stale {
            if let Ok(raw) = get_voicemeeter_raw() {
                unsafe {
                    let _ = raw.VBVMR_Logout();
                }
            }
        }
        VoicemeeterRemote::new().map_err(VmError::from)
    }

    fn ping_server(&self) -> Result<(), VmError> {
        let result = self.with_remote(|remote| {
            remote
                .is_parameters_dirty()
                .map(|_| ())
                .map_err(|_| VmError::NotConnected)
        });
        self.connected
            .store(result.is_ok(), Ordering::Relaxed);
        result
    }

    fn with_remote<F, T>(&self, f: F) -> Result<T, VmError>
    where
        F: FnOnce(&VoicemeeterRemote) -> Result<T, VmError>,
    {
        let guard = self
            .remote
            .lock()
            .map_err(|_| VmError::NotConnected)?;
        let remote = guard.as_ref().ok_or(VmError::NotConnected)?;
        f(remote)
    }

    pub fn flush(&self) -> Result<(), VmError> {
        self.with_remote(|remote| {
            let _ = remote.is_parameters_dirty();
            Ok(())
        })
    }

    pub fn set_mute_inner(&self, bus: usize, muted: bool) -> Result<(), VmError> {
        self.with_remote(|remote| {
            remote
                .parameters()
                .bus(bus)
                .map_err(|e| VmError::Bus {
                    bus,
                    message: e.to_string(),
                })?
                .mute()
                .set(muted)
                .map_err(|e| VmError::Bus {
                    bus,
                    message: e.to_string(),
                })
        })
    }

    pub fn get_gain(&self, bus: usize) -> Result<f32, VmError> {
        self.with_remote(|remote| {
            remote
                .parameters()
                .bus(bus)
                .map_err(|e| VmError::Bus {
                    bus,
                    message: e.to_string(),
                })?
                .gain()
                .get()
                .map_err(|e| VmError::Bus {
                    bus,
                    message: e.to_string(),
                })
        })
    }

    pub fn set_gain(&self, bus: usize, gain: f32) -> Result<(), VmError> {
        let clamped = gain.clamp(GAIN_MIN, GAIN_MAX);
        self.with_remote(|remote| {
            remote
                .parameters()
                .bus(bus)
                .map_err(|e| VmError::Bus {
                    bus,
                    message: e.to_string(),
                })?
                .gain()
                .set(clamped)
                .map_err(|e| VmError::Bus {
                    bus,
                    message: e.to_string(),
                })
        })?;
        self.flush()
    }

    fn version_label_uncached(&self) -> String {
        self.with_remote(|remote| {
            Ok(remote
                .get_voicemeeter_version()
                .map(|v| format!("VM {}", v))
                .unwrap_or_else(|_| "VM OK".into()))
        })
        .unwrap_or_else(|_| "VM déconnecté".into())
    }
}
