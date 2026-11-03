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
    /// The third (upper) button of the left-side cluster.
    ///
    /// The MX Master 4 has three buttons on its left side above the thumb:
    /// `back_button` and `forward_button` are the lower two, and this is the
    /// upper one. Earlier models have only two. Which HID++ control it
    /// corresponds to has not been verified.
    #[serde(skip_serializing_if = "is_default_action")]
    pub upper_side_button: ButtonAction,
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
    pub upper_side_button: u16,
}

impl EvdevCodes {
    /// The codes a given model sends.
    ///
    /// The MX Master 4 has three buttons in its left-side cluster rather than
    /// two, and does not map the thumb control to the same code as the 3S:
    /// codes 275, 276 and 277 are the side cluster bottom-to-top, and the
    /// thumb paddle is `BTN_BACK` (278). Confirmed with `--watch-buttons` on
    /// hardware.
    pub fn for_model(model: crate::hid::DeviceModel) -> Self {
        let base = Self {
            middle_button: 274,  // BTN_MIDDLE
            back_button: 275,    // BTN_SIDE
            forward_button: 276, // BTN_EXTRA
            thumb_button: 277,   // BTN_FORWARD
            // These models have only two side buttons.
            upper_side_button: EVDEV_CODE_UNMAPPED,
        };
        match model {
            crate::hid::DeviceModel::MxMaster4 => Self {
                thumb_button: 278,      // BTN_BACK, the thumb paddle
                upper_side_button: 277, // BTN_FORWARD, top of the side cluster
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
        } else if code == c.upper_side_button {
            Some(("upper_side_button", &self.upper_side_button))
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
            upper_side_button: ButtonAction::Default,
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

/// Highest SmartShift threshold the feature accepts. Values above this were
/// previously writable from the TUI, which had no upper bound.
pub const SMART_SHIFT_MAX: u8 = 50;
/// Highest haptic level, as a percentage.
pub const HAPTIC_LEVEL_MAX: u8 = 100;
/// Bounds on the software-only wheel and thumb settings.
pub const LINES_PER_NOTCH_RANGE: (u8, u8) = (1, 10);
pub const THUMB_SENSITIVITY_RANGE: (u8, u8) = (1, 10);

impl Config {
    /// Clamp out-of-range values, returning a description of each correction.
    ///
    /// Values are pushed to hardware, and until now nothing checked them: a
    /// `smart_shift_threshold` of 164 sat in a config file for weeks because
    /// the setting was broken, and became live the moment it was fixed. An
    /// out-of-range value must be corrected here rather than sent.
    pub fn validate(&mut self) -> Vec<String> {
        let mut fixed = Vec::new();

        let mut clamp = |name: &str, value: &mut u8, lo: u8, hi: u8| {
            let was = *value;
            let now = was.clamp(lo, hi);
            if now != was {
                *value = now;
                fixed.push(format!("{name}: {was} out of range {lo}-{hi}, using {now}"));
            }
        };

        clamp(
            "scroll.smart_shift_threshold",
            &mut self.scroll.smart_shift_threshold,
            0,
            SMART_SHIFT_MAX,
        );
        clamp(
            "scroll.lines_per_notch",
            &mut self.scroll.lines_per_notch,
            LINES_PER_NOTCH_RANGE.0,
            LINES_PER_NOTCH_RANGE.1,
        );
        clamp(
            "thumb_wheel.sensitivity",
            &mut self.thumb_wheel.sensitivity,
            THUMB_SENSITIVITY_RANGE.0,
            THUMB_SENSITIVITY_RANGE.1,
        );
        clamp(
            "haptics.level",
            &mut self.haptics.level,
            0,
            HAPTIC_LEVEL_MAX,
        );

        if self.dpi.profiles.is_empty() {
            self.dpi = DpiConfig::default();
            fixed.push("dpi.profiles was empty, restored defaults".to_string());
        }
        if self.dpi.active >= self.dpi.profiles.len() {
            let was = self.dpi.active;
            self.dpi.active = self.dpi.profiles.len() - 1;
            fixed.push(format!(
                "dpi.active: {was} past the end of dpi.profiles, using {}",
                self.dpi.active
            ));
        }

        fixed
    }

    /// Return the path to the config file.
    pub fn path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("logi-tune")
            .join("config.toml")
    }

    /// Load config from disk, falling back to `Default` if the file is missing.
    ///
    /// Out-of-range values are clamped and logged; see [`Config::validate`].
    pub fn load() -> anyhow::Result<Self> {
        let mut cfg = Self::load_unvalidated()?;
        for correction in cfg.validate() {
            tracing::warn!("config: {correction}");
        }
        Ok(cfg)
    }

    /// Load without clamping out-of-range values.
    ///
    /// Only for callers that want to report the corrections themselves, such
    /// as `--dry-run`; everything else should use [`Config::load`].
    pub fn load_unvalidated() -> anyhow::Result<Self> {
        let path = Self::path();
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = std::fs::read_to_string(&path)
            .with_context(|| format!("reading config from {}", path.display()))?;
        toml::from_str(&raw).with_context(|| format!("parsing config from {}", path.display()))
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
    fn mx4_side_cluster_has_three_buttons() {
        let mx4 = EvdevCodes::for_model(DeviceModel::MxMaster4);
        let mx3s = EvdevCodes::for_model(DeviceModel::MxMaster3S);

        // Confirmed with --watch-buttons on hardware: the MX Master 4's thumb
        // button is BTN_BACK, not BTN_FORWARD.
        assert_eq!(mx4.thumb_button, 278);
        assert_eq!(mx4.upper_side_button, 277);
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
            Some("upper_side_button")
        );
        // Left and right clicks are never intercepted.
        assert!(buttons.for_evdev_code(272, &codes).is_none());
        assert!(buttons.for_evdev_code(273, &codes).is_none());
    }

    #[test]
    fn unmapped_gesture_code_never_swallows_a_press() {
        let buttons = ButtonConfig::default();
        let codes = buttons.codes(DeviceModel::MxMaster3S);
        assert_eq!(codes.upper_side_button, EVDEV_CODE_UNMAPPED);
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

#[cfg(test)]
mod validate_tests {
    use super::*;

    #[test]
    fn clamps_an_out_of_range_smart_shift_threshold() {
        // 164 is the value that sat in a real config file and became live the
        // moment the SmartShift feature was fixed.
        let raw = r#"
            [scroll]
            smart_shift_threshold = 164
        "#;
        let mut cfg: Config = toml::from_str(raw).unwrap();
        let fixed = cfg.validate();

        assert_eq!(cfg.scroll.smart_shift_threshold, SMART_SHIFT_MAX);
        assert_eq!(fixed.len(), 1, "reports exactly what it corrected");
        assert!(fixed[0].contains("smart_shift_threshold"), "{:?}", fixed);
        assert!(fixed[0].contains("164"), "names the offending value");
    }

    #[test]
    fn a_valid_config_is_left_alone() {
        let mut cfg = Config::default();
        assert!(cfg.validate().is_empty());
        assert_eq!(cfg.scroll.smart_shift_threshold, 10);
    }

    #[test]
    fn clamps_every_bounded_field() {
        let raw = r#"
            [scroll]
            lines_per_notch = 99
            [thumb_wheel]
            sensitivity = 0
            [haptics]
            level = 200
        "#;
        let mut cfg: Config = toml::from_str(raw).unwrap();
        let fixed = cfg.validate();

        assert_eq!(cfg.scroll.lines_per_notch, LINES_PER_NOTCH_RANGE.1);
        assert_eq!(cfg.thumb_wheel.sensitivity, THUMB_SENSITIVITY_RANGE.0);
        assert_eq!(cfg.haptics.level, HAPTIC_LEVEL_MAX);
        assert_eq!(fixed.len(), 3);
    }

    #[test]
    fn repairs_a_dpi_index_past_the_end() {
        let raw = r#"
            [dpi]
            profiles = [800, 1600]
            active = 7
        "#;
        let mut cfg: Config = toml::from_str(raw).unwrap();
        let fixed = cfg.validate();

        assert_eq!(cfg.dpi.active, 1, "falls back to the last profile");
        assert_eq!(fixed.len(), 1);
        assert!(fixed[0].contains("dpi.active"));
    }

    #[test]
    fn restores_defaults_for_an_empty_profile_list() {
        let raw = r#"
            [dpi]
            profiles = []
            active = 0
        "#;
        let mut cfg: Config = toml::from_str(raw).unwrap();
        let fixed = cfg.validate();

        assert!(!cfg.dpi.profiles.is_empty());
        assert!(cfg.dpi.active < cfg.dpi.profiles.len());
        assert_eq!(fixed.len(), 1);
    }
}
