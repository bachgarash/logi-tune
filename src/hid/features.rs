//! HID++ 2.0 feature-index resolution and per-feature setter functions.

use crate::config::ButtonAction;

use super::{DeviceModel, HidError, MxMaster};

const FEATURE_ROOT: u16 = 0x0000;
const FEATURE_FEATURE_SET: u16 = 0x0001;
pub const FEATURE_REPROG_CONTROLS_V4: u16 = 0x1b04;
#[allow(dead_code)]
const FEATURE_UNIFIED_BATTERY: u16 = 0x1004;
const FEATURE_SMART_SHIFT: u16 = 0x2100;
const FEATURE_HI_RES_SCROLLING: u16 = 0x2120;
const FEATURE_THUMB_WHEEL: u16 = 0x2150;
const FEATURE_ADJUSTABLE_DPI: u16 = 0x2201;

/// Feature numbers this project knows by name, for `--scan` output.
///
/// Names follow the HID++ 2.0 feature list used by Solaar and libratbag so
/// that scan output can be compared against those projects directly.
const FEATURE_NAMES: &[(u16, &str)] = &[
    (0x0000, "ROOT"),
    (0x0001, "FEATURE_SET"),
    (0x0003, "DEVICE_FW_VERSION"),
    (0x0005, "DEVICE_NAME"),
    (0x0007, "DEVICE_FRIENDLY_NAME"),
    (0x0011, "SEPARATE_VOICE_KEYS"),
    (0x0020, "CONFIG_CHANGE"),
    (0x0021, "CONFIG_DEVICE_PROPS"),
    (0x00d1, "UNKNOWN_00D1"),
    (0x1004, "UNIFIED_BATTERY"),
    (0x1602, "UNKNOWN_1602"),
    (0x1701, "UNKNOWN_1701"),
    (0x1802, "CHANGE_HOST_ENG"),
    (0x1803, "GPIO_ACCESS"),
    (0x1805, "OOBSTATE"),
    (0x1814, "CHANGE_HOST"),
    (0x1815, "HOSTS_INFO"),
    (0x1830, "LOWRES_WHEEL"),
    (0x1861, "BATTERY_VOLTAGE"),
    (0x18a1, "LED_CONTROL"),
    (0x19b0, "HAPTIC_FEEDBACK"),
    (0x19c0, "UNKNOWN_19C0"),
    (0x1b04, "REPROG_CONTROLS_V4"),
    (0x1d4b, "WIRELESS_DEVICE_STATUS"),
    (0x1e00, "ENABLE_HIDDEN"),
    (0x2110, "SMART_SHIFT"),
    (0x2111, "SMART_SHIFT_ENHANCED"),
    (0x2120, "HI_RES_SCROLLING"),
    (0x2121, "HIRES_WHEEL"),
    (0x2150, "THUMB_WHEEL"),
    (0x2201, "ADJUSTABLE_DPI"),
    (0x2202, "EXTENDED_ADJUSTABLE_DPI"),
    (0x2250, "ANALYSIS_MODE"),
    (0x2251, "UNKNOWN_2251"),
];

/// Human-readable name for a feature number, or `"?"` when unknown.
pub fn feature_name(number: u16) -> &'static str {
    FEATURE_NAMES
        .iter()
        .find(|(n, _)| *n == number)
        .map(|(_, name)| *name)
        .unwrap_or("?")
}

/// Resolve a HID++ 2.0 feature number to its runtime index on this device.
pub fn get_feature_index(device: &mut MxMaster, feature_number: u16) -> Result<u8, HidError> {
    let hi = (feature_number >> 8) as u8;
    let lo = (feature_number & 0xff) as u8;

    // ROOT feature is always at index 0x00; function 0x00 is getFeature.
    let response = device.request(0x00, 0x00, &[hi, lo])?;

    // response[4] = feature index (0x00 means not supported)
    let index = response[4];
    if index == 0x00 && feature_number != FEATURE_ROOT {
        return Err(HidError::FeatureMissing(feature_number));
    }
    Ok(index)
}

/// Resolve the first supported feature from `candidates`, most preferred first.
///
/// Newer MX models replace features rather than extending them — the MX
/// Master 4 carries `SMART_SHIFT_ENHANCED` (0x2111) and `HIRES_WHEEL` (0x2121)
/// and does *not* answer for the older 0x2100/0x2120 numbers — so callers need
/// to ask for a list and handle whichever one exists.
#[allow(dead_code)]
pub fn resolve_feature(device: &mut MxMaster, candidates: &[u16]) -> Result<(u16, u8), HidError> {
    for &number in candidates {
        match get_feature_index(device, number) {
            Ok(index) => return Ok((number, index)),
            Err(HidError::FeatureMissing(_)) | Err(HidError::Protocol(_)) => continue,
            Err(e) => return Err(e),
        }
    }
    Err(HidError::FeatureMissing(
        candidates.first().copied().unwrap_or(0),
    ))
}

