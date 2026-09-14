use crate::config::Config;
use std::sync::{Arc, Mutex};
use thiserror::Error;
use voicemeeter::interface::InitializationError;
use voicemeeter::VoicemeeterRemote;

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
    remote: Mutex<Option<VoicemeeterRemote>>,
}

impl VmController {
    pub fn new() -> Result<Arc<Self>, VmError> {
        let controller = Arc::new(Self {
            remote: Mutex::new(Some(VoicemeeterRemote::new()?)),
        });
        Ok(controller)
    }

    pub fn reconnect(self: &Arc<Self>) -> Result<(), VmError> {
        let mut guard = self
            .remote
            .lock()
            .map_err(|_| VmError::NotConnected)?;
        *guard = Some(VoicemeeterRemote::new()?);
        Ok(())
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

    pub fn version_label(&self) -> String {
        self.with_remote(|remote| {
            Ok(remote
                .get_voicemeeter_version()
                .map(|v| format!("VM {}", v))
                .unwrap_or_else(|_| "VM OK".into()))
        })
        .unwrap_or_else(|_| "VM déconnecté".into())
    }

    pub fn tooltip(&self, config: &Config) -> String {
        let bus = config.active_bus;
        let gain = self
            .get_gain(bus)
            .map(|g| format!("{:+.1} dB", g))
            .unwrap_or_else(|_| "?".into());
        format!(
            "{} | Bus {} | {}",
            self.version_label(),
            Config::bus_label(bus),
            gain
        )
    }
}
