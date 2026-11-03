//! Configuration structs, serialisation, and persistence for logi-tune.

use std::fmt;
use std::path::PathBuf;

use anyhow::Context;
use serde::{Deserialize, Serialize};

/// The action assigned to a mouse button.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub enum ButtonAction {
    #[default]
    Default,
    KeyCombo(String),
    /// Context-aware combo: uses `terminal` when the focused window is a
    /// terminal emulator, `default` otherwise.
    ContextCombo {
        default: String,
        terminal: String,
    },
    Exec(String),
    ToggleScrollMode,
    DpiUp,
    DpiDown,
    Disabled,
}

impl fmt::Display for ButtonAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ButtonAction::Default => write!(f, "Default"),
            ButtonAction::KeyCombo(s) => write!(f, "KeyCombo({})", s),
            ButtonAction::ContextCombo { default, terminal } => {
                write!(f, "Smart({} / term:{})", default, terminal)
            }
            ButtonAction::Exec(s) => write!(f, "Exec({})", s),
            ButtonAction::ToggleScrollMode => write!(f, "Toggle Scroll Mode"),
            ButtonAction::DpiUp => write!(f, "DPI Up"),
            ButtonAction::DpiDown => write!(f, "DPI Down"),
            ButtonAction::Disabled => write!(f, "Disabled"),
        }
    }
}

/// Per-button action assignments.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ButtonConfig {
    pub middle_button: ButtonAction,
    pub back_button: ButtonAction,
    pub forward_button: ButtonAction,
    pub thumb_button: ButtonAction,
    /// The thumb paddle, as applied over HID++ (control 0x00c3).
    ///
    /// On the MX Master 4 this is the same physical button as
    /// `thumb_button`, which is the one bound through evdev.
    #[serde(skip_serializing_if = "is_default_action")]
    pub gesture_button: ButtonAction,
    /// The haptic button, on the side above the thumb rest.
    ///
    /// MX Master 4 only; it is HID++ control 0x01a0 and evdev BTN_FORWARD.
    #[serde(skip_serializing_if = "is_default_action")]
    pub haptic_button: ButtonAction,
    #[serde(skip_serializing_if = "is_default_action")]
    pub smart_shift: ButtonAction,
    /// Which evdev code each physical button sends.
    ///
    /// `None` means "use the detected model's defaults", which is what most
    /// users want; set it only to override.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evdev_codes: Option<EvdevCodes>,
}

fn is_default_action(a: &ButtonAction) -> bool {
    matches!(a, ButtonAction::Default)
}

/// Code meaning "this button is not mapped".
///
/// `0` is `KEY_RESERVED`, which no button ever sends, so it is safe as a
/// sentinel for a field we have no verified code for.
pub const EVDEV_CODE_UNMAPPED: u16 = 0;

/// Which evdev `EV_KEY` code each physical button sends.
///
/// The kernel's generic names (`BTN_SIDE`, `BTN_BACK`, …) do not say which
/// physical button a mouse wires to them, and the assignment differs between
/// MX models. Defaults come from [`EvdevCodes::for_model`]; override any of
/// them in the config file after checking with `logi-tune --watch-buttons`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct EvdevCodes {
    pub middle_button: u16,
    pub back_button: u16,
    pub forward_button: u16,
    pub thumb_button: u16,
    pub haptic_button: u16,
}

impl EvdevCodes {
    /// The codes a given model sends.
    ///
    /// The MX Master 4 has one button more than the 3S and does not map the
    /// thumb control to the same code: its thumb paddle is `BTN_BACK` (278),
    /// and `BTN_FORWARD` (277) is the haptic button on the side above the
    /// thumb rest. Both confirmed with `--watch-buttons` on hardware.
    pub fn for_model(model: crate::hid::DeviceModel) -> Self {
        let base = Self {
            middle_button: 274,  // BTN_MIDDLE
            back_button: 275,    // BTN_SIDE
            forward_button: 276, // BTN_EXTRA
            thumb_button: 277,   // BTN_FORWARD
            // Earlier models have no haptic button at all.
            haptic_button: EVDEV_CODE_UNMAPPED,
        };
        match model {
            crate::hid::DeviceModel::MxMaster4 => Self {
                thumb_button: 278,  // BTN_BACK, the thumb paddle
                haptic_button: 277, // BTN_FORWARD, above the thumb rest
                ..base
            },
            _ => base,
        }
    }
}

impl Default for EvdevCodes {
    /// Codes for the MX Master 3S for Mac, the model these were first
    /// confirmed on. Prefer [`EvdevCodes::for_model`] once the device is known.
    fn default() -> Self {
        Self::for_model(crate::hid::DeviceModel::MxMaster3SForMac)
    }
}