/// Set the active DPI via ADJUSTABLE_DPI (0x2201), function 0x03 (setDpiForSensor).
pub fn set_dpi(device: &mut MxMaster, dpi: u16) -> Result<(), HidError> {
    let fi = get_feature_index(device, FEATURE_ADJUSTABLE_DPI)?;
    let dpi_hi = (dpi >> 8) as u8;
    let dpi_lo = (dpi & 0xff) as u8;
    // sensor index 0, then the 16-bit DPI value
    device.request_long(fi, 0x03, &[0x00, dpi_hi, dpi_lo])?;
    Ok(())
}

/// Configure SmartShift via SMART_SHIFT (0x2100), function 0x01 (setSmartShift).
pub fn set_smart_shift(device: &mut MxMaster, threshold: u8) -> Result<(), HidError> {
    let fi = get_feature_index(device, FEATURE_SMART_SHIFT)?;
    // threshold 0 = always free-spin, 255 = always ratchet
    device.request(fi, 0x01, &[threshold])?;
    Ok(())
}

/// Configure hi-res scrolling via HI_RES_SCROLLING (0x2120), function 0x01.
pub fn set_hi_res_scroll(device: &mut MxMaster, multiplier: u8) -> Result<(), HidError> {
    let fi = get_feature_index(device, FEATURE_HI_RES_SCROLLING)?;
    device.request(fi, 0x01, &[multiplier])?;
    Ok(())
}

/// Configure the thumb wheel via THUMB_WHEEL (0x2150), function 0x03.
pub fn set_thumb_wheel(
    device: &mut MxMaster,
    invert: bool,
    sensitivity: u8,
) -> Result<(), HidError> {
    let fi = get_feature_index(device, FEATURE_THUMB_WHEEL)?;
    let invert_byte = if invert { 0x01 } else { 0x00 };
    device.request(fi, 0x03, &[invert_byte, sensitivity])?;
    Ok(())
}

/// Remap a button via REPROG_CONTROLS_V4 (0x1b04), function 0x03 (setControlReport).
pub fn remap_button(
    device: &mut MxMaster,
    control_id: u16,
    action: &ButtonAction,
) -> Result<(), HidError> {
    let fi = get_feature_index(device, FEATURE_REPROG_CONTROLS_V4)?;

    let cid_hi = (control_id >> 8) as u8;
    let cid_lo = (control_id & 0xff) as u8;

    let mut params = [0u8; 16];
    params[0] = cid_hi;
    params[1] = cid_lo;

    match action {
        ButtonAction::Default => {
            // All zeros after control_id = reset to default
        }
        ButtonAction::Disabled => {
            params[2] = 0x03; // mark disabled
        }
        ButtonAction::KeyCombo(_) | ButtonAction::ContextCombo { .. } | ButtonAction::Exec(_) => {
            // Software-handled via evdev; keep device sending normal HID events
            params[2] = 0x00;
        }
        ButtonAction::ToggleScrollMode | ButtonAction::DpiUp | ButtonAction::DpiDown => {
            // Software-handled
            params[2] = 0x00;
        }
    }

    device.request_long(fi, 0x03, &params)?;
    Ok(())
}

/// Battery level and charging state from the UNIFIED_BATTERY feature.
pub struct BatteryStatus {
    /// Charge level 0–100. `None` if the device could not report it.
    pub level: Option<u8>,
    pub charging: bool,
    pub charge_complete: bool,
}

/// Read battery status via UNIFIED_BATTERY (0x1004), function 0x01 (getStatus).
#[allow(dead_code)]
pub fn get_battery_status(device: &mut MxMaster) -> Result<BatteryStatus, HidError> {
    let fi = get_feature_index(device, FEATURE_UNIFIED_BATTERY)?;
    let response = device.request(fi, 0x01, &[])?;
    // response[4] = level (0-100, 255 = unknown)
    // response[5] = charging status: 0=discharging 1=recharging 2=complete 3=error
    let level = if response[4] <= 100 {
        Some(response[4])
    } else {
        None
    };
    let charging = response[5] == 1 || response[5] == 4; // recharging or slow charge
    let charge_complete = response[5] == 2;
    Ok(BatteryStatus {
        level,
        charging,
        charge_complete,
    })
}

/// Control IDs for all remappable buttons on a given model.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ButtonLayout {
    pub gesture_button: u16,
    pub thumb_button: u16,
    pub back: u16,
    pub forward: u16,
    pub smart_shift: u16,
    /// Dedicated haptic button, present on the MX Master 4 only.
    pub haptic_button: Option<u16>,
}

