//! App state struct, event handling, and the main async event loop.

use std::sync::{Arc, Mutex, RwLock};

use anyhow::Context;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{Terminal, backend::Backend};
use tokio::sync::oneshot;

use crate::config::Config;
use crate::hid::{
    HidError, MxMaster,
    features::{
        BatteryStatus, button_layout, remap_button, set_dpi, set_hi_res_scroll, set_smart_shift,
        set_thumb_wheel,
    },
};

use super::ui::render;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tab {
    Buttons,
    Scroll,
    ThumbWheel,
    Dpi,
}

impl Tab {
    pub fn index(self) -> usize {
        match self {
            Tab::Buttons => 0,
            Tab::Scroll => 1,
            Tab::ThumbWheel => 2,
            Tab::Dpi => 3,
        }
    }

    pub fn from_index(i: usize) -> Self {
        match i % 4 {
            0 => Tab::Buttons,
            1 => Tab::Scroll,
            2 => Tab::ThumbWheel,
            _ => Tab::Dpi,
        }
    }
}

/// The current input mode drives how key events are interpreted.
#[derive(Debug, Clone, PartialEq)]
pub enum InputMode {
    Normal,
    ActionPicker,
    TextInput { prompt: String, buffer: String },
}

/// Top-level application state.
pub struct App {
    pub config: Config,
    /// Device wrapped in Arc<Mutex> so apply can run in a background task.
    pub device: Option<Arc<Mutex<MxMaster>>>,
    /// Cached model name so the UI never needs to lock the device.
    pub device_name: &'static str,
    pub active_tab: Tab,
    pub status: Option<String>,
    pub dirty: bool,
    pub should_quit: bool,
    pub show_help: bool,

    pub button_selected: usize,
    pub scroll_selected: usize,
    pub thumb_selected: usize,
    pub dpi_selected: usize,

    pub input_mode: InputMode,
    /// Index of the action being picked (ActionPicker mode)
    pub action_picker_index: usize,
    /// Stores the first combo while awaiting the second in a ContextCombo flow.
    pending_context_default: Option<String>,

    /// Shared with the background monitor task; updated whenever config is applied.
    shared_config: Arc<RwLock<Config>>,

    /// Latest battery reading (refreshed on startup and periodically).
    pub battery: Option<BatteryStatus>,
    /// Tick counter incremented every 100 ms, used for the loading spinner.
    pub tick: u64,
    /// True while an apply is running in the background.
    pub applying: bool,
    /// Receives the apply result from the background task.
    apply_rx: Option<oneshot::Receiver<Result<Config, HidError>>>,
    /// Ticks since last battery refresh.
    battery_ticks: u64,
}

impl App {
    pub fn new(
        config: Config,
        device: Option<Arc<Mutex<MxMaster>>>,
        device_name: &'static str,
        shared_config: Arc<RwLock<Config>>,
    ) -> Self {
        Self {
            config,
            device,
            device_name,
            active_tab: Tab::Buttons,
            status: None,
            dirty: false,
            should_quit: false,
            show_help: false,
            button_selected: 0,
            scroll_selected: 0,
            thumb_selected: 0,
            dpi_selected: 0,
            input_mode: InputMode::Normal,
            action_picker_index: 0,
            pending_context_default: None,
            shared_config,
            battery: None,
            tick: 0,
            applying: false,
            apply_rx: None,
            battery_ticks: 0,
        }
    }

    /// Refresh battery status from the kernel's power_supply sysfs interface.
    /// This is reliable on BT — no HID++ needed.
    pub fn refresh_battery(&mut self) {
        match read_battery_sysfs() {
            Ok(b) => self.battery = Some(b),
            Err(e) => tracing::debug!("battery sysfs read failed: {e}"),
        }
    }

    pub fn handle_event(&mut self, event: Event) -> anyhow::Result<()> {
        if let Event::Key(key) = event {
            if key.kind != KeyEventKind::Press {
                return Ok(());
            }

            match &self.input_mode.clone() {
                InputMode::Normal => self.handle_normal(key.code, key.modifiers),
                InputMode::ActionPicker => self.handle_action_picker(key.code),
                InputMode::TextInput { .. } => self.handle_text_input(key.code),
            }
        }
        Ok(())
    }

