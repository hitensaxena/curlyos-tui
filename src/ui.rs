//! All rendering. Pure functions of `&App` (plus mutable `Sel` state for lists).

use crate::api::*;
use crate::app::{truncate, App, Detail, Overlay, Tab, MEM_SUBS, MIND_SUBS, SYS_SUBS, TABS};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{
        Block, BorderType, Borders, Clear, Gauge, List, ListItem, Paragraph, Tabs, Wrap,
    },
    Frame,
};

// ── palette ────────────────────────────────────────────────────────────────
const GREEN: Color = Color::Rgb(64, 250, 146); // bright primary accent
const MINT: Color = Color::Rgb(110, 240, 180);
const CYAN: Color = Color::Rgb(86, 230, 244);
const LIME: Color = Color::Rgb(178, 245, 96);
const CORAL: Color = Color::Rgb(255, 138, 110);
const PERI: Color = Color::Rgb(132, 170, 255);
const PURPLE: Color = Color::Rgb(192, 150, 255);
const AMBER: Color = Color::Rgb(255, 198, 92);
const RED: Color = Color::Rgb(255, 104, 104);
const TEXT: Color = Color::Rgb(218, 224, 234);
const DIM: Color = Color::Rgb(128, 138, 158);
const FAINT: Color = Color::Rgb(78, 86, 104);
const SEL_BG: Color = Color::Rgb(30, 40, 52);

const BAR: &str = "▌";
const SPARK: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

pub fn draw(f: &mut Frame, app: &mut App) {
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // header / tabs
            Constraint::Min(0),    // body
            Constraint::Length(1), // footer
        ])
        .split(f.area());

    draw_header(f, app, root[0]);
    match app.tab {
        Tab::Home => draw_home(f, app, root[1]),
        Tab::Memory => draw_memory(f, app, root[1]),
        Tab::Mind => draw_mind(f, app, root[1]),
        Tab::Graph => draw_graph(f, app, root[1]),
        Tab::Systems => draw_systems(f, app, root[1]),
    }
    draw_footer(f, app, root[2]);

    match &app.overlay {
        Overlay::Help => draw_help(f),
        Overlay::Detail(d) => draw_detail(f, d),
        Overlay::Form(_) => draw_form(f, app),
        Overlay::Confirm { msg, .. } => draw_confirm(f, msg),
        Overlay::Menu { .. } => draw_menu(f, app),
        Overlay::None => {}
    }
}

// ── header ───────────────────────────────────────────────────────────────────
fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(34)])
        .split(area);

    let titles: Vec<Line> = TABS
        .iter()
        .enumerate()
        .map(|(i, t)| Line::from(format!(" {} {} ", i + 1, t)))
        .collect();
    let host = app.base.strip_prefix("http://").or_else(|| app.base.strip_prefix("https://")).unwrap_or(&app.base);
    let tabs = Tabs::new(titles)
        .select(app.tab.index())
        .divider(Span::styled("·", Style::default().fg(FAINT)))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(FAINT))
                .title(Span::styled(format!(" curlyos ⟡ {host} "), Style::default().fg(CORAL).bold())),
        )
        .style(Style::default().fg(DIM))
        .highlight_style(Style::default().fg(Color::Black).bg(CORAL).bold());
    f.render_widget(tabs, cols[0]);

    // live status chip: scheduler + sync
    let mut spans = vec![Span::raw(" ")];
    if let Some(sys) = &app.systems {
        let (dot, col) = if sys.scheduler.running { ("●", MINT) } else { ("●", RED) };
        spans.push(Span::styled(format!("{dot} "), Style::default().fg(col)));
        spans.push(Span::styled(
            format!("sched · {} jobs", sys.scheduler.jobs),
            Style::default().fg(TEXT),
        ));
        if !sys.scheduler.failing.is_empty() {
            spans.push(Span::styled(
                format!(" · {}✗", sys.scheduler.failing.len()),
                Style::default().fg(RED).bold(),
            ));
        }
    } else {
        spans.push(Span::styled("connecting…", Style::default().fg(DIM)));
    }
    if app.loading() {
        spans.push(Span::styled("  ◍", Style::default().fg(AMBER)));
    }
    let chip = Paragraph::new(Line::from(spans)).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(FAINT)),
    );
    f.render_widget(chip, cols[1]);
}

fn draw_footer(f: &mut Frame, app: &App, area: Rect) {
    if let Some((msg, is_err)) = &app.status {
        let color = if *is_err { RED } else { MINT };
        let prefix = if *is_err { " ✗ " } else { " ✓ " };
        let line = Line::from(vec![
            Span::styled(prefix, Style::default().fg(color).bold()),
            Span::styled(truncate(msg, 220), Style::default().fg(color)),
        ]);
        f.render_widget(Paragraph::new(line), area);
        return;
    }
    let hints: &str = match (app.tab, app.cur_sub()) {
        _ if app.recall_editing => "type your query · enter search · esc cancel",
        _ if app.graph_editing => "type to filter entities · enter apply · esc clear",
        (Tab::Home, _) => "1-5 spaces · h/l sub-view · r refresh",
        (Tab::Memory, 0) => "enter detail · / search · v validity · s status · n/p page · i invalidate",
        (Tab::Memory, 1) => "enter detail · n/p page",
        (Tab::Memory, _) => "/ edit query · m mode · enter open memory",
        (Tab::Mind, 0) => "h/l explore · a propose identity · t trigger cognition job",
        (Tab::Mind, 1) => "↑↓ browse identity · a propose · t trigger",
        (Tab::Mind, _) => "↑↓ browse · t trigger reflection/narrative/consolidation",
        (Tab::Graph, _) => "↵ explore connections · ⌫/esc back · / search · ↑↓ move",
        (Tab::Systems, 1) => "enter inspect · c cancel running · esc close · auto-refreshing",
        (Tab::Systems, 2) => "↑↓ jobs · e toggle · x run now · d delete · n new",
        (Tab::Systems, 3) => "↑↓ events · c cycle category filter",
        (Tab::Systems, 4) => "s cycle log source",
        (Tab::Systems, 5) => "LLM routing health · per-tier usage · auto-refreshing",
        (Tab::Systems, 6) => "ingest pipeline backlog + recall cache · auto-refreshing",
        (Tab::Systems, 7) => "↑↓ settings · ↵ toggle/edit · changes apply live",
        (Tab::Systems, _) => "h/l sub-view · live ops monitor",
    };
    let line = Line::from(vec![
        Span::styled(" ? ", Style::default().fg(Color::Black).bg(PERI).bold()),
        Span::styled(" help ", Style::default().fg(DIM)),
        Span::styled(hints, Style::default().fg(DIM)),
        Span::styled("  ·  A capture · q quit", Style::default().fg(FAINT)),
    ]);
    f.render_widget(Paragraph::new(line), area);
}

// ── shared widgets ───────────────────────────────────────────────────────────
fn panel(title: &str) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(FAINT))
        .title(Span::styled(format!(" {title} "), Style::default().fg(PERI).bold()))
}

fn sel_style() -> Style {
    Style::default().bg(SEL_BG).add_modifier(Modifier::BOLD)
}

fn list_of<'a>(title: String, items: Vec<ListItem<'a>>) -> List<'a> {
    List::new(items)
        .block(panel(&title))
        .highlight_style(sel_style())
        .highlight_symbol(BAR)
}

fn subtabs(labels: &[&str], active: usize) -> Line<'static> {
    let mut spans = vec![];
    for (i, s) in labels.iter().enumerate() {
        let style = if i == active {
            Style::default().fg(Color::Black).bg(MINT).bold()
        } else {
            Style::default().fg(DIM)
        };
        spans.push(Span::styled(format!(" {s} "), style));
        spans.push(Span::styled(" ", Style::default()));
    }
    Line::from(spans)
}

// ── Home: system monitor ─────────────────────────────────────────────────────
fn draw_home(f: &mut Frame, app: &App, area: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(6), Constraint::Length(13), Constraint::Min(0)])
        .split(area);

    draw_home_kpis(f, app, rows[0]);

    let mid = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(38), Constraint::Percentage(32), Constraint::Percentage(30)])
        .split(rows[1]);
    draw_home_activity(f, app, mid[0]);
    draw_home_running(f, app, mid[1]);
    draw_home_signals(f, app, mid[2]);

    let bottom = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(rows[2]);
    draw_home_knowledge(f, app, bottom[0]);
    draw_home_composition(f, app, bottom[1]);
}

fn draw_home_kpis(f: &mut Frame, app: &App, area: Rect) {
    let s = &app.stats;
    let load = app.attention.as_ref().map(|a| a.cognitive_load.score).unwrap_or(0.0);
    let cards: [(&str, String, Color); 6] = [
        ("MEMORIES", fmt_int(s.memories), GREEN),
        ("EPISODES", fmt_int(s.episodes), CYAN),
        ("IDENTITY", fmt_int(s.identity_facts), PERI),
        ("ENTITIES", fmt_int(s.knowledge_entities), PURPLE),
        ("EDGES", fmt_int(s.knowledge_edges), LIME),
        ("COG LOAD", format!("{:.0}%", load * 100.0), load_color(load)),
    ];
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Ratio(1, 6); 6])
        .split(area);
    for (i, (label, val, col)) in cards.iter().enumerate() {
        let p = Paragraph::new(vec![
            Line::from(Span::styled(*label, Style::default().fg(DIM))),
            Line::from(Span::styled(val.clone(), Style::default().fg(*col).bold())),
        ])
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(FAINT)),
        );
        f.render_widget(p, cols[i]);
    }
}

fn draw_home_activity(f: &mut Frame, app: &App, area: Rect) {
    let (series, rate, span_secs) = activity_series(&app.events, 48);
    let mut lines = vec![
        Line::from(Span::styled(spark(&series), Style::default().fg(GREEN))),
        Line::from(vec![
            Span::styled(format!("{:.1}", rate), Style::default().fg(LIME).bold()),
            Span::styled(" events/min", Style::default().fg(DIM)),
            Span::styled(format!("  · {} events / {}", app.events.len(), human_span(span_secs)), Style::default().fg(FAINT)),
        ]),
        Line::from(""),
        head("Recent activity"),
    ];
    for e in app.events.iter().take(7) {
        lines.push(Line::from(vec![
            Span::styled(format!("  {} ", fmt_time(e.created_at.as_deref())), Style::default().fg(FAINT)),
            Span::styled(short_event(&e.etype), Style::default().fg(event_color(&e.etype))),
        ]));
    }
    f.render_widget(Paragraph::new(lines).block(panel("Activity")).wrap(Wrap { trim: true }), area);
}

fn draw_home_running(f: &mut Frame, app: &App, area: Rect) {
    let mut lines = vec![];
    // services strip
    if let Some(h) = &app.health {
        lines.push(Line::from(vec![
            svc_dot("pg", h.postgres.ok),
            svc_dot("redis", h.redis.ok),
            svc_dot("embed", h.embedder.ok),
        ]));
        lines.push(Line::from(""));
    }
    // scheduler
    if let Some(sys) = &app.systems {
        let col = if sys.scheduler.running { GREEN } else { RED };
        lines.push(Line::from(vec![
            Span::styled("  ● ", Style::default().fg(col)),
            Span::styled("scheduler ", Style::default().fg(TEXT)),
            Span::styled(if sys.scheduler.running { "running" } else { "stopped" }, Style::default().fg(col).bold()),
            Span::styled(format!("  {} jobs", sys.scheduler.jobs), Style::default().fg(DIM)),
        ]));
        lines.push(Line::from(Span::styled(format!("     next {}", fmt_date(sys.scheduler.next_due.as_deref())), Style::default().fg(FAINT))));
        lines.push(Line::from(""));
    }
    // running agents
    let running: Vec<&AgentRun> = app.agent_runs.iter().filter(|r| r.status.as_deref() == Some("running")).collect();
    lines.push(Line::from(vec![
        head_inline("Agents"),
        Span::styled(format!("  {} running", running.len()), Style::default().fg(if running.is_empty() { DIM } else { AMBER }).bold()),
    ]));
    if running.is_empty() {
        lines.push(Line::from(Span::styled("  ● idle", Style::default().fg(DIM))));
    } else {
        for r in running.iter().take(5) {
            lines.push(Line::from(vec![
                Span::styled("  ◌ ", Style::default().fg(AMBER)),
                Span::styled(format!("{} ", r.agent.clone().unwrap_or_default()), Style::default().fg(PURPLE).bold()),
                Span::styled(truncate(&r.task, 28), Style::default().fg(TEXT)),
            ]));
        }
    }
    f.render_widget(Paragraph::new(lines).block(panel("Running now")).wrap(Wrap { trim: true }), area);
}

