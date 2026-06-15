//! Blocking HTTP client + typed models for the curlyos-core API.
//! Embedding vectors returned by the API are intentionally omitted from every
//! struct so they are never deserialized into memory or rendered.
//!
//! Some struct fields mirror the API payload for completeness even when the UI
//! does not render them yet.
#![allow(dead_code)]

use anyhow::{Context, Result};
use serde::Deserialize;
use std::time::Duration;

pub struct Client {
    base: String,
    http: reqwest::blocking::Client,
}

impl Client {
    pub fn new(base: impl Into<String>) -> Self {
        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("build http client");
        Self { base: base.into(), http }
    }

    fn get<T: for<'de> Deserialize<'de>>(&self, path: &str) -> Result<T> {
        let url = format!("{}{}", self.base, path);
        let resp = self.http.get(&url).send().with_context(|| format!("GET {url}"))?;
        let status = resp.status();
        let body = resp.text().unwrap_or_default();
        if !status.is_success() {
            anyhow::bail!("GET {url} -> {status}: {}", truncate(&body, 200));
        }
        serde_json::from_str(&body).with_context(|| format!("decode GET {url}"))
    }

    fn post<T: for<'de> Deserialize<'de>>(&self, path: &str, body: serde_json::Value) -> Result<T> {
        let url = format!("{}{}", self.base, path);
        let resp = self
            .http
            .post(&url)
            .json(&body)
            .send()
            .with_context(|| format!("POST {url}"))?;
        let status = resp.status();
        let text = resp.text().unwrap_or_default();
        if !status.is_success() {
            anyhow::bail!("POST {url} -> {status}: {}", truncate(&text, 200));
        }
        serde_json::from_str(&text).with_context(|| format!("decode POST {url}"))
    }