    fn handle_normal(&mut self, code: KeyCode, modifiers: KeyModifiers) {
        match code {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.should_quit = true;
            }
            KeyCode::Char('?') => {
                self.show_help = !self.show_help;
            }
            KeyCode::Char('s') => {
                self.save_config();
            }
            KeyCode::Char('a') => {
                self.apply_to_device();
            }
            KeyCode::Char('r') => {
                self.reset_tab();
            }
            KeyCode::Tab => {
                let next = (self.active_tab.index() + 1) % 4;
                self.active_tab = Tab::from_index(next);
            }
            KeyCode::BackTab => {
                let prev = (self.active_tab.index() + 3) % 4;
                self.active_tab = Tab::from_index(prev);
            }
            _ => self.handle_tab_key(code, modifiers),
        }
    }

    fn handle_tab_key(&mut self, code: KeyCode, _modifiers: KeyModifiers) {
        match self.active_tab {
            Tab::Buttons => self.handle_buttons_key(code),
            Tab::Scroll => self.handle_scroll_key(code),
            Tab::ThumbWheel => self.handle_thumb_key(code),
            Tab::Dpi => self.handle_dpi_key(code),
        }
    }

    fn handle_buttons_key(&mut self, code: KeyCode) {
        const BUTTON_COUNT: usize = 4;
        match code {
            KeyCode::Up => {
                if self.button_selected > 0 {
                    self.button_selected -= 1;
                }
            }
            KeyCode::Down => {
                if self.button_selected < BUTTON_COUNT - 1 {
                    self.button_selected += 1;
                }
            }
            KeyCode::Enter => {
                self.input_mode = InputMode::ActionPicker;
                self.action_picker_index = 0;
            }
            _ => {}
        }
    }

    fn handle_action_picker(&mut self, code: KeyCode) {
        const ACTION_COUNT: usize = 8;
        match code {
            KeyCode::Esc => {
                self.input_mode = InputMode::Normal;
            }
            KeyCode::Up => {
                if self.action_picker_index > 0 {
                    self.action_picker_index -= 1;
                }
            }
            KeyCode::Down => {
                if self.action_picker_index < ACTION_COUNT - 1 {
                    self.action_picker_index += 1;
                }
            }
            KeyCode::Char(c) if ('1'..='8').contains(&c) => {
                let idx = c as usize - '1' as usize;
                self.action_picker_index = idx;
                self.confirm_action_picker();
            }
            KeyCode::Enter => {
                self.confirm_action_picker();
            }
            _ => {}
        }
    }

    fn confirm_action_picker(&mut self) {
        use crate::config::ButtonAction;
        let action = match self.action_picker_index {
            0 => Some(ButtonAction::Default),
            1 => {
                self.input_mode = InputMode::TextInput {
                    prompt: "Key combo (e.g. ctrl+c):".into(),
                    buffer: String::new(),
                };
                return;
            }
            2 => {
                self.input_mode = InputMode::TextInput {
                    prompt: "Command to execute:".into(),
                    buffer: String::new(),
                };
                return;
            }
            3 => Some(ButtonAction::ToggleScrollMode),
            4 => Some(ButtonAction::DpiUp),
            5 => Some(ButtonAction::DpiDown),
            6 => Some(ButtonAction::Disabled),
            7 => {
                // ContextCombo — step 1: ask for default combo
                self.pending_context_default = None;
                self.input_mode = InputMode::TextInput {
                    prompt: "Default combo (e.g. ctrl+c):".into(),
                    buffer: String::new(),
                };
                return;
            }
            _ => None,
        };

        if let Some(a) = action {
            self.set_button_action(a);
            self.input_mode = InputMode::Normal;
        }
    }

    fn handle_text_input(&mut self, code: KeyCode) {
        let current_mode = self.input_mode.clone();
        if let InputMode::TextInput { prompt, mut buffer } = current_mode {
            match code {
                KeyCode::Esc => {
                    self.pending_context_default = None;
                    self.input_mode = InputMode::Normal;
                }
                KeyCode::Enter => {
                    let text = buffer.trim().to_string();
                    if prompt.starts_with("Default combo") {
                        // ContextCombo step 1 done — ask for terminal combo
                        self.pending_context_default = Some(text);
                        self.input_mode = InputMode::TextInput {
                            prompt: "Terminal combo (e.g. ctrl+shift+c):".into(),
                            buffer: String::new(),
                        };
                    } else if prompt.starts_with("Terminal combo") {
                        // ContextCombo step 2 done — build the action
                        let default = self.pending_context_default.take().unwrap_or_default();
                        self.set_button_action(crate::config::ButtonAction::ContextCombo {
                            default,
                            terminal: text,
                        });
                        self.input_mode = InputMode::Normal;
                    } else if prompt.contains("combo") {
                        self.set_button_action(crate::config::ButtonAction::KeyCombo(text));
                        self.input_mode = InputMode::Normal;
                    } else {
                        self.set_button_action(crate::config::ButtonAction::Exec(text));
                        self.input_mode = InputMode::Normal;
                    }
                }
                KeyCode::Backspace => {
                    buffer.pop();
                    self.input_mode = InputMode::TextInput { prompt, buffer };
                }
                KeyCode::Char(c) => {
                    buffer.push(c);
                    self.input_mode = InputMode::TextInput { prompt, buffer };
                }
                _ => {}
            }
        }
    }

    fn set_button_action(&mut self, action: crate::config::ButtonAction) {
        match self.button_selected {
            0 => self.config.buttons.middle_button = action,
            1 => self.config.buttons.back_button = action,
            2 => self.config.buttons.forward_button = action,
            3 => self.config.buttons.thumb_button = action,
            _ => {}
        }
        self.dirty = true;
    }

    fn handle_scroll_key(&mut self, code: KeyCode) {
        const ROW_COUNT: usize = 3;
        match code {
            KeyCode::Up => {
                if self.scroll_selected > 0 {
                    self.scroll_selected -= 1;
                }
            }
            KeyCode::Down => {
                if self.scroll_selected < ROW_COUNT - 1 {
                    self.scroll_selected += 1;
                }
            }
            KeyCode::Left => {
                self.dirty = true;
                match self.scroll_selected {
                    0 => {
                        if self.config.scroll.lines_per_notch > 1 {
                            self.config.scroll.lines_per_notch -= 1;
                        }
                    }
                    2 => {
                        if self.config.scroll.smart_shift_threshold > 0 {
                            self.config.scroll.smart_shift_threshold -= 1;
                        }
                    }
                    _ => {}
                }
            }
            KeyCode::Right => {
                self.dirty = true;
                match self.scroll_selected {
                    0 => {
                        if self.config.scroll.lines_per_notch < 10 {
                            self.config.scroll.lines_per_notch += 1;
                        }
                    }
                    2 => {
                        self.config.scroll.smart_shift_threshold =
                            self.config.scroll.smart_shift_threshold.saturating_add(1);
                    }
                    _ => {}
                }
            }
            KeyCode::Char(' ') => {
                if self.scroll_selected == 1 {
                    self.config.scroll.invert = !self.config.scroll.invert;
                    self.dirty = true;
                }
            }
            _ => {}
        }
    }

    fn handle_thumb_key(&mut self, code: KeyCode) {
        const ROW_COUNT: usize = 2;
        match code {
            KeyCode::Up => {
                if self.thumb_selected > 0 {
                    self.thumb_selected -= 1;
                }
            }
            KeyCode::Down => {
                if self.thumb_selected < ROW_COUNT - 1 {
                    self.thumb_selected += 1;
                }
            }
            KeyCode::Char(' ') => {
                if self.thumb_selected == 0 {
                    self.config.thumb_wheel.invert = !self.config.thumb_wheel.invert;
                    self.dirty = true;
                }
            }
            KeyCode::Left => {
                if self.thumb_selected == 1 && self.config.thumb_wheel.sensitivity > 1 {
                    self.config.thumb_wheel.sensitivity -= 1;
                    self.dirty = true;
                }
            }
            KeyCode::Right => {
                if self.thumb_selected == 1 && self.config.thumb_wheel.sensitivity < 10 {
                    self.config.thumb_wheel.sensitivity += 1;
                    self.dirty = true;
                }
            }
            _ => {}
        }
    }

    fn handle_dpi_key(&mut self, code: KeyCode) {
        let profile_count = self.config.dpi.profiles.len();
        match code {
            KeyCode::Up => {
                if self.dpi_selected > 0 {
                    self.dpi_selected -= 1;
                }
            }
            KeyCode::Down => {
                if profile_count > 0 && self.dpi_selected < profile_count - 1 {
                    self.dpi_selected += 1;
                }
            }
            KeyCode::Left => {
                if let Some(v) = self.config.dpi.profiles.get_mut(self.dpi_selected) {
                    *v = (*v).saturating_sub(50).max(200);
                    self.dirty = true;
                }
            }
            KeyCode::Right => {
                if let Some(v) = self.config.dpi.profiles.get_mut(self.dpi_selected) {
                    *v = (*v + 50).min(8000);
                    self.dirty = true;
                }
            }
            KeyCode::Enter => {
                if self.dpi_selected < profile_count {
                    self.config.dpi.active = self.dpi_selected;
                    self.dirty = true;
                }
            }
            KeyCode::Char('+') => {
                if self.config.dpi.profiles.len() < 5 {
                    self.config.dpi.profiles.push(1600);
                    self.dirty = true;
                }
            }
            KeyCode::Char('-') => {
                if profile_count > 1 {
                    self.config.dpi.profiles.remove(self.dpi_selected);
                    if self.dpi_selected >= self.config.dpi.profiles.len() {
                        self.dpi_selected = self.config.dpi.profiles.len().saturating_sub(1);
                    }
                    if self.config.dpi.active >= self.config.dpi.profiles.len() {
                        self.config.dpi.active = self.config.dpi.profiles.len().saturating_sub(1);
                    }
                    self.dirty = true;
                }
            }
            _ => {}
        }
    }

    pub fn apply_to_device(&mut self) {
        if self.applying {
            return;
        }
        let Some(device_arc) = self.device.clone() else {
            self.status = Some("No device connected".into());
            return;
        };
        let cfg = self.config.clone();
        let (tx, rx) = oneshot::channel();
        self.apply_rx = Some(rx);
        self.applying = true;
        self.status = Some("Applying...".into());
        tokio::spawn(async move {
            let result = tokio::task::spawn_blocking(move || {
                let mut d = device_arc.lock().unwrap();
                apply_config_to_device(&mut d, &cfg).map(|()| cfg)
            })
            .await
            .unwrap_or_else(|e| Err(HidError::Io(std::io::Error::other(e.to_string()))));
            let _ = tx.send(result);
        });
    }

    /// Poll for a completed apply. Call once per event-loop tick.
    pub fn poll_apply(&mut self) {
        let Some(rx) = self.apply_rx.as_mut() else {
            return;
        };
        match rx.try_recv() {
            Ok(Ok(cfg)) => {
                *self.shared_config.write().unwrap() = cfg;
                self.applying = false;
                self.apply_rx = None;
                self.status = Some("Applied to device".into());
            }
            Ok(Err(e)) => {
                self.applying = false;
                self.apply_rx = None;
                self.status = Some(format!("Device error: {e}"));
            }
            Err(oneshot::error::TryRecvError::Empty) => {} // still running
            Err(oneshot::error::TryRecvError::Closed) => {
                self.applying = false;
                self.apply_rx = None;
            }
        }
    }

    pub fn save_config(&mut self) {
        match self.config.save() {
            Ok(()) => {
                self.dirty = false;
                // Update the monitor's config immediately — button mappings
                // work via evdev and don't need a successful HID++ apply.
                *self.shared_config.write().unwrap() = self.config.clone();
                self.status = Some("Config saved".into());
            }
            Err(e) => {
                self.status = Some(format!("Save error: {e}"));
            }
        }
    }

    fn reset_tab(&mut self) {
        match self.active_tab {
            Tab::Buttons => {
                self.config.buttons = Default::default();
            }
            Tab::Scroll => {
                self.config.scroll = Default::default();
            }
            Tab::ThumbWheel => {
                self.config.thumb_wheel = Default::default();
            }
            Tab::Dpi => {
                self.config.dpi = Default::default();
            }
        }
        self.dirty = true;
        self.status = Some("Tab reset to defaults".into());
    }
}

