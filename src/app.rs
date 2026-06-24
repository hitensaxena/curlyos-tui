//! Application state, navigation, and key handling. Rendering lives in `ui.rs`.

use crate::api::*;
use crate::worker::{Req, Resp};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::widgets::ListState;
use std::sync::mpsc::Sender;

pub const TABS: [&str; 5] = ["Home", "Memory", "Mind", "Graph", "Systems"];
pub const MEM_SUBS: [&str; 3] = ["Browse", "Episodes", "Recall"];
pub const MIND_SUBS: [&str; 5] = ["Overview", "Self", "Focus", "Story", "Insights"];
pub const SYS_SUBS: [&str; 8] =
    ["Overview", "Agents", "Scheduler", "Events", "Logs", "Routing", "Pipeline", "Settings"];
pub const EVENT_CATS: [&str; 8] =
    ["All", "Agents", "Goals", "Knowledge", "Memory", "Cognition", "Decisions", "Other"];

/// Bucket an event type into a category index (1..=7; 0 is the "All" filter).
pub fn event_cat_index(etype: &str) -> usize {
    let t = etype;
    if t.contains("agent") {
        1
    } else if t.contains("goal") {
        2
    } else if t.contains("knowledge") || t.contains("entity") || t.contains("edge") {
        3
    } else if t.contains("memory") || t.contains("episode") {
        4
    } else if t.contains("reflection")
        || t.contains("consolidation")
        || t.contains("narrative")
        || t.contains("attention")
        || t.contains("meta")
        || t.contains("identity")
        || t.contains("principle")
    {
        5
    } else if t.contains("decision") {
        6
    } else {
        7
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Tab {
    Home,
    Memory,
    Mind,
    Graph,
    Systems,
}

const TAB_ORDER: [Tab; 5] = [Tab::Home, Tab::Memory, Tab::Mind, Tab::Graph, Tab::Systems];

impl Tab {
    pub fn index(self) -> usize {
        TAB_ORDER.iter().position(|&t| t == self).unwrap_or(0)
    }
    fn from_index(i: usize) -> Tab {
        TAB_ORDER[i.min(TAB_ORDER.len() - 1)]
    }
    /// Live views auto-refresh on a timer.
    fn is_live(self) -> bool {
        matches!(self, Tab::Home | Tab::Systems)
    }
    /// Sub-view labels for this space (empty for single-view spaces).
    pub fn sub_labels(self) -> &'static [&'static str] {
        match self {
            Tab::Memory => &MEM_SUBS,
            Tab::Mind => &MIND_SUBS,
            Tab::Systems => &SYS_SUBS,
            _ => &[],
        }
    }
}

/// One sample of system health, recorded each `/api/systems` poll.
pub struct SysSample {
    pub latency_ms: u64,
    /// (service name, up) for each infrastructure service this poll
    pub services: Vec<(String, bool)>,
}

pub const SYS_HISTORY_CAP: usize = 90;

/// The entity currently in focus in the knowledge-graph explorer.
#[derive(Clone)]
pub struct FocusNode {
    pub id: String,
    pub name: String,
    pub label: Option<String>,
    pub degree: i64,
}

/// One neighbour of the focus entity (a single relation edge).
pub struct NeighborRow {
    pub id: String,
    pub name: String,
    pub label: Option<String>,
    pub rel_type: Option<String>,
    pub outgoing: bool,
}

/// A scrollable list selection wrapping ratatui's `ListState`.
#[derive(Default)]
pub struct Sel {
    pub state: ListState,
    pub len: usize,
}

impl Sel {
    pub fn set_len(&mut self, len: usize) {
        self.len = len;
        if len == 0 {
            self.state.select(None);
        } else {
            let i = self.state.selected().unwrap_or(0).min(len - 1);
            self.state.select(Some(i));
        }
    }
    pub fn selected(&self) -> Option<usize> {
        self.state.selected()
    }
    pub fn next(&mut self) {
        if self.len == 0 {
            return;
        }
        let i = self.state.selected().map_or(0, |i| (i + 1).min(self.len - 1));
        self.state.select(Some(i));
    }
    pub fn prev(&mut self) {
        if self.len == 0 {
            return;
        }
        let i = self.state.selected().map_or(0, |i| i.saturating_sub(1));
        self.state.select(Some(i));
    }
    pub fn first(&mut self) {
        if self.len > 0 {
            self.state.select(Some(0));
        }
    }
    pub fn last(&mut self) {
        if self.len > 0 {
            self.state.select(Some(self.len - 1));
        }
    }
}

pub enum Overlay {
    None,
    Help,
    Detail(Detail),
    Form(Form),
    Confirm { msg: String, action: Pending },
    Menu { title: String, items: Vec<(String, MenuAction)>, sel: usize },
}

pub enum Detail {
    Memory(Box<MemoryDetail>),
    Episode(Box<EpisodeDetail>),
}

pub enum Pending {
    Invalidate(String),
    RunJob { id: String, name: String },
    DeleteJob { id: String, name: String },
    CancelRun { id: String },
}

#[derive(Clone)]
pub enum MenuAction {
    Trigger { path: String, label: String },
    OpenComposeForm,
    OpenLogMoodForm,
}

pub enum FormKind {
    Search,
    Capture,
    Identity,
    Compose,
    NewJob,
    LogMood,
    EditSetting { key: String },
}

pub struct FormField {
    pub label: String,
    pub value: String,
}

pub struct Form {
    pub kind: FormKind,
    pub title: String,
    pub fields: Vec<FormField>,
    pub active: usize,
}

pub struct App {
    pub tx: Sender<Req>,
    pub tab: Tab,
    pub mem_sub: usize,
    pub mind_sub: usize,
    pub overlay: Overlay,
    pub inflight: usize,
    pub status: Option<(String, bool)>, // (message, is_error)
    pub should_quit: bool,
    pub base: String,

    // home
    pub health: Option<Health>,
    pub stats: Stats,
    pub comp: Composition,

    // memories
    pub mq: MemoryQuery,
    pub memories: Vec<Memory>,
    pub mem_sel: Sel,

    // episodes
    pub epi_offset: usize,
    pub episodes: Vec<Episode>,
    pub epi_sel: Sel,

    // identity
    pub identity: Vec<IdentityFact>,
    pub id_sel: Sel,

    // cognition
    pub principles: Vec<Principle>,
    pub prin_sel: Sel,
    pub chapters: Vec<Chapter>,
    pub chap_sel: Sel,
    pub attention: Option<Attention>,
    pub reports: Vec<Report>,
    pub rep_sel: Sel,
    // cognition v2 — mood, health, context
    pub mood_history: Option<MoodHistory>,
    pub health_signals: Option<HealthSignals>,
    pub mental_model_ctx: Option<String>,
    pub assumptions_ctx: Option<String>,

    // graph explorer
    pub graph: Graph,
    pub node_sel: Sel,
    pub graph_focus: Option<FocusNode>,
    pub graph_stack: Vec<FocusNode>,
    pub graph_ego: Option<GraphExpand>,
    pub graph_filter: String,
    pub graph_editing: bool,
    /// KG composition by originating data source (facebook/instagram/netflix/…)
    pub data_sources: Vec<SourceRow>,

    // recall
    pub recall_mode: String,
    pub recall_query: String,
    pub recall_hits: Vec<RecallHit>,
    pub recall_sel: Sel,
    pub recall_editing: bool,
    pub recall_loading: bool,
    pub recall_submitted: bool,
    pub recall_error: Option<String>,
    /// monotonic frame counter, drives spinner animation
    pub frame: u64,

    // systems
    pub sys_sub: usize,
    pub systems: Option<Systems>,
    /// rolling history of /api/systems polls, for uptime strips + latency graph
    pub sys_history: Vec<SysSample>,
    pub scheduler: Option<Scheduler>,
    pub scheduled_jobs: Vec<ScheduledJob>,
    pub sched_sel: Sel,
    pub events: Vec<Event>,
    pub event_sel: Sel,
    pub event_cat: usize,
    pub log_sources: Vec<LogSource>,
    pub log_source_idx: usize,
    pub logs: Option<Logs>,

    // observability v2 (Systems · Routing / Pipeline / Settings)
    pub llm_obs: Option<LlmObservability>,
    pub recall_obs: Option<RecallStats>,
    pub pipeline_obs: Option<PipelineStats>,
    pub settings: Vec<(String, SettingItem)>,
    pub set_sel: Sel,

    // agents
    pub agent_runs: Vec<AgentRun>,
    pub agent_sel: Sel,
    pub agent_detail: Option<AgentRunDetail>,
}

impl App {
    pub fn new(tx: Sender<Req>, base: String) -> Self {
        App {
            tx,
            tab: Tab::Home,
            mem_sub: 0,
            mind_sub: 0,
            overlay: Overlay::None,
            inflight: 0,
            status: None,
            should_quit: false,
            base,
            health: None,
            stats: Stats::default(),
            comp: Composition::default(),
            mq: MemoryQuery::default(),
            memories: vec![],
            mem_sel: Sel::default(),
            epi_offset: 0,
            episodes: vec![],
            epi_sel: Sel::default(),
            identity: vec![],
            id_sel: Sel::default(),
            principles: vec![],
            prin_sel: Sel::default(),
            chapters: vec![],
            chap_sel: Sel::default(),
            attention: None,
            reports: vec![],
            rep_sel: Sel::default(),
            mood_history: None,
            health_signals: None,
            mental_model_ctx: None,
            assumptions_ctx: None,
            graph: Graph { nodes: vec![], links: vec![] },
            node_sel: Sel::default(),
            graph_focus: None,
            graph_stack: Vec::new(),
            graph_ego: None,
            graph_filter: String::new(),
            graph_editing: false,
            data_sources: Vec::new(),
            recall_mode: "fast".into(),
            recall_query: String::new(),
            recall_hits: vec![],
            recall_sel: Sel::default(),
            recall_editing: false,
            recall_loading: false,
            recall_submitted: false,
            recall_error: None,
            frame: 0,
            sys_sub: 0,
            systems: None,
            sys_history: Vec::new(),
            scheduler: None,
            scheduled_jobs: vec![],
            sched_sel: Sel::default(),
            events: vec![],
            event_sel: Sel::default(),
            event_cat: 0,
            log_sources: vec![],
            log_source_idx: 0,
            logs: None,
            llm_obs: None,
            recall_obs: None,
            pipeline_obs: None,
            settings: vec![],
            set_sel: Sel::default(),
            agent_runs: vec![],
            agent_sel: Sel::default(),
            agent_detail: None,
        }
    }

    fn send(&mut self, req: Req) {
        self.inflight += 1;
        let _ = self.tx.send(req);
    }

    pub fn loading(&self) -> bool {
        self.inflight > 0
    }

    /// Called on a timer by the event loop. Live views (Systems/Agents) re-fetch
    /// quietly so the operator can watch runs progress without pressing `r`.
    /// Skipped while an overlay is open, a request is in flight, or a status
    /// toast is showing — so it never fights the user.
    pub fn auto_refresh(&mut self) {
        if !(self.tab.is_live()
            && self.inflight == 0
            && self.status.is_none()
            && matches!(self.overlay, Overlay::None))
        {
            return;
        }
        match self.tab {
            // Home: refresh only the realtime panels each tick (the heavier
            // aggregates — stats, composition, attention — load on entry / `r`).
            Tab::Home => {
                self.send(Req::Systems);
                self.send(Req::Events(40));
                self.send(Req::AgentRuns(40));
            }
            Tab::Systems => {
                self.refresh();
                // keep an open agent-run detail current
                if self.sys_sub == 1 {
                    if let Some(d) = &self.agent_detail {
                        let id = d.run.id.clone();
                        self.send(Req::AgentRun(id));
                    }
                }
            }
            _ => {}
        }
    }

    // -----------------------------------------------------------------
    // knowledge-graph explorer
    // -----------------------------------------------------------------

    /// Top entities (root view), filtered by the name search.
    pub fn graph_root_nodes(&self) -> Vec<&Node> {
        let f = self.graph_filter.to_lowercase();
        self.graph
            .nodes
            .iter()
            .filter(|n| f.is_empty() || n.name.to_lowercase().contains(&f))
            .collect()
    }

    /// Neighbours of the focus entity (one row per relation), name-filtered,
    /// sorted by relation type then name.
    pub fn graph_neighbors(&self) -> Vec<NeighborRow> {
        let (Some(focus), Some(ego)) = (&self.graph_focus, &self.graph_ego) else {
            return vec![];
        };
        let index: std::collections::HashMap<&str, &GNode> =
            ego.entities.iter().map(|e| (e.id.as_str(), e)).collect();
        let name_of = |id: &str| index.get(id).copied();
        let f = self.graph_filter.to_lowercase();
        let mut rows: Vec<NeighborRow> = ego
            .edges
            .iter()
            .filter_map(|e| {
                let (other, outgoing) = if e.src_entity_id == focus.id {
                    (&e.dst_entity_id, true)
                } else if e.dst_entity_id == focus.id {
                    (&e.src_entity_id, false)
                } else {
                    return None; // not incident to focus (shouldn't happen at k=1)
                };
                let node = name_of(other);
                let name = node.map(|n| n.name.clone()).unwrap_or_else(|| other.clone());
                if !f.is_empty() && !name.to_lowercase().contains(&f) {
                    return None;
                }
                Some(NeighborRow {
                    id: other.clone(),
                    name,
                    label: node.and_then(|n| n.label.clone()),
                    rel_type: e.rel_type.clone(),
                    outgoing,
                })
            })
            .collect();
        rows.sort_by(|a, b| {
            a.rel_type
                .cmp(&b.rel_type)
                .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        rows
    }

    /// Current list length (root nodes or neighbours).
    fn graph_list_len(&self) -> usize {
        if self.graph_focus.is_none() {
            self.graph_root_nodes().len()
        } else {
            self.graph_neighbors().len()
        }
    }

    /// Drill into the selected entity/neighbour, loading its real neighbourhood.
    fn graph_drill(&mut self) {
        let Some(sel) = self.node_sel.selected() else { return };
        let target = if self.graph_focus.is_none() {
            self.graph_root_nodes().get(sel).map(|n| FocusNode {
                id: n.id.clone(),
                name: n.name.clone(),
                label: n.label.clone(),
                degree: n.degree,
            })
        } else {
            self.graph_neighbors().get(sel).map(|nb| FocusNode {
                id: nb.id.clone(),
                name: nb.name.clone(),
                label: nb.label.clone(),
                degree: 0,
            })
        };
        if let Some(t) = target {
            if let Some(cur) = self.graph_focus.take() {
                self.graph_stack.push(cur);
            }
            let id = t.id.clone();
            self.graph_focus = Some(t);
            self.graph_filter.clear();
            self.graph_ego = None;
            self.node_sel.set_len(0);
            self.send(Req::Expand { id, k: 1 });
        }
    }

    /// Go back up the exploration path (neighbour → parent → … → root).
    fn graph_back(&mut self) {
        self.graph_filter.clear();
        if let Some(prev) = self.graph_stack.pop() {
            let id = prev.id.clone();
            self.graph_focus = Some(prev);
            self.graph_ego = None;
            self.node_sel.set_len(0);
            self.send(Req::Expand { id, k: 1 });
        } else if self.graph_focus.is_some() {
            self.graph_focus = None;
            self.graph_ego = None;
            self.node_sel.set_len(self.graph_root_nodes().len());
            self.node_sel.first();
        }
    }

    fn handle_graph_input(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true
            }
            KeyCode::Esc => {
                self.graph_filter.clear();
                self.graph_editing = false;
            }
            KeyCode::Enter => self.graph_editing = false,
            KeyCode::Backspace => {
                self.graph_filter.pop();
            }
            KeyCode::Char(c) => self.graph_filter.push(c),
            _ => {}
        }
        let n = self.graph_list_len();
        self.node_sel.set_len(n);
        self.node_sel.first();
    }

    /// Events matching the active category filter (`event_cat`; 0 = All).
    pub fn filtered_events(&self) -> Vec<&Event> {
        self.events
            .iter()
            .filter(|e| self.event_cat == 0 || event_cat_index(&e.etype) == self.event_cat)
            .collect()
    }

    /// Active sub-view index for the current space.
    pub fn cur_sub(&self) -> usize {
        match self.tab {
            Tab::Memory => self.mem_sub,
            Tab::Mind => self.mind_sub,
            Tab::Systems => self.sys_sub,
            _ => 0,
        }
    }

    fn set_sub(&mut self, v: usize) {
        match self.tab {
            Tab::Memory => self.mem_sub = v,
            Tab::Mind => self.mind_sub = v,
            Tab::Systems => self.sys_sub = v,
            _ => {}
        }
    }

    /// (Re)load data for the current space + sub-view.
    pub fn refresh(&mut self) {
        match self.tab {
            Tab::Home => {
                self.send(Req::Home);
                self.send(Req::Systems);
                self.send(Req::Attention);
                self.send(Req::Events(40));
                self.send(Req::AgentRuns(40));
            }
            Tab::Memory => match self.mem_sub {
                0 => {
                    let q = self.mq.clone();
                    self.send(Req::Memories(q));
                }
                1 => {
                    let off = self.epi_offset;
                    self.send(Req::Episodes { limit: 50, offset: off });
                }
                _ => self.submit_recall(),
            },
            Tab::Mind => match self.mind_sub {
                // Overview composes every source into one monitor.
                0 => {
                    self.send(Req::Identity);
                    self.send(Req::Principles);
                    self.send(Req::Attention);
                    self.send(Req::Narrative);
                    self.send(Req::Reflections);
                    self.send(Req::MentalModelContext);
                    self.send(Req::AssumptionsContext);
                }
                1 => {
                    self.send(Req::Identity);
                    self.send(Req::Principles);
                }
                2 => {
                    self.send(Req::Attention);
                    self.send(Req::MoodHistory { days: 14 });
                    self.send(Req::HealthSignals { days: 14 });
                }
                3 => self.send(Req::Narrative),
                _ => self.send(Req::Reflections),
            },
            Tab::Graph => {
                self.send(Req::Graph(120));
                self.send(Req::Attention); // whole-graph type distribution
                self.send(Req::DataSources); // KG composition by data source
                if let Some(fc) = &self.graph_focus {
                    let id = fc.id.clone();
                    self.send(Req::Expand { id, k: 1 });
                }
            }
            Tab::Systems => match self.sys_sub {
                0 => {
                    self.send(Req::Systems);
                    self.send(Req::AgentRuns(40));
                }
                1 => self.send(Req::AgentRuns(60)),
                2 => {
                    self.send(Req::Scheduler);
                    self.send(Req::ScheduledJobs);
                }
                3 => self.send(Req::Events(80)),
                4 => {
                    if let Some(src) = self.log_sources.get(self.log_source_idx) {
                        let source = src.name.clone();
                        self.send(Req::Logs { source, limit: 300 });
                    } else {
                        self.send(Req::LogSources);
                    }
                }
                5 => self.send(Req::LlmObs),
                6 => {
                    self.send(Req::PipelineObs);
                    self.send(Req::RecallObs);
                }
                _ => self.send(Req::Settings),
            },
        }
    }

    pub fn apply(&mut self, resp: Resp) {
        self.inflight = self.inflight.saturating_sub(1);
        match resp {
            Resp::Home(h, s, c) => {
                self.health = Some(*h);
                self.stats = s;
                self.comp = c;
            }
            Resp::Memories(m) => {
                self.mem_sel.set_len(m.len());
                if self.mem_sel.selected().is_none() {
                    self.mem_sel.first();
                }
                self.memories = m;
            }
            Resp::MemoryDetail(d) => self.overlay = Overlay::Detail(Detail::Memory(d)),
            Resp::Episodes(e) => {
                self.epi_sel.set_len(e.len());
                if self.epi_sel.selected().is_none() {
                    self.epi_sel.first();
                }
                self.episodes = e;
            }
            Resp::EpisodeDetail(d) => self.overlay = Overlay::Detail(Detail::Episode(d)),
            Resp::Identity(v) => {
                self.id_sel.set_len(v.len());
                self.identity = v;
            }
            Resp::Principles(v) => {
                self.prin_sel.set_len(v.len());
                self.principles = v;
            }
            Resp::Narrative(v) => {
                self.chap_sel.set_len(v.len());
                self.chapters = v;
            }
            Resp::Attention(a) => self.attention = Some(*a),
            Resp::DataSources(d) => self.data_sources = d.sources,
            Resp::MoodHistory(m) => self.mood_history = Some(*m),
            Resp::HealthSignals(h) => self.health_signals = Some(*h),
            Resp::MentalModelContext(c) => self.mental_model_ctx = Some(c.context),
            Resp::AssumptionsContext(c) => self.assumptions_ctx = Some(c.context),
            Resp::Reflections(v) => {
                self.rep_sel.set_len(v.len());
                self.reports = v;
            }
            Resp::Graph(g) => {
                self.graph = *g;
                if self.graph_focus.is_none() {
                    let n = self.graph_root_nodes().len();
                    self.node_sel.set_len(n);
                }
            }
            Resp::Expand(e) => {
                self.graph_ego = Some(*e);
                let n = self.graph_neighbors().len();
                self.node_sel.set_len(n);
                self.node_sel.first();
            }
            Resp::Recall(v) => {
                self.recall_loading = false;
                self.recall_error = None;
                self.recall_sel.set_len(v.len());
                self.recall_sel.first();
                self.recall_hits = v;
            }
            Resp::Systems(s, latency_ms) => {
                let services = s.infrastructure.iter().map(|i| (i.name.clone(), i.ok)).collect();
                self.sys_history.push(SysSample { latency_ms, services });
                if self.sys_history.len() > SYS_HISTORY_CAP {
                    let drop = self.sys_history.len() - SYS_HISTORY_CAP;
                    self.sys_history.drain(0..drop);
                }
                self.systems = Some(*s);
            }
            Resp::Scheduler(s) => self.scheduler = Some(*s),
            Resp::LlmObs(v) => self.llm_obs = Some(*v),
            Resp::RecallObs(v) => self.recall_obs = Some(*v),
            Resp::PipelineObs(v) => self.pipeline_obs = Some(*v),
            Resp::Settings(v) => {
                self.set_sel.set_len(v.len());
                if self.set_sel.selected().is_none() {
                    self.set_sel.first();
                }
                self.settings = v;
            }
            Resp::Events(v) => {
                self.events = v;
                let n = self.filtered_events().len();
                self.event_sel.set_len(n);
                if self.event_sel.selected().is_none() {
                    self.event_sel.first();
                }
            }
            Resp::LogSources(v) => {
                self.log_source_idx = self.log_source_idx.min(v.len().saturating_sub(1));
                let need = !v.is_empty();
                self.log_sources = v;
                if need && self.tab == Tab::Systems && self.sys_sub == 4 {
                    if let Some(src) = self.log_sources.get(self.log_source_idx) {
                        let source = src.name.clone();
                        self.send(Req::Logs { source, limit: 300 });
                    }
                }
            }
            Resp::Logs(l) => self.logs = Some(*l),
            Resp::AgentRuns(v) => {
                self.agent_sel.set_len(v.len());
                if self.agent_sel.selected().is_none() {
                    self.agent_sel.first();
                }
                self.agent_runs = v;
            }
            Resp::AgentRun(d) => self.agent_detail = Some(*d),
            Resp::ScheduledJobs(v) => {
                self.sched_sel.set_len(v.len());
                if self.sched_sel.selected().is_none() {
                    self.sched_sel.first();
                }
                self.scheduled_jobs = v;
            }
            Resp::ActionOk { msg, refresh } => {
                self.status = Some((msg, false));
                if refresh {
                    self.refresh();
                }
            }
            Resp::Error(e) => {
                if self.recall_loading {
                    self.recall_loading = false;
                    self.recall_error = Some(e.clone());
                }
                self.status = Some((e, true));
            }
        }
    }

    // -----------------------------------------------------------------
    // key handling
    // -----------------------------------------------------------------

    pub fn on_key(&mut self, key: KeyEvent) {
        // Overlays consume keys first.
        match &mut self.overlay {
            Overlay::Help => {
                self.overlay = Overlay::None;
                return;
            }
            Overlay::Detail(_) => {
                if matches!(key.code, KeyCode::Esc | KeyCode::Char('q') | KeyCode::Enter) {
                    self.overlay = Overlay::None;
                }
                return;
            }
            Overlay::Confirm { .. } => {
                self.handle_confirm(key);
                return;
            }
            Overlay::Menu { .. } => {
                self.handle_menu(key);
                return;
            }
            Overlay::Form(_) => {
                self.handle_form(key);
                return;
            }
            Overlay::None => {}
        }

        // Inline editors capture all keys while focused.
        if self.recall_editing {
            self.handle_recall_input(key);
            return;
        }
        if self.graph_editing {
            self.handle_graph_input(key);
            return;
        }

        self.status = None;
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('c') if ctrl => self.should_quit = true,
            KeyCode::Char('?') => self.overlay = Overlay::Help,
            KeyCode::Char('r') => self.refresh(),
            KeyCode::Esc => {
                if self.tab == Tab::Systems && self.sys_sub == 1 {
                    self.agent_detail = None;
                } else if self.tab == Tab::Graph {
                    self.graph_back();
                }
            }
            KeyCode::Backspace if self.tab == Tab::Graph => self.graph_back(),
            KeyCode::Char('A') => self.open_form(FormKind::Capture),
            KeyCode::Tab => self.switch_tab((self.tab.index() + 1) % TABS.len()),
            KeyCode::BackTab => {
                self.switch_tab((self.tab.index() + TABS.len() - 1) % TABS.len())
            }
            KeyCode::Char(c @ '1'..='5') => {
                self.switch_tab(c as usize - '1' as usize);
            }
            KeyCode::Char('l') | KeyCode::Right => self.cycle_sub(1),
            KeyCode::Char('h') | KeyCode::Left => self.cycle_sub(-1),
            KeyCode::Down | KeyCode::Char('j') => self.cur_sel().next(),
            KeyCode::Up | KeyCode::Char('k') => self.cur_sel().prev(),
            KeyCode::Char('g') => self.cur_sel().first(),
            KeyCode::Char('G') => self.cur_sel().last(),
            KeyCode::Enter => self.on_enter(),
            _ => self.on_tab_key(key),
        }
    }

    fn switch_tab(&mut self, i: usize) {
        self.tab = Tab::from_index(i);
        self.refresh();
    }

    /// Move between sub-views of the current space.
    fn cycle_sub(&mut self, dir: i32) {
        let n = self.tab.sub_labels().len() as i32;
        if n == 0 {
            return;
        }
        let next = (self.cur_sub() as i32 + dir).rem_euclid(n) as usize;
        self.set_sub(next);
        self.refresh();
    }

    /// Fire a recall request for the current query + mode, entering the
    /// loading state. No-op (clears results) on an empty query.
    fn submit_recall(&mut self) {
        self.recall_editing = false;
        let q = self.recall_query.trim().to_string();
        if q.is_empty() {
            self.recall_submitted = false;
            self.recall_loading = false;
            self.recall_error = None;
            self.recall_hits.clear();
            self.recall_sel.set_len(0);
            return;
        }
        self.recall_submitted = true;
        self.recall_loading = true;
        self.recall_error = None;
        let m = self.recall_mode.clone();
        self.send(Req::Recall { query: q, mode: m, k: 20 });
    }

    /// Inline query editor (active only on Memory · Recall).
    fn handle_recall_input(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true
            }
            KeyCode::Esc => self.recall_editing = false,
            KeyCode::Enter => self.submit_recall(),
            KeyCode::Backspace => {
                self.recall_query.pop();
            }
            KeyCode::Char(c) => self.recall_query.push(c),
            _ => {}
        }
    }

    /// The list selection relevant to the current view.
    fn cur_sel(&mut self) -> &mut Sel {
        match self.tab {
            Tab::Memory => match self.mem_sub {
                0 => &mut self.mem_sel,
                1 => &mut self.epi_sel,
                _ => &mut self.recall_sel,
            },
            Tab::Mind => match self.mind_sub {
                1 => &mut self.id_sel,   // Self → identity list
                3 => &mut self.chap_sel, // Story → chapters
                4 => &mut self.rep_sel,  // Insights → reports
                _ => &mut self.node_sel, // Overview / Focus have no list
            },
            Tab::Graph => &mut self.node_sel,
            Tab::Systems => match self.sys_sub {
                1 => &mut self.agent_sel,
                2 => &mut self.sched_sel,
                3 => &mut self.event_sel,
                7 => &mut self.set_sel,
                _ => &mut self.node_sel, // overview/logs/routing/pipeline have no list
            },
            Tab::Home => &mut self.node_sel,
        }
    }

    fn on_enter(&mut self) {
        match (self.tab, self.cur_sub()) {
            (Tab::Memory, 0) => {
                if let Some(m) = self.mem_sel.selected().and_then(|i| self.memories.get(i)) {
                    let id = m.id.clone();
                    self.send(Req::MemoryDetail(id));
                }
            }
            (Tab::Memory, 1) => {
                if let Some(e) = self.epi_sel.selected().and_then(|i| self.episodes.get(i)) {
                    let id = e.id.clone();
                    self.send(Req::EpisodeDetail(id));
                }
            }
            (Tab::Memory, 2) => {
                if let Some(h) = self.recall_sel.selected().and_then(|i| self.recall_hits.get(i)) {
                    if h.id.starts_with("mem_") {
                        let id = h.id.clone();
                        self.send(Req::MemoryDetail(id));
                    }
                }
            }
            (Tab::Systems, 1) => {
                if let Some(r) = self.agent_sel.selected().and_then(|i| self.agent_runs.get(i)) {
                    let id = r.id.clone();
                    self.send(Req::AgentRun(id));
                }
            }
            (Tab::Systems, 7) => self.activate_setting(),
            (Tab::Graph, _) => self.graph_drill(),
            _ => {}
        }
    }

    /// Per-view feature keys that don't collide with the global set.
    fn on_tab_key(&mut self, key: KeyEvent) {
        match (self.tab, self.cur_sub()) {
            // Memory · Browse
            (Tab::Memory, 0) => match key.code {
                KeyCode::Char('/') => self.open_form(FormKind::Search),
                KeyCode::Char('v') => {
                    self.mq.valid = match self.mq.valid {
                        Some(true) => Some(false),
                        Some(false) => None,
                        None => Some(true),
                    };
                    self.mq.offset = 0;
                    self.refresh();
                }
                KeyCode::Char('s') => {
                    self.mq.epistemic_status = match self.mq.epistemic_status.as_deref() {
                        None => Some("canonical".into()),
                        Some("canonical") => Some("belief".into()),
                        Some("belief") => Some("hypothesis".into()),
                        _ => None,
                    };
                    self.mq.offset = 0;
                    self.refresh();
                }
                KeyCode::Char('n') => {
                    self.mq.offset += self.mq.limit;
                    self.refresh();
                }
                KeyCode::Char('p') => {
                    self.mq.offset = self.mq.offset.saturating_sub(self.mq.limit);
                    self.refresh();
                }
                KeyCode::Char('i') => {
                    if let Some(m) = self.mem_sel.selected().and_then(|i| self.memories.get(i)) {
                        self.overlay = Overlay::Confirm {
                            msg: format!("Invalidate this memory?\n\n{}", truncate(&m.statement, 160)),
                            action: Pending::Invalidate(m.id.clone()),
                        };
                    }
                }
                _ => {}
            },
            // Memory · Episodes
            (Tab::Memory, 1) => match key.code {
                KeyCode::Char('n') => {
                    self.epi_offset += 50;
                    self.refresh();
                }
                KeyCode::Char('p') => {
                    self.epi_offset = self.epi_offset.saturating_sub(50);
                    self.refresh();
                }
                _ => {}
            },
            // Memory · Recall
            (Tab::Memory, 2) => match key.code {
                KeyCode::Char('/') | KeyCode::Char('i') => self.recall_editing = true,
                KeyCode::Char('m') => {
                    self.recall_mode = match self.recall_mode.as_str() {
                        "fast" => "deep".into(),
                        "deep" => "divergent".into(),
                        _ => "fast".into(),
                    };
                    // re-run with the new mode if a query is present
                    if self.recall_submitted && !self.recall_query.trim().is_empty() {
                        self.submit_recall();
                    }
                }
                _ => {}
            },
            // Graph explorer — search/filter the current list
            (Tab::Graph, _) => {
                if key.code == KeyCode::Char('/') {
                    self.graph_editing = true;
                }
            }
            // Mind (cognition triggers available throughout; propose on Overview/Self)
            (Tab::Mind, sub) => match key.code {
                KeyCode::Char('a') if sub == 0 || sub == 1 => self.open_form(FormKind::Identity),
                KeyCode::Char('t') => self.open_cognition_menu(),
                _ => {}
            },
            // Systems · Agents — cancel a running run
            (Tab::Systems, 1) => {
                if key.code == KeyCode::Char('c') {
                    if let Some(r) = self.agent_sel.selected().and_then(|i| self.agent_runs.get(i)) {
                        if r.status.as_deref() == Some("running") {
                            self.overlay = Overlay::Confirm {
                                msg: format!("Cancel this running agent?\n\n{}", truncate(&r.task, 140)),
                                action: Pending::CancelRun { id: r.id.clone() },
                            };
                        } else {
                            self.status = Some(("only running agents can be cancelled".into(), true));
                        }
                    }
                }
            }
            // Systems · Scheduler — manage your jobs
            (Tab::Systems, 2) => match key.code {
                KeyCode::Char('x') => {
                    if let Some(j) = self.sched_sel.selected().and_then(|i| self.scheduled_jobs.get(i)) {
                        self.overlay = Overlay::Confirm {
                            msg: format!("Run job now?\n\n{}", j.name),
                            action: Pending::RunJob { id: j.id.clone(), name: j.name.clone() },
                        };
                    }
                }
                KeyCode::Char('e') => {
                    if let Some(j) = self.sched_sel.selected().and_then(|i| self.scheduled_jobs.get(i)) {
                        let (id, now) = (j.id.clone(), j.enabled);
                        let label = format!("Job '{}' {}", j.name, if now { "disabled" } else { "enabled" });
                        self.send(Req::UpdateJob {
                            id,
                            label,
                            body: serde_json::json!({ "enabled": !now }),
                        });
                    }
                }
                KeyCode::Char('d') => {
                    if let Some(j) = self.sched_sel.selected().and_then(|i| self.scheduled_jobs.get(i)) {
                        self.overlay = Overlay::Confirm {
                            msg: format!("Delete this scheduled job?\n\n{}", j.name),
                            action: Pending::DeleteJob { id: j.id.clone(), name: j.name.clone() },
                        };
                    }
                }
                KeyCode::Char('n') => self.open_form(FormKind::NewJob),
                _ => {}
            },
            // Systems · Events — cycle category filter
            (Tab::Systems, 3) => {
                if key.code == KeyCode::Char('c') {
                    self.event_cat = (self.event_cat + 1) % EVENT_CATS.len();
                    let n = self.filtered_events().len();
                    self.event_sel.set_len(n);
                    self.event_sel.first();
                }
            }
            // Systems · Logs — cycle source
            (Tab::Systems, 4) => {
                if key.code == KeyCode::Char('s') && !self.log_sources.is_empty() {
                    self.log_source_idx = (self.log_source_idx + 1) % self.log_sources.len();
                    self.refresh();
                }
            }
            _ => {}
        }
    }

    /// Systems · Settings: Enter toggles a bool setting in place; for int/float/str
    /// it opens a small edit form. The server validates/coerces the new value.
    fn activate_setting(&mut self) {
        let Some(i) = self.set_sel.selected() else { return };
        let Some((key, item)) = self.settings.get(i) else { return };
        let (key, item) = (key.clone(), item.clone());
        if item.ty.as_deref() == Some("bool") {
            let cur = item.value.as_bool().unwrap_or(false);
            self.send(Req::PutSetting {
                key: key.clone(),
                value: serde_json::Value::Bool(!cur),
                label: format!("{key} → {}", !cur),
            });
        } else {
            self.open_setting_edit(key, item);
        }
    }

    fn open_setting_edit(&mut self, key: String, item: SettingItem) {
        let cur = match &item.value {
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        let ty = item.ty.clone().unwrap_or_default();
        self.overlay = Overlay::Form(Form {
            kind: FormKind::EditSetting { key },
            title: format!("Edit setting ({ty})"),
            fields: vec![FormField { label: format!("value [{ty}]"), value: cur }],
            active: 0,
        });
    }

    fn open_cognition_menu(&mut self) {
        self.overlay = Overlay::Menu {
            title: "Trigger cognition job".into(),
            items: vec![
                (
                    "Daily reflection".into(),
                    MenuAction::Trigger {
                        path: "/api/reflection/daily".into(),
                        label: "Daily reflection".into(),
                    },
                ),
                (
                    "Weekly reflection".into(),
                    MenuAction::Trigger {
                        path: "/api/reflection/weekly".into(),
                        label: "Weekly reflection".into(),
                    },
                ),
                (
                    "Monthly reflection".into(),
                    MenuAction::Trigger {
                        path: "/api/reflection/monthly".into(),
                        label: "Monthly reflection".into(),
                    },
                ),
                (
                    "Run consolidation".into(),
                    MenuAction::Trigger {
                        path: "/api/consolidation/run".into(),
                        label: "Consolidation".into(),
                    },
                ),
                (
                    "Generate narrative".into(),
                    MenuAction::Trigger {
                        path: "/api/narrative/generate".into(),
                        label: "Narrative generate".into(),
                    },
                ),
                (
                    "Attention scan".into(),
                    MenuAction::Trigger {
                        path: "/api/attention/scan".into(),
                        label: "Attention scan".into(),
                    },
                ),
                (
                    "Infer mood from episode".into(),
                    MenuAction::Trigger {
                        path: "/api/attention/mood/infer".into(),
                        label: "Mood infer".into(),
                    },
                ),
                (
                    "Derive goals from memories".into(),
                    MenuAction::Trigger {
                        path: "/api/goals/derive".into(),
                        label: "Goal derive".into(),
                    },
                ),
                (
                    "Extract decisions from episodes".into(),
                    MenuAction::Trigger {
                        path: "/api/decisions/extract".into(),
                        label: "Extract decisions".into(),
                    },
                ),
                ("Log mood…".into(), MenuAction::OpenLogMoodForm),
                ("Compose narrative (query)…".into(), MenuAction::OpenComposeForm),
            ],
            sel: 0,
        };
    }

    fn open_form(&mut self, kind: FormKind) {
        let form = match kind {
            FormKind::Search => Form {
                kind,
                title: "Search memories (full-text)".into(),
                fields: vec![FormField {
                    label: "query".into(),
                    value: self.mq.q.clone().unwrap_or_default(),
                }],
                active: 0,
            },
            FormKind::Capture => Form {
                kind,
                title: "Capture episode (ingest)".into(),
                fields: vec![FormField { label: "content".into(), value: String::new() }],
                active: 0,
            },
            FormKind::Compose => Form {
                kind,
                title: "Compose narrative".into(),
                fields: vec![FormField { label: "query".into(), value: String::new() }],
                active: 0,
            },
            FormKind::Identity => Form {
                kind,
                title: "Propose identity fact".into(),
                fields: vec![
                    FormField { label: "predicate".into(), value: String::new() },
                    FormField { label: "object".into(), value: String::new() },
                    FormField { label: "confidence (0-1)".into(), value: "0.6".into() },
                ],
                active: 0,
            },
            FormKind::NewJob => Form {
                kind,
                title: "New scheduled job".into(),
                fields: vec![
                    FormField { label: "name".into(), value: String::new() },
                    FormField { label: "task (what the agent should do)".into(), value: String::new() },
                    FormField {
                        label: "cadence — e.g. 'daily 09:00' · 'every 60' · 'weekly 0,2 18:00' · 'monthly 1 09:00'".into(),
                        value: "daily 09:00".into(),
                    },
                ],
                active: 0,
            },
            FormKind::LogMood => Form {
                kind,
                title: "Log mood".into(),
                fields: vec![
                    FormField { label: "mood (focused|tired|anxious|energetic|calm|sad|…)".into(), value: String::new() },
                    FormField { label: "valence (-1.0 to 1.0)".into(), value: "0.0".into() },
                    FormField { label: "energy (0.0 to 1.0)".into(), value: "0.5".into() },
                ],
                active: 0,
            },
            // Built via open_setting_edit (it needs the current value); never here.
            FormKind::EditSetting { .. } => return,
        };
        self.overlay = Overlay::Form(form);
    }

    fn handle_confirm(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                if let Overlay::Confirm { action, .. } =
                    std::mem::replace(&mut self.overlay, Overlay::None)
                {
                    match action {
                        Pending::Invalidate(id) => self.send(Req::Invalidate(id)),
                        Pending::RunJob { id, name } => self.send(Req::Trigger {
                            path: format!("/api/scheduled-jobs/{id}/run-now"),
                            label: format!("Run '{name}'"),
                            body: serde_json::json!({}),
                        }),
                        Pending::DeleteJob { id, name } => self.send(Req::DeleteJob { id, name }),
                        Pending::CancelRun { id } => self.send(Req::CancelRun { id }),
                    }
                }
            }
            _ => self.overlay = Overlay::None,
        }
    }

    fn handle_menu(&mut self, key: KeyEvent) {
        if let Overlay::Menu { items, sel, .. } = &mut self.overlay {
            match key.code {
                KeyCode::Down | KeyCode::Char('j') => *sel = (*sel + 1).min(items.len() - 1),
                KeyCode::Up | KeyCode::Char('k') => *sel = sel.saturating_sub(1),
                KeyCode::Esc => self.overlay = Overlay::None,
                KeyCode::Enter => {
                    let action = items[*sel].1.clone();
                    self.overlay = Overlay::None;
                    match action {
                        MenuAction::Trigger { path, label } => {
                            self.send(Req::Trigger { path, label, body: serde_json::json!({}) })
                        }
                        MenuAction::OpenComposeForm => self.open_form(FormKind::Compose),
                        MenuAction::OpenLogMoodForm => self.open_form(FormKind::LogMood),
                    }
                }
                _ => {}
            }
        }
    }

    fn handle_form(&mut self, key: KeyEvent) {
        let submit = matches!(key.code, KeyCode::Enter)
            && !matches!(
                self.overlay,
                Overlay::Form(Form { kind: FormKind::Capture, .. })
            );
        // Capture is multi-line-ish: Enter inserts newline, Ctrl-S submits.
        let capture_submit = matches!(self.overlay, Overlay::Form(Form { kind: FormKind::Capture, .. }))
            && key.code == KeyCode::Char('s')
            && key.modifiers.contains(KeyModifiers::CONTROL);

        match key.code {
            KeyCode::Esc => {
                self.overlay = Overlay::None;
                return;
            }
            KeyCode::Tab => {
                if let Overlay::Form(f) = &mut self.overlay {
                    f.active = (f.active + 1) % f.fields.len();
                }
                return;
            }
            KeyCode::BackTab => {
                if let Overlay::Form(f) = &mut self.overlay {
                    f.active = (f.active + f.fields.len() - 1) % f.fields.len();
                }
                return;
            }
            KeyCode::Backspace => {
                if let Overlay::Form(f) = &mut self.overlay {
                    f.fields[f.active].value.pop();
                }
                return;
            }
            KeyCode::Char(c) => {
                if !(submit || capture_submit) {
                    if let Overlay::Form(f) = &mut self.overlay {
                        f.fields[f.active].value.push(c);
                    }
                    return;
                }
            }
            KeyCode::Enter => {
                if let Overlay::Form(f) = &mut self.overlay {
                    if matches!(f.kind, FormKind::Capture) {
                        f.fields[0].value.push('\n');
                        return;
                    }
                }
            }
            _ => return,
        }

        if submit || capture_submit {
            self.submit_form();
        }
    }

    fn submit_form(&mut self) {
        let form = match std::mem::replace(&mut self.overlay, Overlay::None) {
            Overlay::Form(f) => f,
            other => {
                self.overlay = other;
                return;
            }
        };
        match form.kind {
            FormKind::Search => {
                let q = form.fields[0].value.trim().to_string();
                self.mq.q = if q.is_empty() { None } else { Some(q) };
                self.mq.offset = 0;
                self.refresh();
            }
            FormKind::Capture => {
                let c = form.fields[0].value.trim().to_string();
                if !c.is_empty() {
                    self.send(Req::Ingest(c));
                }
            }
            FormKind::Compose => {
                let q = form.fields[0].value.trim().to_string();
                if !q.is_empty() {
                    self.send(Req::Trigger {
                        path: "/api/cognition/narrative/compose".into(),
                        label: "Narrative compose".into(),
                        body: serde_json::json!({ "query": q }),
                    });
                }
            }
            FormKind::Identity => {
                let predicate = form.fields[0].value.trim().to_string();
                let object = form.fields[1].value.trim().to_string();
                let confidence = form.fields[2].value.trim().parse::<f64>().unwrap_or(0.6);
                if predicate.is_empty() || object.is_empty() {
                    self.status = Some(("predicate and object are required".into(), true));
                } else {
                    self.send(Req::ProposeIdentity { predicate, object, confidence });
                }
            }
            FormKind::LogMood => {
                let mood = form.fields[0].value.trim().to_string();
                let valence = form.fields[1].value.trim().parse::<f64>().unwrap_or(0.0);
                let energy = form.fields[2].value.trim().parse::<f64>().unwrap_or(0.5);
                if mood.is_empty() {
                    self.status = Some(("mood is required".into(), true));
                } else {
                    self.send(Req::LogMood { mood, valence, energy });
                }
            }
            FormKind::NewJob => {
                let name = form.fields[0].value.trim().to_string();
                let task = form.fields[1].value.trim().to_string();
                let cadence = form.fields[2].value.trim();
                if name.is_empty() || task.is_empty() {
                    self.status = Some(("name and task are required".into(), true));
                } else if let Some((cadence_type, cadence_json)) = parse_cadence_input(cadence) {
                    self.send(Req::CreateJob { name, task, cadence_type, cadence_json });
                } else {
                    self.status = Some((
                        "bad cadence — try 'daily 09:00', 'every 60', 'weekly 0,2 18:00', 'monthly 1 09:00'".into(),
                        true,
                    ));
                }
            }
            FormKind::EditSetting { key } => {
                let v = form.fields[0].value.trim().to_string();
                if v.is_empty() {
                    self.status = Some(("value is required".into(), true));
                } else {
                    // Send as a string; the server coerces it to the registry type.
                    self.send(Req::PutSetting {
                        key: key.clone(),
                        value: serde_json::Value::String(v.clone()),
                        label: format!("{key} → {v}"),
                    });
                }
            }
        }
    }
}

