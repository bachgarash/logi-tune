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
    /// The MX Master 4's second thumb control.
    ///
    /// This was previously assumed not to be capturable via evdev on
    /// Bluetooth and so was applied over HID++ only. It is: the MX Master 4
    /// emits an evdev code for all seven of its buttons.
    #[serde(skip_serializing_if = "is_default_action")]
    pub gesture_button: ButtonAction,
    #[serde(skip_serializing_if = "is_default_action")]
    pub smart_shift: ButtonAction,
    /// Which evdev code each physical button sends on this device.
    pub evdev_codes: EvdevCodes,
}

fn is_default_action(a: &ButtonAction) -> bool {
    matches!(a, ButtonAction::Default)
}

/// Which evdev `EV_KEY` code each physical button sends.
///
/// These differ between MX models — the MX Master 4 declares codes 272–278
/// and does not use the same assignment as the 3S — and the kernel's generic
/// names (`BTN_SIDE`, `BTN_BACK`, …) do not say which physical button a mouse
/// wires to them. So this is configuration, not a hardcoded per-model table:
/// run `logi-tune --watch-buttons`, press each button, and put the codes here.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct EvdevCodes {
    pub middle_button: u16,
    pub back_button: u16,
    pub forward_button: u16,
    pub thumb_button: u16,
    pub gesture_button: u16,
}

impl Default for EvdevCodes {
    fn default() -> Self {
        // BTN_MIDDLE, BTN_SIDE, BTN_EXTRA, BTN_FORWARD, BTN_BACK.
        //
        // Confirmed with --watch-buttons on an MX Master 4, which emits all
        // five; the first four match the MX Master 3S for Mac. 278 was
        // previously claimed by no field, so that button did nothing.
        Self {
            middle_button: 274,
            back_button: 275,
            forward_button: 276,
            thumb_button: 277,
            gesture_button: 278,
        }
    }
}

impl ButtonConfig {
    /// The action bound to an evdev button code, and the config field it came
    /// from.
    ///
    /// Returns `None` when no field claims the code, which is how an
    /// unconfigured button reaches here.
    pub fn for_evdev_code(&self, code: u16) -> Option<(&'static str, &ButtonAction)> {
        let c = &self.evdev_codes;
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
        } else if code == c.gesture_button {
            Some(("gesture_button", &self.gesture_button))
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
            smart_shift: ButtonAction::Default,
            evdev_codes: EvdevCodes::default(),
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

    fn with_thumb(combo: &str) -> ButtonConfig {
        ButtonConfig {
            thumb_button: ButtonAction::KeyCombo(combo.into()),
            ..Default::default()
        }
    }

    #[test]
    fn evdev_codes_resolve_to_their_fields() {
        let buttons = with_thumb("super");

        let (field, action) = buttons
            .for_evdev_code(277)
            .expect("277 is the thumb button");
        assert_eq!(field, "thumb_button");
        assert_eq!(action, &ButtonAction::KeyCombo("super".into()));

        // All seven MX Master 4 codes now resolve; 278 was previously
        // claimed by no field, which is why that button did nothing.
        assert_eq!(
            buttons.for_evdev_code(278).map(|(f, _)| f),
            Some("gesture_button")
        );
        // Left and right clicks are deliberately never intercepted.
        assert!(buttons.for_evdev_code(272).is_none());
        assert!(buttons.for_evdev_code(273).is_none());
    }

    #[test]
    fn remapping_a_code_moves_the_binding() {
        let buttons = ButtonConfig {
            evdev_codes: EvdevCodes {
                thumb_button: 279,
                ..Default::default()
            },
            ..with_thumb("super")
        };

        assert_eq!(
            buttons.for_evdev_code(279).map(|(f, _)| f),
            Some("thumb_button")
        );
        assert!(buttons.for_evdev_code(277).is_none());
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
        assert_eq!(cfg.buttons.evdev_codes, EvdevCodes::default());
        assert_eq!(
            cfg.buttons.for_evdev_code(277).map(|(f, _)| f),
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
