//! logi-tune entry point: initialise logging, load config, detect device, launch TUI.
//!
//! Run with `--daemon` to start in headless mode: the button monitor runs
//! permanently and reloads the config file whenever it changes on disk.

use std::os::unix::process::CommandExt;
use std::sync::{Arc, Mutex, RwLock};

use tracing_subscriber::EnvFilter;

mod apply;
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

    let args: Vec<String> = std::env::args().collect();
    let daemon_mode = args.iter().any(|a| a == "--daemon");

    // These modes run before any uinput or evdev setup: they only talk HID++
    // and must not disturb a daemon that is already grabbing the device.
    if args.iter().any(|a| a == "--scan") {
        return run_scan();
    }
    if args.iter().any(|a| a == "--haptic-list") {
        return run_haptic_list();
    }
    if args.iter().any(|a| a == "--watch-buttons") {
        return run_watch_buttons();
    }
    if args.iter().any(|a| a == "--dry-run") {
        return run_dry_run();
    }
    if let Some(i) = args.iter().position(|a| a == "--haptic") {
        let name = args
            .get(i + 1)
            .ok_or_else(|| anyhow::anyhow!("--haptic needs a waveform name; see --haptic-list"))?;
        return run_haptic_play(name);
    }

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
        let model = d.model;
        match (
            input::UinputKeyboard::new(),
            input::UinputMouse::new(),
            open_monitor_with_retry(&path),
        ) {
            (Ok(keyboard), Ok(mouse), Ok(monitor)) => {
                let shared = Arc::clone(&shared_config);
                std::thread::spawn(move || run_monitor(monitor, shared, keyboard, mouse, model));
                tracing::info!("Button monitor started (evdev, exclusive grab)");
            }
            (Err(e), _, _) | (_, Err(e), _) => tracing::warn!("uinput init failed: {e}"),
            (_, _, Err(e)) => tracing::warn!("Monitor open failed: {e}"),
        }
        Arc::new(Mutex::new(d))
    });

    // Held until main returns so the tracker is stopped when we exit.
    let _focus_tracker = spawn_focus_tracker();

    if daemon_mode {
        tracing::info!("Running in daemon mode — TUI disabled");
        run_daemon(shared_config, device).await
    } else {
        tui::run(cfg, device, device_name, shared_config).await
    }
}

/// Print the device's HID++ feature table and reprogrammable controls.
///
/// This is the tool to reach for when adding a new model: the feature numbers
/// and control IDs it prints are what the hardware actually reports, which has
/// turned out to differ between MX generations.
fn run_scan() -> anyhow::Result<()> {
    let mut device = hid::MxMaster::open()
        .map_err(|e| anyhow::anyhow!("{e}\n\nSee README for the udev rule."))?;

    println!(
        "{} at {} via {:?}",
        device.model.display_name(),
        device.path,
        device.bus
    );

    let scan =
        hid::features::scan(&mut device).map_err(|e| anyhow::anyhow!("HID++ scan failed: {e}"))?;

    println!("HID++ {}.{}\n", scan.protocol.0, scan.protocol.1);

    println!("features ({}):", scan.features.len());
    println!("  idx   number  ver  name                      flags");
    for f in &scan.features {
        println!(
            "  {:#04x}  {:#06x}  {:<3}  {:<24}  {}",
            f.index,
            f.number,
            f.version,
            hid::features::feature_name(f.number),
            f.flag_names().join(",")
        );
    }

    println!("\nreprogrammable controls ({}):", scan.controls.len());
    println!("  cid     tid     flags  group  mask  rawXY");
    for c in &scan.controls {
        println!(
            "  {:#06x}  {:#06x}  {:#04x}   {:<5}  {:#04x}  {:#04x}",
            c.control_id, c.task_id, c.flags, c.group, c.group_mask, c.raw_xy
        );
    }

    let layout = hid::features::button_layout(device.model);
    println!(
        "\nlayout this build uses for {}:",
        device.model.display_name()
    );
    let reported: Vec<u16> = scan.controls.iter().map(|c| c.control_id).collect();
    for (name, cid) in [
        ("gesture_button", Some(layout.gesture_button)),
        ("thumb_button", Some(layout.thumb_button)),
        ("back", Some(layout.back)),
        ("forward", Some(layout.forward)),
        ("smart_shift", Some(layout.smart_shift)),
        ("haptic_button", layout.haptic_button),
    ] {
        match cid {
            Some(cid) if reported.contains(&cid) => println!("  {name:<16} {cid:#06x}"),
            Some(cid) => println!("  {name:<16} {cid:#06x}  NOT REPORTED BY THIS DEVICE"),
            None => println!("  {name:<16} -"),
        }
    }

    Ok(())
}

