//! Pushing a [`Config`] to the device over HID++.
//!
//! This lives outside the TUI because the daemon needs it too: settings like
//! DPI, scroll mode and haptic level are device state, and nothing else
//! restores them when the mouse reconnects or the daemon restarts.

use crate::config::Config;
use crate::hid::features::{
    button_layout, dpi_range, remap_button, set_dpi, set_scroll_mode, set_smart_shift,
    set_thumb_wheel,
};
use crate::hid::haptics::set_level as set_haptic_level;
use crate::hid::{HidError, MxMaster};

/// Apply every HID++-backed setting in `cfg` to `device`.
///
/// A setting the device does not implement is logged and skipped rather than
/// aborting the rest: the MX generations do not share a feature set, and
/// button mappings work through evdev regardless of HID++.
pub fn apply_config_to_device(device: &mut MxMaster, cfg: &Config) -> Result<(), HidError> {
    macro_rules! hidpp {
        ($e:expr) => {
            match $e {
                Err(
                    e @ (HidError::Protocol(_)
                    | HidError::Io(_)
                    | HidError::Timeout
                    | HidError::FeatureMissing(_)
                    | HidError::WaveformUnsupported(_)),
                ) => {
                    tracing::warn!("HID++ setting skipped: {e}");
                }
                result => result?,
            }
        };
    }

    if let Some(&dpi) = cfg.dpi.profiles.get(cfg.dpi.active) {
        // Clamp to what the sensor accepts; an out-of-range value is rejected
        // outright, so the whole DPI setting would otherwise be lost.
        let dpi = match dpi_range(device) {
            Ok(range) => range.clamp(dpi),
            Err(_) => dpi,
        };
        hidpp!(set_dpi(device, dpi));
    }
    hidpp!(set_smart_shift(device, cfg.scroll.smart_shift_threshold));
    hidpp!(set_scroll_mode(
        device,
        cfg.scroll.hi_res,
        cfg.scroll.invert,
        cfg.scroll.hi_res_multiplier
    ));
    hidpp!(set_thumb_wheel(device, cfg.thumb_wheel.invert));
    // Devices without a haptic motor report no 0x19b0 and are skipped.
    hidpp!(set_haptic_level(
        device,
        cfg.haptics.enabled,
        cfg.haptics.level
    ));

    let layout = button_layout(device.model);
    hidpp!(remap_button(
        device,
        layout.gesture_button,
        &cfg.buttons.gesture_button
    ));
    hidpp!(remap_button(
        device,
        layout.thumb_button,
        &cfg.buttons.thumb_button
    ));
    hidpp!(remap_button(device, layout.back, &cfg.buttons.back_button));
    hidpp!(remap_button(
        device,
        layout.forward,
        &cfg.buttons.forward_button
    ));
    hidpp!(remap_button(
        device,
        layout.smart_shift,
        &cfg.buttons.smart_shift
    ));

    Ok(())
}