fn draw_home_signals(f: &mut Frame, app: &App, area: Rect) {
    let s = &app.stats;
    let density = if s.knowledge_entities > 0 { s.knowledge_edges as f64 / s.knowledge_entities as f64 } else { 0.0 };
    let extraction = if s.episodes > 0 { s.memories as f64 / s.episodes as f64 } else { 0.0 };
    let belief = *app.comp.memories_by_status.get("belief").unwrap_or(&0);
    let hypoth = *app.comp.memories_by_status.get("hypothesis").unwrap_or(&0);
    let engine_24h: i64 = app.systems.as_ref().map(|s| s.engines.iter().map(|e| e.runs_24h).sum()).unwrap_or(0);
    let (types, concentration) = app.attention.as_ref().map(|a| (a.breadth.distinct_types, a.breadth.concentration)).unwrap_or((0, 0.0));

    let lines = vec![
        signal("graph density", &format!("{density:.2}"), "edges/entity", CYAN),
        signal("extraction", &format!("{extraction:.1}"), "mem/episode", GREEN),
        signal("uncertain", &fmt_int(belief + hypoth), "belief+hypoth", AMBER),
        signal("changed 7d", &format!("+{}", fmt_int(app.comp.memories_changed_7d)), "memories", LIME),
        signal("engine runs", &fmt_int(engine_24h), "last 24h", PERI),
        signal("breadth", &fmt_int(types), "entity types", PURPLE),
        signal("focus conc.", &format!("{concentration:.2}"), "0=broad 1=narrow", CORAL),
    ];
    f.render_widget(Paragraph::new(lines).block(panel("Signals")).wrap(Wrap { trim: true }), area);
}

fn draw_home_knowledge(f: &mut Frame, app: &App, area: Rect) {
    let mut lines = vec![];
    if let Some(a) = &app.attention {
        let max = a.breadth.by_type.values().copied().max().unwrap_or(1);
        // sort by count desc
        let mut entries: Vec<(&String, &i64)> = a.breadth.by_type.iter().collect();
        entries.sort_by(|x, y| y.1.cmp(x.1));
        for (k, v) in entries.into_iter().take(12) {
            lines.push(hbar(k, *v, max, 18, type_color(k)));
        }
    } else {
        lines.push(Line::from(Span::styled("loading knowledge breadth…", Style::default().fg(DIM))));
    }
    f.render_widget(Paragraph::new(lines).block(panel("Knowledge graph · entities by type")), area);
}

fn draw_home_composition(f: &mut Frame, app: &App, area: Rect) {
    let mut lines = vec![head("Memories by status")];
    let mmax = app.comp.memories_by_status.values().copied().max().unwrap_or(1);
    for (k, v) in &app.comp.memories_by_status {
        lines.push(hbar(k, *v, mmax, 18, status_color(k)));
    }
    lines.push(Line::from(""));
    lines.push(head("Identity by status"));
    let imax = app.comp.identity_by_status.values().copied().max().unwrap_or(1);
    for (k, v) in &app.comp.identity_by_status {
        lines.push(hbar(k, *v, imax, 18, status_color(k)));
    }
    f.render_widget(Paragraph::new(lines).block(panel("Composition")).wrap(Wrap { trim: true }), area);
}

fn svc_dot(name: &str, ok: bool) -> Span<'static> {
    let col = if ok { GREEN } else { RED };
    Span::styled(format!("  ● {name}"), Style::default().fg(col))
}

fn head_inline(t: &str) -> Span<'static> {
    Span::styled(format!(" {t}"), Style::default().fg(CORAL).bold())
}

fn signal(label: &str, value: &str, note: &str, col: Color) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("  {value:>7} "), Style::default().fg(col).bold()),
        Span::styled(format!("{label:<13}"), Style::default().fg(TEXT)),
        Span::styled(note.to_string(), Style::default().fg(FAINT)),
    ])
}

fn load_color(l: f64) -> Color {
    if l > 0.8 {
        CORAL
    } else if l > 0.5 {
        AMBER
    } else {
        GREEN
    }
}

fn type_color(t: &str) -> Color {
    // stable-ish hue per type so the bars read as a legend
    let palette = [GREEN, CYAN, PERI, PURPLE, LIME, AMBER, CORAL, MINT];
    let idx = t.bytes().fold(0u32, |a, b| a.wrapping_add(b as u32)) as usize % palette.len();
    palette[idx]
}

fn status_color(s: &str) -> Color {
    match s {
        "canonical" => GREEN,
        "belief" => CYAN,
        "hypothesis" => AMBER,
        _ => DIM,
    }
}

// ── Memory space ─────────────────────────────────────────────────────────────
fn draw_memory(f: &mut Frame, app: &mut App, area: Rect) {
    let rows = subsplit(area);
    f.render_widget(Paragraph::new(subtabs(&MEM_SUBS, app.mem_sub)), rows[0]);
    match app.mem_sub {
        0 => draw_memories(f, app, rows[1]),
        1 => draw_episodes(f, app, rows[1]),
        _ => draw_recall(f, app, rows[1]),
    }
}

fn draw_memories(f: &mut Frame, app: &mut App, area: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1), Constraint::Min(0), Constraint::Length(9)])
        .split(area);

    let g = |k: &str| *app.comp.memories_by_status.get(k).unwrap_or(&0);
    f.render_widget(
        Paragraph::new(stat_bar(&[
            ("total", fmt_int(app.stats.memories), GREEN),
            ("canonical", fmt_int(g("canonical")), MINT),
            ("belief", fmt_int(g("belief")), CYAN),
            ("hypothesis", fmt_int(g("hypothesis")), AMBER),
            ("changed 7d", format!("+{}", fmt_int(app.comp.memories_changed_7d)), LIME),
        ])),
        rows[0],
    );

    let valid = match app.mq.valid {
        Some(true) => "live",
        Some(false) => "invalidated",
        None => "all",
    };
    let status = app.mq.epistemic_status.as_deref().unwrap_or("any");
    let page = app.mq.offset / app.mq.limit + 1;
    f.render_widget(
        Paragraph::new(Line::from(vec![
            filter_chip("validity", valid, app.mq.valid != Some(true)),
            filter_chip("status", status, app.mq.epistemic_status.is_some()),
            filter_chip("query", &app.mq.q.clone().unwrap_or_else(|| "—".into()), app.mq.q.is_some()),
            Span::styled(format!("  page {page} · {} shown", app.memories.len()), Style::default().fg(DIM)),
        ])),
        rows[1],
    );

    let items: Vec<ListItem> = app
        .memories
        .iter()
        .map(|m| {
            ListItem::new(Line::from(vec![
                status_badge(m.epistemic_status.as_deref()),
                Span::raw(" "),
                Span::styled(truncate(&m.statement, 200), Style::default().fg(TEXT)),
            ]))
        })
        .collect();
    f.render_stateful_widget(
        list_of(format!("Memories · {} shown", app.memories.len()), items),
        rows[2],
        &mut app.mem_sel.state,
    );

    let prev = app
        .mem_sel
        .selected()
        .and_then(|i| app.memories.get(i))
        .map(|m| {
            Text::from(vec![
                Line::from(Span::styled(
                    format!(
                        "{} · {} · {} · {}",
                        m.kind.clone().unwrap_or_default(),
                        m.tier.clone().unwrap_or_default(),
                        m.epistemic_status.clone().unwrap_or_default(),
                        fmt_date(m.created_at.as_deref()),
                    ),
                    Style::default().fg(DIM),
                )),
                Line::from(""),
                Line::from(Span::styled(m.statement.clone(), Style::default().fg(TEXT))),
            ])
        })
        .unwrap_or_else(|| Text::from("no selection"));
    f.render_widget(Paragraph::new(prev).block(panel("Preview")).wrap(Wrap { trim: true }), rows[3]);
}

fn status_badge(s: Option<&str>) -> Span<'static> {
    let (txt, col) = match s {
        Some("canonical") => ("CANON", MINT),
        Some("belief") => ("BELIEF", PERI),
        Some("hypothesis") => ("HYPOTH", AMBER),
        Some(other) => return Span::styled(format!("[{}]", other.to_uppercase()), Style::default().fg(DIM)),
        None => ("·", DIM),
    };
    Span::styled(format!("{txt:<6}"), Style::default().fg(col).bold())
}

fn draw_episodes(f: &mut Frame, app: &mut App, area: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(area);
    let extraction = if app.stats.episodes > 0 { app.stats.memories as f64 / app.stats.episodes as f64 } else { 0.0 };
    f.render_widget(
        Paragraph::new(stat_bar(&[
            ("episodes", fmt_int(app.stats.episodes), CYAN),
            ("memories/episode", format!("{extraction:.1}"), GREEN),
            ("page", (app.epi_offset / 50 + 1).to_string(), PERI),
        ])),
        rows[0],
    );

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(rows[1]);

    let items: Vec<ListItem> = app
        .episodes
        .iter()
        .map(|e| {
            ListItem::new(Line::from(vec![
                Span::styled(format!("{} ", fmt_date(e.created_at.as_deref())), Style::default().fg(DIM)),
                Span::styled(truncate(&e.content, 120), Style::default().fg(TEXT)),
            ]))
        })
        .collect();
    f.render_stateful_widget(
        list_of(format!("Episodes · {} shown", app.episodes.len()), items),
        cols[0],
        &mut app.epi_sel.state,
    );

    let prev = app
        .epi_sel
        .selected()
        .and_then(|i| app.episodes.get(i))
        .map(|e| {
            Text::from(vec![
                Line::from(Span::styled(format!("source: {}", e.source_ref.clone().unwrap_or_default()), Style::default().fg(MINT))),
                Line::from(Span::styled(
                    format!("{} · {}", e.modality.clone().unwrap_or_default(), fmt_date(e.created_at.as_deref())),
                    Style::default().fg(DIM),
                )),
                Line::from(""),
                Line::from(Span::styled(e.content.clone(), Style::default().fg(TEXT))),
            ])
        })
        .unwrap_or_else(|| Text::from("no selection"));
    f.render_widget(Paragraph::new(prev).block(panel("Episode")).wrap(Wrap { trim: true }), cols[1]);
}

// ── Mind space ───────────────────────────────────────────────────────────────
fn draw_mind(f: &mut Frame, app: &mut App, area: Rect) {
    let rows = subsplit(area);
    f.render_widget(Paragraph::new(subtabs(&MIND_SUBS, app.mind_sub)), rows[0]);
    match app.mind_sub {
        0 => draw_mind_overview(f, app, rows[1]),
        1 => draw_self(f, app, rows[1]),
        2 => draw_focus(f, app, rows[1]),
        3 => draw_story(f, app, rows[1]),
        _ => draw_insights(f, app, rows[1]),
    }
}

/// Principles sorted canonical-first (beliefs before hypotheses).
fn principles_ranked(app: &App) -> Vec<&Principle> {
    let rank = |s: Option<&str>| match s {
        Some("canonical") => 0,
        Some("belief") => 1,
        _ => 2,
    };
    let mut v: Vec<&Principle> = app.principles.iter().collect();
    v.sort_by_key(|p| rank(p.epistemic_status.as_deref()));
    v
}

/// The chapter the user is living now: the open one (no end_date), else newest.
fn current_chapter(app: &App) -> Option<&Chapter> {
    app.chapters
        .iter()
        .find(|c| c.end_date.is_none())
        .or_else(|| app.chapters.first())
}

