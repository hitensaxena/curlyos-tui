//! Background network worker. A single thread owns the blocking `Client` and
//! processes `Req`s FIFO, emitting `Resp`s. Because it is one thread draining a
//! channel in order, responses always arrive in request order — no staleness
//! races to reconcile in the UI.

use crate::api::*;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;

pub enum Req {
    Home,
    Memories(MemoryQuery),
    MemoryDetail(String),
    Episodes { limit: usize, offset: usize },
    EpisodeDetail(String),
    Identity,
    Principles,
    Narrative,
    Attention,
    Reflections,
    Graph(usize),
    Recall { query: String, mode: String, k: usize },
    Systems,
    Scheduler,
    Events(usize),
    LogSources,
    Logs { source: String, limit: usize },
    AgentRuns(usize),
    AgentRun(String),
    ScheduledJobs,
    Invalidate(String),
    ProposeIdentity { predicate: String, object: String, confidence: f64 },
    Ingest(String),
    Trigger { path: String, label: String, body: serde_json::Value },
}

pub enum Resp {
    Home(Box<Health>, Stats, Composition),
    Memories(Vec<Memory>),
    MemoryDetail(Box<MemoryDetail>),
    Episodes(Vec<Episode>),
    EpisodeDetail(Box<EpisodeDetail>),
    Identity(Vec<IdentityFact>),
    Principles(Vec<Principle>),
    Narrative(Vec<Chapter>),
    Attention(Box<Attention>),
    Reflections(Vec<Report>),
    Graph(Box<Graph>),
    Recall(Vec<RecallHit>),
    Systems(Box<Systems>),
    Scheduler(Box<Scheduler>),
    Events(Vec<Event>),
    LogSources(Vec<LogSource>),
    Logs(Box<Logs>),
    AgentRuns(Vec<AgentRun>),
    AgentRun(Box<AgentRunDetail>),
    ScheduledJobs(Vec<ScheduledJob>),
    /// A write/trigger succeeded; carries a human-facing message + whether the
    /// caller should refresh the active view.
    ActionOk { msg: String, refresh: bool },
    Error(String),
}

pub fn spawn(client: Client, rx: Receiver<Req>, tx: Sender<Resp>) {
    thread::spawn(move || {
        for req in rx {
            let resp = handle(&client, req);
            if tx.send(resp).is_err() {
                break; // UI gone
            }
        }
    });
}

fn handle(c: &Client, req: Req) -> Resp {
    match req {
        Req::Home => match (c.health(), c.stats(), c.composition()) {
            (Ok(h), Ok(s), Ok(comp)) => Resp::Home(Box::new(h), s, comp),
            (Err(e), ..) | (_, Err(e), _) | (.., Err(e)) => Resp::Error(format!("home: {e}")),
        },
        Req::Memories(q) => wrap(c.memories(&q), Resp::Memories),
        Req::MemoryDetail(id) => wrap(c.memory_detail(&id), |d| Resp::MemoryDetail(Box::new(d))),
        Req::Episodes { limit, offset } => wrap(c.episodes(limit, offset), Resp::Episodes),
        Req::EpisodeDetail(id) => {
            wrap(c.episode_detail(&id), |d| Resp::EpisodeDetail(Box::new(d)))
        }
        Req::Identity => wrap(c.identity(), Resp::Identity),
        Req::Principles => wrap(c.principles(), Resp::Principles),
        Req::Narrative => wrap(c.narrative(), Resp::Narrative),
        Req::Attention => wrap(c.attention(), |a| Resp::Attention(Box::new(a))),
        Req::Reflections => wrap(c.reflections(), Resp::Reflections),
        Req::Graph(limit) => wrap(c.graph(limit), |g| Resp::Graph(Box::new(g))),
        Req::Recall { query, mode, k } => wrap(c.recall(&query, &mode, k), Resp::Recall),
        Req::Systems => wrap(c.systems(), |s| Resp::Systems(Box::new(s))),
        Req::Scheduler => wrap(c.scheduler(), |s| Resp::Scheduler(Box::new(s))),
        Req::Events(n) => wrap(c.events(n), Resp::Events),
        Req::LogSources => wrap(c.log_sources(), Resp::LogSources),
        Req::Logs { source, limit } => wrap(c.logs(&source, limit), |l| Resp::Logs(Box::new(l))),
        Req::AgentRuns(n) => wrap(c.agent_runs(n), Resp::AgentRuns),
        Req::AgentRun(id) => wrap(c.agent_run(&id), |d| Resp::AgentRun(Box::new(d))),
        Req::ScheduledJobs => wrap(c.scheduled_jobs(), Resp::ScheduledJobs),
        Req::Invalidate(id) => match c.invalidate_memory(&id) {
            Ok(()) => Resp::ActionOk { msg: format!("Invalidated {id}"), refresh: true },
            Err(e) => Resp::Error(format!("invalidate: {e}")),
        },
        Req::ProposeIdentity { predicate, object, confidence } => {
            match c.propose_identity(&predicate, &object, confidence) {
                Ok(()) => Resp::ActionOk {
                    msg: format!("Proposed identity: {predicate} = {object}"),
                    refresh: true,
                },
                Err(e) => Resp::Error(format!("propose identity: {e}")),
            }
        }
        Req::Ingest(content) => match c.ingest(&content) {
            Ok(()) => Resp::ActionOk { msg: "Captured episode (ingest queued)".into(), refresh: false },
            Err(e) => Resp::Error(format!("ingest: {e}")),
        },
        Req::Trigger { path, label, body } => match c.trigger(&path, body) {
            Ok(()) => Resp::ActionOk { msg: format!("{label} triggered"), refresh: true },
            Err(e) => Resp::Error(format!("{label}: {e}")),
        },
    }
}

fn wrap<T>(r: anyhow::Result<T>, ok: impl FnOnce(T) -> Resp) -> Resp {
    match r {
        Ok(v) => ok(v),
        Err(e) => Resp::Error(format!("{e}")),
    }
}
