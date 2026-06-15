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
        (Tab::Home, _) => "1-5 spaces · h/l sub-view · r refresh",
        (Tab::Memory, 0) => "enter detail · / search · v validity · s status · n/p page · i invalidate",
        (Tab::Memory, 1) => "enter detail · n/p page",
        (Tab::Memory, _) => "/ edit query · m mode · enter open memory",
        (Tab::Mind, 0) => "a propose fact · t trigger job",
        (Tab::Mind, _) => "↑↓ browse · t trigger reflection/narrative/consolidation",
        (Tab::Graph, _) => "↑↓ neighbors update",
        (Tab::Systems, 1) => "enter inspect run · esc close · auto-refreshing",
        (Tab::Systems, 2) => "↑↓ jobs · x run now",
        (Tab::Systems, 4) => "s cycle log source",
        (Tab::Systems, _) => "h/l sub-view · live",
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
        0 => draw_identity(f, app, rows[1]),
        1 => draw_principles(f, app, rows[1]),
        2 => draw_narrative(f, app, rows[1]),
        3 => draw_attention(f, app, rows[1]),
        _ => draw_reflections(f, app, rows[1]),
    }
}

fn draw_identity(f: &mut Frame, app: &mut App, area: Rect) {
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
    let hyp = app.identity.iter().filter(|f| f.epistemic_status.as_deref() == Some("hypothesis")).count();
    f.render_widget(
        Paragraph::new(stat_bar(&[
            ("facts", fmt_int(app.identity.len() as i64), PERI),
            ("canonical", fmt_int(canon as i64), MINT),
            ("hypothesis", fmt_int(hyp as i64), AMBER),
            ("avg confidence", format!("{:.0}%", avg * 100.0), GREEN),
        ])),
        rows[0],
    );

    let items: Vec<ListItem> = app
        .identity
        .iter()
        .map(|fct| {
            let filled = (fct.confidence.clamp(0.0, 1.0) * 6.0).round() as usize;
            let bar: String = "▰".repeat(filled) + &"▱".repeat(6usize.saturating_sub(filled));
            ListItem::new(Line::from(vec![
                Span::styled(format!("{bar} "), Style::default().fg(conf_color(fct.confidence))),
                Span::styled(format!("{:<20}", truncate(&fct.predicate, 20)), Style::default().fg(PERI).bold()),
                Span::styled(truncate(&fct.object, 80), Style::default().fg(TEXT)),
                Span::styled(format!("  ({})", fct.epistemic_status.clone().unwrap_or_default()), Style::default().fg(FAINT)),
            ]))
        })
        .collect();
    f.render_stateful_widget(
        list_of(format!("Identity facts ({})  ·  a: propose", app.identity.len()), items),
        rows[1],
        &mut app.id_sel.state,
    );
}

fn draw_principles(f: &mut Frame, app: &mut App, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
        .split(area);
    let items: Vec<ListItem> = app
        .principles
        .iter()
        .map(|p| {
            ListItem::new(Line::from(vec![
                Span::styled("◆ ", Style::default().fg(PURPLE)),
                Span::styled(truncate(&p.statement, 80), Style::default().fg(TEXT)),
            ]))
        })
        .collect();
    f.render_stateful_widget(list_of(format!("Principles ({}) · t: distill", app.principles.len()), items), cols[0], &mut app.prin_sel.state);
    let prev = app
        .prin_sel
        .selected()
        .and_then(|i| app.principles.get(i))
        .map(|p| {
            Text::from(vec![
                Line::from(Span::styled(p.statement.clone(), Style::default().fg(TEXT))),
                Line::from(""),
                kv("domain", p.domain.as_deref().unwrap_or("—")),
                kv("status", p.epistemic_status.as_deref().unwrap_or("—")),
            ])
        })
        .unwrap_or_else(|| Text::from("no selection"));
    f.render_widget(Paragraph::new(prev).block(panel("Principle")).wrap(Wrap { trim: true }), cols[1]);
}

fn draw_narrative(f: &mut Frame, app: &mut App, area: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(area);
    f.render_widget(
        Paragraph::new(stat_bar(&[("chapters", fmt_int(app.chapters.len() as i64), CORAL), ("t", "compose / generate".into(), DIM)])),
        rows[0],
    );
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(rows[1]);
    let items: Vec<ListItem> = app
        .chapters
        .iter()
        .map(|c| {
            ListItem::new(vec![
                Line::from(Span::styled(truncate(&c.title, 46), Style::default().fg(CORAL).bold())),
                Line::from(Span::styled(format!("   {}", fmt_date(c.start_date.as_deref())), Style::default().fg(FAINT))),
            ])
        })
        .collect();
    f.render_stateful_widget(list_of(format!("Chapters ({})", app.chapters.len()), items), cols[0], &mut app.chap_sel.state);
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
        .unwrap_or_else(|| Text::from("no selection"));
    f.render_widget(Paragraph::new(prev).block(panel("Chapter")).wrap(Wrap { trim: true }), cols[1]);
}

