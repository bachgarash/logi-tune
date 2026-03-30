# logi-tune

A terminal UI for configuring Logitech MX Master mice on Linux — button remapping, DPI profiles, scroll wheel tuning, and thumb wheel settings, all saved to a TOML config file.

![Rust](https://img.shields.io/badge/rust-stable-orange)

## Supported devices

MX Master 2S, MX Master 3, MX Master 3 for Mac, MX Master 3S, MX Master 3S for Mac (USB and Bluetooth).

## Requirements

- Linux kernel with hidraw and evdev support
- `python3` with `python3-gi` and `gir1.2-atspi-2.0` (for context-aware key combos)
- User must be in the `input` group (or root)

## Setup

**1. Install the udev rule** so you can access the device without root:

```
sudo cp udev/99-mx-master.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules && sudo udevadm trigger
```

Then log out and back in (or run `newgrp input`).

**2. Build and install:**

```
cargo build --release
cp target/release/logi-tune ~/.local/bin/
```

**3. (Optional) Run as a systemd user service:**

```
cp systemd/logi-tune.service ~/.config/systemd/user/
systemctl --user enable --now logi-tune
```

This runs `logi-tune --daemon` in the background so button mappings stay active without the TUI open.

## Usage

```
logi-tune          # open the TUI
logi-tune --daemon # headless mode, reloads config on change
```

### TUI keys

| Key | Action |
|-----|--------|
| `Tab` / `Shift+Tab` | Switch tabs |
| `s` | Save config |
| `a` | Apply settings to device |
| `r` | Reset current tab to defaults |
| `q` / `Esc` | Quit |
| `?` | Help overlay |

## Config file

`~/.config/logi-tune/config.toml` — created automatically on first save. Edit by hand or via the TUI.

### Button actions

| Action | Description |
|--------|-------------|
| `Default` | Standard mouse button behavior |
| `KeyCombo` | Inject a key combo, e.g. `"super+left"` |
| `ContextCombo` | Different combo depending on whether a terminal is focused |
| `Exec` | Run a shell command |
| `ToggleScrollMode` | Switch between ratchet and free-spin scroll |
| `DpiUp` / `DpiDown` | Cycle DPI profiles |
| `Disabled` | Swallow the button event |

### Example config

```toml
[buttons]
thumb_button = { KeyCombo = "super+left" }
back_button  = { ContextCombo = { default = "alt+left", terminal = "ctrl+b" } }

[scroll]
lines_per_notch       = 3
smart_shift_threshold = 20

[dpi]
profiles = [400, 800, 1600, 3200]
active   = 2
```

## How it works

Button events are captured via evdev with an exclusive grab so the OS never sees them directly. Remapped buttons are injected back as key combos via a uinput virtual keyboard. Device settings (DPI, SmartShift, scroll config) are sent over HID++ 2.0 through the hidraw interface.

Context-aware combos use AT-SPI2 to detect which app is currently focused.

## License

MIT
