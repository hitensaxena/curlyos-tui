//! curlyos-tui — a terminal dashboard for curlyos-core.
//!
//! Architecture: the UI thread owns terminal + state and stays responsive by
//! delegating every network call to a single background worker thread
//! (`worker.rs`) over channels. The worker drains requests FIFO, so responses
//! arrive in request order — no staleness races.

mod api;
mod app;
mod ui;
mod worker;

use anyhow::Result;
use app::App;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::stdout;
use std::sync::mpsc;
use std::time::{Duration, Instant};

fn main() -> Result<()> {
    let base = std::env::var("CURLYOS_API")
        .unwrap_or_else(|_| "http://127.0.0.1:8643".to_string());

    if std::env::args().nth(1).as_deref() == Some("selftest") {
        return selftest(&base);
    }

    // Channels: UI -> worker (Req), worker -> UI (Resp).
    let (req_tx, req_rx) = mpsc::channel();
    let (resp_tx, resp_rx) = mpsc::channel();
    worker::spawn(api::Client::new(base.clone()), req_rx, resp_tx);

    let mut app = App::new(req_tx, base);
    app.refresh(); // load Home

    // terminal setup
    enable_raw_mode()?;
    let mut out = stdout();
    execute!(out, EnterAlternateScreen, EnableMouseCapture)?;
    let mut term = Terminal::new(CrosstermBackend::new(out))?;

    let res = run(&mut term, &mut app, &resp_rx);

    // teardown (always, even on error)
    disable_raw_mode()?;
    execute!(term.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    term.show_cursor()?;

    if let Err(e) = res {
        eprintln!("error: {e:#}");
    }
    Ok(())
}

/// Headless check that every read endpoint deserializes against the live API.
fn selftest(base: &str) -> Result<()> {
    let c = api::Client::new(base.to_string());
    println!("curlyos-tui selftest → {base}");
    let h = c.health()?;
    println!("  health      pg={} redis={} embedder={}", h.postgres.ok, h.redis.ok, h.embedder.ok);
    let s = c.stats()?;
    println!("  stats       mem={} epi={} idf={} ent={} edge={}", s.memories, s.episodes, s.identity_facts, s.knowledge_entities, s.knowledge_edges);
    println!("  composition {} statuses", c.composition()?.memories_by_status.len());
    println!("  memories    {} rows", c.memories(&api::MemoryQuery::default())?.len());
    println!("  episodes    {} rows", c.episodes(5, 0)?.len());
    println!("  identity    {} facts", c.identity()?.len());
    println!("  principles  {}", c.principles()?.len());
    println!("  narrative   {} chapters", c.narrative()?.len());
    let a = c.attention()?;
    println!("  attention   load={:.2} focus={}", a.cognitive_load.score, a.focus_areas.len());
    println!("  reflections {}", c.reflections()?.len());
    println!("  graph       {} nodes", c.graph(20)?.nodes.len());
    println!("  recall      {} hits", c.recall("who am I", "fast", 3)?.len());
    let sys = c.systems()?;
    println!("  systems     {} infra · {} engines · sched_running={}", sys.infrastructure.len(), sys.engines.len(), sys.scheduler.running);
    println!("  scheduler   {} cron jobs", c.scheduler()?.jobs.len());
    println!("  events      {} recent", c.events(10)?.len());
    let srcs = c.log_sources()?;
    println!("  logsources  {}", srcs.len());
    if let Some(s) = srcs.first() {
        println!("  logs[{}]    {} lines", s.name, c.logs(&s.name, 50)?.lines.len());
    }
    let runs = c.agent_runs(10)?;
    println!("  agent runs  {}", runs.len());
    if let Some(r) = runs.first() {
        println!("  run detail  {} actions", c.agent_run(&r.id)?.actions.len());
    }
    println!("  sched jobs  {}", c.scheduled_jobs()?.len());
    println!("OK");
    Ok(())
}

fn run<B: ratatui::backend::Backend>(
    term: &mut Terminal<B>,
    app: &mut App,
    resp_rx: &mpsc::Receiver<worker::Resp>,
) -> Result<()> {
    const TICK: Duration = Duration::from_secs(3);
    let mut last_tick = Instant::now();
    loop {
        term.draw(|f| ui::draw(f, app))?;

        // Drain any worker responses without blocking.
        while let Ok(resp) = resp_rx.try_recv() {
            app.apply(resp);
        }

        // Poll for input with a short timeout so responses get applied promptly.
        if event::poll(Duration::from_millis(120))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    app.on_key(key);
                }
            }
        }

        // Auto-refresh live views on a timer.
        if last_tick.elapsed() >= TICK {
            app.auto_refresh();
            last_tick = Instant::now();
        }

        if app.should_quit {
            break;
        }
    }
    Ok(())
}