fn draw_reflections(f: &mut Frame, app: &mut App, area: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(area);
    let findings: usize = app.reports.iter().map(|r| r.findings.len()).sum();
    f.render_widget(
        Paragraph::new(stat_bar(&[
            ("reports", fmt_int(app.reports.len() as i64), PERI),
            ("findings", fmt_int(findings as i64), GREEN),
            ("t", "weekly / monthly".into(), DIM),
        ])),
        rows[0],
    );
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(rows[1]);
    let items: Vec<ListItem> = app
        .reports
        .iter()
        .map(|r| {
            ListItem::new(Line::from(vec![
                Span::styled(format!("{:<8}", r.report_type.clone().unwrap_or_default()), Style::default().fg(PERI).bold()),
                Span::styled(format!("{} findings", r.findings.len()), Style::default().fg(GREEN)),
            ]))
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
        .unwrap_or_else(|| Text::from("no selection"));
    f.render_widget(Paragraph::new(prev).block(panel("Findings")).wrap(Wrap { trim: true }), cols[1]);
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

fn draw_attention(f: &mut Frame, app: &App, area: Rect) {
    let Some(a) = &app.attention else {
        f.render_widget(Paragraph::new("press r to load attention").block(panel("Attention")), area);
        return;
    };
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    let load = a.cognitive_load.score.clamp(0.0, 1.0);
    let g = Gauge::default()
        .block(panel("Cognitive load"))
        .gauge_style(Style::default().fg(if load > 0.8 { CORAL } else { MINT }))
        .ratio(load)
        .label(format!(
            "{:.0}%  density {:.2} · switching {:.2} · {} eps/{}d",
            load * 100.0,
            a.cognitive_load.breakdown.density,
            a.cognitive_load.breakdown.topic_switching,
            a.cognitive_load.breakdown.episode_count,
            a.cognitive_load.breakdown.window_days
        ));
    f.render_widget(g, rows[0]);

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(34), Constraint::Percentage(33), Constraint::Percentage(33)])
        .split(rows[1]);
    let focus: Vec<Line> = a.focus_areas.iter().take(15).map(|x| weight_line(x, MINT)).collect();
    f.render_widget(Paragraph::new(focus).block(panel("Focus areas")), cols[0]);
    let neglect: Vec<Line> = a.neglected.iter().take(15).map(|x| weight_line(x, AMBER)).collect();
    f.render_widget(Paragraph::new(neglect).block(panel("Neglected")), cols[1]);
    let mut breadth: Vec<Line> = vec![
        kv("total entities", &fmt_int(a.breadth.total_entities)),
        kv("distinct types", &fmt_int(a.breadth.distinct_types)),
        kv("concentration", &format!("{:.2}", a.breadth.concentration)),
        Line::from(""),
    ];
    for (k, v) in &a.breadth.by_type {
        breadth.push(kv(k, &fmt_int(*v)));
    }
    f.render_widget(Paragraph::new(breadth).block(panel("Breadth")), cols[2]);
}

fn weight_line(x: &FocusArea, col: Color) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{:>4} ", x.weight), Style::default().fg(col).bold()),
        Span::styled(truncate(&x.name, 22), Style::default().fg(TEXT)),
        Span::styled(format!(" · {}", x.label.clone().unwrap_or_default()), Style::default().fg(FAINT)),
    ])
}