/// Parse a compact cadence string into (cadence_type, cadence_json) for the API.
/// Forms: `every <minutes>` · `daily HH:MM` · `weekly <d,d> HH:MM` (0=Mon..6=Sun)
/// · `monthly <day> HH:MM`.
fn parse_cadence_input(s: &str) -> Option<(String, serde_json::Value)> {
    let toks: Vec<&str> = s.split_whitespace().collect();
    let hhmm_ok = |t: &str| -> Option<String> {
        let (h, m) = t.split_once(':')?;
        let (h, m) = (h.parse::<u32>().ok()?, m.parse::<u32>().ok()?);
        (h <= 23 && m <= 59).then(|| format!("{h:02}:{m:02}"))
    };
    match toks.as_slice() {
        ["every", n] => {
            let m = n.parse::<i64>().ok()?;
            Some(("every".into(), serde_json::json!({ "minutes": m })))
        }
        ["daily", t] => Some(("daily_at".into(), serde_json::json!({ "hhmm": hhmm_ok(t)? }))),
        ["weekly", days, t] => {
            let wd: Vec<i64> = days.split(',').filter_map(|d| d.trim().parse::<i64>().ok()).filter(|d| (0..=6).contains(d)).collect();
            if wd.is_empty() {
                return None;
            }
            Some(("weekly_at".into(), serde_json::json!({ "weekdays": wd, "hhmm": hhmm_ok(t)? })))
        }
        ["monthly", day, t] => {
            let d = day.parse::<i64>().ok().filter(|d| (1..=28).contains(d))?;
            Some(("monthly_at".into(), serde_json::json!({ "day": d, "hhmm": hhmm_ok(t)? })))
        }
        _ => None,
    }
}

pub fn truncate(s: &str, n: usize) -> String {
    let s = s.replace('\n', " ");
    if s.chars().count() <= n {
        s
    } else {
        let t: String = s.chars().take(n).collect();
        format!("{t}…")
    }
}