/// Show what applying the config would change on the device, without writing.
///
/// Worth running before a first apply: config values are pushed to persistent
/// device state, and a file can hold a value that has never taken effect.
fn run_dry_run() -> anyhow::Result<()> {
    // Unvalidated, so the corrections can be shown rather than applied silently.
    let mut cfg = config::Config::load_unvalidated()?;
    let corrections = cfg.validate();

    let mut device = hid::MxMaster::open()
        .map_err(|e| anyhow::anyhow!("{e}\n\nSee README for the udev rule."))?;
    println!("{} at {}\n", device.model.display_name(), device.path);

    if !corrections.is_empty() {
        println!("config corrections:");
        for c in &corrections {
            println!("  {c}");
        }
        println!();
    }

    let plan = apply::plan(&mut device, &cfg);
    let changes: Vec<_> = plan.iter().filter(|p| p.differs()).collect();

    println!("  {:<30} {:<12} config", "setting", "device");
    for p in &plan {
        println!(
            "  {:<30} {:<12} {}{}",
            p.setting,
            p.current,
            p.desired,
            if p.differs() {
                "   <- would change"
            } else {
                ""
            }
        );
    }

    if changes.is_empty() {
        println!("\nNothing to change.");
    } else {
        println!("\n{} setting(s) would change.", changes.len());
    }
    Ok(())
}