// ── Mind · Overview (the monitor) ────────────────────────────────────────────
fn draw_mind_overview(f: &mut Frame, app: &App, area: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(6), Constraint::Length(13), Constraint::Min(0)])
        .split(area);

    // KPI cards
    let load = app.attention.as_ref().map(|a| a.cognitive_load.score).unwrap_or(0.0);
    let findings: usize = app.reports.iter().map(|r| r.findings.len()).sum();
    let cards: [(&str, String, Color); 5] = [
        ("MENTAL LOAD", format!("{:.0}%", load * 100.0), load_color(load)),
        ("IDENTITY", fmt_int(app.identity.len() as i64), PERI),
        ("PRINCIPLES", fmt_int(app.principles.len() as i64), PURPLE),
        ("CHAPTERS", fmt_int(app.chapters.len() as i64), CORAL),
        ("INSIGHTS", fmt_int(findings as i64), GREEN),
    ];
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Ratio(1, 5); 5])
        .split(rows[0]);
    for (i, (label, val, col)) in cards.iter().enumerate() {
        f.render_widget(
            Paragraph::new(vec![
                Line::from(Span::styled(*label, Style::default().fg(DIM))),
                Line::from(Span::styled(val.clone(), Style::default().fg(*col).bold())),
            ])
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).border_style(Style::default().fg(FAINT))),
            cols[i],
        );
    }

    // mid row: who I am · how I think · focus now
    let mid = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(34), Constraint::Percentage(33), Constraint::Percentage(33)])
        .split(rows[1]);

    let who: Vec<Line> = if app.identity.is_empty() {
        vec![Line::from(Span::styled("  loading…", Style::default().fg(DIM)))]
    } else {
        app.identity
            .iter()
            .take(8)
            .map(|fct| {
                Line::from(vec![
                    Span::styled(conf_dots(fct.confidence), Style::default().fg(conf_color(fct.confidence))),
                    Span::styled(format!(" {}", truncate(&fct.predicate, 16)), Style::default().fg(PERI).bold()),
                    Span::styled(format!("  {}", truncate(&fct.object, 26)), Style::default().fg(TEXT)),
                ])
            })
            .collect()
    };
    f.render_widget(Paragraph::new(who).block(panel("Who I am")).wrap(Wrap { trim: true }), mid[0]);

    let how: Vec<Line> = if app.principles.is_empty() {
        vec![Line::from(Span::styled("  loading…", Style::default().fg(DIM)))]
    } else {
        principles_ranked(app)
            .iter()
            .take(7)
            .map(|p| {
                Line::from(vec![
                    Span::styled("◆ ", Style::default().fg(PURPLE)),
                    Span::styled(truncate(&p.statement, 40), Style::default().fg(TEXT)),
                ])
            })
            .collect()
    };
    f.render_widget(Paragraph::new(how).block(panel("How I think")).wrap(Wrap { trim: true }), mid[1]);

    let mut focus: Vec<Line> = vec![];
    if let Some(a) = &app.attention {
        focus.push(Line::from(vec![
            Span::styled("  load ", Style::default().fg(DIM)),
            Span::styled(load_bar(a.cognitive_load.score, 10), Style::default().fg(load_color(a.cognitive_load.score))),
            Span::styled(format!(" {:.0}%", a.cognitive_load.score * 100.0), Style::default().fg(load_color(a.cognitive_load.score)).bold()),
        ]));
        focus.push(Line::from(""));
        for x in a.focus_areas.iter().take(6) {
            focus.push(Line::from(vec![
                Span::styled(format!("  {:>3} ", x.weight), Style::default().fg(GREEN).bold()),
                Span::styled(truncate(&x.name, 22), Style::default().fg(TEXT)),
            ]));
        }
    } else {
        focus.push(Line::from(Span::styled("  loading…", Style::default().fg(DIM))));
    }
    f.render_widget(Paragraph::new(focus).block(panel("What I'm on")), mid[2]);

    // bottom: current chapter · latest insight
    let bottom = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(rows[2]);

    let chap: Vec<Line> = match current_chapter(app) {
        Some(c) => vec![
            Line::from(Span::styled(c.title.clone(), Style::default().fg(CORAL).bold())),
            Line::from(Span::styled(
                format!("since {} · {}", fmt_date(c.start_date.as_deref()), if c.end_date.is_none() { "ongoing" } else { "closed" }),
                Style::default().fg(FAINT),
            )),
            Line::from(""),
            Line::from(Span::styled(c.summary.clone(), Style::default().fg(TEXT))),
        ],
        None => vec![Line::from(Span::styled("loading…", Style::default().fg(DIM)))],
    };
    f.render_widget(Paragraph::new(chap).block(panel("Where I am — current chapter")).wrap(Wrap { trim: true }), bottom[0]);

    let insight: Vec<Line> = match app.reports.first() {
        Some(r) => {
            let mut v = vec![Line::from(Span::styled(
                format!("{} · {}", r.report_type.clone().unwrap_or_default(), fmt_date(r.time_window_end.as_deref())),
                Style::default().fg(MINT),
            ))];
            for fnd in r.findings.iter().take(4) {
                v.push(Line::from(vec![
                    Span::styled("• ", Style::default().fg(GREEN)),
                    Span::styled(truncate(&fnd.statement, 90), Style::default().fg(TEXT)),
                ]));
            }
            v
        }
        None => vec![Line::from(Span::styled("no reflections yet · press t to run one", Style::default().fg(DIM)))],
    };
    f.render_widget(Paragraph::new(insight).block(panel("What I've realized")).wrap(Wrap { trim: true }), bottom[1]);
}

// ── Mind · Self (identity + principles) ──────────────────────────────────────
fn draw_self(f: &mut Frame, app: &mut App, area: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(area);
    let avg = if app.identity.is_empty() {
        0.0
    } else {
        app.identity.iter().map(|f| f.confidence).sum::<f64>() / app.identity.len() as f64
    };
    let canon = app.identity.iter().filter(|f| f.epistemic_status.as_deref() == Some("canonical")).count();
    f.render_widget(
        Paragraph::new(stat_bar(&[
            ("identity facts", fmt_int(app.identity.len() as i64), PERI),
            ("canonical", fmt_int(canon as i64), MINT),
            ("avg confidence", format!("{:.0}%", avg * 100.0), GREEN),
            ("principles", fmt_int(app.principles.len() as i64), PURPLE),
        ])),
        rows[0],
    );

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
        .split(rows[1]);

    let items: Vec<ListItem> = app
        .identity
        .iter()
        .map(|fct| {
            let filled = (fct.confidence.clamp(0.0, 1.0) * 6.0).round() as usize;
            let bar: String = "▰".repeat(filled) + &"▱".repeat(6usize.saturating_sub(filled));
            ListItem::new(Line::from(vec![
                Span::styled(format!("{bar} "), Style::default().fg(conf_color(fct.confidence))),
                Span::styled(format!("{:<18}", truncate(&fct.predicate, 18)), Style::default().fg(PERI).bold()),
                Span::styled(truncate(&fct.object, 60), Style::default().fg(TEXT)),
            ]))
        })
        .collect();
    f.render_stateful_widget(
        list_of(format!("Who I am — identity ({}) · a propose", app.identity.len()), items),
        cols[0],
        &mut app.id_sel.state,
    );

    let how: Vec<Line> = principles_ranked(app)
        .iter()
        .flat_map(|p| {
            vec![
                Line::from(vec![
                    Span::styled("◆ ", Style::default().fg(PURPLE)),
                    Span::styled(p.statement.clone(), Style::default().fg(TEXT)),
                ]),
                Line::from(Span::styled(
                    format!("    {} · {}", p.domain.clone().unwrap_or_default(), p.epistemic_status.clone().unwrap_or_default()),
                    Style::default().fg(FAINT),
                )),
            ]
        })
        .collect();
    f.render_widget(
        Paragraph::new(how).block(panel(&format!("How I think — principles ({})", app.principles.len()))).wrap(Wrap { trim: true }),
        cols[1],
    );
}

// ── Mind · Focus (attention, rebuilt) ────────────────────────────────────────
fn draw_focus(f: &mut Frame, app: &App, area: Rect) {
    let Some(a) = &app.attention else {
        f.render_widget(Paragraph::new("press r to load attention").block(panel("Focus")), area);
        return;
    };
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    let load = a.cognitive_load.score.clamp(0.0, 1.0);
    f.render_widget(
        Gauge::default()
            .block(panel("Mental load — how much my mind is juggling"))
            .gauge_style(Style::default().fg(load_color(load)))
            .ratio(load)
            .label(format!(
                "{:.0}%   density {:.2} · topic-switching {:.2} · {} episodes / {}d",
                load * 100.0,
                a.cognitive_load.breakdown.density,
                a.cognitive_load.breakdown.topic_switching,
                a.cognitive_load.breakdown.episode_count,
                a.cognitive_load.breakdown.window_days
            )),
        rows[0],
    );

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(rows[1]);

    // focus areas as a bar chart
    let fmax = a.focus_areas.iter().map(|x| x.weight).max().unwrap_or(1).max(1);
    let focus: Vec<Line> = a
        .focus_areas
        .iter()
        .take(14)
        .map(|x| hbar(&x.name, x.weight, fmax, 16, GREEN))
        .collect();
    f.render_widget(Paragraph::new(focus).block(panel("On my mind — top focus")), cols[0]);

    // right column: neglected + breadth
    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(cols[1]);
    let neglect: Vec<Line> = a
        .neglected
        .iter()
        .take(8)
        .map(|x| {
            Line::from(vec![
                Span::styled(format!("  {:>3} ", x.weight), Style::default().fg(AMBER).bold()),
                Span::styled(truncate(&x.name, 24), Style::default().fg(TEXT)),
                Span::styled(format!("  {}", x.label.clone().unwrap_or_default()), Style::default().fg(FAINT)),
            ])
        })
        .collect();
    f.render_widget(Paragraph::new(neglect).block(panel("Fading — neglected")), right[0]);

    let breadth = vec![
        kv("entities tracked", &fmt_int(a.breadth.total_entities)),
        kv("distinct types", &fmt_int(a.breadth.distinct_types)),
        Line::from(vec![
            Span::styled("  concentration  ", Style::default().fg(DIM)),
            Span::styled(load_bar(a.breadth.concentration, 10), Style::default().fg(CYAN)),
            Span::styled(format!(" {:.2}", a.breadth.concentration), Style::default().fg(TEXT)),
        ]),
        Line::from(Span::styled("  0 = broad · 1 = narrow", Style::default().fg(FAINT))),
    ];
    f.render_widget(Paragraph::new(breadth).block(panel("Breadth of mind")).wrap(Wrap { trim: true }), right[1]);
}

// ── Mind · Story (narrative) ─────────────────────────────────────────────────
fn draw_story(f: &mut Frame, app: &mut App, area: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(7), Constraint::Min(0)])
        .split(area);

    // current chapter hero
    let hero: Vec<Line> = match current_chapter(app) {
        Some(c) => vec![
            Line::from(vec![
                Span::styled("▶ ", Style::default().fg(CORAL).bold()),
                Span::styled(c.title.clone(), Style::default().fg(CORAL).bold()),
                Span::styled(format!("   since {}", fmt_date(c.start_date.as_deref())), Style::default().fg(FAINT)),
            ]),
            Line::from(Span::styled(c.summary.clone(), Style::default().fg(TEXT))),
        ],
        None => vec![Line::from(Span::styled("loading…", Style::default().fg(DIM)))],
    };
    f.render_widget(
        Paragraph::new(hero).block(panel("Current chapter · t to compose / generate")).wrap(Wrap { trim: true }),
        rows[0],
    );

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
        .split(rows[1]);
    let items: Vec<ListItem> = app
        .chapters
        .iter()
        .map(|c| {
            let marker = if c.end_date.is_none() { "● " } else { "○ " };
            ListItem::new(Line::from(vec![
                Span::styled(marker, Style::default().fg(if c.end_date.is_none() { CORAL } else { FAINT })),
                Span::styled(truncate(&c.title, 40), Style::default().fg(TEXT)),
            ]))
        })
        .collect();
    f.render_stateful_widget(list_of(format!("Timeline ({} chapters)", app.chapters.len()), items), cols[0], &mut app.chap_sel.state);
    let prev = app
        .chap_sel
        .selected()
        .and_then(|i| app.chapters.get(i))
        .map(|c| {
            Text::from(vec![
                Line::from(Span::styled(c.title.clone(), Style::default().fg(CORAL).bold())),
                Line::from(Span::styled(
                    format!("{} → {}", fmt_date(c.start_date.as_deref()), fmt_date(c.end_date.as_deref())),
                    Style::default().fg(DIM),
                )),
                Line::from(""),
                Line::from(Span::styled(c.summary.clone(), Style::default().fg(TEXT))),
            ])
        })
        .unwrap_or_else(|| Text::from("select a chapter"));
    f.render_widget(Paragraph::new(prev).block(panel("Chapter")).wrap(Wrap { trim: true }), cols[1]);
}

// ── Mind · Insights (reflections) ────────────────────────────────────────────
fn draw_insights(f: &mut Frame, app: &mut App, area: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(area);
    let findings: usize = app.reports.iter().map(|r| r.findings.len()).sum();
    f.render_widget(
        Paragraph::new(stat_bar(&[
            ("reports", fmt_int(app.reports.len() as i64), PERI),
            ("findings", fmt_int(findings as i64), GREEN),
            ("latest", fmt_date(app.reports.first().and_then(|r| r.time_window_end.as_deref())), MINT),
            ("t", "weekly / monthly".into(), DIM),
        ])),
        rows[0],
    );
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(34), Constraint::Percentage(66)])
        .split(rows[1]);
    let items: Vec<ListItem> = app
        .reports
        .iter()
        .map(|r| {
            ListItem::new(vec![
                Line::from(Span::styled(r.report_type.clone().unwrap_or_default(), Style::default().fg(PERI).bold())),
                Line::from(Span::styled(
                    format!("  {} · {} findings", fmt_date(r.time_window_end.as_deref()), r.findings.len()),
                    Style::default().fg(FAINT),
                )),
            ])
        })
        .collect();
    f.render_stateful_widget(list_of(format!("Reflections ({})", app.reports.len()), items), cols[0], &mut app.rep_sel.state);
    let prev = app
        .rep_sel
        .selected()
        .and_then(|i| app.reports.get(i))
        .map(|r| {
            let mut lines = vec![
                Line::from(Span::styled(
                    format!("{} episodes scanned · {} findings", r.episodes_scanned, r.findings.len()),
                    Style::default().fg(MINT),
                )),
                Line::from(Span::styled(
                    format!("{} → {}", fmt_date(r.time_window_start.as_deref()), fmt_date(r.time_window_end.as_deref())),
                    Style::default().fg(FAINT),
                )),
                Line::from(""),
            ];
            for fnd in &r.findings {
                lines.push(Line::from(vec![
                    Span::styled("• ", Style::default().fg(CORAL)),
                    Span::styled(fnd.statement.clone(), Style::default().fg(TEXT)),
                ]));
                lines.push(Line::from(Span::styled(
                    format!("   {:.0}% · {}", fnd.confidence * 100.0, fnd.tags.join(", ")),
                    Style::default().fg(FAINT),
                )));
            }
            Text::from(lines)
        })
        .unwrap_or_else(|| Text::from("select a report"));
    f.render_widget(Paragraph::new(prev).block(panel("What I noticed")).wrap(Wrap { trim: true }), cols[1]);
}

