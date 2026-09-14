use crate::config::Config;
use crate::vm::{VmController, VmError};
use std::sync::Arc;

/// Applique l'état mute : seul le bus actif est audible, tous les autres switchables sont mutés.
pub fn apply_active_bus(vm: &Arc<VmController>, config: &Config) -> Result<(), VmError> {
    for &bus in &config.switch_buses {
        let muted = bus != config.active_bus;
        vm.set_mute_inner(bus, muted)?;
    }
    vm.flush()?;
    Ok(())
}

/// Passe au bus suivant dans la liste switchable et mute les autres.
pub fn switch_next(vm: &Arc<VmController>, config: &mut Config) -> Result<(), VmError> {
    config.active_bus = config.next_switch_bus();
    apply_active_bus(vm, config)?;
    config.save().map_err(|e| VmError::Bus {
        bus: config.active_bus,
        message: e.to_string(),
    })?;
    Ok(())
}

/// Sélectionne un bus actif et mute les autres switchables.
pub fn set_active_bus(
    vm: &Arc<VmController>,
    config: &mut Config,
    bus: usize,
) -> Result<(), VmError> {
    config.set_active_bus(bus);
    if config.active_bus != bus {
        return Ok(());
    }
    apply_active_bus(vm, config)?;
    config.save().map_err(|e| VmError::Bus {
        bus: config.active_bus,
        message: e.to_string(),
    })?;
    Ok(())
}

/// Active/désactive un bus dans la liste switchable.
pub fn toggle_switch_bus(
    vm: &Arc<VmController>,
    config: &mut Config,
    bus: usize,
) -> Result<bool, VmError> {
    if !config.toggle_switch_bus(bus) {
        return Ok(false);
    }
    apply_active_bus(vm, config)?;
    config.save().map_err(|e| VmError::Bus {
        bus: config.active_bus,
        message: e.to_string(),
    })?;
    Ok(true)
}

pub fn adjust_volume(
    vm: &Arc<VmController>,
    config: &Config,
    direction: f32,
) -> Result<(), VmError> {
    apply_active_bus(vm, config)?;

    let bus = config.active_bus;
    let current = vm.get_gain(bus)?;
    let delta = config.volume_increment * direction.signum();
    vm.set_gain(bus, current + delta)?;
    Ok(())
}