// ── Graph ────────────────────────────────────────────────────────────────────
fn draw_graph(f: &mut Frame, app: &mut App, area: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(area);
    let density = if app.stats.knowledge_entities > 0 {
        app.stats.knowledge_edges as f64 / app.stats.knowledge_entities as f64
    } else {
        0.0
    };
    let types = app.attention.as_ref().map(|a| a.breadth.distinct_types).unwrap_or(0);
    f.render_widget(
        Paragraph::new(stat_bar(&[
            ("entities", fmt_int(app.stats.knowledge_entities), PURPLE),
            ("edges", fmt_int(app.stats.knowledge_edges), LIME),
            ("density", format!("{density:.2}"), CYAN),
            ("types", fmt_int(types), GREEN),
        ])),
        rows[0],
    );
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(rows[1]);

    let maxdeg = app.graph.nodes.iter().map(|n| n.degree).max().unwrap_or(1).max(1);
    let items: Vec<ListItem> = app
        .graph
        .nodes
        .iter()
        .map(|n| {
            let filled = ((n.degree as f64 / maxdeg as f64) * 8.0).round() as usize;
            let bar: String = "▰".repeat(filled) + &"▱".repeat(8usize.saturating_sub(filled));
            ListItem::new(Line::from(vec![
                Span::styled(format!("{bar} "), Style::default().fg(LIME)),
                Span::styled(format!("{:>5} ", n.degree), Style::default().fg(MINT).bold()),
                Span::styled(format!("{:<30}", truncate(&n.name, 30)), Style::default().fg(TEXT)),
                Span::styled(n.label.clone().unwrap_or_default(), Style::default().fg(PERI)),
            ]))
        })
        .collect();
    f.render_stateful_widget(list_of(format!("Top entities ({})", app.graph.nodes.len()), items), cols[0], &mut app.node_sel.state);

    let mut lines: Vec<Line> = vec![];
    if let Some(node) = app.node_sel.selected().and_then(|i| app.graph.nodes.get(i)) {
        lines.push(Line::from(Span::styled(node.name.clone(), Style::default().fg(CORAL).bold())));
        lines.push(Line::from(Span::styled(format!("degree {}", node.degree), Style::default().fg(DIM))));
        lines.push(Line::from(""));
        let name_of = |id: &str| {
            app.graph.nodes.iter().find(|n| n.id == id).map(|n| n.name.clone()).unwrap_or_else(|| id.to_string())
        };
        let mut shown = 0;
        for l in &app.graph.links {
            if l.source == node.id {
                lines.push(rel_line(&l.rel_type, "→", &name_of(&l.target)));
                shown += 1;
            } else if l.target == node.id {
                lines.push(rel_line(&l.rel_type, "←", &name_of(&l.source)));
                shown += 1;
            }
        }
        if shown == 0 {
            lines.push(Line::from(Span::styled("no edges within loaded set", Style::default().fg(DIM))));
        }
    } else {
        lines.push(Line::from("no selection"));
    }
    f.render_widget(Paragraph::new(lines).block(panel("Neighbors")).wrap(Wrap { trim: true }), cols[1]);
}

fn rel_line(rel: &Option<String>, arrow: &str, other: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{arrow} "), Style::default().fg(MINT)),
        Span::styled(format!("{:<16}", rel.clone().unwrap_or_default()), Style::default().fg(DIM)),
        Span::styled(other.to_string(), Style::default().fg(TEXT)),
    ])
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
        _ => draw_sys_logs(f, app, rows[1]),
    }
}

fn draw_sys_overview(f: &mut Frame, app: &App, area: Rect) {
    let Some(sys) = &app.systems else {
        f.render_widget(Paragraph::new("loading systems…").block(panel("Overview")), area);
        return;
    };
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);

    let mut infra: Vec<Line> = vec![head("Infrastructure")];
    for i in &sys.infrastructure {
        let col = if i.ok { MINT } else { RED };
        infra.push(Line::from(vec![
            Span::styled("  ● ", Style::default().fg(col)),
            Span::styled(format!("{:<12}", i.name), Style::default().fg(TEXT)),
        ]));
        infra.push(Line::from(Span::styled(format!("     {}", truncate(i.detail.as_deref().unwrap_or(""), 40)), Style::default().fg(FAINT))));
    }
    infra.push(Line::from(""));
    infra.push(head("Scheduler"));
    let (dot, col) = if sys.scheduler.running { ("●", MINT) } else { ("●", RED) };
    infra.push(Line::from(vec![
        Span::styled(format!("  {dot} "), Style::default().fg(col)),
        Span::styled(format!("{} jobs", sys.scheduler.jobs), Style::default().fg(TEXT)),
        Span::styled(format!("  ·  {} failing", sys.scheduler.failing.len()), Style::default().fg(if sys.scheduler.failing.is_empty() { DIM } else { RED })),
    ]));
    infra.push(kv("next due", &fmt_date(sys.scheduler.next_due.as_deref())));
    f.render_widget(Paragraph::new(infra).block(panel("Health")).wrap(Wrap { trim: true }), cols[0]);

    let mut eng: Vec<Line> = vec![Line::from(vec![
        Span::styled(format!("  {:<20}", "engine"), Style::default().fg(CORAL).bold()),
        Span::styled(format!("{:<7}", "24h"), Style::default().fg(CORAL).bold()),
        Span::styled(format!("{:<7}", "7d"), Style::default().fg(CORAL).bold()),
        Span::styled("last run", Style::default().fg(CORAL).bold()),
    ])];
    for e in &sys.engines {
        let active = e.runs_24h > 0;
        eng.push(Line::from(vec![
            Span::styled(format!("  {:<20}", truncate(e.label.as_deref().unwrap_or(&e.name), 20)), Style::default().fg(TEXT)),
            Span::styled(format!("{:<7}", e.runs_24h), Style::default().fg(if active { MINT } else { DIM })),
            Span::styled(format!("{:<7}", e.runs_7d), Style::default().fg(DIM)),
            Span::styled(fmt_date(e.last_run.as_deref()), Style::default().fg(FAINT)),
        ]));
    }
    f.render_widget(Paragraph::new(eng).block(panel(&format!("Engines ({})", sys.engines.len()))), cols[1]);
}

