//! HAPTIC_FEEDBACK (0x19b0): the MX Master 4's haptic motor.
//!
//! The feature carries four functions used here:
//!
//! | fn | name | payload |
//! |----|------|---------|
//! | `0x00` | getInfo | reply `[_, _, levelFlags, defaultLevel, waveformMask(4)]` |
//! | `0x01` | getLevel | reply `[enabled, level]` |
//! | `0x02` | setLevel | `[enabled, level]` |
//! | `0x04` | play | `[waveform]` |
//!
//! Verified against an MX Master 4 over Bluetooth, which reports a waveform
//! mask of `0x00007fff` — waveforms `0x00`–`0x0e`.

use super::{HidError, MxMaster};

pub const FEATURE_HAPTIC: u16 = 0x19b0;

/// Built-in haptic waveforms.
///
/// Values come from the HID++ 2.0 haptic waveform list. A device advertises
/// which of them it implements; see [`HapticInfo::supports`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Waveform {
    SharpStateChange = 0x00,
    DampStateChange = 0x01,
    SharpCollision = 0x02,
    DampCollision = 0x03,
    SubtleCollision = 0x04,
    HappyAlert = 0x05,
    AngryAlert = 0x06,
    Completed = 0x07,
    Square = 0x08,
    Wave = 0x09,
    Firework = 0x0a,
    Mad = 0x0b,
    Knock = 0x0c,
    Jingle = 0x0d,
    Ringing = 0x0e,
    WhisperCollision = 0x1b,
}

impl Waveform {
    pub const ALL: &'static [Waveform] = &[
        Waveform::SharpStateChange,
        Waveform::DampStateChange,
        Waveform::SharpCollision,
        Waveform::DampCollision,
        Waveform::SubtleCollision,
        Waveform::HappyAlert,
        Waveform::AngryAlert,
        Waveform::Completed,
        Waveform::Square,
        Waveform::Wave,
        Waveform::Firework,
        Waveform::Mad,
        Waveform::Knock,
        Waveform::Jingle,
        Waveform::Ringing,
        Waveform::WhisperCollision,
    ];

    pub fn id(self) -> u8 {
        self as u8
    }

    /// Lowercase, hyphenated name, as accepted on the command line.
    pub fn name(self) -> &'static str {
        match self {
            Waveform::SharpStateChange => "sharp-state-change",
            Waveform::DampStateChange => "damp-state-change",
            Waveform::SharpCollision => "sharp-collision",
            Waveform::DampCollision => "damp-collision",
            Waveform::SubtleCollision => "subtle-collision",
            Waveform::HappyAlert => "happy-alert",
            Waveform::AngryAlert => "angry-alert",
            Waveform::Completed => "completed",
            Waveform::Square => "square",
            Waveform::Wave => "wave",
            Waveform::Firework => "firework",
            Waveform::Mad => "mad",
            Waveform::Knock => "knock",
            Waveform::Jingle => "jingle",
            Waveform::Ringing => "ringing",
            Waveform::WhisperCollision => "whisper-collision",
        }
    }

    /// Parse a name, accepting either hyphens or underscores.
    pub fn parse(s: &str) -> Option<Waveform> {
        let want = s.trim().to_ascii_lowercase().replace('_', "-");
        Waveform::ALL.iter().copied().find(|w| w.name() == want)
    }
}

/// What a device's haptic motor supports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HapticInfo {
    /// Bit `n` set means waveform id `n` is implemented.
    pub waveform_mask: u32,
    /// Whether the level is restricted to coarse steps rather than 0–100.
    pub discrete_levels: bool,
    /// The device's own default level.
    pub default_level: u8,
}

impl HapticInfo {
    pub fn supports(&self, waveform: Waveform) -> bool {
        let id = waveform.id();
        // Only ids below 32 are representable in the mask.
        id < 32 && self.waveform_mask & (1 << id) != 0
    }

    /// Every waveform this device implements.
    pub fn supported(&self) -> Vec<Waveform> {
        Waveform::ALL
            .iter()
            .copied()
            .filter(|w| self.supports(*w))
            .collect()
    }
}