    fn send_empty(&self, method: reqwest::Method, path: &str, body: serde_json::Value) -> Result<()> {
        let url = format!("{}{}", self.base, path);
        let resp = self
            .http
            .request(method.clone(), &url)
            .json(&body)
            .send()
            .with_context(|| format!("{method} {url}"))?;
        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().unwrap_or_default();
            anyhow::bail!("{method} {url} -> {status}: {}", truncate(&text, 200));
        }
        Ok(())
    }

    // ---- reads -----------------------------------------------------------

    pub fn health(&self) -> Result<Health> {
        self.get("/api/health")
    }
    pub fn stats(&self) -> Result<Stats> {
        self.get("/api/stats")
    }
    pub fn composition(&self) -> Result<Composition> {
        self.get("/api/stats/composition")
    }

    pub fn memories(&self, p: &MemoryQuery) -> Result<Vec<Memory>> {
        let mut q = format!("/api/memories?limit={}&offset={}", p.limit, p.offset);
        match p.valid {
            Some(true) => q.push_str("&valid=true"),
            Some(false) => q.push_str("&valid=false"),
            None => q.push_str("&valid=none"),
        }
        if let Some(k) = &p.kind {
            q.push_str(&format!("&kind={}", urlencode(k)));
        }
        if let Some(s) = &p.epistemic_status {
            q.push_str(&format!("&epistemic_status={}", urlencode(s)));
        }
        if let Some(text) = &p.q {
            if !text.is_empty() {
                q.push_str(&format!("&q={}", urlencode(text)));
            }
        }
        let r: Items<Memory> = self.get(&q)?;
        Ok(r.items)
    }

    pub fn memory_detail(&self, id: &str) -> Result<MemoryDetail> {
        self.get(&format!("/api/memories/{id}"))
    }

    pub fn episodes(&self, limit: usize, offset: usize) -> Result<Vec<Episode>> {
        let r: Items<Episode> = self.get(&format!("/api/episodes?limit={limit}&offset={offset}"))?;
        Ok(r.items)
    }

    pub fn episode_detail(&self, id: &str) -> Result<EpisodeDetail> {
        self.get(&format!("/api/episodes/{id}"))
    }

    pub fn identity(&self) -> Result<Vec<IdentityFact>> {
        let r: Items<IdentityFact> = self.get("/api/identity?valid=true")?;
        Ok(r.items)
    }

    pub fn principles(&self) -> Result<Vec<Principle>> {
        let r: Meta = self.get("/api/cognition/meta")?;
        Ok(r.principles)
    }
    pub fn narrative(&self) -> Result<Vec<Chapter>> {
        let r: Narrative = self.get("/api/cognition/narrative")?;
        Ok(r.chapters)
    }
    pub fn attention(&self) -> Result<Attention> {
        self.get("/api/cognition/attention")
    }
    pub fn reflections(&self) -> Result<Vec<Report>> {
        let r: Reflections = self.get("/api/cognition/reflection")?;
        Ok(r.reports)
    }

    pub fn graph(&self, limit: usize) -> Result<Graph> {
        self.get(&format!("/api/graph?limit={limit}"))
    }

    pub fn recall(&self, query: &str, mode: &str, k: usize) -> Result<Vec<RecallHit>> {
        let r: Recall = self.post(
            "/api/recall",
            serde_json::json!({ "query": query, "mode": mode, "k": k }),
        )?;
        Ok(r.results)
    }

    // ---- observability ---------------------------------------------------

    pub fn systems(&self) -> Result<Systems> {
        self.get("/api/systems")
    }
    pub fn scheduler(&self) -> Result<Scheduler> {
        self.get("/api/scheduler")
    }
    pub fn events(&self, limit: usize) -> Result<Vec<Event>> {
        let r: Items<Event> = self.get(&format!("/api/events?limit={limit}"))?;
        Ok(r.items)
    }
    pub fn log_sources(&self) -> Result<Vec<LogSource>> {
        let r: LogSources = self.get("/api/logs/sources")?;
        Ok(r.sources)
    }
    pub fn logs(&self, source: &str, limit: usize) -> Result<Logs> {
        self.get(&format!("/api/logs?source={}&limit={limit}", urlencode(source)))
    }
    pub fn agent_runs(&self, limit: usize) -> Result<Vec<AgentRun>> {
        let r: Items<AgentRun> = self.get(&format!("/api/agents/runs?limit={limit}"))?;
        Ok(r.items)
    }
    pub fn agent_run(&self, id: &str) -> Result<AgentRunDetail> {
        self.get(&format!("/api/agents/runs/{id}"))
    }
    pub fn scheduled_jobs(&self) -> Result<Vec<ScheduledJob>> {
        let r: Items<ScheduledJob> = self.get("/api/scheduled-jobs")?;
        Ok(r.items)
    }

    // ---- safe writes -----------------------------------------------------

    pub fn invalidate_memory(&self, id: &str) -> Result<()> {
        let _: serde_json::Value =
            self.post(&format!("/api/memories/{id}/invalidate"), serde_json::json!({}))?;
        Ok(())
    }

    pub fn propose_identity(&self, predicate: &str, object: &str, confidence: f64) -> Result<()> {
        let _: serde_json::Value = self.post(
            "/api/identity",
            serde_json::json!({
                "predicate": predicate, "object": object, "confidence": confidence
            }),
        )?;
        Ok(())
    }

    pub fn ingest(&self, content: &str) -> Result<()> {
        let _: serde_json::Value = self.post(
            "/api/ingest",
            serde_json::json!({ "content": content, "source_ref": "curlyos-tui:capture" }),
        )?;
        Ok(())
    }

    pub fn trigger(&self, path: &str, body: serde_json::Value) -> Result<()> {
        let _: serde_json::Value = self.post(path, body)?;
        Ok(())
    }

    // ---- scheduled-job management ----------------------------------------

    pub fn create_scheduled_job(
        &self,
        name: &str,
        task: &str,
        cadence_type: &str,
        cadence_json: serde_json::Value,
    ) -> Result<()> {
        self.send_empty(
            reqwest::Method::POST,
            "/api/scheduled-jobs",
            serde_json::json!({
                "name": name, "task": task,
                "cadence_type": cadence_type, "cadence_json": cadence_json,
                "enabled": true,
            }),
        )
    }

    pub fn update_scheduled_job(&self, id: &str, body: serde_json::Value) -> Result<()> {
        self.send_empty(reqwest::Method::PATCH, &format!("/api/scheduled-jobs/{id}"), body)
    }

    pub fn delete_scheduled_job(&self, id: &str) -> Result<()> {
        self.send_empty(reqwest::Method::DELETE, &format!("/api/scheduled-jobs/{id}"), serde_json::json!({}))
    }

    pub fn cancel_agent_run(&self, id: &str) -> Result<()> {
        self.send_empty(reqwest::Method::POST, &format!("/api/agents/runs/{id}/cancel"), serde_json::json!({}))
    }
}

// ---------------------------------------------------------------------------
// Query params
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct MemoryQuery {
    pub limit: usize,
    pub offset: usize,
    pub valid: Option<bool>,
    pub kind: Option<String>,
    pub epistemic_status: Option<String>,
    pub q: Option<String>,
}

