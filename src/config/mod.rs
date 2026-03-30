//! Configuration structs, serialisation, and persistence for logi-tune.

use std::fmt;
use std::path::PathBuf;

use anyhow::Context;
use serde::{Deserialize, Serialize};

/// The action assigned to a mouse button.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ButtonAction {
    Default,
    KeyCombo(String),
    /// Context-aware combo: uses `terminal` when the focused window is a
    /// terminal emulator, `default` otherwise.
    ContextCombo { default: String, terminal: String },
    Exec(String),
    ToggleScrollMode,
    DpiUp,
    DpiDown,
    Disabled,
}

impl Default for ButtonAction {
    fn default() -> Self {
        ButtonAction::Default
    }
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
    // Kept for config file compatibility and HID++ apply; not capturable via evdev on BT
    #[serde(skip_serializing_if = "is_default_action")]
    pub gesture_button: ButtonAction,
    #[serde(skip_serializing_if = "is_default_action")]
    pub smart_shift: ButtonAction,
}

fn is_default_action(a: &ButtonAction) -> bool {
    matches!(a, ButtonAction::Default)
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
        }
    }
}

/// Scroll wheel settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ScrollConfig {
    pub lines_per_notch: u8,
    pub invert: bool,
    pub smart_shift_threshold: u8,
    pub hi_res_multiplier: u8,
}

impl Default for ScrollConfig {
    fn default() -> Self {
        Self {
            lines_per_notch: 3,
            invert: false,
            smart_shift_threshold: 10,
            hi_res_multiplier: 1,
        }
    }
}

/// Horizontal thumb wheel settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ThumbWheelConfig {
    pub invert: bool,
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

/// Root configuration struct.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub buttons: ButtonConfig,
    pub scroll: ScrollConfig,
    pub thumb_wheel: ThumbWheelConfig,
    pub dpi: DpiConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            buttons: ButtonConfig::default(),
            scroll: ScrollConfig::default(),
            thumb_wheel: ThumbWheelConfig::default(),
            dpi: DpiConfig::default(),
        }
    }
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