impl ButtonConfig {
    /// The codes in effect: the config's own, else the model's defaults.
    pub fn codes(&self, model: crate::hid::DeviceModel) -> EvdevCodes {
        self.evdev_codes
            .clone()
            .unwrap_or_else(|| EvdevCodes::for_model(model))
    }

    /// The action bound to an evdev button code, and the config field it came
    /// from.
    ///
    /// Returns `None` when no field claims the code, which is how an
    /// unconfigured button reaches here.
    pub fn for_evdev_code(
        &self,
        code: u16,
        c: &EvdevCodes,
    ) -> Option<(&'static str, &ButtonAction)> {
        // A button we have no code for must never swallow a real press.
        if code == EVDEV_CODE_UNMAPPED {
            return None;
        }
        // Checked in order, so an ambiguous config resolves predictably
        // rather than depending on map iteration.
        if code == c.middle_button {
            Some(("middle_button", &self.middle_button))
        } else if code == c.back_button {
            Some(("back_button", &self.back_button))
        } else if code == c.forward_button {
            Some(("forward_button", &self.forward_button))
        } else if code == c.thumb_button {
            Some(("thumb_button", &self.thumb_button))
        } else if code == c.haptic_button {
            Some(("haptic_button", &self.haptic_button))
        } else {
            None
        }
    }
}

impl Default for ButtonConfig {
    fn default() -> Self {
        Self {
            middle_button: ButtonAction::Default,
            back_button: ButtonAction::Default,
            forward_button: ButtonAction::Default,
            thumb_button: ButtonAction::Default,
            gesture_button: ButtonAction::Default,
            haptic_button: ButtonAction::Default,
            smart_shift: ButtonAction::Default,
            evdev_codes: None,
        }
    }
}

/// Scroll wheel settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ScrollConfig {
    /// Software-only: no HID++ feature on the MX series exposes this.
    pub lines_per_notch: u8,
    /// Applied in hardware via HIRES_WHEEL (0x2121) on the MX Master 4.
    pub invert: bool,
    pub smart_shift_threshold: u8,
    /// High-resolution (smooth) scrolling. Defaults to on, which is how the
    /// hardware ships.
    pub hi_res: bool,
    /// Only used by the legacy HI_RES_SCROLLING (0x2120) feature, which took a
    /// multiplier instead of an on/off mode. Ignored on devices that report
    /// HIRES_WHEEL (0x2121).
    pub hi_res_multiplier: u8,
}

impl Default for ScrollConfig {
    fn default() -> Self {
        Self {
            lines_per_notch: 3,
            invert: false,
            smart_shift_threshold: 10,
            hi_res: true,
            hi_res_multiplier: 1,
        }
    }
}

/// Horizontal thumb wheel settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ThumbWheelConfig {
    /// Applied in hardware via THUMB_WHEEL (0x2150).
    pub invert: bool,
    /// Software-only: THUMB_WHEEL (0x2150) exposes no sensitivity field.
    pub sensitivity: u8,
}

impl Default for ThumbWheelConfig {
    fn default() -> Self {
        Self {
            invert: false,
            sensitivity: 5,
        }
    }
}

/// DPI profile list and active slot.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DpiConfig {
    pub profiles: Vec<u16>,
    pub active: usize,
}

impl Default for DpiConfig {
    fn default() -> Self {
        Self {
            profiles: vec![400, 800, 1600, 3200, 6400],
            active: 2,
        }
    }
}

/// Haptic motor settings. Only the MX Master 4 has one.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct HapticsConfig {
    pub enabled: bool,
    /// Strength 0–100.
    pub level: u8,
}

impl Default for HapticsConfig {
    fn default() -> Self {
        // 60 is the level the MX Master 4 ships with.
        Self {
            enabled: true,
            level: 60,
        }
    }
}

/// Root configuration struct.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub buttons: ButtonConfig,
    pub scroll: ScrollConfig,
    pub thumb_wheel: ThumbWheelConfig,
    pub dpi: DpiConfig,
    pub haptics: HapticsConfig,
}