fn conf_color(c: f64) -> Color {
    if c >= 0.8 {
        MINT
    } else if c >= 0.5 {
        AMBER
    } else {
        DIM
    }
}

/// A compact 5-dot confidence indicator.
fn conf_dots(c: f64) -> String {
    let filled = (c.clamp(0.0, 1.0) * 5.0).round() as usize;
    "●".repeat(filled) + &"·".repeat(5usize.saturating_sub(filled))
}

/// A small inline load bar (█/░), used in dashboards.
fn load_bar(v: f64, width: usize) -> String {
    let filled = (v.clamp(0.0, 1.0) * width as f64).round() as usize;
    "█".repeat(filled) + &"░".repeat(width.saturating_sub(filled))
}
// ── Graph: knowledge explorer ────────────────────────────────────────────────
fn draw_graph(f: &mut Frame, app: &mut App, area: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(area);

    // breadcrumb / search line
    let mut crumb = vec![Span::styled(" ⌖ ", Style::default().fg(CYAN))];
    crumb.push(Span::styled("Graph", Style::default().fg(if app.graph_focus.is_none() { CYAN } else { DIM }).bold()));
    for fc in app.graph_stack.iter().chain(app.graph_focus.iter()) {
        crumb.push(Span::styled(" › ", Style::default().fg(FAINT)));
        crumb.push(Span::styled(truncate(&fc.name, 24), Style::default().fg(TEXT).bold()));
    }
    if app.graph_editing || !app.graph_filter.is_empty() {
        crumb.push(Span::styled("    /", Style::default().fg(DIM)));
        crumb.push(Span::styled(
            if app.graph_editing { format!("{}▏", app.graph_filter) } else { app.graph_filter.clone() },
            Style::default().fg(AMBER).bold(),
        ));
    }
    f.render_widget(Paragraph::new(Line::from(crumb)), rows[0]);

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(46), Constraint::Percentage(54)])
        .split(rows[1]);

    // ── left list + right card (build owned, then render) ────────────────
    let (items, list_title): (Vec<ListItem>, String) = if app.graph_focus.is_none() {
        let nodes = app.graph_root_nodes();
        let maxdeg = nodes.iter().map(|n| n.degree).max().unwrap_or(1).max(1);
        let items = nodes
            .iter()
            .map(|n| {
                ListItem::new(Line::from(vec![
                    Span::styled(bar_str(n.degree, maxdeg, 8).replace('█', "▰").replace('░', "▱"), Style::default().fg(LIME)),
                    Span::styled(format!(" {:>4} ", n.degree), Style::default().fg(MINT).bold()),
                    Span::styled(fit(&n.name, 26), Style::default().fg(TEXT)),
                    Span::styled(format!(" {}", n.label.clone().unwrap_or_default()), Style::default().fg(type_color(n.label.as_deref().unwrap_or("")))),
                ]))
            })
            .collect();
        (items, format!("Entities ({}) · ↵ explore · / search", nodes.len()))
    } else {
        let nbrs = app.graph_neighbors();
        let items = nbrs
            .iter()
            .map(|nb| {
                let arrow = if nb.outgoing { "→" } else { "←" };
                ListItem::new(Line::from(vec![
                    Span::styled(format!("{:<18}", truncate(nb.rel_type.as_deref().unwrap_or("·"), 18)), Style::default().fg(MINT)),
                    Span::styled(format!("{arrow} "), Style::default().fg(FAINT)),
                    Span::styled(fit(&nb.name, 24), Style::default().fg(TEXT)),
                    Span::styled(format!(" {}", nb.label.clone().unwrap_or_default()), Style::default().fg(type_color(nb.label.as_deref().unwrap_or("")))),
                ]))
            })
            .collect();
        (items, format!("Connections ({}) · ↵ go · ⌫ back · / filter", nbrs.len()))
    };

    let card = if app.graph_focus.is_none() {
        graph_overview_card(app)
    } else {
        graph_focus_card(app)
    };

    f.render_stateful_widget(list_of(list_title, items), cols[0], &mut app.node_sel.state);
    f.render_widget(Paragraph::new(card).block(panel("Detail")).wrap(Wrap { trim: true }), cols[1]);
}

fn graph_overview_card(app: &App) -> Text<'static> {
    let density = if app.stats.knowledge_entities > 0 {
        app.stats.knowledge_edges as f64 / app.stats.knowledge_entities as f64
    } else {
        0.0
    };
    let mut lines = vec![
        Line::from(Span::styled("Knowledge graph", Style::default().fg(CYAN).bold())),
        Line::from(""),
        kv("entities", &fmt_int(app.stats.knowledge_entities)),
        kv("edges", &fmt_int(app.stats.knowledge_edges)),
        kv("density", &format!("{density:.2} edges/entity")),
    ];
    if let Some(a) = &app.attention {
        lines.push(kv("distinct types", &fmt_int(a.breadth.distinct_types)));
        lines.push(Line::from(""));
        lines.push(head("Entities by type"));
        let max = a.breadth.by_type.values().copied().max().unwrap_or(1);
        let mut entries: Vec<(&String, &i64)> = a.breadth.by_type.iter().collect();
        entries.sort_by(|x, y| y.1.cmp(x.1));
        for (k, v) in entries.into_iter().take(12) {
            lines.push(hbar(k, *v, max, 16, type_color(k)));
        }
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled("  Select an entity and press ↵ to explore its connections.", Style::default().fg(DIM))));
    Text::from(lines)
}

fn graph_focus_card(app: &App) -> Text<'static> {
    let Some(focus) = &app.graph_focus else { return Text::from("") };
    let Some(ego) = &app.graph_ego else {
        let sp = SPIN[(app.frame as usize / 2) % SPIN.len()];
        return Text::from(vec![
            Line::from(Span::styled(focus.name.clone(), Style::default().fg(CORAL).bold())),
            Line::from(""),
            Line::from(Span::styled(format!("  {sp}  mapping connections…"), Style::default().fg(MINT))),
        ]);
    };

    // group edges by rel_type
    use std::collections::{BTreeMap, HashMap};
    let index: HashMap<&str, &crate::api::GNode> = ego.entities.iter().map(|e| (e.id.as_str(), e)).collect();
    let mut groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut type_counts: BTreeMap<String, i64> = BTreeMap::new();
    for e in &ego.edges {
        let other = if e.src_entity_id == focus.id {
            &e.dst_entity_id
        } else if e.dst_entity_id == focus.id {
            &e.src_entity_id
        } else {
            continue;
        };
        let ent = index.get(other.as_str()).copied();
        let name = ent.map(|n| n.name.clone()).unwrap_or_else(|| other.clone());
        groups.entry(e.rel_type.clone().unwrap_or_else(|| "related".into())).or_default().push(name);
        if let Some(ent) = ent {
            *type_counts.entry(ent.label.clone().unwrap_or_else(|| "?".into())).or_default() += 1;
        }
    }
    let total: usize = groups.values().map(|v| v.len()).sum();

    let mut lines = vec![
        Line::from(vec![
            Span::styled(focus.name.clone(), Style::default().fg(CORAL).bold()),
            Span::styled(format!("  {}", focus.label.clone().unwrap_or_default()), Style::default().fg(type_color(focus.label.as_deref().unwrap_or("")))),
        ]),
        Line::from(Span::styled(
            if focus.degree > 0 {
                format!("degree {} · {total} loaded · {} relation types", focus.degree, groups.len())
            } else {
                format!("{total} connections · {} relation types", groups.len())
            },
            Style::default().fg(DIM),
        )),
        Line::from(""),
    ];

    // relations grouped by type, sorted by count desc
    let mut gv: Vec<(&String, &Vec<String>)> = groups.iter().collect();
    gv.sort_by(|a, b| b.1.len().cmp(&a.1.len()));
    for (rel, names) in gv.into_iter().take(12) {
        lines.push(Line::from(vec![
            Span::styled(format!("  {} ", rel), Style::default().fg(MINT).bold()),
            Span::styled(format!("×{}", names.len()), Style::default().fg(FAINT)),
        ]));
        let sample = names.iter().take(6).cloned().collect::<Vec<_>>().join(", ");
        lines.push(Line::from(Span::styled(
            format!("    {}{}", truncate(&sample, 70), if names.len() > 6 { " …" } else { "" }),
            Style::default().fg(TEXT),
        )));
    }

    if !type_counts.is_empty() {
        lines.push(Line::from(""));
        lines.push(head("Neighbour types"));
        let max = type_counts.values().copied().max().unwrap_or(1);
        let mut tv: Vec<(&String, &i64)> = type_counts.iter().collect();
        tv.sort_by(|a, b| b.1.cmp(a.1));
        for (k, v) in tv.into_iter().take(8) {
            lines.push(hbar(k, *v, max, 14, type_color(k)));
        }
    }
    Text::from(lines)
}

// ── Recall ───────────────────────────────────────────────────────────────────
const SPIN: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

fn recall_mode_note(mode: &str) -> &'static str {
    match mode {
        "deep" => "wider beam · 2-hop graph · more rounds — higher recall, slower",
        "divergent" => "MMR diversity · older + speculative tiers — unexpected connections",
        _ => "quick single-pass vector search — closest matches",
    }
}

fn draw_recall(f: &mut Frame, app: &mut App, area: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // query field
            Constraint::Length(4), // mode selector + note
            Constraint::Min(0),    // results
            Constraint::Length(7), // preview
        ])
        .split(area);

    // ── query field ──────────────────────────────────────────────────────
    let editing = app.recall_editing;
    let qcol = if editing { MINT } else { FAINT };
    let qline = if app.recall_query.is_empty() && !editing {
        Span::styled("press / to type a query, enter to search", Style::default().fg(DIM))
    } else if editing {
        Span::styled(format!("{}▏", app.recall_query), Style::default().fg(TEXT))
    } else {
        Span::styled(app.recall_query.clone(), Style::default().fg(TEXT))
    };
    let qtitle = if editing { "Query — typing… (enter search · esc cancel)" } else { "Query — / to edit" };
    f.render_widget(
        Paragraph::new(Line::from(qline)).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(qcol))
                .title(Span::styled(format!(" {qtitle} "), Style::default().fg(qcol).bold())),
        ),
        rows[0],
    );

    // ── mode selector + note ─────────────────────────────────────────────
    let mut seg = vec![Span::raw(" ")];
    for m in ["fast", "deep", "divergent"] {
        let active = app.recall_mode == m;
        let style = if active {
            Style::default().fg(Color::Black).bg(GREEN).bold()
        } else {
            Style::default().fg(DIM)
        };
        seg.push(Span::styled(format!(" {m} "), style));
        seg.push(Span::raw(" "));
    }
    let mode_block = Paragraph::new(vec![
        Line::from(seg),
        Line::from(Span::styled(format!(" {}", recall_mode_note(&app.recall_mode)), Style::default().fg(DIM))),
    ])
    .block(panel("Mode — m to switch"));
    f.render_widget(mode_block, rows[1]);

    // ── results: loading / error / empty / list ──────────────────────────
    if app.recall_loading {
        let sp = SPIN[(app.frame as usize / 2) % SPIN.len()];
        let body = Text::from(vec![
            Line::from(""),
            Line::from(Span::styled(
                format!("  {sp}  searching memories — {} mode…", app.recall_mode),
                Style::default().fg(MINT).bold(),
            )),
        ]);
        f.render_widget(Paragraph::new(body).block(panel("Recall")), rows[2]);
    } else if let Some(err) = &app.recall_error {
        let body = Text::from(vec![
            Line::from(Span::styled("  ✗ recall failed", Style::default().fg(RED).bold())),
            Line::from(Span::styled(format!("  {}", truncate(err, 160)), Style::default().fg(RED))),
            Line::from(""),
            Line::from(Span::styled("  press r to retry, or / to edit the query", Style::default().fg(DIM))),
        ]);
        f.render_widget(Paragraph::new(body).block(panel("Recall")), rows[2]);
    } else if !app.recall_submitted {
        let body = Text::from(vec![
            Line::from(Span::styled("  Semantic recall over your memories.", Style::default().fg(TEXT))),
            Line::from(""),
            mode_help_line("fast", "quick single-pass vector search — closest matches"),
            mode_help_line("deep", "wider beam, 2-hop graph, more rounds — higher recall"),
            mode_help_line("divergent", "MMR diversity + older/speculative — unexpected links"),
            Line::from(""),
            Line::from(Span::styled("  Press / to type a query · m to switch mode · enter to search.", Style::default().fg(DIM))),
        ]);
        f.render_widget(Paragraph::new(body).block(panel("Recall")), rows[2]);
    } else if app.recall_hits.is_empty() {
        let body = Text::from(vec![
            Line::from(""),
            Line::from(Span::styled(
                format!("  no memories matched “{}”", truncate(&app.recall_query, 80)),
                Style::default().fg(AMBER).bold(),
            )),
            Line::from(Span::styled(
                "  try rephrasing, or switch to deep / divergent mode (m)",
                Style::default().fg(DIM),
            )),
        ]);
        f.render_widget(Paragraph::new(body).block(panel("Recall · 0 hits")), rows[2]);
    } else {
        let items: Vec<ListItem> = app
            .recall_hits
            .iter()
            .map(|h| {
                let filled = (h.score.clamp(0.0, 1.0) * 5.0).round() as usize;
                let sb: String = "▰".repeat(filled) + &"▱".repeat(5usize.saturating_sub(filled));
                ListItem::new(Line::from(vec![
                    Span::styled(format!("{sb} "), Style::default().fg(score_color(h.score))),
                    Span::styled(format!("{:.3} ", h.score), Style::default().fg(score_color(h.score)).bold()),
                    status_badge(h.epistemic_status.as_deref()),
                    Span::raw(" "),
                    Span::styled(truncate(&h.text, 150), Style::default().fg(TEXT)),
                ]))
            })
            .collect();
        f.render_stateful_widget(
            list_of(format!("Recall hits ({}) · enter to open", app.recall_hits.len()), items),
            rows[2],
            &mut app.recall_sel.state,
        );
    }

    // ── preview ──────────────────────────────────────────────────────────
    let prev = app
        .recall_sel
        .selected()
        .and_then(|i| app.recall_hits.get(i))
        .filter(|_| !app.recall_hits.is_empty())
        .map(|h| {
            Text::from(vec![
                Line::from(Span::styled(
                    format!("score {:.3} · {}", h.score, h.epistemic_status.clone().unwrap_or_default()),
                    Style::default().fg(DIM),
                )),
                Line::from(""),
                Line::from(Span::styled(h.text.clone(), Style::default().fg(TEXT))),
            ])
        })
        .unwrap_or_else(|| Text::from(Span::styled("select a hit to preview", Style::default().fg(DIM))));
    f.render_widget(Paragraph::new(prev).block(panel("Preview")).wrap(Wrap { trim: true }), rows[3]);
}

