//! uinput virtual keyboard for injecting key combos.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::unix::io::AsRawFd;

use anyhow::{Context, Result};

// ---------------------------------------------------------------------------
// Event and device constants
// ---------------------------------------------------------------------------

const EV_SYN: u16 = 0x00;
const EV_KEY: u16 = 0x01;
const SYN_REPORT: u16 = 0;
const BUS_VIRTUAL: u16 = 0x06;

// ---------------------------------------------------------------------------
// C structs (must match kernel layout on 64-bit Linux)
// ---------------------------------------------------------------------------

#[repr(C)]
struct InputId {
    bustype: u16,
    vendor: u16,
    product: u16,
    version: u16,
}

#[repr(C)]
struct UinputSetup {
    id: InputId,
    name: [u8; 80], // UINPUT_MAX_NAME_SIZE
    ff_effects_max: u32,
}

/// input_event on 64-bit Linux: two i64 (sec, usec), u16 type, u16 code, i32 value.
#[repr(C)]
struct InputEvent {
    sec: i64,
    usec: i64,
    type_: u16,
    code: u16,
    value: i32,
}

// ---------------------------------------------------------------------------
// ioctl definitions (linux/uinput.h)
// ---------------------------------------------------------------------------

nix::ioctl_write_int!(ui_set_evbit, b'U', 100);
nix::ioctl_write_int!(ui_set_keybit, b'U', 101);
nix::ioctl_write_int!(ui_set_relbit, b'U', 102);
nix::ioctl_write_int!(ui_set_mscbit, b'U', 105);
nix::ioctl_write_ptr!(ui_dev_setup, b'U', 3, UinputSetup);
nix::ioctl_none!(ui_dev_create, b'U', 1);
nix::ioctl_none!(ui_dev_destroy, b'U', 2);

// ---------------------------------------------------------------------------
// UinputKeyboard
// ---------------------------------------------------------------------------

pub struct UinputKeyboard {
    file: File,
}

impl UinputKeyboard {
    pub fn new() -> Result<Self> {
        let file = OpenOptions::new()
            .write(true)
            .open("/dev/uinput")
            .context("open /dev/uinput (need write access — are you in the input group?)")?;

        let fd = file.as_raw_fd();

        unsafe {
            ui_set_evbit(fd, EV_KEY as u64).context("UI_SET_EVBIT EV_KEY")?;
            ui_set_evbit(fd, EV_SYN as u64).context("UI_SET_EVBIT EV_SYN")?;
            // Enable all standard key codes
            for code in 1u16..=255 {
                let _ = ui_set_keybit(fd, code as u64);
            }
            let mut setup = UinputSetup {
                id: InputId { bustype: BUS_VIRTUAL, vendor: 0, product: 0, version: 1 },
                name: [0u8; 80],
                ff_effects_max: 0,
            };
            let name = b"logi-tune virtual keyboard";
            setup.name[..name.len()].copy_from_slice(name);
            ui_dev_setup(fd, &setup).context("UI_DEV_SETUP")?;
            ui_dev_create(fd).context("UI_DEV_CREATE")?;
        }

        // Give the kernel a moment to register the virtual device
        std::thread::sleep(std::time::Duration::from_millis(100));

        Ok(Self { file })
    }

    /// Inject a key combo, e.g. `"super"`, `"ctrl+c"`, `"super+left"`.
    pub fn inject_combo(&mut self, combo: &str) -> Result<()> {
        let keys: Vec<u16> = combo
            .split('+')
            .filter_map(|part| key_name_to_code(part.trim()))
            .collect();

        if keys.is_empty() {
            anyhow::bail!("unknown key combo: {combo}");
        }

        // Press all keys in order, then release in reverse
        for &k in &keys {
            self.write_event(EV_KEY, k, 1)?;
        }
        self.write_event(EV_SYN, SYN_REPORT, 0)?;

        for &k in keys.iter().rev() {
            self.write_event(EV_KEY, k, 0)?;
        }
        self.write_event(EV_SYN, SYN_REPORT, 0)?;

        Ok(())
    }

    fn write_event(&mut self, type_: u16, code: u16, value: i32) -> Result<()> {
        let ev = InputEvent { sec: 0, usec: 0, type_, code, value };
        let bytes: &[u8] = unsafe {
            std::slice::from_raw_parts(
                &ev as *const InputEvent as *const u8,
                std::mem::size_of::<InputEvent>(),
            )
        };
        self.file.write_all(bytes).context("write to /dev/uinput")?;
        Ok(())
    }
}

impl Drop for UinputKeyboard {
    fn drop(&mut self) {
        unsafe {
            let _ = ui_dev_destroy(self.file.as_raw_fd());
        }
    }
}

// ---------------------------------------------------------------------------
// UinputMouse — pass-through virtual mouse
// ---------------------------------------------------------------------------

const EV_REL: u16 = 0x02;
const EV_MSC: u16 = 0x04;

pub struct UinputMouse {
    file: File,
}

