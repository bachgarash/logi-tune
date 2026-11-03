//! MxMaster device struct, hidraw scanning, and device open logic.

use std::fs::{File, OpenOptions};

use super::protocol::{send_long, send_short};
use super::{DeviceModel, HidBus, HidError, SUPPORTED_DEVICES};

/// An open handle to a Logitech MX series mouse via hidraw.
pub struct MxMaster {
    pub(crate) file: File,
    pub model: DeviceModel,
    pub(crate) device_index: u8,
    /// Path to the hidraw device node (e.g. `/dev/hidraw5`).
    pub path: String,
    /// Transport this device is attached by; decides the HID++ report size.
    pub bus: HidBus,
}

impl MxMaster {
    /// Issue a HID++ 2.0 request using whichever report size this transport
    /// supports, and return the raw reply.
    ///
    /// Callers pass parameters as a slice and read the reply from byte 4
    /// onwards, so they do not need to care which report size was used.
    pub fn request(
        &mut self,
        feature_index: u8,
        func: u8,
        params: &[u8],
    ) -> Result<[u8; 64], HidError> {
        self.retrying(|s| {
            if params.len() <= 3 && s.bus.supports_short_reports() {
                let mut p = [0u8; 3];
                p[..params.len()].copy_from_slice(params);
                send_short(&mut s.file, s.device_index, feature_index, func, p)
            } else {
                let mut p = [0u8; 16];
                let n = params.len().min(16);
                p[..n].copy_from_slice(&params[..n]);
                send_long(&mut s.file, s.device_index, feature_index, func, p)
            }
        })
    }

    /// Retry a request that timed out.
    ///
    /// A reply can go missing when another HID++ client is talking to the same
    /// device — its reader may consume our reply before we see it. Without a
    /// retry that surfaces as a setting silently not applying. Every request
    /// here is a getter or an idempotent setter, so re-sending is safe.
    fn retrying<F>(&mut self, mut attempt: F) -> Result<[u8; 64], HidError>
    where
        F: FnMut(&mut Self) -> Result<[u8; 64], HidError>,
    {
        const ATTEMPTS: u32 = 3;
        let mut last = HidError::Timeout;
        for n in 0..ATTEMPTS {
            match attempt(self) {
                Err(HidError::Timeout) => {
                    last = HidError::Timeout;
                    if n + 1 < ATTEMPTS {
                        tracing::debug!(attempt = n + 1, "HID++ timeout, retrying");
                        std::thread::sleep(std::time::Duration::from_millis(50));
                    }
                }
                other => return other,
            }
        }
        Err(last)
    }

    /// Issue a HID++ 2.0 request forcing a long (20-byte) report, for calls
    /// that are known to need one on every transport.
    pub fn request_long(
        &mut self,
        feature_index: u8,
        func: u8,
        params: &[u8],
    ) -> Result<[u8; 64], HidError> {
        self.retrying(|s| {
            let mut p = [0u8; 16];
            let n = params.len().min(16);
            p[..n].copy_from_slice(&params[..n]);
            send_long(&mut s.file, s.device_index, feature_index, func, p)
        })
    }
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

            let uevent = match std::fs::read_to_string(&uevent_path) {
                Ok(s) => s,
                Err(_) => continue,
            };

            let (bus, vid, pid) = match parse_hid_id(&uevent) {
                Some(triple) => triple,
                None => continue,
            };

            let model = match SUPPORTED_DEVICES
                .iter()
                .find(|(v, p, _)| *v == vid && *p == pid)
                .map(|(_, _, m)| *m)
            {
                Some(m) => m,
                None => continue,
            };

            let file = match OpenOptions::new().read(true).write(true).open(&dev_path) {
                Ok(f) => f,
                Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                    return Err(HidError::Permission { path: dev_path });
                }
                Err(_) => continue,
            };

            let bus = HidBus::from_id(bus);
            tracing::info!(
                "Opened {} ({}) via {:?} at {}",
                model.display_name(),
                dev_path,
                bus,
                uevent_path
            );

            return Ok(Self {
                file,
                model,
                device_index: super::DEVICE_INDEX_DIRECT,
                path: dev_path,
                bus,
            });
        }

        Err(HidError::NotFound)
    }
}

/// Parse the `HID_ID=BUS:VID:PID` line from a uevent file into
/// `(bus, vid, pid)`.
///
/// The format is `HID_ID=BBBBBBBB:VVVVVVVV:PPPPPPPP` (all 8 hex digits).
fn parse_hid_id(uevent: &str) -> Option<(u16, u16, u16)> {
    for line in uevent.lines() {
        if let Some(rest) = line.strip_prefix("HID_ID=") {
            // rest = "BBBBBBBB:VVVVVVVV:PPPPPPPP"
            let parts: Vec<&str> = rest.split(':').collect();
            if parts.len() != 3 {
                continue;
            }
            let bus = u32::from_str_radix(parts[0], 16).ok()? as u16;
            let vid = u32::from_str_radix(parts[1], 16).ok()? as u16;
            let pid = u32::from_str_radix(parts[2], 16).ok()? as u16;
            return Some((bus, vid, pid));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bluetooth_hid_id() {
        let uevent = "DRIVER=logitech-hidpp-device\n\
                      HID_ID=0005:0000046D:0000B042\n\
                      HID_NAME=Logitech MX Master 4\n";
        assert_eq!(parse_hid_id(uevent), Some((0x0005, 0x046d, 0xb042)));
        assert_eq!(HidBus::from_id(0x0005), HidBus::Bluetooth);
        assert!(!HidBus::from_id(0x0005).supports_short_reports());
    }

    #[test]
    fn parses_usb_hid_id() {
        let uevent = "HID_ID=0003:0000046D:0000B034\n";
        assert_eq!(parse_hid_id(uevent), Some((0x0003, 0x046d, 0xb034)));
        assert!(HidBus::from_id(0x0003).supports_short_reports());
    }

    #[test]
    fn rejects_malformed_hid_id() {
        assert_eq!(parse_hid_id("HID_ID=0005:0000046D\n"), None);
        assert_eq!(parse_hid_id("HID_NAME=nope\n"), None);
    }
}
