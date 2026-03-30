//! Evdev-based button monitor.
//!
//! The logitech-hidpp-device kernel driver consumes HID++ responses before
//! they reach hidraw, so we cannot use HID++ divert. Instead we read processed
//! input events from the kernel's evdev interface, which always works.
//!
//! We grab the device exclusively (EVIOCGRAB) so the original button events
//! don't also reach the compositor/applications. All events we don't intercept
//! are forwarded via a uinput pass-through mouse.

use std::fs::{File, OpenOptions};
use std::io::Read;
use std::os::unix::io::AsRawFd;

use super::HidError;

// EVIOCGRAB = _IOW('E', 0x90, int) = 0x40044590
nix::ioctl_write_int_bad!(eviocgrab, 0x40044590);

/// A raw input_event read from the evdev device.
pub struct RawEvent {
    pub ev_type: u16,
    pub code: u16,
    pub value: i32,
}

/// Reads raw input events from the mouse's evdev node.
pub struct DeviceMonitor {
    file: File,
}

impl DeviceMonitor {
    /// Open the evdev device corresponding to the given hidraw path
    /// (e.g. `/dev/hidraw6` → looks up `/sys/class/hidraw/hidraw6/device/input/…/eventN`).
    ///
    /// Grabs the device exclusively so the original events are not also delivered
    /// to other consumers (compositor, applications).
    pub fn open(hidraw_path: &str) -> Result<Self, HidError> {
        let n = hidraw_path
            .trim_start_matches("/dev/hidraw")
            .parse::<u8>()
            .map_err(|_| HidError::NotFound)?;

        let event_path = find_event_device(n)?;
        tracing::info!(event_path, "evdev monitor opening");

        let file = OpenOptions::new()
            .read(true)
            .open(&event_path)
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::PermissionDenied {
                    HidError::Permission { path: event_path.clone() }
                } else {
                    HidError::Io(e)
                }
            })?;

        // Grab exclusively so the original events don't reach apps.
        // This prevents double-action when we remap a button to a key combo.
        match unsafe { eviocgrab(file.as_raw_fd(), 1) } {
            Ok(_) => tracing::info!("evdev exclusive grab acquired"),
            Err(e) => tracing::warn!("evdev grab failed (pass-through may double-fire): {e}"),
        }

        Ok(Self { file })
    }

    /// Block until an input event arrives and return the raw event.
    pub fn read_raw(&mut self) -> Result<RawEvent, HidError> {
        // input_event on 64-bit Linux: sec(8) + usec(8) + type(2) + code(2) + value(4) = 24 bytes
        let mut buf = [0u8; 24];
        self.file.read_exact(&mut buf)?;

        let ev_type = u16::from_le_bytes([buf[16], buf[17]]);
        let code    = u16::from_le_bytes([buf[18], buf[19]]);
        let value   = i32::from_le_bytes([buf[20], buf[21], buf[22], buf[23]]);

        Ok(RawEvent { ev_type, code, value })
    }
}

impl Drop for DeviceMonitor {
    fn drop(&mut self) {
        unsafe { let _ = eviocgrab(self.file.as_raw_fd(), 0); }
    }
}

fn find_event_device(hidraw_n: u8) -> Result<String, HidError> {
    let input_dir = format!("/sys/class/hidraw/hidraw{}/device/input", hidraw_n);
    for input_entry in std::fs::read_dir(&input_dir).map_err(HidError::Io)? {
        let input_path = input_entry.map_err(HidError::Io)?.path();
        for event_entry in std::fs::read_dir(&input_path).map_err(HidError::Io)? {
            let name = event_entry.map_err(HidError::Io)?.file_name();
            let s = name.to_string_lossy();
            if s.starts_with("event") {
                return Ok(format!("/dev/input/{}", s));
            }
        }
    }
    Err(HidError::NotFound)
}
