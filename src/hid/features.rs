//! HID++ 2.0 feature-index resolution and per-feature setter functions.

use crate::config::ButtonAction;

use super::{DeviceModel, HidError, MxMaster};
use super::protocol::{send_long, send_short};

const FEATURE_ROOT: u16 = 0x0000;
pub const FEATURE_REPROG_CONTROLS_V4: u16 = 0x1b04;
#[allow(dead_code)]
const FEATURE_UNIFIED_BATTERY: u16 = 0x1004;
const FEATURE_SMART_SHIFT: u16 = 0x2100;
const FEATURE_HI_RES_SCROLLING: u16 = 0x2120;
const FEATURE_THUMB_WHEEL: u16 = 0x2150;
const FEATURE_ADJUSTABLE_DPI: u16 = 0x2201;

/// Resolve a HID++ 2.0 feature number to its runtime index on this device.
pub fn get_feature_index(device: &mut MxMaster, feature_number: u16) -> Result<u8, HidError> {
    let hi = (feature_number >> 8) as u8;
    let lo = (feature_number & 0xff) as u8;

    let response = send_short(
        &mut device.file,
        device.device_index,
        // ROOT feature is always at index 0x00
        0x00,
        // function 0x00 (getFeature)
        0x00,
        [hi, lo, 0x00],
    )?;

    // response[4] = feature index (0x00 means not supported)
    let index = response[4];
    if index == 0x00 && feature_number != FEATURE_ROOT {
        return Err(HidError::Protocol(0x00));
    }
    Ok(index)
}

/// Set the active DPI via ADJUSTABLE_DPI (0x2201), function 0x03 (setDpiForSensor).
pub fn set_dpi(device: &mut MxMaster, dpi: u16) -> Result<(), HidError> {
    let fi = get_feature_index(device, FEATURE_ADJUSTABLE_DPI)?;
    let dpi_hi = (dpi >> 8) as u8;
    let dpi_lo = (dpi & 0xff) as u8;

    let mut params = [0u8; 16];
    params[0] = 0x00; // sensor index 0
    params[1] = dpi_hi;
    params[2] = dpi_lo;

    send_long(&mut device.file, device.device_index, fi, 0x03, params)?;
    Ok(())
}

/// Configure SmartShift via SMART_SHIFT (0x2100), function 0x01 (setSmartShift).
pub fn set_smart_shift(device: &mut MxMaster, threshold: u8) -> Result<(), HidError> {
    let fi = get_feature_index(device, FEATURE_SMART_SHIFT)?;

    // threshold 0 = always free-spin, 255 = always ratchet
    send_short(
        &mut device.file,
        device.device_index,
        fi,
        0x01,
        [threshold, 0x00, 0x00],
    )?;
    Ok(())
}

/// Configure hi-res scrolling via HI_RES_SCROLLING (0x2120), function 0x01.
pub fn set_hi_res_scroll(device: &mut MxMaster, multiplier: u8) -> Result<(), HidError> {
    let fi = get_feature_index(device, FEATURE_HI_RES_SCROLLING)?;
    send_short(
        &mut device.file,
        device.device_index,
        fi,
        0x01,
        [multiplier, 0x00, 0x00],
    )?;
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
    send_short(
        &mut device.file,
        device.device_index,
        fi,
        0x03,
        [invert_byte, sensitivity, 0x00],
    )?;
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

    send_long(&mut device.file, device.device_index, fi, 0x03, params)?;
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
    let response = send_short(&mut device.file, device.device_index, fi, 0x01, [0, 0, 0])?;
    // response[4] = level (0-100, 255 = unknown)
    // response[5] = charging status: 0=discharging 1=recharging 2=complete 3=error
    let level = if response[4] <= 100 { Some(response[4]) } else { None };
    let charging = response[5] == 1 || response[5] == 4; // recharging or slow charge
    let charge_complete = response[5] == 2;
    Ok(BatteryStatus { level, charging, charge_complete })
}

/// Control IDs for all remappable buttons on a given model.
#[derive(Clone, Copy)]
pub struct ButtonLayout {
    pub gesture_button: u16,
    pub thumb_button: u16,
    pub back: u16,
    pub forward: u16,
    pub smart_shift: u16,
}

/// Return the button control IDs for the given device model.
///
/// Control IDs are currently identical across the entire MX Master series;
/// the per-model dispatch is retained for forward compatibility.
pub fn button_layout(_model: DeviceModel) -> ButtonLayout {
    ButtonLayout {
        gesture_button: 0x00c3,
        thumb_button: 0x00c4,
        back: 0x00c5,
        forward: 0x00c6,
        smart_shift: 0x00c7,
    }
}