/// Current haptic level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HapticLevel {
    pub enabled: bool,
    /// Strength 0–100.
    pub level: u8,
}

/// Read the haptic motor's capabilities (function 0x00).
pub fn info(device: &mut MxMaster) -> Result<HapticInfo, HidError> {
    let fi = super::features::get_feature_index(device, FEATURE_HAPTIC)?;
    let r = device.request(fi, 0x00, &[])?;
    // Payload: [_, _, levelFlags, defaultLevel, waveformMask hi..lo]
    Ok(HapticInfo {
        waveform_mask: u32::from_be_bytes([r[8], r[9], r[10], r[11]]),
        discrete_levels: r[6] & 0x01 != 0,
        default_level: r[7],
    })
}

/// Read the current haptic level (function 0x01).
pub fn level(device: &mut MxMaster) -> Result<HapticLevel, HidError> {
    let fi = super::features::get_feature_index(device, FEATURE_HAPTIC)?;
    let r = device.request(fi, 0x01, &[])?;
    Ok(HapticLevel {
        enabled: r[4] & 0x01 != 0,
        level: r[5],
    })
}

/// Set the haptic level (function 0x02).
///
/// Disabling keeps a mid-range level in the register, matching what other
/// HID++ tools write, so re-enabling does not come back at zero strength.
pub fn set_level(device: &mut MxMaster, enabled: bool, level: u8) -> Result<(), HidError> {
    let fi = super::features::get_feature_index(device, FEATURE_HAPTIC)?;
    let params = if enabled {
        [0x01, level.min(100)]
    } else {
        [0x00, 50]
    };
    device.request(fi, 0x02, &params)?;
    Ok(())
}

/// Play a waveform on the haptic motor (function 0x04).
///
/// Returns [`HidError::WaveformUnsupported`] if the device does not advertise
/// the waveform, rather than sending a command the motor will ignore.
pub fn play(device: &mut MxMaster, waveform: Waveform) -> Result<(), HidError> {
    let fi = super::features::get_feature_index(device, FEATURE_HAPTIC)?;
    if !info(device)?.supports(waveform) {
        return Err(HidError::WaveformUnsupported(waveform.name()));
    }
    device.request(fi, 0x04, &[waveform.id()])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mx4_waveform_mask_decodes() {
        // getInfo payload read from an MX Master 4: 00 01 00 3c 00 00 7f ff
        let info = HapticInfo {
            waveform_mask: 0x00007fff,
            discrete_levels: false,
            default_level: 0x3c,
        };
        assert_eq!(info.default_level, 60);
        // Waveforms 0x00-0x0e are advertised; whisper-collision (0x1b) is not.
        assert!(info.supports(Waveform::SharpStateChange));
        assert!(info.supports(Waveform::Knock));
        assert!(info.supports(Waveform::Ringing));
        assert!(!info.supports(Waveform::WhisperCollision));
        assert_eq!(info.supported().len(), 15);
    }

    #[test]
    fn waveform_names_round_trip() {
        for w in Waveform::ALL {
            assert_eq!(Waveform::parse(w.name()), Some(*w), "{}", w.name());
        }
        // Underscores and case are accepted, as Solaar spells them.
        assert_eq!(Waveform::parse("HAPPY_ALERT"), Some(Waveform::HappyAlert));
        assert_eq!(Waveform::parse("  knock "), Some(Waveform::Knock));
        assert_eq!(Waveform::parse("nope"), None);
    }

    #[test]
    fn waveform_ids_match_the_hidpp_list() {
        assert_eq!(Waveform::SharpStateChange.id(), 0x00);
        assert_eq!(Waveform::HappyAlert.id(), 0x05);
        assert_eq!(Waveform::Knock.id(), 0x0c);
        assert_eq!(Waveform::Ringing.id(), 0x0e);
        assert_eq!(Waveform::WhisperCollision.id(), 0x1b);
    }

    #[test]
    fn empty_mask_supports_nothing() {
        let info = HapticInfo {
            waveform_mask: 0,
            discrete_levels: true,
            default_level: 0,
        };
        assert!(info.supported().is_empty());
    }
}
