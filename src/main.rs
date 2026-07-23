use std::time::Duration;

use anyhow::{Context, Result};
use clap::Parser;
use cloud_memos_cli::{
    ApiClient, App, EventOutcome, KeyringSecretStore,
    cli::{Cli, Command, handle_profile_command, resolve_runtime_credentials},
    editor::edit_externally,
    terminal::TerminalSession,
    ui,
};
use crossterm::event::{self, Event, KeyEventKind};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let config_path = cloud_memos_cli::Config::path()?;
    let store = KeyringSecretStore;

    if let Some(Command::Profile { command }) = cli.command {
        if cli.profile.is_some() {
            anyhow::bail!("--profile 只用于启动 TUI，不能与 profile 管理命令同时使用");
        }
        return handle_profile_command(command, &config_path, &store).await;
    }

    let runtime = resolve_runtime_credentials(cli.profile.as_deref(), &config_path, &store)?;
    let client = ApiClient::new(runtime.url, runtime.token)?;
    let mut app = App::new(client, runtime.mode.into());
    app.bootstrap()
        .await
        .with_context(|| format!("无法启动 profile “{}”", runtime.label))?;
    run_tui(&mut app).await
}

async fn run_tui(app: &mut App) -> Result<()> {
    let mut terminal = TerminalSession::start()?;
    while !app.should_quit {
        terminal
            .terminal_mut()
            .draw(|frame| ui::render(frame, app))
            .context("无法绘制终端界面")?;

        if !event::poll(Duration::from_millis(200)).context("无法轮询终端事件")? {
            continue;
        }
        let Event::Key(key) = event::read().context("无法读取终端事件")? else {
            continue;
        };
        if key.kind == KeyEventKind::Release {
            continue;
        }
        if app.handle_key(key).await == EventOutcome::OpenExternalEditor {
            let Some(content) = app.editor_content() else {
                continue;
            };
            terminal.suspend()?;
            let edit_result = edit_externally(&content);
            let resume_result = terminal.resume();
            resume_result?;
            match edit_result {
                Ok(edited) => app.replace_editor_content(edited),
                Err(error) => app.set_status_error(error.to_string()),
            }
        }
    }
    Ok(())
}
