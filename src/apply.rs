//! Pushing a [`Config`] to the device over HID++.
//!
//! This lives outside the TUI because the daemon needs it too: settings like
//! DPI, scroll mode and haptic level are device state, and nothing else
//! restores them when the mouse reconnects or the daemon restarts.

use crate::config::Config;
use crate::hid::features::{
    button_layout, current_dpi, current_scroll_mode, current_smart_shift, current_thumb_invert,
    dpi_range, remap_button, set_dpi, set_scroll_mode, set_smart_shift, set_thumb_wheel,
};
use crate::hid::haptics::{level as haptic_level, set_level as set_haptic_level};
use crate::hid::{HidError, MxMaster};

/// One setting's current device value against what the config asks for.
pub struct Planned {
    pub setting: &'static str,
    pub current: String,
    pub desired: String,
}

impl Planned {
    pub fn differs(&self) -> bool {
        self.current != self.desired
    }
}

/// Read what the device currently reports for each setting the config drives.
///
/// Read-only. This exists so an apply can say what it is about to change, and
/// skip what it does not need to: these values are persistent device state,
/// and a config file can hold a value that has never been applied.
pub fn plan(device: &mut MxMaster, cfg: &Config) -> Vec<Planned> {
    fn show<T: std::fmt::Display, E>(r: Result<T, E>) -> String {
        r.map(|v| v.to_string())
            .unwrap_or_else(|_| "unavailable".to_string())
    }

    let mut out = Vec::new();

    if let Some(&dpi) = cfg.dpi.profiles.get(cfg.dpi.active) {
        let desired = match dpi_range(device) {
            Ok(range) => range.clamp(dpi),
            Err(_) => dpi,
        };
        out.push(Planned {
            setting: "dpi",
            current: show(current_dpi(device)),
            desired: desired.to_string(),
        });
    }

    out.push(Planned {
        setting: "scroll.smart_shift_threshold",
        current: show(current_smart_shift(device).map(|(_, threshold)| threshold)),
        desired: cfg.scroll.smart_shift_threshold.to_string(),
    });

    let scroll = current_scroll_mode(device);
    out.push(Planned {
        setting: "scroll.hi_res",
        current: show(scroll.as_ref().map(|(hi_res, _)| *hi_res).map_err(|_| ())),
        desired: cfg.scroll.hi_res.to_string(),
    });
    out.push(Planned {
        setting: "scroll.invert",
        current: show(scroll.as_ref().map(|(_, invert)| *invert).map_err(|_| ())),
        desired: cfg.scroll.invert.to_string(),
    });
    out.push(Planned {
        setting: "thumb_wheel.invert",
        current: show(current_thumb_invert(device)),
        desired: cfg.thumb_wheel.invert.to_string(),
    });
    out.push(Planned {
        setting: "haptics.level",
        current: show(haptic_level(device).map(|l| {
            if l.enabled {
                l.level.to_string()
            } else {
                "off".to_string()
            }
        })),
        desired: if cfg.haptics.enabled {
            cfg.haptics.level.to_string()
        } else {
            "off".to_string()
        },
    });

    out
}

/// Apply every HID++-backed setting in `cfg` to `device`.
///
/// A setting the device does not implement is logged and skipped rather than
/// aborting the rest: the MX generations do not share a feature set, and
/// button mappings work through evdev regardless of HID++.
pub fn apply_config_to_device(device: &mut MxMaster, cfg: &Config) -> Result<(), HidError> {
    // Report what is about to change before changing it. These are persistent
    // device settings, so an apply that silently overwrites them is hard to
    // account for after the fact.
    for p in plan(device, cfg) {
        if p.differs() {
            tracing::info!("{}: {} -> {}", p.setting, p.current, p.desired);
        }
    }

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
