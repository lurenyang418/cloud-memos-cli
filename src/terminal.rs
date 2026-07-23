use std::io::{self, Stdout};

use anyhow::{Context, Result};
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

pub type AppTerminal = Terminal<CrosstermBackend<Stdout>>;

pub struct TerminalSession {
    terminal: AppTerminal,
    active: bool,
}

impl TerminalSession {
    pub fn start() -> Result<Self> {
        enable_raw_mode().context("无法启用终端 raw mode")?;
        let mut stdout = io::stdout();
        if let Err(error) = execute!(stdout, EnterAlternateScreen) {
            let _ = disable_raw_mode();
            return Err(error).context("无法进入终端备用屏幕");
        }
        let mut terminal =
            Terminal::new(CrosstermBackend::new(stdout)).context("无法初始化终端")?;
        terminal.clear().context("无法清空终端")?;
        Ok(Self {
            terminal,
            active: true,
        })
    }

    pub fn terminal_mut(&mut self) -> &mut AppTerminal {
        &mut self.terminal
    }

    pub fn suspend(&mut self) -> Result<()> {
        if !self.active {
            return Ok(());
        }
        disable_raw_mode().context("无法暂停终端 raw mode")?;
        execute!(self.terminal.backend_mut(), LeaveAlternateScreen)
            .context("无法暂停终端备用屏幕")?;
        self.terminal.show_cursor().context("无法显示光标")?;
        self.active = false;
        Ok(())
    }

    pub fn resume(&mut self) -> Result<()> {
        if self.active {
            return Ok(());
        }
        enable_raw_mode().context("无法恢复终端 raw mode")?;
        execute!(self.terminal.backend_mut(), EnterAlternateScreen)
            .context("无法恢复终端备用屏幕")?;
        self.terminal.clear().context("无法清空终端")?;
        self.active = true;
        Ok(())
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        if self.active {
            let _ = disable_raw_mode();
            let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
            let _ = self.terminal.show_cursor();
        }
    }
}