impl Default for MemoryQuery {
    fn default() -> Self {
        Self {
            limit: 50,
            offset: 0,
            valid: Some(true),
            kind: None,
            epistemic_status: None,
            q: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Models
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct Items<T> {
    items: Vec<T>,
}

#[derive(Deserialize)]
pub struct Health {
    pub postgres: Service,
    pub redis: Service,
    pub embedder: Service,
}

#[derive(Deserialize)]
pub struct Service {
    pub ok: bool,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
}

#[derive(Deserialize, Default)]
pub struct Stats {
    #[serde(default)]
    pub episodes: i64,
    #[serde(default)]
    pub memories: i64,
    #[serde(default)]
    pub identity_facts: i64,
    #[serde(default)]
    pub knowledge_entities: i64,
    #[serde(default)]
    pub knowledge_edges: i64,
}

#[derive(Deserialize, Default)]
pub struct Composition {
    #[serde(default)]
    pub memories_by_status: std::collections::BTreeMap<String, i64>,
    #[serde(default)]
    pub memories_by_tier: std::collections::BTreeMap<String, i64>,
    #[serde(default)]
    pub identity_by_status: std::collections::BTreeMap<String, i64>,
    #[serde(default)]
    pub memories_changed_7d: i64,
}

#[derive(Deserialize, Clone)]
pub struct Memory {
    pub id: String,
    pub statement: String,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub tier: Option<String>,
    #[serde(default)]
    pub epistemic_status: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub valid_to: Option<String>,
    #[serde(default)]
    pub source_episode_id: Option<String>,
}

#[derive(Deserialize)]
pub struct MemoryDetail {
    pub memory: Memory,
    #[serde(default)]
    pub source_episode: Option<Episode>,
}

#[derive(Deserialize, Clone)]
pub struct Episode {
    pub id: String,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub source_ref: Option<String>,
    #[serde(default)]
    pub modality: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
}

#[derive(Deserialize)]
pub struct EpisodeDetail {
    pub episode: Episode,
    #[serde(default)]
    pub memories: Vec<Memory>,
}

#[derive(Deserialize, Clone)]
pub struct IdentityFact {
    pub id: String,
    pub predicate: String,
    pub object: String,
    #[serde(default)]
    pub confidence: f64,
    #[serde(default)]
    pub epistemic_status: Option<String>,
    #[serde(default)]
    pub valid_from: Option<String>,
}

#[derive(Deserialize)]
struct Meta {
    #[serde(default)]
    principles: Vec<Principle>,
}

#[derive(Deserialize, Clone)]
pub struct Principle {
    pub statement: String,
    #[serde(default)]
    pub domain: Option<String>,
    #[serde(default)]
    pub epistemic_status: Option<String>,
}

#[derive(Deserialize)]
struct Narrative {
    #[serde(default)]
    chapters: Vec<Chapter>,
}

#[derive(Deserialize, Clone)]
pub struct Chapter {
    pub title: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub start_date: Option<String>,
    #[serde(default)]
    pub end_date: Option<String>,
    #[serde(default)]
    pub epistemic_status: Option<String>,
}

#[derive(Deserialize)]
pub struct Attention {
    #[serde(default)]
    pub focus_areas: Vec<FocusArea>,
    #[serde(default)]
    pub neglected: Vec<FocusArea>,
    #[serde(default)]
    pub alignment_gaps: Vec<serde_json::Value>,
    pub breadth: Breadth,
    pub cognitive_load: CognitiveLoad,
}

#[derive(Deserialize, Clone)]
pub struct FocusArea {
    pub name: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub weight: i64,
}

#[derive(Deserialize)]
pub struct Breadth {
    #[serde(default)]
    pub total_entities: i64,
    #[serde(default)]
    pub distinct_types: i64,
    #[serde(default)]
    pub concentration: f64,
    #[serde(default)]
    pub by_type: std::collections::BTreeMap<String, i64>,
}

#[derive(Deserialize)]
pub struct CognitiveLoad {
    #[serde(default)]
    pub score: f64,
    pub breakdown: LoadBreakdown,
}

#[derive(Deserialize, Default)]
pub struct LoadBreakdown {
    #[serde(default)]
    pub density: f64,
    #[serde(default)]
    pub topic_switching: f64,
    #[serde(default)]
    pub episode_count: i64,
    #[serde(default)]
    pub window_days: i64,
}

#[derive(Deserialize)]
struct Reflections {
    #[serde(default)]
    reports: Vec<Report>,
}

#[derive(Deserialize, Clone)]
pub struct Report {
    #[serde(default)]
    pub report_type: Option<String>,
    #[serde(default)]
    pub time_window_start: Option<String>,
    #[serde(default)]
    pub time_window_end: Option<String>,
    #[serde(default)]
    pub episodes_scanned: i64,
    #[serde(default)]
    pub findings: Vec<Finding>,
}

#[derive(Deserialize, Clone)]
pub struct Finding {
    pub statement: String,
    #[serde(default)]
    pub confidence: f64,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Deserialize)]
pub struct Graph {
    #[serde(default)]
    pub nodes: Vec<Node>,
    #[serde(default)]
    pub links: Vec<Link>,
}

#[derive(Deserialize, Clone)]
pub struct Node {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub degree: i64,
}

#[derive(Deserialize, Clone)]
pub struct Link {
    pub source: String,
    pub target: String,
    #[serde(default)]
    pub rel_type: Option<String>,
}

#[derive(Deserialize)]
struct Recall {
    #[serde(default)]
    results: Vec<RecallHit>,
}

#[derive(Deserialize, Clone)]
pub struct RecallHit {
    pub id: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub score: f64,
    #[serde(default)]
    pub tier: Option<String>,
    #[serde(default)]
    pub epistemic_status: Option<String>,
}

// ---------------------------------------------------------------------------
// Observability models
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct Systems {
    #[serde(default)]
    pub infrastructure: Vec<Infra>,
    #[serde(default)]
    pub engines: Vec<Engine>,
    pub scheduler: SchedulerSummary,
    #[serde(default)]
    pub stats: Stats,
}

#[derive(Deserialize, Clone)]
pub struct Infra {
    pub name: String,
    pub ok: bool,
    #[serde(default)]
    pub detail: Option<String>,
}

#[derive(Deserialize, Clone)]
pub struct Engine {
    pub name: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub last_run: Option<String>,
    #[serde(default)]
    pub last_event_type: Option<String>,
    #[serde(default)]
    pub runs_24h: i64,
    #[serde(default)]
    pub runs_7d: i64,
}

#[derive(Deserialize, Default)]
pub struct SchedulerSummary {
    #[serde(default)]
    pub running: bool,
    #[serde(default)]
    pub jobs: i64,
    #[serde(default)]
    pub failing: Vec<serde_json::Value>,
    #[serde(default)]
    pub next_due: Option<String>,
}

#[derive(Deserialize)]
pub struct Scheduler {
    #[serde(default)]
    pub running: bool,
    #[serde(default)]
    pub started_at: Option<String>,
    #[serde(default)]
    pub jobs: Vec<SchedJob>,
}

#[derive(Deserialize, Clone)]
pub struct SchedJob {
    pub name: String,
    #[serde(default)]
    pub cadence: Option<String>,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub next_due: Option<String>,
    #[serde(default)]
    pub last_fired: Option<String>,
    #[serde(default)]
    pub last_status: Option<String>,
    #[serde(default)]
    pub last_error: Option<String>,
    #[serde(default)]
    pub consecutive_failures: i64,
}

#[derive(Deserialize, Clone)]
pub struct Event {
    pub id: String,
    #[serde(rename = "type", default)]
    pub etype: String,
    #[serde(default)]
    pub subject: Option<String>,
    #[serde(default)]
    pub data: serde_json::Value,
    #[serde(default)]
    pub seq: i64,
    #[serde(default)]
    pub created_at: Option<String>,
}

#[derive(Deserialize)]
struct LogSources {
    #[serde(default)]
    sources: Vec<LogSource>,
}

#[derive(Deserialize, Clone)]
pub struct LogSource {
    pub name: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub exists: bool,
    #[serde(default)]
    pub size_bytes: i64,
    #[serde(default)]
    pub modified: Option<String>,
}

#[derive(Deserialize)]
pub struct Logs {
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub exists: bool,
    #[serde(default)]
    pub size_bytes: i64,
    #[serde(default)]
    pub lines: Vec<serde_json::Value>,
}

#[derive(Deserialize, Clone)]
pub struct AgentRun {
    pub id: String,
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default)]
    pub task: String,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub result: serde_json::Value,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub finished_at: Option<String>,
}

#[derive(Deserialize)]
pub struct AgentRunDetail {
    #[serde(flatten)]
    pub run: AgentRun,
    #[serde(default)]
    pub actions: Vec<AgentAction>,
}

#[derive(Deserialize, Clone)]
pub struct AgentAction {
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub payload: serde_json::Value,
    #[serde(default)]
    pub observation: serde_json::Value,
    #[serde(default)]
    pub created_at: Option<String>,
}

#[derive(Deserialize, Clone)]
pub struct ScheduledJob {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub task: String,
    #[serde(default)]
    pub cadence_display: Option<String>,
    #[serde(default)]
    pub delivery: Option<String>,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub last_fired: Option<String>,
    #[serde(default)]
    pub last_status: Option<String>,
    #[serde(default)]
    pub last_error: Option<String>,
    #[serde(default)]
    pub next_due: Option<String>,
    #[serde(default)]
    pub registered: bool,
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn truncate(s: &str, n: usize) -> String {
    if s.len() <= n {
        s.to_string()
    } else {
        format!("{}…", &s[..n])
    }
}

/// Minimal percent-encoding for query values (spaces, &, etc).
fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}