fn mode_help_line(mode: &str, note: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("  {mode:<10}"), Style::default().fg(GREEN).bold()),
        Span::styled(note.to_string(), Style::default().fg(DIM)),
    ])
}

fn score_color(s: f64) -> Color {
    if s >= 0.6 {
        MINT
    } else if s >= 0.4 {
        AMBER
    } else {
        DIM
    }
}

// ── Systems ──────────────────────────────────────────────────────────────────
fn draw_systems(f: &mut Frame, app: &mut App, area: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(area);
    f.render_widget(Paragraph::new(subtabs(&SYS_SUBS, app.sys_sub)), rows[0]);

    match app.sys_sub {
        0 => draw_sys_overview(f, app, rows[1]),
        1 => draw_agents(f, app, rows[1]),
        2 => draw_sys_scheduler(f, app, rows[1]),
        3 => draw_sys_events(f, app, rows[1]),
        4 => draw_sys_logs(f, app, rows[1]),
        5 => draw_sys_routing(f, app, rows[1]),
        6 => draw_sys_pipeline(f, app, rows[1]),
        _ => draw_sys_settings(f, app, rows[1]),
    }
}

fn draw_sys_overview(f: &mut Frame, app: &App, area: Rect) {
    let Some(sys) = &app.systems else {
        f.render_widget(Paragraph::new("loading systems…").block(panel("Overview")), area);
        return;
    };
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(6), Constraint::Length(9), Constraint::Min(0)])
        .split(area);

    // ── KPI cards ─────────────────────────────────────────────────────────
    let up = sys.infrastructure.iter().filter(|i| i.ok).count();
    let running = app.agent_runs.iter().filter(|r| r.status.as_deref() == Some("running")).count();
    let failing = sys.scheduler.failing.len();
    let latency = app.sys_history.last().map(|s| s.latency_ms).unwrap_or(0);
    let lat_col = if latency > 1500 { RED } else if latency > 600 { AMBER } else { GREEN };
    let cards: [(&str, String, Color); 5] = [
        ("SERVICES", format!("{}/{}", up, sys.infrastructure.len()), if up == sys.infrastructure.len() { GREEN } else { RED }),
        ("API LATENCY", format!("{latency}ms"), lat_col),
        ("AGENTS RUN", fmt_int(running as i64), if running > 0 { AMBER } else { DIM }),
        ("JOBS", fmt_int(sys.scheduler.jobs), PERI),
        ("FAILING", fmt_int(failing as i64), if failing > 0 { RED } else { DIM }),
    ];
    let cs = Layout::default().direction(Direction::Horizontal).constraints([Constraint::Ratio(1, 5); 5]).split(rows[0]);
    for (i, (label, val, col)) in cards.iter().enumerate() {
        f.render_widget(
            Paragraph::new(vec![
                Line::from(Span::styled(*label, Style::default().fg(DIM))),
                Line::from(Span::styled(val.clone(), Style::default().fg(*col).bold())),
            ])
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).border_style(Style::default().fg(FAINT))),
            cs[i],
        );
    }

    // ── services (uptime strips) + latency graph ─────────────────────────
    let midcols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(62), Constraint::Percentage(38)])
        .split(rows[1]);

    let mut svc: Vec<Line> = vec![];
    for i in &sys.infrastructure {
        let (strip, pct) = uptime_strip(&app.sys_history, &i.name, 26);
        let mut spans = vec![
            Span::styled("● ", Style::default().fg(if i.ok { GREEN } else { RED })),
            Span::styled(fit(&i.name, 10), Style::default().fg(TEXT).bold()),
            Span::styled(fit(&svc_version(&i.name, i.detail.as_deref()), 16), Style::default().fg(FAINT)),
        ];
        spans.extend(strip);
        spans.push(Span::styled(format!(" {pct:>3.0}%"), Style::default().fg(if pct >= 99.9 { GREEN } else { AMBER })));
        svc.push(Line::from(spans));
    }
    let checks = app.sys_history.len();
    f.render_widget(
        Paragraph::new(svc).block(panel(&format!("Services · last {checks} checks"))),
        midcols[0],
    );

    // latency sparkline
    let lats: Vec<u64> = app.sys_history.iter().map(|s| s.latency_ms).collect();
    let (lmin, lmax, lavg) = stats_u64(&lats);
    let lat_lines = vec![
        Line::from(Span::styled(spark(&lats), Style::default().fg(CYAN))),
        Line::from(""),
        Line::from(vec![
            Span::styled(format!("{}ms", lats.last().copied().unwrap_or(0)), Style::default().fg(lat_col).bold()),
            Span::styled(" now", Style::default().fg(DIM)),
        ]),
        Line::from(Span::styled(format!("min {lmin} · avg {lavg} · max {lmax} ms", ), Style::default().fg(FAINT))),
    ];
    f.render_widget(Paragraph::new(lat_lines).block(panel("API latency")), midcols[1]);

    // ── engines (bar chart) + scheduler/live ─────────────────────────────
    let botcols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(62), Constraint::Percentage(38)])
        .split(rows[2]);

    let emax = sys.engines.iter().map(|e| e.runs_7d).max().unwrap_or(1).max(1);
    let mut eng: Vec<Line> = vec![];
    for e in &sys.engines {
        let active = e.runs_24h > 0;
        let col = if active { MINT } else { FAINT };
        eng.push(Line::from(vec![
            Span::styled(format!("  {}", fit(e.label.as_deref().unwrap_or(&e.name), 22)), Style::default().fg(if active { TEXT } else { DIM })),
            Span::styled(bar_str(e.runs_7d, emax, 14), Style::default().fg(col)),
            Span::styled(format!(" {:>5}/7d", e.runs_7d), Style::default().fg(DIM)),
            Span::styled(format!("  {:>3}/24h", e.runs_24h), Style::default().fg(if active { MINT } else { FAINT })),
        ]));
        eng.push(Line::from(Span::styled(
            format!("    last run {}", e.last_run.as_deref().map(|d| fmt_date(Some(d))).unwrap_or_else(|| "—".into())),
            Style::default().fg(FAINT),
        )));
    }
    f.render_widget(Paragraph::new(eng).block(panel(&format!("Engines · 7-day activity ({})", sys.engines.len()))), botcols[0]);

    let mut right: Vec<Line> = vec![head("Scheduler")];
    right.push(Line::from(vec![
        Span::styled("  ● ", Style::default().fg(if sys.scheduler.running { GREEN } else { RED })),
        Span::styled(if sys.scheduler.running { "running" } else { "stopped" }, Style::default().fg(TEXT).bold()),
        Span::styled(format!("  {} jobs", sys.scheduler.jobs), Style::default().fg(DIM)),
    ]));
    right.push(Line::from(Span::styled(format!("    next {}", fmt_date(sys.scheduler.next_due.as_deref())), Style::default().fg(FAINT))));
    right.push(Line::from(""));
    right.push(head(&format!("Agents · {running} running")));
    if running == 0 {
        right.push(Line::from(Span::styled("  ● idle", Style::default().fg(DIM))));
    } else {
        for r in app.agent_runs.iter().filter(|r| r.status.as_deref() == Some("running")).take(3) {
            right.push(Line::from(vec![
                Span::styled("  ◌ ", Style::default().fg(AMBER)),
                Span::styled(truncate(&r.agent.clone().unwrap_or_default(), 22), Style::default().fg(PURPLE).bold()),
            ]));
        }
    }
    right.push(Line::from(""));
    right.push(head("Data"));
    right.push(kv("memories", &fmt_int(sys.stats.memories)));
    right.push(kv("episodes", &fmt_int(sys.stats.episodes)));
    right.push(kv("entities", &fmt_int(sys.stats.knowledge_entities)));
    right.push(kv("edges", &fmt_int(sys.stats.knowledge_edges)));
    f.render_widget(Paragraph::new(right).block(panel("Live")).wrap(Wrap { trim: true }), botcols[1]);
}

/// A coloured uptime strip for one service from the poll history (oldest→newest).
fn uptime_strip(history: &[crate::app::SysSample], name: &str, width: usize) -> (Vec<Span<'static>>, f64) {
    let recent: Vec<&crate::app::SysSample> = {
        let mut v: Vec<&crate::app::SysSample> = history.iter().rev().take(width).collect();
        v.reverse();
        v
    };
    let mut spans = Vec::with_capacity(recent.len());
    let (mut up, mut total) = (0usize, 0usize);
    for s in &recent {
        match s.services.iter().find(|(n, _)| n == name) {
            Some((_, true)) => {
                up += 1;
                total += 1;
                spans.push(Span::styled("▮", Style::default().fg(GREEN)));
            }
            Some((_, false)) => {
                total += 1;
                spans.push(Span::styled("▮", Style::default().fg(RED)));
            }
            None => spans.push(Span::styled("·", Style::default().fg(FAINT))),
        }
    }
    if spans.is_empty() {
        spans.push(Span::styled("· waiting", Style::default().fg(FAINT)));
    }
    let pct = if total > 0 { up as f64 / total as f64 * 100.0 } else { 100.0 };
    (spans, pct)
}

/// Compact a service's detail string into a short version label.
fn svc_version(name: &str, detail: Option<&str>) -> String {
    let d = detail.unwrap_or("").trim();
    if d.is_empty() {
        return "—".into();
    }
    if name.contains("postgres") {
        // "PostgreSQL 16.14 (Debian …) … +pgvector" → "16.14 +pgvector"
        let ver = d.split_whitespace().nth(1).unwrap_or("");
        let vec = if d.contains("pgvector") { " +pgvector" } else { "" };
        format!("{ver}{vec}")
    } else {
        truncate(d, 16)
    }
}

fn stats_u64(xs: &[u64]) -> (u64, u64, u64) {
    if xs.is_empty() {
        return (0, 0, 0);
    }
    let min = *xs.iter().min().unwrap();
    let max = *xs.iter().max().unwrap();
    let avg = xs.iter().sum::<u64>() / xs.len() as u64;
    (min, max, avg)
}

/// Truncate-or-pad a string to exactly `n` display columns.
fn fit(s: &str, n: usize) -> String {
    let len = s.chars().count();
    if len > n {
        let t: String = s.chars().take(n.saturating_sub(1)).collect();
        format!("{t}…")
    } else {
        format!("{s}{}", " ".repeat(n - len))
    }
}