/// Return the button control IDs for the given device model.
///
/// The MX Master 4 reports a different control set from the 2S/3/3S, verified
/// against `0x1b04 getControlIdInfo` on hardware: back and forward are the
/// generic `0x0053`/`0x0056` controls rather than the `0x00c5`/`0x00c6` pair,
/// SmartShift sits on `0x00c4`, and there is an extra haptic button at
/// `0x01a0` with no counterpart on earlier models.  Run `logi-tune --scan` to
/// print the control list a given device actually reports.
pub fn button_layout(model: DeviceModel) -> ButtonLayout {
    match model {
        DeviceModel::MxMaster4 => ButtonLayout {
            gesture_button: 0x00c3,
            thumb_button: 0x00c4,
            back: 0x0053,
            forward: 0x0056,
            smart_shift: 0x00c4,
            haptic_button: Some(0x01a0),
        },
        _ => ButtonLayout {
            gesture_button: 0x00c3,
            thumb_button: 0x00c4,
            back: 0x00c5,
            forward: 0x00c6,
            smart_shift: 0x00c7,
            haptic_button: None,
        },
    }
}

/// One entry of the device's HID++ feature table.
#[derive(Debug, Clone, Copy)]
pub struct FeatureEntry {
    pub index: u8,
    pub number: u16,
    pub version: u8,
    pub flags: u8,
}

impl FeatureEntry {
    /// Feature flags that mark a feature as not meant for normal use.
    pub fn flag_names(&self) -> Vec<&'static str> {
        let mut v = Vec::new();
        if self.flags & 0x80 != 0 {
            v.push("obsolete");
        }
        if self.flags & 0x40 != 0 {
            v.push("hidden");
        }
        if self.flags & 0x20 != 0 {
            v.push("engineering");
        }
        if self.flags & 0x10 != 0 {
            v.push("manufacturing");
        }
        v
    }
}

/// One entry of the device's `0x1b04` reprogrammable-control table.
#[derive(Debug, Clone, Copy)]
pub struct ControlEntry {
    pub control_id: u16,
    pub task_id: u16,
    pub flags: u8,
    pub group: u8,
    pub group_mask: u8,
    pub raw_xy: u8,
}

/// Everything `--scan` reports about a device.
pub struct DeviceScan {
    pub protocol: (u8, u8),
    pub features: Vec<FeatureEntry>,
    pub controls: Vec<ControlEntry>,
}

/// Enumerate the device's HID++ feature table and reprogrammable controls.
///
/// Read-only: every call is a getter.  This is the ground truth for adding a
/// new model, since the feature set differs between MX generations.
pub fn scan(device: &mut MxMaster) -> Result<DeviceScan, HidError> {
    // ROOT function 0x01 = getProtocolVersion; the 0xaa is an echo ping byte.
    let ping = device.request(0x00, 0x01, &[0x00, 0x00, 0xaa])?;
    let protocol = (ping[4], ping[5]);

    let fs = get_feature_index(device, FEATURE_FEATURE_SET)?;
    // FEATURE_SET function 0x00 = getCount (excludes ROOT).
    let count = device.request(fs, 0x00, &[])?[4];

    let mut features = Vec::with_capacity(count as usize);
    for index in 1..=count {
        // function 0x01 = getFeatureID
        let r = device.request(fs, 0x01, &[index])?;
        features.push(FeatureEntry {
            index,
            number: u16::from_be_bytes([r[4], r[5]]),
            flags: r[6],
            version: r[7],
        });
    }

    let mut controls = Vec::new();
    if let Ok(ci) = get_feature_index(device, FEATURE_REPROG_CONTROLS_V4) {
        // function 0x00 = getControlIdCount
        let n = device.request(ci, 0x00, &[])?[4];
        for i in 0..n {
            // function 0x01 = getControlIdInfo
            let r = device.request(ci, 0x01, &[i])?;
            controls.push(ControlEntry {
                control_id: u16::from_be_bytes([r[4], r[5]]),
                task_id: u16::from_be_bytes([r[6], r[7]]),
                flags: r[8],
                group: r[10],
                group_mask: r[11],
                raw_xy: r[12],
            });
        }
    }

    Ok(DeviceScan {
        protocol,
        features,
        controls,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mx4_layout_differs_from_earlier_models() {
        let mx4 = button_layout(DeviceModel::MxMaster4);
        let mx3s = button_layout(DeviceModel::MxMaster3S);
        assert_eq!(mx4.back, 0x0053);
        assert_eq!(mx4.forward, 0x0056);
        assert_eq!(mx4.haptic_button, Some(0x01a0));
        assert_eq!(mx3s.back, 0x00c5);
        assert_eq!(mx3s.haptic_button, None);
    }

    #[test]
    fn known_feature_names_resolve() {
        assert_eq!(feature_name(0x19b0), "HAPTIC_FEEDBACK");
        assert_eq!(feature_name(0x2111), "SMART_SHIFT_ENHANCED");
        assert_eq!(feature_name(0xffff), "?");
    }

    #[test]
    fn feature_flags_decode() {
        let e = FeatureEntry {
            index: 1,
            number: 0x1803,
            version: 1,
            flags: 0x70,
        };
        assert_eq!(
            e.flag_names(),
            vec!["hidden", "engineering", "manufacturing"]
        );
    }
}
