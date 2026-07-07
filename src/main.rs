mod app;
mod config;
mod fire;
mod ingest;

use std::io;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;

use clap::Parser;
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use app::App;

/// A bonfire of glyphs in your terminal, burning the tokens your Claude
/// Code session burns. Attaches to the current project's latest session
/// transcript by default.
#[derive(Parser)]
#[command(version, about)]
struct Cli {
    /// Synthetic mode: no real ingestion, space bar fires a burst
    #[arg(long)]
    demo: bool,

    /// Watch one specific session transcript (.jsonl) — no auto-detection,
    /// no rotation to newer sessions
    #[arg(long, value_name = "PATH", conflicts_with = "demo")]
    session: Option<PathBuf>,

    /// Watch the latest session of this project directory instead of the
    /// current working directory
    #[arg(long, value_name = "DIR", conflicts_with_all = ["demo", "session"])]
    project: Option<PathBuf>,
}

fn main() -> io::Result<()> {
    let cli = Cli::parse();
    install_panic_hook();

    // Config and ingestion errors surface here, before the terminal is
    // touched, so the user actually sees them.
    let config = config::Config::load()?;
    let app = if cli.demo {
        App::demo(config)
    } else {
        App::live(spawn_live_watcher(&cli)?, config)
    };

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = app.run(&mut terminal);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

/// Starts the background ingestion thread for whichever source the CLI
/// selected: a pinned transcript (`--session`), a named project directory
/// (`--project`), or the current working directory's latest session.
fn spawn_live_watcher(cli: &Cli) -> io::Result<Receiver<ingest::TokenBurnEvent>> {
    if let Some(session) = &cli.session {
        return ingest::spawn_session_watcher(session.clone());
    }
    // Canonicalised because the log-dir name is derived from the absolute
    // path — a relative `--project` would otherwise slug to the wrong dir.
    let project_dir = match &cli.project {
        Some(dir) => dir.canonicalize()?,
        None => std::env::current_dir()?,
    };
    let log_dir = ingest::project_log_dir(&project_dir).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "could not determine home directory for Claude Code session logs",
        )
    })?;
    Ok(ingest::spawn_watcher(log_dir))
}

// Without this, a panic mid-run leaves the terminal stuck in raw mode /
// the alternate screen, so the panic message is invisible and the shell
// is left in a broken state until the user runs `reset`.
fn install_panic_hook() {
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        original_hook(panic_info);
    }));
}
