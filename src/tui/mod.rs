//! TUI entry point: terminal setup, panic hook, and event-loop orchestration.

use std::io;

use anyhow::Context;
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

use std::sync::{Arc, Mutex, RwLock};

use crate::config::Config;
use crate::hid::MxMaster;

pub mod app;
pub mod tabs;
pub mod ui;

use app::run_app;

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Initialise the terminal and run the interactive TUI.
pub async fn run(cfg: Config, device: Option<Arc<Mutex<MxMaster>>>, device_name: &'static str, shared_config: Arc<RwLock<Config>>) -> anyhow::Result<()> {
    // Set up raw mode + alternate screen
    enable_raw_mode().context("enabling raw mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).context("entering alternate screen")?;

    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend).context("creating terminal")?;

    // Install a panic hook that restores the terminal before unwinding
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        // Best-effort restore — ignore errors inside the panic hook
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        original_hook(info);
    }));

    let result = run_app(&mut terminal, cfg, device, device_name, shared_config).await;

    // Restore terminal
    disable_raw_mode().context("disabling raw mode")?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)
        .context("leaving alternate screen")?;
    terminal.show_cursor().context("showing cursor")?;

    result
}
