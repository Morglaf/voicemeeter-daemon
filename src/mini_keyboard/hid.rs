use super::mapping::PhysicalKey;
use std::ffi::{CString, NulError};
use std::io;
use std::time::Duration;
use thiserror::Error;

/// Consumer control usages (HID Usage Page 0x0C).
const USAGE_PAGE_CONSUMER: u16 = 0x0C;
const USAGE_PAGE_VENDOR: u16 = 0xFF00;
const USAGE_VOLUME_UP: u16 = 0xE9;
const USAGE_VOLUME_DOWN: u16 = 0xEA;
const USAGE_MUTE: u16 = 0xE2;
const USAGE_PLAY: u16 = 0xB0;
const USAGE_PLAY_PAUSE: u16 = 0xCD;
const USAGE_SCAN_NEXT: u16 = 0xB5;
const USAGE_SCAN_PREV: u16 = 0xB6;

#[derive(Debug, Clone)]
pub struct HidDeviceInfo {
    pub path: String,
    pub vid: u16,
    pub pid: u16,
    pub product: String,
    pub manufacturer: String,
    pub usage_page: u16,
    pub usage: u16,
    pub interface_number: i32,
}

impl HidDeviceInfo {
    pub fn label(&self) -> String {
        let name = if !self.product.is_empty() {
            self.product.as_str()
        } else if !self.manufacturer.is_empty() {
            self.manufacturer.as_str()
        } else {
            "HID"
        };
        format!(
            "{name} · MI_{:02} · {:04X}:{:04X} ({:04X}:{:04X})",
            self.interface_number.max(0) as u32,
            self.usage_page,
            self.usage,
            self.vid,
            self.pid
        )
    }

    pub fn is_consumer_control(&self) -> bool {
        self.usage_page == USAGE_PAGE_CONSUMER
    }

    pub fn is_vendor_interface(&self) -> bool {
        self.usage_page == USAGE_PAGE_VENDOR
    }
}