fn read_battery_sysfs() -> anyhow::Result<BatteryStatus> {
    for entry in std::fs::read_dir("/sys/class/power_supply")? {
        let path = entry?.path();
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        if !name.starts_with("hidpp_battery") {
            continue;
        }
        let level: u8 = std::fs::read_to_string(path.join("capacity"))?
            .trim()
            .parse()?;
        let status = std::fs::read_to_string(path.join("status"))?
            .trim()
            .to_lowercase();
        return Ok(BatteryStatus {
            level: Some(level),
            charging: status == "charging",
            charge_complete: status == "full",
        });
    }
    anyhow::bail!("no hidpp_battery found in /sys/class/power_supply")
}

fn apply_config_to_device(device: &mut MxMaster, cfg: &Config) -> Result<(), HidError> {
    // HID++ commands may silently fail over BT (kernel driver intercepts responses).
    // We log warnings but never propagate Protocol errors — button mappings work
    // via evdev regardless.
    macro_rules! hidpp {
        ($e:expr) => {
            if let Err(HidError::Protocol(_) | HidError::Io(_)) = $e {
                tracing::debug!("HID++ command skipped (BT limitation)");
            } else if let Err(e) = $e {
                return Err(e);
            }
        };
    }

    if let Some(&dpi) = cfg.dpi.profiles.get(cfg.dpi.active) {
        hidpp!(set_dpi(device, dpi));
    }
    hidpp!(set_smart_shift(device, cfg.scroll.smart_shift_threshold));
    hidpp!(set_hi_res_scroll(device, cfg.scroll.hi_res_multiplier));
    hidpp!(set_thumb_wheel(
        device,
        cfg.thumb_wheel.invert,
        cfg.thumb_wheel.sensitivity
    ));

    let layout = button_layout(device.model);
    hidpp!(remap_button(
        device,
        layout.gesture_button,
        &cfg.buttons.gesture_button
    ));
    hidpp!(remap_button(
        device,
        layout.thumb_button,
        &cfg.buttons.thumb_button
    ));
    hidpp!(remap_button(device, layout.back, &cfg.buttons.back_button));
    hidpp!(remap_button(
        device,
        layout.forward,
        &cfg.buttons.forward_button
    ));
    hidpp!(remap_button(
        device,
        layout.smart_shift,
        &cfg.buttons.smart_shift
    ));

    Ok(())
}

/// Run the ratatui event loop until the user quits.
pub async fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    cfg: Config,
    device: Option<Arc<Mutex<MxMaster>>>,
    device_name: &'static str,
    shared_config: Arc<RwLock<Config>>,
) -> anyhow::Result<()> {
    let mut app = App::new(cfg, device, device_name, shared_config);
    app.refresh_battery();

    loop {
        app.tick = app.tick.wrapping_add(1);
        app.poll_apply();

        // Refresh battery every ~30 s (300 ticks × 100 ms)
        app.battery_ticks += 1;
        if app.battery_ticks >= 300 {
            app.battery_ticks = 0;
            app.refresh_battery();
        }

        terminal
            .draw(|f| render(f, &mut app))
            .context("drawing frame")?;

        if event::poll(std::time::Duration::from_millis(100)).context("polling for events")? {
            let ev = event::read().context("reading event")?;
            app.handle_event(ev)?;
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}