impl UinputMouse {
    /// Create a virtual mouse that mirrors the physical mouse's capabilities.
    /// Used to pass through events we don't intercept after exclusive grabbing.
    pub fn new() -> Result<Self> {
        let file = OpenOptions::new()
            .write(true)
            .open("/dev/uinput")
            .context("open /dev/uinput for virtual mouse")?;

        let fd = file.as_raw_fd();

        unsafe {
            ui_set_evbit(fd, EV_SYN as u64).context("UI_SET_EVBIT EV_SYN")?;
            ui_set_evbit(fd, EV_KEY as u64).context("UI_SET_EVBIT EV_KEY")?;
            ui_set_evbit(fd, EV_REL as u64).context("UI_SET_EVBIT EV_REL")?;
            ui_set_evbit(fd, EV_MSC as u64).context("UI_SET_EVBIT EV_MSC")?;

            // All standard mouse buttons
            for code in 272u16..=287 {
                let _ = ui_set_keybit(fd, code as u64);
            }

            // Relative axes: X, Y, HWHEEL, WHEEL, WHEEL_HI_RES, HWHEEL_HI_RES
            for axis in [0u16, 1, 6, 8, 11, 12] {
                ui_set_relbit(fd, axis as u64).context("UI_SET_RELBIT")?;
            }

            // MSC_SCAN for scancode pass-through
            ui_set_mscbit(fd, 4u64).context("UI_SET_MSCBIT")?;

            let mut setup = UinputSetup {
                id: InputId { bustype: BUS_VIRTUAL, vendor: 0, product: 0, version: 1 },
                name: [0u8; 80],
                ff_effects_max: 0,
            };
            let name = b"logi-tune virtual mouse";
            setup.name[..name.len()].copy_from_slice(name);
            ui_dev_setup(fd, &setup).context("UI_DEV_SETUP")?;
            ui_dev_create(fd).context("UI_DEV_CREATE")?;
        }

        std::thread::sleep(std::time::Duration::from_millis(100));

        Ok(Self { file })
    }

    pub fn write_event(&mut self, ev_type: u16, code: u16, value: i32) -> Result<()> {
        let ev = InputEvent { sec: 0, usec: 0, type_: ev_type, code, value };
        let bytes: &[u8] = unsafe {
            std::slice::from_raw_parts(
                &ev as *const InputEvent as *const u8,
                std::mem::size_of::<InputEvent>(),
            )
        };
        self.file.write_all(bytes).context("write to virtual mouse")?;
        Ok(())
    }
}

impl Drop for UinputMouse {
    fn drop(&mut self) {
        unsafe {
            let _ = ui_dev_destroy(self.file.as_raw_fd());
        }
    }
}

// ---------------------------------------------------------------------------
// Key name → Linux key code mapping
// ---------------------------------------------------------------------------

fn key_name_to_code(name: &str) -> Option<u16> {
    match name.to_lowercase().as_str() {
        "super" | "meta" | "win" | "logo" => Some(125),
        "ctrl" | "control" | "leftctrl" => Some(29),
        "rightctrl" => Some(97),
        "alt" | "leftalt" => Some(56),
        "rightalt" | "altgr" => Some(100),
        "shift" | "leftshift" => Some(42),
        "rightshift" => Some(54),
        "space" => Some(57),
        "return" | "enter" => Some(28),
        "tab" => Some(15),
        "esc" | "escape" => Some(1),
        "backspace" => Some(14),
        "del" | "delete" => Some(111),
        "left" => Some(105),
        "right" => Some(106),
        "up" => Some(103),
        "down" => Some(108),
        "home" => Some(102),
        "end" => Some(107),
        "pageup" | "pgup" => Some(104),
        "pagedown" | "pgdn" => Some(109),
        "f1" => Some(59), "f2" => Some(60), "f3" => Some(61), "f4" => Some(62),
        "f5" => Some(63), "f6" => Some(64), "f7" => Some(65), "f8" => Some(66),
        "f9" => Some(67), "f10" => Some(68), "f11" => Some(87), "f12" => Some(88),
        s if s.len() == 1 => char_to_key_code(s.chars().next()?),
        _ => None,
    }
}

fn char_to_key_code(c: char) -> Option<u16> {
    Some(match c {
        'a' => 30, 'b' => 48, 'c' => 46, 'd' => 32, 'e' => 18, 'f' => 33,
        'g' => 34, 'h' => 35, 'i' => 23, 'j' => 36, 'k' => 37, 'l' => 38,
        'm' => 50, 'n' => 49, 'o' => 24, 'p' => 25, 'q' => 16, 'r' => 19,
        's' => 31, 't' => 20, 'u' => 22, 'v' => 47, 'w' => 17, 'x' => 45,
        'y' => 21, 'z' => 44,
        '0' => 11, '1' => 2,  '2' => 3,  '3' => 4,  '4' => 5,  '5' => 6,
        '6' => 7,  '7' => 8,  '8' => 9,  '9' => 10,
        _ => return None,
    })
}