/// Print each button press as it happens, so a user can find out which evdev
/// code their physical buttons send.
///
/// The codes differ between MX models, and the kernel's generic names
/// (`BTN_SIDE`, `BTN_BACK`, …) do not say which physical button a mouse wires
/// to them. This observes without grabbing, so the buttons keep working
/// normally while you press them.
fn run_watch_buttons() -> anyhow::Result<()> {
    let device = hid::MxMaster::open()
        .map_err(|e| anyhow::anyhow!("{e}\n\nSee README for the udev rule."))?;

    println!("{} at {}", device.model.display_name(), device.path);

    // The daemon takes an exclusive grab, which would starve this reader.
    let daemon_running = std::process::Command::new("systemctl")
        .args(["--user", "is-active", "--quiet", "logi-tune"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if daemon_running {
        println!(
            "\nThe logi-tune daemon is running and holds an exclusive grab, so no\n\
             events will reach this watcher. Stop it first:\n\
             \n    systemctl --user stop logi-tune\n"
        );
        return Ok(());
    }

    let cfg = config::Config::load()?;
    let codes = cfg.buttons.codes(device.model);
    let mut monitor = hid::monitor::DeviceMonitor::open_observer(&device.path)?;
    println!("\nPress each button. Ctrl-C when done.\n");
    println!("  code  name           bound to");

    loop {
        let ev = monitor.read_raw()?;
        // EV_KEY presses only; releases and movement are noise here.
        if ev.ev_type == 1 && ev.value == 1 {
            let bound = match cfg.buttons.for_evdev_code(ev.code, &codes) {
                Some((field, action)) => format!("{field} = {action}"),
                None => "nothing — set this code in [buttons.evdev_codes]".to_string(),
            };
            println!(
                "  {:<5} {:<14} {}",
                ev.code,
                hid::monitor::DeviceMonitor::button_name(ev.code),
                bound
            );
        }
    }
}

/// List the haptic waveforms the connected device implements.
fn run_haptic_list() -> anyhow::Result<()> {
    let mut device = hid::MxMaster::open()
        .map_err(|e| anyhow::anyhow!("{e}\n\nSee README for the udev rule."))?;

    let info = hid::haptics::info(&mut device)
        .map_err(|e| anyhow::anyhow!("{} has no haptic motor: {e}", device.model.display_name()))?;
    let level = hid::haptics::level(&mut device)?;

    println!("{}", device.model.display_name());
    println!(
        "haptics {} at level {}/100 (device default {})",
        if level.enabled { "enabled" } else { "disabled" },
        level.level,
        info.default_level
    );
    println!("waveform mask {:#010x}\n", info.waveform_mask);
    println!("supported waveforms:");
    for w in info.supported() {
        println!("  {:#04x}  {}", w.id(), w.name());
    }

    let unsupported: Vec<_> = hid::haptics::Waveform::ALL
        .iter()
        .filter(|w| !info.supports(**w))
        .collect();
    if !unsupported.is_empty() {
        println!("\nnot implemented by this device:");
        for w in unsupported {
            println!("  {:#04x}  {}", w.id(), w.name());
        }
    }
    Ok(())
}

/// Play one haptic waveform, for trying them out from a shell.
fn run_haptic_play(name: &str) -> anyhow::Result<()> {
    let waveform = hid::haptics::Waveform::parse(name).ok_or_else(|| {
        anyhow::anyhow!("unknown waveform {name:?}; run --haptic-list to see the names")
    })?;

    let mut device = hid::MxMaster::open()
        .map_err(|e| anyhow::anyhow!("{e}\n\nSee README for the udev rule."))?;

    hid::haptics::play(&mut device, waveform).map_err(|e| {
        anyhow::anyhow!(
            "could not play {}: {e}\n\nRun --haptic-list to see what this device supports.",
            waveform.name()
        )
    })?;
    println!("played {} ({:#04x})", waveform.name(), waveform.id());
    Ok(())
}

/// Open the evdev monitor, retrying on permission denied to tolerate the udev
/// race where the event device node exists but its group hasn't been set yet.
fn open_monitor_with_retry(path: &str) -> Result<hid::monitor::DeviceMonitor, hid::HidError> {
    const ATTEMPTS: u32 = 10;
    const DELAY: std::time::Duration = std::time::Duration::from_millis(200);
    let mut last_err = hid::HidError::NotFound;
    for attempt in 0..ATTEMPTS {
        match hid::monitor::DeviceMonitor::open(path) {
            Ok(m) => return Ok(m),
            Err(e @ hid::HidError::Permission { .. }) => {
                last_err = e;
                if attempt + 1 < ATTEMPTS {
                    tracing::debug!(attempt, "evdev permission denied, retrying");
                    std::thread::sleep(DELAY);
                }
            }
            Err(e) => return Err(e),
        }
    }
    Err(last_err)
}

/// Push the config's HID++ settings to the device, logging the outcome.
///
/// Settings like DPI, scroll mode and haptic level are device state; nothing
/// else restores them, so the daemon has to apply them itself rather than
/// relying on someone opening the TUI and pressing apply.
fn apply_to_device(device: Option<&Arc<Mutex<hid::MxMaster>>>, cfg: &config::Config) {
    let Some(device) = device else { return };
    let mut guard = match device.lock() {
        Ok(g) => g,
        Err(e) => {
            tracing::error!("device lock poisoned: {e}");
            return;
        }
    };
    match apply::apply_config_to_device(&mut guard, cfg) {
        Ok(()) => tracing::info!("applied config to device"),
        Err(e) => tracing::warn!("could not apply config to device: {e}"),
    }
}

async fn run_daemon(
    shared_config: Arc<RwLock<config::Config>>,
    device: Option<Arc<Mutex<hid::MxMaster>>>,
) -> anyhow::Result<()> {
    let config_path = config::Config::path();
    let mut last_modified = std::fs::metadata(&config_path)
        .and_then(|m| m.modified())
        .ok();

    // Apply once at startup: the daemon starts at login, which is exactly
    // when the device has come back with its own defaults.
    {
        let cfg = shared_config.read().unwrap().clone();
        apply_to_device(device.as_ref(), &cfg);
    }

    loop {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        let modified = std::fs::metadata(&config_path)
            .and_then(|m| m.modified())
            .ok();

        if modified != last_modified {
            last_modified = modified;
            match config::Config::load() {
                Ok(cfg) => {
                    *shared_config.write().unwrap() = cfg.clone();
                    tracing::info!("Config reloaded from disk");
                    // Button bindings are read per event, but the HID++
                    // settings are device state and need pushing again.
                    apply_to_device(device.as_ref(), &cfg);
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
    model: hid::DeviceModel,
) {
    // Track which button codes had their press swallowed so we can also suppress
    // their release events — forwarding an orphaned release confuses libinput's
    // button-state tracking for the virtual mouse, which can glitch other buttons.
    let mut swallowed: std::collections::HashSet<u16> = Default::default();

    loop {
        let ev = match monitor.read_raw() {
            Ok(e) => e,
            Err(e) => {
                tracing::error!("Monitor read error: {e} — exiting so systemd can restart");
                std::process::exit(1);
            }
        };

        // EV_MSC (type 4) carries scan-code metadata.  When a press is swallowed the
        // MSC_SCAN that preceded it would reach the virtual mouse as an orphaned event;
        // dropping all MSC events is safe — virtual mice don't need them.
        if ev.ev_type == 4 {
            continue;
        }

        let action = if ev.ev_type == 1 && ev.value == 1 {
            tracing::info!(code = ev.code, "evdev button press");
            let cfg = shared_config.read().unwrap();
            action_for_evdev_code(ev.code, &cfg, model)
        } else {
            None
        };

        match action {
            Some(action) => {
                swallowed.insert(ev.code);
                perform_action(&action, &mut keyboard);
            }
            None => {
                // Suppress the release that pairs with a swallowed press.
                if ev.ev_type == 1 && ev.value == 0 && swallowed.remove(&ev.code) {
                    continue;
                }
                if let Err(e) = mouse.write_event(ev.ev_type, ev.code, ev.value) {
                    tracing::error!("Mouse pass-through failed: {e}");
                }
            }
        }
    }
}

/// Map evdev key codes to button actions.
///
/// The code for each physical button comes from `[buttons.evdev_codes]`,
/// because it varies by model. An unrecognised code is passed through
/// untouched, but logged once so it is discoverable: a button that sends a
/// code no field claims would otherwise look simply broken.
fn action_for_evdev_code(
    code: u16,
    cfg: &config::Config,
    model: hid::DeviceModel,
) -> Option<config::ButtonAction> {
    let codes = cfg.buttons.codes(model);
    let Some((field, action)) = cfg.buttons.for_evdev_code(code, &codes) else {
        tracing::debug!(
            code,
            name = hid::monitor::DeviceMonitor::button_name(code),
            "button code not claimed by any config field; run --watch-buttons"
        );
        return None;
    };
    match action {
        // Default = behave like a normal mouse button (pass through)
        config::ButtonAction::Default => None,
        other => {
            tracing::debug!(code, field, "button matched");
            Some(other.clone())
        }
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

/// Owns the AT-SPI2 focus tracker subprocess and stops it on drop.
///
/// Without an owner the tracker outlived every `logi-tune` process that
/// started one, so each launch of the TUI or the daemon left another python
/// process running forever.
struct FocusTracker(std::process::Child);

impl Drop for FocusTracker {
    fn drop(&mut self) {
        // kill() errors only if the child already exited; either way it has to
        // be reaped so it does not linger as a zombie.
        let _ = self.0.kill();
        let _ = self.0.wait();
        tracing::debug!("focus tracker stopped");
    }
}

/// Spawn the AT-SPI2 focus tracker as a background Python subprocess.
/// Writes the embedded script to a temp file and runs it with python3.
///
/// The returned handle must be held for as long as the tracker is wanted:
/// dropping it kills the tracker. The child is additionally given
/// `PR_SET_PDEATHSIG`, so it still dies if we are killed outright and `Drop`
/// never runs.
fn spawn_focus_tracker() -> Option<FocusTracker> {
    let script_path = "/tmp/logi-tune-focus-tracker.py";
    if let Err(e) = std::fs::write(script_path, FOCUS_TRACKER_SCRIPT) {
        tracing::warn!("Could not write focus tracker script: {e}");
        return None;
    }

    let mut cmd = std::process::Command::new("python3");
    cmd.arg(script_path)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());

    // SAFETY: set_pdeathsig is async-signal-safe and touches only this
    // process's prctl state, which is what pre_exec requires.
    unsafe {
        cmd.pre_exec(|| {
            nix::sys::prctl::set_pdeathsig(nix::sys::signal::Signal::SIGTERM)
                .map_err(std::io::Error::from)
        });
    }

    match cmd.spawn() {
        Ok(child) => {
            tracing::info!(pid = child.id(), "AT-SPI2 focus tracker started");
            Some(FocusTracker(child))
        }
        Err(e) => {
            tracing::warn!("Could not start focus tracker: {e}");
            None
        }
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