impl Config {
    /// Return the path to the config file.
    pub fn path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("logi-tune")
            .join("config.toml")
    }

    /// Load config from disk, falling back to `Default` if the file is missing.
    pub fn load() -> anyhow::Result<Self> {
        let path = Self::path();
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = std::fs::read_to_string(&path)
            .with_context(|| format!("reading config from {}", path.display()))?;
        let cfg: Self = toml::from_str(&raw)
            .with_context(|| format!("parsing config from {}", path.display()))?;
        Ok(cfg)
    }

    /// Persist config to disk, creating parent directories as needed.
    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating config dir {}", parent.display()))?;
        }
        let raw = toml::to_string_pretty(self).context("serialising config to TOML")?;
        std::fs::write(&path, raw)
            .with_context(|| format!("writing config to {}", path.display()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hid::DeviceModel;

    fn with_thumb(combo: &str) -> ButtonConfig {
        ButtonConfig {
            thumb_button: ButtonAction::KeyCombo(combo.into()),
            ..Default::default()
        }
    }

    #[test]
    fn mx4_thumb_and_haptic_codes_differ_from_3s() {
        let mx4 = EvdevCodes::for_model(DeviceModel::MxMaster4);
        let mx3s = EvdevCodes::for_model(DeviceModel::MxMaster3S);

        // Confirmed with --watch-buttons on hardware: the MX Master 4's thumb
        // button is BTN_BACK, not BTN_FORWARD.
        assert_eq!(mx4.thumb_button, 278);
        assert_eq!(mx4.haptic_button, 277);
        assert_eq!(mx3s.thumb_button, 277);

        // The first three are the same on both.
        assert_eq!(mx4.middle_button, mx3s.middle_button);
        assert_eq!(mx4.back_button, mx3s.back_button);
        assert_eq!(mx4.forward_button, mx3s.forward_button);
    }

    #[test]
    fn model_defaults_bind_the_mx4_thumb_button() {
        let buttons = with_thumb("super");
        let codes = buttons.codes(DeviceModel::MxMaster4);

        let (field, action) = buttons
            .for_evdev_code(278, &codes)
            .expect("278 is the MX Master 4 thumb button");
        assert_eq!(field, "thumb_button");
        assert_eq!(action, &ButtonAction::KeyCombo("super".into()));

        assert_eq!(
            buttons.for_evdev_code(277, &codes).map(|(f, _)| f),
            Some("haptic_button")
        );
        // Left and right clicks are never intercepted.
        assert!(buttons.for_evdev_code(272, &codes).is_none());
        assert!(buttons.for_evdev_code(273, &codes).is_none());
    }

    #[test]
    fn unmapped_gesture_code_never_swallows_a_press() {
        let buttons = ButtonConfig::default();
        let codes = buttons.codes(DeviceModel::MxMaster3S);
        assert_eq!(codes.haptic_button, EVDEV_CODE_UNMAPPED);
        // Code 0 must not match the unmapped field.
        assert!(buttons.for_evdev_code(0, &codes).is_none());
        assert!(buttons.for_evdev_code(278, &codes).is_none());
    }

    #[test]
    fn explicit_codes_override_the_model_defaults() {
        let buttons = ButtonConfig {
            evdev_codes: Some(EvdevCodes {
                thumb_button: 279, // BTN_TASK, claimed by nothing else
                ..EvdevCodes::for_model(DeviceModel::MxMaster4)
            }),
            ..with_thumb("super")
        };
        let codes = buttons.codes(DeviceModel::MxMaster4);
        assert_eq!(
            buttons.for_evdev_code(279, &codes).map(|(f, _)| f),
            Some("thumb_button")
        );
        // The model default for the thumb button no longer applies.
        assert_eq!(
            buttons.for_evdev_code(278, &codes).map(|(f, _)| f),
            None,
            "278 should be free once the thumb button moves"
        );
    }

    #[test]
    fn duplicate_codes_resolve_in_field_order() {
        // Two fields claiming one code is a user error, but it must resolve
        // deterministically rather than depending on iteration order.
        let buttons = ButtonConfig {
            evdev_codes: Some(EvdevCodes {
                thumb_button: 275, // same as back_button
                ..EvdevCodes::for_model(DeviceModel::MxMaster4)
            }),
            ..with_thumb("super")
        };
        let codes = buttons.codes(DeviceModel::MxMaster4);
        assert_eq!(
            buttons.for_evdev_code(275, &codes).map(|(f, _)| f),
            Some("back_button"),
            "earlier field wins"
        );
    }

    #[test]
    fn existing_config_files_load_without_evdev_codes() {
        // A file written before [buttons.evdev_codes] existed.
        let raw = r#"
            [buttons]
            middle_button = "Default"

            [buttons.thumb_button]
            KeyCombo = "super"
        "#;
        let cfg: Config = toml::from_str(raw).expect("old config still parses");
        assert!(cfg.buttons.evdev_codes.is_none(), "falls back to the model");
        let codes = cfg.buttons.codes(DeviceModel::MxMaster4);
        assert_eq!(
            cfg.buttons.for_evdev_code(278, &codes).map(|(f, _)| f),
            Some("thumb_button")
        );
        // Defaults for sections the file omits entirely.
        assert!(cfg.haptics.enabled);
    }
}