fn draw_sys_scheduler(f: &mut Frame, app: &mut App, area: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(area);

    let enabled = app.scheduled_jobs.iter().filter(|j| j.enabled).count();
    let failed = app.scheduled_jobs.iter().filter(|j| j.last_error.is_some()).count();
    let sys_jobs = app.scheduler.as_ref().map(|s| s.jobs.len()).unwrap_or(0);
    f.render_widget(
        Paragraph::new(stat_bar(&[
            ("your jobs", fmt_int(app.scheduled_jobs.len() as i64), GREEN),
            ("enabled", fmt_int(enabled as i64), MINT),
            ("errored", fmt_int(failed as i64), if failed > 0 { RED } else { DIM }),
            ("system jobs", fmt_int(sys_jobs as i64), PERI),
        ])),
        rows[0],
    );

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(rows[1]);

    // your jobs — selectable + manageable
    let items: Vec<ListItem> = app
        .scheduled_jobs
        .iter()
        .map(|j| {
            ListItem::new(vec![
                Line::from(vec![
                    enabled_badge(j.enabled),
                    job_status_dot(j.last_status.as_deref()),
                    Span::styled(truncate(&j.name, 26), Style::default().fg(TEXT).bold()),
                ]),
                Line::from(vec![
                    Span::styled(format!("    {} ", j.cadence_display.clone().unwrap_or_default()), Style::default().fg(PERI)),
                    Span::styled(format!("· next {}", fmt_date(j.next_due.as_deref())), Style::default().fg(FAINT)),
                ]),
            ])
        })
        .collect();
    f.render_stateful_widget(
        list_of(format!("Your jobs ({}) · e toggle · x run · d delete · n new", app.scheduled_jobs.len()), items),
        cols[0],
        &mut app.sched_sel.state,
    );

    // right: selected detail + system cron jobs
    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(cols[1]);

    let detail = app
        .sched_sel
        .selected()
        .and_then(|i| app.scheduled_jobs.get(i))
        .map(|j| {
            let mut lines = vec![
                Line::from(vec![
                    enabled_badge(j.enabled),
                    Span::styled(j.name.clone(), Style::default().fg(TEXT).bold()),
                ]),
                kv("cadence", j.cadence_display.as_deref().unwrap_or("")),
                kv("delivery", j.delivery.as_deref().unwrap_or("")),
                kv("last run", &fmt_date(j.last_fired.as_deref())),
                kv("last status", j.last_status.as_deref().unwrap_or("never")),
                kv("next due", &fmt_date(j.next_due.as_deref())),
                Line::from(""),
                head("task"),
                Line::from(Span::styled(truncate(&j.task, 300), Style::default().fg(TEXT))),
            ];
            if let Some(err) = &j.last_error {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(format!("error: {}", truncate(err, 120)), Style::default().fg(RED))));
            }
            Text::from(lines)
        })
        .unwrap_or_else(|| Text::from(Span::styled("select a job · n to create one", Style::default().fg(DIM))));
    f.render_widget(Paragraph::new(detail).block(panel("Job")).wrap(Wrap { trim: true }), right[0]);

    let mut cron: Vec<Line> = vec![];
    if let Some(s) = &app.scheduler {
        for j in &s.jobs {
            cron.push(Line::from(vec![
                job_status_dot(j.last_status.as_deref()),
                Span::styled(format!("{:<22}", truncate(&j.name, 22)), Style::default().fg(TEXT)),
                Span::styled(truncate(j.cadence.as_deref().unwrap_or(""), 14), Style::default().fg(FAINT)),
                if j.consecutive_failures > 0 {
                    Span::styled(format!("  {}✗", j.consecutive_failures), Style::default().fg(RED).bold())
                } else {
                    Span::raw("")
                },
            ]));
        }
    }
    f.render_widget(
        Paragraph::new(cron).block(panel(&format!("System jobs ({}) · read-only", app.scheduler.as_ref().map(|s| s.jobs.len()).unwrap_or(0)))),
        right[1],
    );
}

fn draw_sys_events(f: &mut Frame, app: &mut App, area: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(area);

    // category filter chips with counts
    let mut counts = [0usize; 8];
    for e in &app.events {
        counts[crate::app::event_cat_index(&e.etype)] += 1;
        counts[0] += 1;
    }
    let mut chips = vec![Span::styled(" c ", Style::default().fg(Color::Black).bg(PERI).bold()), Span::raw(" ")];
    for (i, name) in crate::app::EVENT_CATS.iter().enumerate() {
        let active = i == app.event_cat;
        let style = if active {
            Style::default().fg(Color::Black).bg(event_cat_color(i)).bold()
        } else {
            Style::default().fg(if counts[i] == 0 { FAINT } else { event_cat_color(i) })
        };
        chips.push(Span::styled(format!(" {name} {} ", counts[i]), style));
        chips.push(Span::raw(" "));
    }
    f.render_widget(Paragraph::new(Line::from(chips)), rows[0]);

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(rows[1]);

    let filtered = app.filtered_events();
    let items: Vec<ListItem> = filtered
        .iter()
        .map(|e| {
            let ci = crate::app::event_cat_index(&e.etype);
            ListItem::new(Line::from(vec![
                Span::styled(format!("{} ", fmt_time(e.created_at.as_deref())), Style::default().fg(FAINT)),
                Span::styled(format!("{:<10}", crate::app::EVENT_CATS[ci]), Style::default().fg(event_cat_color(ci)).bold()),
                Span::styled(format!("{:<24}", short_event(&e.etype)), Style::default().fg(TEXT)),
                Span::styled(truncate(e.subject.as_deref().unwrap_or(""), 26), Style::default().fg(DIM)),
            ]))
        })
        .collect();
    let cat_label = crate::app::EVENT_CATS[app.event_cat];
    let list_title = format!("Events · {} ({})", cat_label, filtered.len());

    // Build the preview (owned) before taking the mutable list-state borrow.
    let prev = app
        .event_sel
        .selected()
        .and_then(|i| filtered.get(i).copied())
        .map(|e| {
            let ci = crate::app::event_cat_index(&e.etype);
            let mut lines = vec![
                Line::from(vec![
                    Span::styled(format!(" {} ", crate::app::EVENT_CATS[ci]), Style::default().fg(Color::Black).bg(event_cat_color(ci)).bold()),
                    Span::raw(" "),
                    Span::styled(short_event(&e.etype), Style::default().fg(CORAL).bold()),
                ]),
                kv("subject", e.subject.as_deref().unwrap_or("")),
                kv("seq", &e.seq.to_string()),
                kv("at", &fmt_date(e.created_at.as_deref())),
                Line::from(""),
                head("data"),
            ];
            for l in json_lines(&e.data, 0) {
                lines.push(l);
            }
            Text::from(lines)
        })
        .unwrap_or_else(|| Text::from("no selection"));
    drop(filtered);

    f.render_stateful_widget(list_of(list_title, items), cols[0], &mut app.event_sel.state);
    f.render_widget(Paragraph::new(prev).block(panel("Event")).wrap(Wrap { trim: true }), cols[1]);
}

fn draw_sys_logs(f: &mut Frame, app: &App, area: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(area);

    let chips: Vec<Span> = app
        .log_sources
        .iter()
        .enumerate()
        .flat_map(|(i, s)| {
            let active = i == app.log_source_idx;
            let col = if !s.exists { FAINT } else if active { Color::Black } else { TEXT };
            let style = if active { Style::default().fg(col).bg(MINT).bold() } else { Style::default().fg(col) };
            vec![Span::styled(format!(" {} ", s.name), style), Span::raw(" ")]
        })
        .collect();
    f.render_widget(Paragraph::new(Line::from(chips)), rows[0]);

    let (title, body) = match &app.logs {
        Some(l) => {
            let title = format!(
                "{} · {} · {}",
                l.source,
                if l.exists { fmt_bytes(l.size_bytes) } else { "missing".into() },
                l.path
            );
            if l.lines.is_empty() {
                (title, Text::from(Span::styled("(no log lines — file empty or not yet created)", Style::default().fg(DIM))))
            } else {
                let lines: Vec<Line> = l
                    .lines
                    .iter()
                    .rev()
                    .take(500)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .map(|v| Line::from(Span::styled(log_line(v), log_color(v))))
                    .collect();
                (title, Text::from(lines))
            }
        }
        None => ("logs".into(), Text::from("press s to pick a source")),
    };
    f.render_widget(Paragraph::new(body).block(panel(&title)).wrap(Wrap { trim: false }), rows[1]);
}

// ── Agents ───────────────────────────────────────────────────────────────────
fn draw_agents(f: &mut Frame, app: &mut App, area: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(area);

    let count = |s: &str| app.agent_runs.iter().filter(|r| r.status.as_deref() == Some(s)).count();
    let running = count("running");
    f.render_widget(
        Paragraph::new(stat_bar(&[
            ("runs", fmt_int(app.agent_runs.len() as i64), PURPLE),
            ("running", fmt_int(running as i64), if running > 0 { AMBER } else { DIM }),
            ("completed", fmt_int(count("completed") as i64), GREEN),
            ("failed", fmt_int((count("failed") + count("error")) as i64), if count("failed") + count("error") > 0 { RED } else { DIM }),
        ])),
        rows[0],
    );

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(rows[1]);

    let items: Vec<ListItem> = app
        .agent_runs
        .iter()
        .map(|r| {
            ListItem::new(vec![
                Line::from(vec![
                    run_badge(r.status.as_deref()),
                    Span::raw(" "),
                    Span::styled(format!("{:<10}", truncate(&r.agent.clone().unwrap_or_default(), 10)), Style::default().fg(PURPLE).bold()),
                    Span::styled(truncate(&r.task, 52), Style::default().fg(TEXT)),
                ]),
                Line::from(Span::styled(format!("    {}", fmt_date(r.created_at.as_deref())), Style::default().fg(FAINT))),
            ])
        })
        .collect();
    f.render_stateful_widget(
        list_of(format!("Agent runs ({}) · enter inspect · c cancel", app.agent_runs.len()), items),
        cols[0],
        &mut app.agent_sel.state,
    );

    let body = match &app.agent_detail {
        Some(d) => {
            let r = &d.run;
            let mut lines = vec![
                Line::from(vec![
                    run_dot(r.status.as_deref()),
                    Span::styled(r.agent.clone().unwrap_or_default(), Style::default().fg(PURPLE).bold()),
                    Span::styled(format!("  {}", run_status(r.status.as_deref())), Style::default().fg(DIM)),
                ]),
                kv("id", &r.id),
                kv("created", &fmt_date(r.created_at.as_deref())),
                kv("finished", &fmt_date(r.finished_at.as_deref())),
                Line::from(""),
                head("task"),
                Line::from(Span::styled(truncate(&r.task, 400), Style::default().fg(TEXT))),
            ];
            if let Some(err) = &r.error {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(format!("error: {err}"), Style::default().fg(RED))));
            }
            lines.push(Line::from(""));
            lines.push(head(&format!("actions ({})", d.actions.len())));
            for (i, a) in d.actions.iter().enumerate() {
                lines.push(Line::from(vec![
                    Span::styled(format!(" {:>2} ", i + 1), Style::default().fg(FAINT)),
                    Span::styled(format!("{:<10}", a.kind.clone().unwrap_or_default()), Style::default().fg(action_color(a.kind.as_deref())).bold()),
                    Span::styled(action_summary(&a.payload), Style::default().fg(TEXT)),
                ]));
            }
            Text::from(lines)
        }
        None => Text::from(Span::styled("press enter to inspect a run (auto-refreshes while open)", Style::default().fg(DIM))),
    };
    f.render_widget(Paragraph::new(body).block(panel("Run detail")).wrap(Wrap { trim: true }), cols[1]);
}

// ── overlays ─────────────────────────────────────────────────────────────────
fn draw_detail(f: &mut Frame, d: &Detail) {
    let area = centered(80, 80, f.area());
    f.render_widget(Clear, area);
    let (title, text) = match d {
        Detail::Memory(m) => {
            let mem = &m.memory;
            let mut lines = vec![
                kv("id", &mem.id),
                kv("kind", mem.kind.as_deref().unwrap_or("")),
                kv("tier", mem.tier.as_deref().unwrap_or("")),
                kv("status", mem.epistemic_status.as_deref().unwrap_or("")),
                kv("created", &fmt_date(mem.created_at.as_deref())),
                kv("valid_to", mem.valid_to.as_deref().unwrap_or("— (live)")),
                Line::from(""),
                Line::from(Span::styled(mem.statement.clone(), Style::default().fg(TEXT))),
            ];
            if let Some(epi) = &m.source_episode {
                lines.push(Line::from(""));
                lines.push(head("Source episode"));
                lines.push(Line::from(Span::styled(truncate(&epi.content, 400), Style::default().fg(DIM))));
            }
            ("Memory", Text::from(lines))
        }
        Detail::Episode(e) => {
            let epi = &e.episode;
            let mut lines = vec![
                kv("id", &epi.id),
                kv("source", epi.source_ref.as_deref().unwrap_or("")),
                kv("modality", epi.modality.as_deref().unwrap_or("")),
                kv("created", &fmt_date(epi.created_at.as_deref())),
                Line::from(""),
                Line::from(Span::styled(epi.content.clone(), Style::default().fg(TEXT))),
                Line::from(""),
                head(&format!("Derived memories ({})", e.memories.len())),
            ];
            for m in &e.memories {
                lines.push(Line::from(vec![
                    Span::styled("• ", Style::default().fg(MINT)),
                    Span::styled(truncate(&m.statement, 160), Style::default().fg(DIM)),
                ]));
            }
            ("Episode", Text::from(lines))
        }
    };
    f.render_widget(
        Paragraph::new(text).block(panel(&format!("{title}  (esc to close)"))).wrap(Wrap { trim: true }),
        area,
    );
}

