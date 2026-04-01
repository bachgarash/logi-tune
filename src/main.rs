//! logi-tune entry point: initialise logging, load config, detect device, launch TUI.
//!
//! Run with `--daemon` to start in headless mode: the button monitor runs
//! permanently and reloads the config file whenever it changes on disk.

use std::sync::{Arc, Mutex, RwLock};

use tracing_subscriber::EnvFilter;

mod config;
mod hid;
mod input;
mod tui;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Logging goes to stderr so it never corrupts the TUI
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();

    let daemon_mode = std::env::args().any(|a| a == "--daemon");

    let cfg = config::Config::load()?;

    let raw_device = match hid::MxMaster::open() {
        Ok(d) => {
            tracing::info!("Opened device: {}", d.model.display_name());
            Some(d)
        }
        Err(hid::HidError::NotFound) => {
            if daemon_mode {
                return Err(anyhow::anyhow!("No supported device found — will retry"));
            }
            tracing::warn!("No supported device found — running without device");
            None
        }
        Err(e @ hid::HidError::Permission { .. }) => {
            return Err(anyhow::anyhow!(
                "{e}\n\nInstall the udev rule in udev/99-mx-master.rules and reload."
            ));
        }
        Err(e) => return Err(anyhow::anyhow!("Failed to open device: {e}")),
    };

    let device_name = raw_device
        .as_ref()
        .map(|d| d.model.display_name())
        .unwrap_or("No device");

    let shared_config = Arc::new(RwLock::new(cfg.clone()));

    let device: Option<Arc<Mutex<hid::MxMaster>>> = raw_device.map(|d| {
        // Create uinput devices before grabbing the physical device so the compositor
        // discovers the virtual mouse before we take exclusive ownership.
        let path = d.path.clone();
        match (
            input::UinputKeyboard::new(),
            input::UinputMouse::new(),
            hid::monitor::DeviceMonitor::open(&path),
        ) {
            (Ok(keyboard), Ok(mouse), Ok(monitor)) => {
                let shared = Arc::clone(&shared_config);
                std::thread::spawn(move || run_monitor(monitor, shared, keyboard, mouse));
                tracing::info!("Button monitor started (evdev, exclusive grab)");
            }
            (Err(e), _, _) | (_, Err(e), _) => tracing::warn!("uinput init failed: {e}"),
            (_, _, Err(e)) => tracing::warn!("Monitor open failed: {e}"),
        }
        Arc::new(Mutex::new(d))
    });

    spawn_focus_tracker();

    if daemon_mode {
        tracing::info!("Running in daemon mode — TUI disabled");
        run_daemon(shared_config).await
    } else {
        tui::run(cfg, device, device_name, shared_config).await
    }
}

async fn run_daemon(shared_config: Arc<RwLock<config::Config>>) -> anyhow::Result<()> {
    let config_path = config::Config::path();
    let mut last_modified = std::fs::metadata(&config_path)
        .and_then(|m| m.modified())
        .ok();

    loop {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        let modified = std::fs::metadata(&config_path)
            .and_then(|m| m.modified())
            .ok();

        if modified != last_modified {
            last_modified = modified;
            match config::Config::load() {
                Ok(cfg) => {
                    *shared_config.write().unwrap() = cfg;
                    tracing::info!("Config reloaded from disk");
                }
                Err(e) => tracing::warn!("Failed to reload config: {e}"),
            }
        }
    }
}

fn run_monitor(
    mut monitor: hid::monitor::DeviceMonitor,
    shared_config: Arc<RwLock<config::Config>>,
    mut keyboard: input::UinputKeyboard,
    mut mouse: input::UinputMouse,
) {
    loop {
        let ev = match monitor.read_raw() {
            Ok(e) => e,
            Err(e) => {
                tracing::error!("Monitor read error: {e} — exiting so systemd can restart");
                std::process::exit(1);
            }
        };

        let action = if ev.ev_type == 1 && ev.value == 1 {
            tracing::info!(code = ev.code, "evdev button press");
            let cfg = shared_config.read().unwrap();
            action_for_evdev_code(ev.code, &cfg)
        } else {
            None
        };

        match action {
            Some(action) => {
                // Swallow the event; send SYN so the compositor sees a clean state.
                let _ = mouse.write_event(0, 0, 0); // EV_SYN
                perform_action(&action, &mut keyboard);
            }
            None => {
                if let Err(e) = mouse.write_event(ev.ev_type, ev.code, ev.value) {
                    tracing::error!("Mouse pass-through failed: {e}");
                }
            }
        }
    }
}

/// Map evdev key codes to button actions for the MX Master series.
fn action_for_evdev_code(code: u16, cfg: &config::Config) -> Option<config::ButtonAction> {
    // MX Master 3S For Mac (BT): confirmed codes
    let action = match code {
        274 => &cfg.buttons.middle_button,  // scroll wheel click
        275 => &cfg.buttons.back_button,    // back button
        276 => &cfg.buttons.forward_button, // forward button
        277 => &cfg.buttons.thumb_button,   // thumb/gesture button
        _ => return None,
    };
    match action {
        // Default = behave like a normal mouse button (pass through)
        config::ButtonAction::Default => None,
        other => Some(other.clone()),
    }
}

