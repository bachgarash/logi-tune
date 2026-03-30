//! MxMaster device struct, hidraw scanning, and device open logic.

use std::fs::{File, OpenOptions};

use super::{DeviceModel, HidError, SUPPORTED_DEVICES};

// ---------------------------------------------------------------------------
// MxMaster
// ---------------------------------------------------------------------------

/// An open handle to a Logitech MX series mouse via hidraw.
pub struct MxMaster {
    pub(crate) file: File,
    pub model: DeviceModel,
    pub(crate) device_index: u8,
    /// Path to the hidraw device node (e.g. `/dev/hidraw5`).
    pub path: String,
}

impl MxMaster {
    /// Scan `/dev/hidraw0`–`/dev/hidraw15`, match against `SUPPORTED_DEVICES`,
    /// and return an open `MxMaster`.
    ///
    /// Returns `HidError::NotFound` if no supported device is found.
    /// Returns `HidError::Permission` if a matching device cannot be opened.
    pub fn open() -> Result<Self, HidError> {
        for n in 0..16u8 {
            let dev_path = format!("/dev/hidraw{}", n);
            let uevent_path = format!("/sys/class/hidraw/hidraw{}/device/uevent", n);

            // Read the uevent file to extract HID_ID
            let uevent = match std::fs::read_to_string(&uevent_path) {
                Ok(s) => s,
                Err(_) => continue,
            };

            let (vid, pid) = match parse_hid_id(&uevent) {
                Some(pair) => pair,
                None => continue,
            };

            // Look up in the supported devices table
            let model = match SUPPORTED_DEVICES
                .iter()
                .find(|(v, p, _)| *v == vid && *p == pid)
                .map(|(_, _, m)| *m)
            {
                Some(m) => m,
                None => continue,
            };

            // Try to open the device
            let file = match OpenOptions::new()
                .read(true)
                .write(true)
                .open(&dev_path)
            {
                Ok(f) => f,
                Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                    return Err(HidError::Permission { path: dev_path });
                }
                Err(_) => continue,
            };

            tracing::info!(
                "Opened {} ({}) at {}",
                model.display_name(),
                dev_path,
                uevent_path
            );

            return Ok(Self {
                file,
                model,
                device_index: super::DEVICE_INDEX_DIRECT,
                path: dev_path,
            });
        }

        Err(HidError::NotFound)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Parse the `HID_ID=BUS:VID:PID` line from a uevent file.
///
/// The format is `HID_ID=BBBBBBBB:VVVVVVVV:PPPPPPPP` (all 8 hex digits).
fn parse_hid_id(uevent: &str) -> Option<(u16, u16)> {
    for line in uevent.lines() {
        if let Some(rest) = line.strip_prefix("HID_ID=") {
            // rest = "BBBBBBBB:VVVVVVVV:PPPPPPPP"
            let parts: Vec<&str> = rest.split(':').collect();
            if parts.len() != 3 {
                continue;
            }
            let vid = u32::from_str_radix(parts[1], 16).ok()? as u16;
            let pid = u32::from_str_radix(parts[2], 16).ok()? as u16;
            return Some((vid, pid));
        }
    }
    None
}