fn draw_form(f: &mut Frame, app: &App) {
    let Overlay::Form(form) = &app.overlay else { return };
    let h = (form.fields.len() as u16) * 3 + 4;
    let area = centered(70, h.min(90), f.area());
    f.render_widget(Clear, area);
    let inner = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(form.fields.iter().map(|_| Constraint::Length(3)).chain([Constraint::Min(1)]).collect::<Vec<_>>())
        .split(area);
    f.render_widget(panel(&form.title), area);
    for (i, fld) in form.fields.iter().enumerate() {
        let active = i == form.active;
        let val = if active { format!("{}▏", fld.value) } else { fld.value.clone() };
        let bcol = if active { MINT } else { FAINT };
        let b = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(bcol))
            .title(Span::styled(format!(" {} ", fld.label), Style::default().fg(bcol)));
        f.render_widget(
            Paragraph::new(val).style(Style::default().fg(if active { MINT } else { TEXT })).block(b).wrap(Wrap { trim: false }),
            inner[i],
        );
    }
    let hint = if matches!(form.kind, crate::app::FormKind::Capture) {
        "enter newline · ctrl-s submit · esc cancel"
    } else if form.fields.len() > 1 {
        "tab next field · enter submit · esc cancel"
    } else {
        "enter submit · esc cancel"
    };
    f.render_widget(
        Paragraph::new(Span::styled(hint, Style::default().fg(DIM))).alignment(Alignment::Center),
        *inner.last().unwrap(),
    );
}

fn draw_confirm(f: &mut Frame, msg: &str) {
    let area = centered(60, 30, f.area());
    f.render_widget(Clear, area);
    let text = Text::from(vec![
        Line::from(Span::styled(msg.to_string(), Style::default().fg(TEXT))),
        Line::from(""),
        Line::from(vec![
            Span::styled(" y ", Style::default().fg(Color::Black).bg(CORAL).bold()),
            Span::styled(" confirm    ", Style::default().fg(DIM)),
            Span::styled(" n/esc ", Style::default().fg(Color::Black).bg(MINT).bold()),
            Span::styled(" cancel", Style::default().fg(DIM)),
        ]),
    ]);
    f.render_widget(Paragraph::new(text).block(panel("Confirm")).wrap(Wrap { trim: true }), area);
}

fn draw_menu(f: &mut Frame, app: &App) {
    let Overlay::Menu { title, items, sel } = &app.overlay else { return };
    let area = centered(50, 40, f.area());
    f.render_widget(Clear, area);
    let list_items: Vec<ListItem> = items
        .iter()
        .enumerate()
        .map(|(i, (label, _))| {
            let style = if i == *sel {
                Style::default().fg(Color::Black).bg(MINT).bold()
            } else {
                Style::default().fg(TEXT)
            };
            ListItem::new(Line::from(Span::styled(format!(" {label} "), style)))
        })
        .collect();
    f.render_widget(List::new(list_items).block(panel(&format!("{title}  (enter to run)"))), area);
}

fn draw_help(f: &mut Frame) {
    let area = centered(66, 86, f.area());
    f.render_widget(Clear, area);
    let line = |k: &str, d: &str| {
        Line::from(vec![
            Span::styled(format!("  {k:<14}"), Style::default().fg(MINT).bold()),
            Span::styled(d.to_string(), Style::default().fg(TEXT)),
        ])
    };
    let text = Text::from(vec![
        head("Spaces  (1-5)"),
        line("1 Home", "live system monitor"),
        line("2 Memory", "Browse · Episodes · Recall"),
        line("3 Mind", "Identity · Principles · Narrative · Attention · Reflections"),
        line("4 Graph", "knowledge entities + neighbors"),
        line("5 Systems", "Overview · Agents · Scheduler · Events · Logs"),
        Line::from(""),
        head("Navigation"),
        line("h / l ← →", "switch sub-view within a space"),
        line("j/k ↑↓", "move selection"),
        line("g / G", "top / bottom"),
        line("enter", "open detail / inspect"),
        line("esc", "close overlay / detail"),
        line("r", "refresh current view"),
        Line::from(""),
        head("Memory · Browse"),
        line("/  v  s", "search · cycle validity · cycle status"),
        line("n / p", "next / prev page"),
        line("i", "invalidate selected memory"),
        line("m", "recall mode (in Recall sub)"),
        Line::from(""),
        head("Mind"),
        line("a", "propose identity fact"),
        line("t", "trigger reflection/consolidation/narrative"),
        Line::from(""),
        head("Graph"),
        line("enter", "explore an entity's connections"),
        line("⌫ / esc", "go back up the path"),
        line("/", "search / filter entities"),
        Line::from(""),
        head("Systems"),
        line("c", "Agents: cancel run · Events: cycle category"),
        line("e / x", "Scheduler: enable·disable / run now"),
        line("d / n", "Scheduler: delete / new job"),
        line("s", "Logs: cycle source"),
        Line::from(""),
        head("Global"),
        line("A", "capture episode (ingest)"),
        line("? / q", "help / quit"),
    ]);
    f.render_widget(Paragraph::new(text).block(panel("Help  (any key closes)")).wrap(Wrap { trim: true }), area);
}

// ── small helpers ────────────────────────────────────────────────────────────
fn head(t: &str) -> Line<'static> {
    Line::from(Span::styled(format!(" {t}"), Style::default().fg(CORAL).bold()))
}

/// Split a space area into [sub-tab bar (1 row), content].
fn subsplit(area: Rect) -> std::rc::Rc<[Rect]> {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(area)
}

/// A one-line KPI header: bold value + dim label, dot-separated.
// ── Systems · Routing (LLM tier health) ────────────────────────────────────
fn draw_sys_routing(f: &mut Frame, app: &App, area: Rect) {
    let Some(obs) = &app.llm_obs else {
        f.render_widget(Paragraph::new("loading routing…").block(panel("Routing")), area);
        return;
    };
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(area);

    let total_calls: i64 = obs.tiers.values().map(|t| t.calls).sum();
    let total_err: i64 = obs.tiers.values().map(|t| t.errors).sum();
    let total_fb: i64 = obs.tiers.values().map(|t| t.fallbacks).sum();
    f.render_widget(
        Paragraph::new(stat_bar(&[
            ("LLM calls", fmt_int(total_calls), GREEN),
            ("errors", fmt_int(total_err), if total_err > 0 { RED } else { DIM }),
            ("fallbacks", fmt_int(total_fb), if total_fb > 0 { AMBER } else { DIM }),
            ("uptime", fmt_dur(obs.uptime_seconds), PERI),
        ])),
        rows[0],
    );

    // one card per tier, in routing order (fast = high-volume, deep = heavy)
    let order = ["fast", "agentic", "deep"];
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Ratio(1, 3); 3])
        .split(rows[1]);
    let row = |k: &str, v: String, col: Color| {
        Line::from(vec![
            Span::styled(format!("  {k:<10}"), Style::default().fg(DIM)),
            Span::styled(v, Style::default().fg(col)),
        ])
    };
    for (i, name) in order.iter().enumerate() {
        let title = format!("{} tier", name);
        let Some(t) = obs.tiers.get(*name) else {
            f.render_widget(Paragraph::new("not configured").block(panel(&title)), cols[i]);
            continue;
        };
        let dot = Span::styled("● ", Style::default().fg(if t.configured { GREEN } else { FAINT }));
        let err_col = if t.errors > 0 { RED } else { DIM };
        let lat_col = if t.avg_latency_ms > 4000.0 { RED } else if t.avg_latency_ms > 1500.0 { AMBER } else { GREEN };
        let mut lines = vec![
            Line::from(vec![dot, Span::styled(fit(&t.model, 24), Style::default().fg(TEXT).bold())]),
            Line::from(Span::styled(format!("  {}", fit(&t.endpoint, 30)), Style::default().fg(FAINT))),
            Line::raw(""),
            row("calls", fmt_int(t.calls), GREEN),
            row("errors", fmt_int(t.errors), err_col),
            row("err rate", format!("{:.1}%", t.error_rate * 100.0), err_col),
            row("fallbacks", fmt_int(t.fallbacks), if t.fallbacks > 0 { AMBER } else { DIM }),
            row("avg lat", format!("{:.0}ms", t.avg_latency_ms), lat_col),
        ];
        if let Some(lm) = &t.last_model {
            lines.push(row("last", fit(lm, 22), DIM));
        }
        if let Some(le) = &t.last_error {
            lines.push(Line::from(Span::styled(format!("  ⚠ {}", fit(le, 28)), Style::default().fg(RED))));
        }
        f.render_widget(Paragraph::new(lines).block(panel(&title)), cols[i]);
    }
}

// ── Systems · Pipeline (ingest backlog + recall cache) ──────────────────────
fn draw_sys_pipeline(f: &mut Frame, app: &App, area: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(area);

    let pl = app.pipeline_obs.as_ref();
    let rc = app.recall_obs.as_ref();
    let i1h = pl.map(|p| p.ingest_rate.last_1h).unwrap_or(0);
    let i24 = pl.map(|p| p.ingest_rate.last_24h).unwrap_or(0);
    let hit = rc.map(|r| r.hit_rate).unwrap_or(0.0);
    f.render_widget(
        Paragraph::new(stat_bar(&[
            ("ingest 1h", fmt_int(i1h), GREEN),
            ("ingest 24h", fmt_int(i24), MINT),
            ("recall hit-rate", format!("{:.0}%", hit * 100.0), if hit >= 0.5 { GREEN } else { AMBER }),
        ])),
        rows[0],
    );

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(rows[1]);

    // left: pipeline backlog + KG size
    let mut left: Vec<Line> = vec![];
    if let Some(p) = pl {
        let b = &p.backlog;
        let warn = |n: i64| if n > 50 { AMBER } else if n > 0 { TEXT } else { DIM };
        left.push(backlog_row("unembedded episodes", b.unembedded_episodes, warn(b.unembedded_episodes)));
        left.push(backlog_row("unembedded memories", b.unembedded_memories, warn(b.unembedded_memories)));
        left.push(backlog_row("awaiting distillation", b.episodes_awaiting_distillation, warn(b.episodes_awaiting_distillation)));
        left.push(Line::raw(""));
        left.push(backlog_row("KG entities", p.knowledge_graph.entities, PERI));
        left.push(backlog_row("KG edges", p.knowledge_graph.edges, PERI));
    } else {
        left.push(Line::from(Span::styled("  loading pipeline…", Style::default().fg(DIM))));
    }
    f.render_widget(Paragraph::new(left).block(panel("Pipeline backlog")), cols[0]);

    // right: recall cache metrics
    let mut right: Vec<Line> = vec![];
    if let Some(r) = rc {
        right.push(backlog_row("requests", r.requests, TEXT));
        right.push(backlog_row("cache hits", r.cache_hits, GREEN));
        right.push(backlog_row("cache misses", r.cache_misses, DIM));
        right.push(backlog_row("errors", r.errors, if r.errors > 0 { RED } else { DIM }));
        right.push(Line::raw(""));
        right.push(Line::from(vec![
            Span::styled(format!("  {:<22}", "hit-rate"), Style::default().fg(DIM)),
            Span::styled(format!("{:.1}%", r.hit_rate * 100.0), Style::default().fg(if r.hit_rate >= 0.5 { GREEN } else { AMBER }).bold()),
        ]));
        right.push(Line::from(vec![
            Span::styled(format!("  {:<22}", "avg latency (cold)"), Style::default().fg(DIM)),
            Span::styled(format!("{:.0}ms", r.avg_latency_ms), Style::default().fg(TEXT)),
        ]));
        right.push(Line::from(vec![
            Span::styled(format!("  {:<22}", "avg latency (cached)"), Style::default().fg(DIM)),
            Span::styled(format!("{:.0}ms", r.avg_latency_cached_ms), Style::default().fg(GREEN)),
        ]));
    } else {
        right.push(Line::from(Span::styled("  loading recall…", Style::default().fg(DIM))));
    }
    f.render_widget(Paragraph::new(right).block(panel("Recall · cache")), cols[1]);
}

fn backlog_row(label: &str, n: i64, col: Color) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("  {:<22}", label), Style::default().fg(DIM)),
        Span::styled(fmt_int(n), Style::default().fg(col).bold()),
    ])
}

