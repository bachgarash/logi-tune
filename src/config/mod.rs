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