fn draw_sys_scheduler(f: &mut Frame, app: &mut App, area: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // system cron jobs
    let mut cron: Vec<Line> = vec![];
    if let Some(s) = &app.scheduler {
        for j in &s.jobs {
            cron.push(Line::from(vec![
                status_dot(j.last_status.as_deref()),
                Span::styled(format!("{:<24}", truncate(&j.name, 24)), Style::default().fg(TEXT)),
                Span::styled(format!("{:<16}", j.cadence.clone().unwrap_or_default()), Style::default().fg(PERI)),
                Span::styled(format!("next {}", fmt_date(j.next_due.as_deref())), Style::default().fg(DIM)),
                if j.consecutive_failures > 0 {
                    Span::styled(format!("  {}✗", j.consecutive_failures), Style::default().fg(RED).bold())
                } else {
                    Span::raw("")
                },
            ]));
        }
    }
    let cron_title = app.scheduler.as_ref().map(|s| format!("System jobs ({}) · {}", s.jobs.len(), if s.running { "running" } else { "stopped" })).unwrap_or_else(|| "System jobs".into());
    f.render_widget(Paragraph::new(cron).block(panel(&cron_title)), rows[0]);

    // user scheduled jobs (selectable, run-now with x)
    let items: Vec<ListItem> = app
        .scheduled_jobs
        .iter()
        .map(|j| {
            ListItem::new(vec![
                Line::from(vec![
                    status_dot(j.last_status.as_deref()),
                    Span::styled(format!("{:<28}", truncate(&j.name, 28)), Style::default().fg(TEXT)),
                    Span::styled(format!("{:<14}", j.cadence_display.clone().unwrap_or_default()), Style::default().fg(PERI)),
                    Span::styled(if j.enabled { "on " } else { "off" }, Style::default().fg(if j.enabled { MINT } else { DIM })),
                    Span::styled(format!(" → {}", j.delivery.clone().unwrap_or_default()), Style::default().fg(DIM)),
                ]),
                Line::from(Span::styled(
                    format!("    last {} · next {}{}", fmt_date(j.last_fired.as_deref()), fmt_date(j.next_due.as_deref()),
                        j.last_error.as_deref().map(|e| format!(" · err: {}", truncate(e, 40))).unwrap_or_default()),
                    Style::default().fg(FAINT),
                )),
            ])
        })
        .collect();
    f.render_stateful_widget(
        list_of(format!("Scheduled jobs ({}) · x run now", app.scheduled_jobs.len()), items),
        rows[1],
        &mut app.sched_sel.state,
    );
}

fn draw_sys_events(f: &mut Frame, app: &mut App, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
        .split(area);

    let items: Vec<ListItem> = app
        .events
        .iter()
        .map(|e| {
            ListItem::new(Line::from(vec![
                Span::styled(format!("{} ", fmt_time(e.created_at.as_deref())), Style::default().fg(FAINT)),
                Span::styled(format!("{:<26}", short_event(&e.etype)), Style::default().fg(event_color(&e.etype))),
                Span::styled(truncate(e.subject.as_deref().unwrap_or(""), 30), Style::default().fg(DIM)),
            ]))
        })
        .collect();
    f.render_stateful_widget(list_of(format!("Events ({})", app.events.len()), items), cols[0], &mut app.event_sel.state);

    let prev = app
        .event_sel
        .selected()
        .and_then(|i| app.events.get(i))
        .map(|e| {
            let mut lines = vec![
                Line::from(Span::styled(e.etype.clone(), Style::default().fg(CORAL).bold())),
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
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(area);

    let items: Vec<ListItem> = app
        .agent_runs
        .iter()
        .map(|r| {
            ListItem::new(vec![
                Line::from(vec![
                    run_dot(r.status.as_deref()),
                    Span::styled(format!("{:<11}", r.agent.clone().unwrap_or_default()), Style::default().fg(PURPLE).bold()),
                    Span::styled(truncate(&r.task, 60), Style::default().fg(TEXT)),
                ]),
                Line::from(Span::styled(
                    format!("    {} · {}", run_status(r.status.as_deref()), fmt_date(r.created_at.as_deref())),
                    Style::default().fg(FAINT),
                )),
            ])
        })
        .collect();
    let running = app.agent_runs.iter().filter(|r| r.status.as_deref() == Some("running")).count();
    f.render_stateful_widget(
        list_of(format!("Agent runs ({}) · {} running", app.agent_runs.len(), running), items),
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
        head("Mind / Systems"),
        line("a", "propose identity fact (Identity)"),
        line("t", "trigger reflection/consolidation/narrative"),
        line("x", "run scheduled job now (Scheduler)"),
        line("s", "cycle log source (Logs)"),
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