fn perform_action(action: &config::ButtonAction, uinput: &mut input::UinputKeyboard) {
    match action {
        config::ButtonAction::KeyCombo(combo) => {
            tracing::debug!(combo, "injecting key combo");
            if let Err(e) = uinput.inject_combo(combo) {
                tracing::error!("Key injection failed: {e}");
            }
        }
        config::ButtonAction::ContextCombo { default, terminal } => {
            let combo = if active_window_is_terminal() {
                terminal
            } else {
                default
            };
            tracing::debug!(combo, "injecting context combo");
            if let Err(e) = uinput.inject_combo(combo) {
                tracing::error!("Key injection failed: {e}");
            }
        }
        config::ButtonAction::Exec(cmd) => {
            tracing::debug!(cmd, "spawning exec");
            if let Err(e) = std::process::Command::new("sh").arg("-c").arg(cmd).spawn() {
                tracing::error!("Exec failed: {e}");
            }
        }
        _ => {}
    }
}

/// Detect whether the currently focused window is a terminal emulator.
/// Reads from /tmp/logi-tune-focus written by the AT-SPI2 focus tracker subprocess.
fn active_window_is_terminal() -> bool {
    if let Ok(contents) = std::fs::read_to_string("/tmp/logi-tune-focus") {
        let class = contents.trim().to_lowercase();
        if !class.is_empty() {
            return is_terminal_class(&class);
        }
    }
    false
}

/// Embedded Python AT-SPI2 focus tracker script.
const FOCUS_TRACKER_SCRIPT: &str = r#"
import sys
import os
import signal
import gi
gi.require_version('Atspi', '2.0')
from gi.repository import Atspi, GLib

FOCUS_FILE = '/tmp/logi-tune-focus'

def write_focus(name):
    try:
        with open(FOCUS_FILE, 'w') as f:
            f.write(name)
    except Exception:
        pass

def resolve_app_name(app):
    """Get real binary name via PID -> /proc/PID/cmdline (works for unnamed Wayland apps)."""
    try:
        pid = app.get_process_id()
        if pid > 0:
            with open(f'/proc/{pid}/cmdline', 'rb') as f:
                cmdline = f.read().split(b'\x00')[0].decode(errors='replace')
            return os.path.basename(cmdline)
    except Exception:
        pass
    # fallback to AT-SPI name
    try:
        name = app.get_name()
        if name and name != 'Unnamed':
            return name
    except Exception:
        pass
    return ''

SHELL_APPS = {'gnome-shell', 'ibus-extension-gtk3', 'xdg-desktop-portal-gtk', ''}

def on_focus(event):
    try:
        app = event.source.get_application()
        if app:
            name = resolve_app_name(app)
            if name and name not in SHELL_APPS:
                write_focus(name)
    except Exception:
        pass

def poll_active():
    """Scan desktop for the active/focused non-shell window. Returns True if found."""
    try:
        desktop = Atspi.get_desktop(0)
        for i in range(desktop.get_child_count()):
            app = desktop.get_child_at_index(i)
            if app is None:
                continue
            try:
                name = resolve_app_name(app)
                if not name or name in SHELL_APPS:
                    continue
                for j in range(app.get_child_count()):
                    win = app.get_child_at_index(j)
                    if win is None:
                        continue
                    try:
                        ss = win.get_state_set()
                        if ss.contains(Atspi.StateType.ACTIVE) or ss.contains(Atspi.StateType.FOCUSED):
                            write_focus(name)
                            return True
                    except Exception:
                        pass
            except Exception:
                pass
    except Exception:
        pass
    return False

signal.signal(signal.SIGTERM, lambda *_: sys.exit(0))
signal.signal(signal.SIGINT, lambda *_: sys.exit(0))

Atspi.init()

listener = Atspi.EventListener.new(on_focus)
listener.register('window:activate')

# Poll every 3s continuously — AT-SPI2 events alone are not reliable for
# apps like Chrome that don't emit window:activate by default.
def poll_loop():
    poll_active()
    return True  # always reschedule

poll_loop()
GLib.timeout_add(3000, poll_loop)

loop = GLib.MainLoop()
loop.run()
"#;

/// Spawn the AT-SPI2 focus tracker as a background Python subprocess.
/// Writes the embedded script to a temp file and runs it with python3.
fn spawn_focus_tracker() {
    let script_path = "/tmp/logi-tune-focus-tracker.py";
    if let Err(e) = std::fs::write(script_path, FOCUS_TRACKER_SCRIPT) {
        tracing::warn!("Could not write focus tracker script: {e}");
        return;
    }
    match std::process::Command::new("python3")
        .arg(script_path)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        Ok(_) => tracing::info!("AT-SPI2 focus tracker started"),
        Err(e) => tracing::warn!("Could not start focus tracker: {e}"),
    }
}

fn is_terminal_class(s: &str) -> bool {
    const TERMINALS: &[&str] = &[
        "alacritty",
        "kitty",
        "gnome-terminal",
        "tilix",
        "konsole",
        "xterm",
        "wezterm",
        "foot",
        "st",
        "urxvt",
        "terminator",
        "mate-terminal",
        "xfce4-terminal",
        "terminal",
        "ghostty",
    ];
    TERMINALS.iter().any(|t| s.contains(t))
}
