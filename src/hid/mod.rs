//! HID layer public re-exports, constants, supported device table, and error type.

pub mod device;
pub mod features;
pub mod monitor;
pub mod protocol;

pub use device::MxMaster;

#[allow(dead_code)]
pub const LOGITECH_VID: u16 = 0x046d;
pub const REPORT_ID_SHORT: u8 = 0x10; // 7-byte report
pub const REPORT_ID_LONG: u8 = 0x11; // 20-byte report
pub const DEVICE_INDEX_DIRECT: u8 = 0xff; // USB or BT direct connection

/// Transport the device is attached by, taken from the `HID_ID=BUS:VID:PID`
/// bus field in sysfs.
///
/// This matters because the two transports do not expose the same HID++
/// report IDs.  Bluetooth MX devices declare **only** report ID `0x11`
/// (20-byte long) in their HID report descriptor — there is no `0x10`
/// (7-byte short) report — so a short request is dropped on the floor and
/// every HID++ call appears to time out.  USB accepts both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HidBus {
    Usb,
    Bluetooth,
    Other(u16),
}

impl HidBus {
    /// `BUS_USB` / `BUS_BLUETOOTH` as reported in `HID_ID`.
    pub fn from_id(bus: u16) -> Self {
        match bus {
            0x0003 => HidBus::Usb,
            0x0005 => HidBus::Bluetooth,
            other => HidBus::Other(other),
        }
    }

    /// Whether short (7-byte) HID++ reports can be used on this transport.
    pub fn supports_short_reports(&self) -> bool {
        !matches!(self, HidBus::Bluetooth)
    }
}

/// Enumeration of all supported Logitech MX series models.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DeviceModel {
    MxMaster2S,
    MxMaster3,
    MxMaster3ForMac,
    MxMaster3S,
    MxMaster3SForMac,
    MxMaster4,
}

impl DeviceModel {
    /// Human-readable name shown in the TUI title bar.
    pub fn display_name(&self) -> &'static str {
        match self {
            DeviceModel::MxMaster2S => "MX Master 2S",
            DeviceModel::MxMaster3 => "MX Master 3",
            DeviceModel::MxMaster3ForMac => "MX Master 3 for Mac",
            DeviceModel::MxMaster3S => "MX Master 3S",
            DeviceModel::MxMaster3SForMac => "MX Master 3S for Mac",
            DeviceModel::MxMaster4 => "MX Master 4",
        }
    }
}

/// `(vid, pid, model)` lookup table for all supported devices.
pub static SUPPORTED_DEVICES: &[(u16, u16, DeviceModel)] = &[
    (0x046d, 0xb012, DeviceModel::MxMaster2S),
    (0x046d, 0xb019, DeviceModel::MxMaster2S),
    (0x046d, 0xb023, DeviceModel::MxMaster3),
    (0x046d, 0xb022, DeviceModel::MxMaster3),
    (0x046d, 0xb026, DeviceModel::MxMaster3ForMac),
    (0x046d, 0xb034, DeviceModel::MxMaster3S),
    (0x046d, 0xb028, DeviceModel::MxMaster3S),
    (0x046d, 0xb02a, DeviceModel::MxMaster3SForMac),
    (0x046d, 0xb042, DeviceModel::MxMaster4), // Bluetooth
];

/// Errors that can arise from HID device communication.
#[derive(thiserror::Error, Debug)]
pub enum HidError {
    #[error("device not found — is the mouse connected?")]
    NotFound,
    #[error("permission denied on {path}: install udev rule")]
    Permission { path: String },
    #[error("HID++ error code {0:#04x}")]
    Protocol(u8),
    #[error("no HID++ reply from device")]
    Timeout,
    #[error("device does not support HID++ feature {0:#06x}")]
    FeatureMissing(u16),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}
