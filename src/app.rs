use std::io::Stdout;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use rand::RngExt;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::widgets::Paragraph;
use ratatui::{Frame, Terminal};

use crate::config::Config;
use crate::fire::ParticleSystem;
use crate::ingest::TokenBurnEvent;

const TICK_RATE: Duration = Duration::from_millis(33); // ~30fps
const TICK_SECONDS: f32 = TICK_RATE.as_millis() as f32 / 1000.0;

// Real turns run ~20k–285k tokens (cache reads dominate; median ~75k) —
// sample log-uniformly across that range so demo bursts cover the same
// spread as real ones do.
const DEMO_TOKEN_MAGNITUDE: std::ops::Range<f32> = 4.1..5.5;

type AppTerminal = Terminal<CrosstermBackend<Stdout>>;

pub struct App {
    should_quit: bool,
    fire: ParticleSystem,
    area: Rect,
    demo: bool,
    events: Option<Receiver<TokenBurnEvent>>,
    session_tokens: u64,
    last_burst_tokens: u64,
    config: Config,
    // None until the first burst, so a freshly started app flickers gently
    // rather than opening on a full smoulder it hasn't earned yet.
    last_burst_at: Option<Instant>,
}

impl App {
    /// Synthetic mode: no real ingestion — space bar fires a random demo
    /// burst. Used for development and screenshots.
    pub fn demo(config: Config) -> Self {
        Self {
            should_quit: false,
            fire: ParticleSystem::new(),
            area: Rect::default(),
            demo: true,
            events: None,
            session_tokens: 0,
            last_burst_tokens: 0,
            config,
            last_burst_at: None,
        }
    }

    /// Live mode: the fire is driven by real `TokenBurnEvent`s arriving on
    /// `events` from a background ingestion thread.
    pub fn live(events: Receiver<TokenBurnEvent>, config: Config) -> Self {
        Self {
            should_quit: false,
            fire: ParticleSystem::new(),
            area: Rect::default(),
            demo: false,
            events: Some(events),
            session_tokens: 0,
            last_burst_tokens: 0,
            config,
            last_burst_at: None,
        }
    }

    pub fn run(mut self, terminal: &mut AppTerminal) -> std::io::Result<()> {
        let mut last_tick = Instant::now();
        while !self.should_quit {
            // ratatui re-queries the terminal size on every draw(), so a
            // resized terminal is picked up automatically — no explicit
            // resize handling needed here.
            terminal.draw(|frame| self.draw(frame))?;

            let timeout = TICK_RATE.saturating_sub(last_tick.elapsed());
            if event::poll(timeout)? {
                self.handle_event(event::read()?);
            }

            if last_tick.elapsed() >= TICK_RATE {
                self.tick();
                last_tick = Instant::now();
            }
        }
        Ok(())
    }

    fn handle_event(&mut self, event: Event) {
        let Event::Key(key) = event else { return };
        if key.kind != KeyEventKind::Press {
            return;
        }
        let is_quit = key.code == KeyCode::Char('q')
            || (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL));
        if is_quit {
            self.should_quit = true;
            return;
        }
        if self.demo && key.code == KeyCode::Char(' ') {
            self.synthetic_burst();
        }
    }

    /// Demo-only: fires a burst as if a real Claude Code turn had just
    /// burned a random number of tokens. Replaced by real token events once
    /// ingestion is wired up in Phase 3.
    fn synthetic_burst(&mut self) {
        let mut rng = rand::rng();
        let magnitude = rng.random_range(DEMO_TOKEN_MAGNITUDE);
        let tokens = 10f32.powf(magnitude) as u64;
        self.record_burst(tokens);
        self.fire.burst(self.area.width, tokens);
    }

    fn tick(&mut self) {
        self.drain_events();
        self.fire
            .spawn_ambient(self.area.width, self.ambient_rate(), TICK_SECONDS);
        self.fire.update(TICK_SECONDS);
    }

    /// Full smoulder while a burst happened recently; once the session has
    /// been quiet for `idle.after_seconds`, die down to a gentle flicker.
    fn ambient_rate(&self) -> f32 {
        let idle = &self.config.idle;
        match self.last_burst_at {
            Some(at) if at.elapsed().as_secs_f32() < idle.after_seconds => {
                idle.active_embers_per_second
            }
            _ => idle.embers_per_second,
        }
    }

    /// Turns any real token events received since the last tick into bursts.
    /// Collects into a `Vec` first so the immutable borrow of `self.events`
    /// ends before `self.fire.burst` needs `&mut self`.
    fn drain_events(&mut self) {
        let Some(events) = &self.events else { return };
        let bursts: Vec<u64> = events
            .try_iter()
            .map(|event| event.total_tokens())
            .collect();
        for tokens in bursts {
            self.record_burst(tokens);
            self.fire.burst(self.area.width, tokens);
        }
    }

    fn record_burst(&mut self, tokens: u64) {
        self.last_burst_tokens = tokens;
        self.session_tokens = self.session_tokens.saturating_add(tokens);
        self.last_burst_at = Some(Instant::now());
    }

    fn draw(&mut self, frame: &mut Frame) {
        self.area = frame.area();
        self.fire.render(self.area, frame.buffer_mut());
        let mode = if self.demo { " [demo]" } else { "" };
        let keys = if self.demo {
            "q to quit, space for a burst"
        } else {
            "q to quit"
        };
        let status = format!(
            "burnr{mode} — burned {} · last {} — {keys}",
            humanise_tokens(self.session_tokens),
            humanise_tokens(self.last_burst_tokens),
        );
        frame.render_widget(Paragraph::new(status), self.area);
    }
}

/// Compact human-readable token counts for the status line: 950, 24.6k, 1.3M.
fn humanise_tokens(tokens: u64) -> String {
    match tokens {
        0..=999 => tokens.to_string(),
        1_000..=999_999 => format!("{:.1}k", tokens as f64 / 1_000.0),
        _ => format!("{:.1}M", tokens as f64 / 1_000_000.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn humanise_tokens_covers_each_magnitude() {
        assert_eq!(humanise_tokens(0), "0");
        assert_eq!(humanise_tokens(950), "950");
        assert_eq!(humanise_tokens(24_600), "24.6k");
        assert_eq!(humanise_tokens(1_300_000), "1.3M");
    }

    #[test]
    fn record_burst_accumulates_session_total_and_tracks_last() {
        let mut app = App::demo(Config::default());
        app.record_burst(50_000);
        app.record_burst(70_000);
        assert_eq!(app.session_tokens, 120_000);
        assert_eq!(app.last_burst_tokens, 70_000);
    }

    #[test]
    fn ambient_rate_flickers_before_any_burst() {
        let app = App::demo(Config::default());
        assert_eq!(app.ambient_rate(), app.config.idle.embers_per_second);
    }

    #[test]
    fn ambient_rate_is_active_after_a_recent_burst() {
        let mut app = App::demo(Config::default());
        app.record_burst(50_000);
        assert_eq!(app.ambient_rate(), app.config.idle.active_embers_per_second);
    }

    #[test]
    fn ambient_rate_returns_to_flicker_once_idle() {
        let mut app = App::demo(Config::default());
        app.config.idle.after_seconds = 0.0;
        app.record_burst(50_000);
        assert_eq!(app.ambient_rate(), app.config.idle.embers_per_second);
    }
}