#[derive(Error, Debug)]
pub enum HidError {
    #[error("hidapi: {0}")]
    Api(#[from] hidapi::HidError),
    #[error("IO: {0}")]
    Io(#[from] io::Error),
    #[error("chemin invalide: {0}")]
    Path(#[from] NulError),
    #[error("périphérique introuvable")]
    NotFound,
    #[error("Windows: {0}")]
    Win(String),
}

pub struct DeviceReader {
    device: hidapi::HidDevice,
    pub info: HidDeviceInfo,
}

impl DeviceReader {
    pub fn read_report(&self, buf: &mut [u8]) -> Result<Option<usize>, HidError> {
        match self.device.read_timeout(buf, 2) {
            Ok(0) => Ok(None),
            Ok(n) => Ok(Some(n)),
            Err(e) if e.to_string().contains("timeout") => Ok(None),
            Err(e) => Err(HidError::Api(e)),
        }
    }
}

pub fn enumerate_devices() -> Result<Vec<HidDeviceInfo>, HidError> {
    let api = hidapi::HidApi::new()?;
    let mut devices = Vec::new();
    for dev in api.device_list() {
        devices.push(HidDeviceInfo {
            path: dev.path().to_string_lossy().into_owned(),
            vid: dev.vendor_id(),
            pid: dev.product_id(),
            product: dev.product_string().unwrap_or_default().to_string(),
            manufacturer: dev.manufacturer_string().unwrap_or_default().to_string(),
            usage_page: dev.usage_page(),
            usage: dev.usage(),
            interface_number: dev.interface_number(),
        });
    }
    Ok(devices)
}

pub fn list_matching_devices(vid: u16, pid: u16) -> Vec<HidDeviceInfo> {
    let Ok(mut devices) = enumerate_devices() else {
        return Vec::new();
    };
    devices.retain(|d| d.vid == vid && d.pid == pid);
    devices.sort_by(|a, b| {
        a.interface_number
            .cmp(&b.interface_number)
            .then(a.path.cmp(&b.path))
    });
    devices
}

pub fn capture_targets(vid: u16, pid: u16, path: Option<&str>) -> Vec<HidDeviceInfo> {
    let matches = list_matching_devices(vid, pid);
    if matches.is_empty() {
        return Vec::new();
    }
    if let Some(path) = path {
        return matches
            .into_iter()
            .filter(|d| d.path == path)
            .collect();
    }
    matches
}

pub fn open_readers(vid: u16, pid: u16, path: Option<&str>) -> Result<Vec<DeviceReader>, HidError> {
    let targets = capture_targets(vid, pid, path);
    if targets.is_empty() {
        return Err(HidError::NotFound);
    }

    let api = hidapi::HidApi::new()?;
    let mut readers = Vec::new();
    for info in targets {
        match open_reader(&api, &info) {
            Ok(reader) => {
                eprintln!("HID ouvert: {} ({})", info.label(), info.path);
                readers.push(reader);
            }
            Err(e) => eprintln!("HID échec {}: {e}", info.path),
        }
    }

    if readers.is_empty() {
        return Err(HidError::Win("aucune interface HID ouverte".into()));
    }
    Ok(readers)
}

fn open_reader(api: &hidapi::HidApi, info: &HidDeviceInfo) -> Result<DeviceReader, HidError> {
    let c_path = CString::new(info.path.as_str())?;
    let device = api.open_path(&c_path)?;
    device.set_blocking_mode(false)?;
    Ok(DeviceReader {
        device,
        info: info.clone(),
    })
}

pub fn parse_report(data: &[u8], device: &HidDeviceInfo) -> Option<PhysicalKey> {
    let payloads = report_payloads(data);
    for payload in payloads {
        let key = if device.is_consumer_control() {
            parse_consumer_report(payload)
        } else if device.is_vendor_interface() {
            parse_vendor_report(payload)
        } else {
            parse_generic_report(payload)
        };
        if key.is_some() {
            return key;
        }
    }
    None
}

fn report_payloads(data: &[u8]) -> Vec<&[u8]> {
    let mut out = vec![data];
    if data.len() > 1 {
        out.push(&data[1..]);
    }
    out
}

/// Knob / volume consumer uniquement (interface 0x0C).
fn parse_consumer_report(data: &[u8]) -> Option<PhysicalKey> {
    for chunk in data.windows(2) {
        if let Some(key) = consumer_usage_to_key(u16::from_le_bytes([chunk[0], chunk[1]])) {
            return Some(key);
        }
    }
    for &byte in data {
        if byte == 0 {
            continue;
        }
        if let Some(key) = consumer_usage_to_key(u16::from(byte)) {
            return Some(key);
        }
    }
    None
}

/// Boutons physiques uniquement (interface FF00) — pas le knob.
fn parse_vendor_report(data: &[u8]) -> Option<PhysicalKey> {
    for &byte in data {
        if byte == 0 {
            continue;
        }
        if let Some(key) = vendor_button_to_key(byte) {
            return Some(key);
        }
    }
    None
}

fn parse_generic_report(data: &[u8]) -> Option<PhysicalKey> {
    parse_consumer_report(data).or_else(|| parse_vendor_report(data))
}

fn consumer_usage_to_key(usage: u16) -> Option<PhysicalKey> {
    match usage & 0xFFFF {
        USAGE_SCAN_PREV => Some(PhysicalKey::Prev),
        USAGE_SCAN_NEXT => Some(PhysicalKey::Next),
        USAGE_PLAY | USAGE_PLAY_PAUSE => Some(PhysicalKey::Play),
        USAGE_MUTE => Some(PhysicalKey::KnobClick),
        USAGE_VOLUME_DOWN => Some(PhysicalKey::KnobLeft),
        USAGE_VOLUME_UP => Some(PhysicalKey::KnobRight),
        _ => None,
    }
}

fn vendor_button_to_key(byte: u8) -> Option<PhysicalKey> {
    match byte {
        0x01 | 0x02 | 0x10 => None,
        0x04 | 0x08 => Some(PhysicalKey::KnobClick),
        0x20 => Some(PhysicalKey::Play),
        0x40 => Some(PhysicalKey::Next),
        0x80 => Some(PhysicalKey::Prev),
        0xB0 | 0xCD => Some(PhysicalKey::Play),
        0xB5 => Some(PhysicalKey::Next),
        0xB6 => Some(PhysicalKey::Prev),
        _ => None,
    }
}

pub const RETRY_INTERVAL: Duration = Duration::from_secs(5);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_single_byte_usages() {
        let consumer = HidDeviceInfo {
            path: String::new(),
            vid: 0,
            pid: 0,
            product: String::new(),
            manufacturer: String::new(),
            usage_page: 0x0C,
            usage: 0x01,
            interface_number: 2,
        };
        assert_eq!(
            parse_report(&[0xE9], &consumer),
            Some(PhysicalKey::KnobRight)
        );
        assert_eq!(
            parse_report(&[0xEA], &consumer),
            Some(PhysicalKey::KnobLeft)
        );
        assert_eq!(
            parse_report(&[0xE2], &consumer),
            Some(PhysicalKey::KnobClick)
        );
    }

    #[test]
    fn vendor_ignores_knob_bytes() {
        let vendor = HidDeviceInfo {
            path: String::new(),
            vid: 0,
            pid: 0,
            product: String::new(),
            manufacturer: String::new(),
            usage_page: 0xFF00,
            usage: 0x01,
            interface_number: 1,
        };
        assert_eq!(parse_report(&[0x01], &vendor), None);
        assert_eq!(parse_report(&[0xE9], &vendor), None);
        assert_eq!(parse_report(&[0x40], &vendor), Some(PhysicalKey::Next));
    }

    #[test]
    fn vendor_play_button_bytes() {
        let vendor = HidDeviceInfo {
            path: String::new(),
            vid: 0,
            pid: 0,
            product: String::new(),
            manufacturer: String::new(),
            usage_page: 0xFF00,
            usage: 0x01,
            interface_number: 1,
        };
        assert_eq!(parse_report(&[0x20], &vendor), Some(PhysicalKey::Play));
        assert_eq!(parse_report(&[0xCD], &vendor), Some(PhysicalKey::Play));
    }

    #[test]
    fn consumer_play_pause_byte() {
        let consumer = HidDeviceInfo {
            path: String::new(),
            vid: 0,
            pid: 0,
            product: String::new(),
            manufacturer: String::new(),
            usage_page: 0x0C,
            usage: 0x01,
            interface_number: 2,
        };
        assert_eq!(parse_report(&[0xCD], &consumer), Some(PhysicalKey::Play));
    }

    #[test]
    fn parse_with_report_id_prefix() {
        let consumer = HidDeviceInfo {
            path: String::new(),
            vid: 0,
            pid: 0,
            product: String::new(),
            manufacturer: String::new(),
            usage_page: 0x0C,
            usage: 0x01,
            interface_number: 2,
        };
        assert_eq!(
            parse_report(&[0x01, 0xE9], &consumer),
            Some(PhysicalKey::KnobRight)
        );
    }
}