// ── Systems · Settings (typed runtime knobs, editable) ──────────────────────
fn draw_sys_settings(f: &mut Frame, app: &mut App, area: Rect) {
    if app.settings.is_empty() {
        f.render_widget(Paragraph::new("loading settings…").block(panel("Settings")), area);
        return;
    }
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(area);

    let items: Vec<ListItem> = app
        .settings
        .iter()
        .map(|(key, it)| {
            let val = setting_value_str(&it.value);
            let val_col = match it.ty.as_deref() {
                Some("bool") if it.value.as_bool() == Some(true) => GREEN,
                Some("bool") => FAINT,
                _ => MINT,
            };
            let marker = if it.is_default {
                Span::styled("  ", Style::default())
            } else {
                Span::styled("• ", Style::default().fg(AMBER))
            };
            ListItem::new(Line::from(vec![
                marker,
                Span::styled(fit(key, 28), Style::default().fg(TEXT).bold()),
                Span::styled("= ", Style::default().fg(DIM)),
                Span::styled(val, Style::default().fg(val_col)),
            ]))
        })
        .collect();
    f.render_stateful_widget(
        list_of(format!("Settings ({}) · ↵ toggle/edit", app.settings.len()), items),
        cols[0],
        &mut app.set_sel.state,
    );

    // detail of the selected setting
    let detail = app.set_sel.selected().and_then(|i| app.settings.get(i));
    let mut lines: Vec<Line> = vec![];
    if let Some((key, it)) = detail {
        lines.push(Line::from(Span::styled(key.clone(), Style::default().fg(PERI).bold())));
        lines.push(Line::raw(""));
        lines.push(kv("category", it.category.as_deref().unwrap_or("-")));
        lines.push(kv("type", it.ty.as_deref().unwrap_or("-")));
        lines.push(kv("current", &setting_value_str(&it.value)));
        lines.push(kv("default", &setting_value_str(&it.default)));
        lines.push(kv("source", if it.is_default { "default" } else { "overridden" }));
        if let Some(u) = &it.updated_at {
            lines.push(kv("updated", &fmt_date(Some(u))));
        }
        lines.push(Line::raw(""));
        if let Some(d) = &it.description {
            for w in wrap_text(d, 40) {
                lines.push(Line::from(Span::styled(w, Style::default().fg(DIM))));
            }
        }
    }
    f.render_widget(Paragraph::new(lines).block(panel("Detail")), cols[1]);
}

fn setting_value_str(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Null => "—".into(),
        other => other.to_string(),
    }
}

/// Greedy word-wrap to `width` columns (approx; counts chars, not graphemes).
fn wrap_text(s: &str, width: usize) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    let mut line = String::new();
    for word in s.split_whitespace() {
        if !line.is_empty() && line.chars().count() + 1 + word.chars().count() > width {
            out.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() {
        out.push(line);
    }
    out
}

/// Seconds → compact human duration (e.g. "45s", "12m", "3.2h").
fn fmt_dur(secs: f64) -> String {
    if secs < 90.0 {
        format!("{secs:.0}s")
    } else if secs < 5400.0 {
        format!("{:.0}m", secs / 60.0)
    } else {
        format!("{:.1}h", secs / 3600.0)
    }
}

fn stat_bar(items: &[(&str, String, Color)]) -> Line<'static> {
    let mut spans = vec![Span::raw(" ")];
    for (i, (label, value, col)) in items.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("  ·  ", Style::default().fg(FAINT)));
        }
        spans.push(Span::styled(format!("{value} "), Style::default().fg(*col).bold()));
        spans.push(Span::styled(label.to_string(), Style::default().fg(DIM)));
    }
    Line::from(spans)
}

/// A filter chip: filled (active) or outlined (default/off).
fn filter_chip(k: &str, v: &str, active: bool) -> Span<'static> {
    let style = if active {
        Style::default().fg(Color::Black).bg(AMBER).bold()
    } else {
        Style::default().fg(DIM)
    };
    Span::styled(format!(" {k}:{v} "), style)
}

fn kv(k: &str, v: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("  {k:<16}"), Style::default().fg(DIM)),
        Span::styled(v.to_string(), Style::default().fg(TEXT)),
    ])
}

/// A labelled horizontal bar scaled to `max`, sized to `width` cells.
fn hbar(label: &str, v: i64, max: i64, width: usize, col: Color) -> Line<'static> {
    let frac = if max > 0 { (v as f64 / max as f64).clamp(0.0, 1.0) } else { 0.0 };
    let filled = (frac * width as f64).round() as usize;
    let bar: String = "█".repeat(filled) + &"░".repeat(width.saturating_sub(filled));
    Line::from(vec![
        Span::styled(format!("  {:<13}", truncate(label, 13)), Style::default().fg(TEXT)),
        Span::styled(bar, Style::default().fg(col)),
        Span::styled(format!(" {}", fmt_int(v)), Style::default().fg(DIM)),
    ])
}

/// A bare scaled bar (no label), for inline use.
fn bar_str(v: i64, max: i64, width: usize) -> String {
    let frac = if max > 0 { (v as f64 / max as f64).clamp(0.0, 1.0) } else { 0.0 };
    let filled = (frac * width as f64).round() as usize;
    "█".repeat(filled) + &"░".repeat(width.saturating_sub(filled))
}

/// `[ON]` / `[OFF]` pill for a scheduled job.
fn enabled_badge(on: bool) -> Span<'static> {
    if on {
        Span::styled(" ON  ", Style::default().fg(Color::Black).bg(GREEN).bold())
    } else {
        Span::styled(" OFF ", Style::default().fg(TEXT).bg(SEL_BG))
    }
}

fn job_status_dot(status: Option<&str>) -> Span<'static> {
    let col = match status {
        Some("completed") | Some("success") | Some("ok") => GREEN,
        Some("failed") | Some("error") => RED,
        Some("running") => AMBER,
        _ => FAINT,
    };
    Span::styled(" ● ", Style::default().fg(col))
}

/// Filled status pill for an agent run.
fn run_badge(status: Option<&str>) -> Span<'static> {
    let (txt, fg, bg) = match status {
        Some("running") => ("RUN ", Color::Black, AMBER),
        Some("completed") | Some("success") => ("DONE", Color::Black, GREEN),
        Some("failed") | Some("error") => ("FAIL", Color::Black, RED),
        Some("cancelled") | Some("canceled") => ("CANC", TEXT, SEL_BG),
        _ => ("·   ", DIM, SEL_BG),
    };
    Span::styled(format!(" {txt} "), Style::default().fg(fg).bg(bg).bold())
}

fn event_cat_color(idx: usize) -> Color {
    match idx {
        0 => TEXT,    // All
        1 => PURPLE,  // Agents
        2 => CORAL,   // Goals
        3 => CYAN,    // Knowledge
        4 => GREEN,   // Memory
        5 => PERI,    // Cognition
        6 => AMBER,   // Decisions
        _ => DIM,     // Other
    }
}

/// Render a unicode block sparkline from a series, normalized to its own max.
fn spark(values: &[u64]) -> String {
    let max = values.iter().copied().max().unwrap_or(0);
    if max == 0 {
        return "▁".repeat(values.len().max(1));
    }
    values
        .iter()
        .map(|&v| {
            let idx = ((v as f64 / max as f64) * (SPARK.len() - 1) as f64).round() as usize;
            SPARK[idx.min(SPARK.len() - 1)]
        })
        .collect()
}

/// Bucket recent events into `width` time bins. Returns (series, events/min, span_secs).
fn activity_series(events: &[Event], width: usize) -> (Vec<u64>, f64, i64) {
    let times: Vec<i64> = events.iter().filter_map(|e| e.created_at.as_deref().and_then(ts_secs)).collect();
    if times.is_empty() {
        return (vec![0; width], 0.0, 0);
    }
    let min = *times.iter().min().unwrap();
    let max = *times.iter().max().unwrap();
    let span = (max - min).max(1);
    let mut bins = vec![0u64; width];
    for t in &times {
        let b = (((t - min) as f64 / span as f64) * (width - 1) as f64).round() as usize;
        bins[b.min(width - 1)] += 1;
    }
    let minutes = (span as f64 / 60.0).max(1.0 / 60.0);
    let rate = times.len() as f64 / minutes;
    (bins, rate, span)
}

/// Approximate UTC timestamp → seconds. Exact within a month (linear in real
/// time); only used for relative binning/rate over a recent window.
fn ts_secs(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    if b.len() < 19 {
        return None;
    }
    let p = |a: usize, z: usize| s.get(a..z)?.parse::<i64>().ok();
    let (y, mo, d) = (p(0, 4)?, p(5, 7)?, p(8, 10)?);
    let (h, mi, se) = (p(11, 13)?, p(14, 16)?, p(17, 19)?);
    Some((((y * 372 + mo * 31 + d) * 24 + h) * 3600) + mi * 60 + se)
}

fn human_span(secs: i64) -> String {
    if secs <= 0 {
        "—".into()
    } else if secs < 90 {
        format!("{secs}s")
    } else if secs < 5400 {
        format!("{}m", secs / 60)
    } else {
        format!("{}h", secs / 3600)
    }
}

fn status_dot(status: Option<&str>) -> Span<'static> {
    let col = match status {
        Some("completed") | Some("success") | Some("ok") => MINT,
        Some("failed") | Some("error") => RED,
        Some("running") => AMBER,
        Some("never") | None => FAINT,
        _ => DIM,
    };
    Span::styled("  ● ", Style::default().fg(col))
}

fn run_dot(status: Option<&str>) -> Span<'static> {
    status_dot(status)
}

fn run_status(status: Option<&str>) -> String {
    status.unwrap_or("unknown").to_string()
}

fn action_color(kind: Option<&str>) -> Color {
    match kind {
        Some("file_edit") | Some("write") => CORAL,
        Some("read") => PERI,
        Some("recall") => PURPLE,
        _ => MINT,
    }
}

fn action_summary(payload: &serde_json::Value) -> String {
    let tool = payload.get("tool").and_then(|v| v.as_str()).unwrap_or("");
    let args = payload.get("args");
    let arg = args
        .and_then(|a| a.get("path").or_else(|| a.get("query")).or_else(|| a.get("command")))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if tool.is_empty() && arg.is_empty() {
        truncate(&payload.to_string(), 70)
    } else {
        truncate(&format!("{tool}  {arg}"), 70)
    }
}

fn event_color(t: &str) -> Color {
    if t.contains("failed") || t.contains("error") {
        RED
    } else if t.contains("started") || t.contains("run") {
        AMBER
    } else if t.contains("updated") || t.contains("created") {
        MINT
    } else {
        PERI
    }
}

fn short_event(t: &str) -> String {
    t.strip_prefix("art.curlybrackets.curlyos.").unwrap_or(t).to_string()
}

fn json_lines(v: &serde_json::Value, depth: usize) -> Vec<Line<'static>> {
    let pad = "  ".repeat(depth + 1);
    let mut out = vec![];
    match v {
        serde_json::Value::Object(m) => {
            for (k, val) in m {
                match val {
                    serde_json::Value::Object(_) | serde_json::Value::Array(_) => {
                        out.push(Line::from(Span::styled(format!("{pad}{k}:"), Style::default().fg(PERI))));
                        out.extend(json_lines(val, depth + 1));
                    }
                    _ => out.push(Line::from(vec![
                        Span::styled(format!("{pad}{k}: "), Style::default().fg(PERI)),
                        Span::styled(truncate(&json_scalar(val), 80), Style::default().fg(TEXT)),
                    ])),
                }
            }
        }
        serde_json::Value::Array(a) => {
            for (i, val) in a.iter().take(8).enumerate() {
                out.push(Line::from(vec![
                    Span::styled(format!("{pad}[{i}] "), Style::default().fg(FAINT)),
                    Span::styled(truncate(&json_scalar(val), 80), Style::default().fg(TEXT)),
                ]));
            }
        }
        _ => out.push(Line::from(Span::styled(format!("{pad}{}", json_scalar(v)), Style::default().fg(TEXT)))),
    }
    out
}

fn json_scalar(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn log_line(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Object(m) => {
            let msg = m.get("message").or_else(|| m.get("msg")).and_then(|x| x.as_str()).unwrap_or("");
            if msg.is_empty() {
                v.to_string()
            } else {
                let lvl = m.get("level").and_then(|x| x.as_str()).unwrap_or("");
                format!("{lvl} {msg}").trim().to_string()
            }
        }
        other => other.to_string(),
    }
}

fn log_color(v: &serde_json::Value) -> Style {
    let s = log_line(v).to_lowercase();
    let col = if s.contains("error") || s.contains("traceback") || s.contains("critical") {
        RED
    } else if s.contains("warn") {
        AMBER
    } else if s.contains("info") {
        DIM
    } else {
        TEXT
    };
    Style::default().fg(col)
}

fn centered(pct_x: u16, pct_y: u16, r: Rect) -> Rect {
    let v = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - pct_y) / 2),
            Constraint::Percentage(pct_y),
            Constraint::Percentage((100 - pct_y) / 2),
        ])
        .split(r);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - pct_x) / 2),
            Constraint::Percentage(pct_x),
            Constraint::Percentage((100 - pct_x) / 2),
        ])
        .split(v[1])[1]
}

fn fmt_int(n: i64) -> String {
    let s = n.abs().to_string();
    let mut out = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    let rev: String = out.chars().rev().collect();
    if n < 0 {
        format!("-{rev}")
    } else {
        rev
    }
}

fn fmt_bytes(n: i64) -> String {
    if n < 1024 {
        format!("{n} B")
    } else if n < 1_048_576 {
        format!("{:.1} KB", n as f64 / 1024.0)
    } else {
        format!("{:.1} MB", n as f64 / 1_048_576.0)
    }
}

fn fmt_date(s: Option<&str>) -> String {
    match s {
        Some(t) if t.len() >= 16 => t[..16].replace('T', " "),
        Some(t) => t.to_string(),
        None => "—".into(),
    }
}

fn fmt_time(s: Option<&str>) -> String {
    match s {
        Some(t) if t.len() >= 19 => t[11..19].to_string(),
        Some(t) if t.len() >= 16 => t[11..16].to_string(),
        _ => "--:--".into(),
    }
}
