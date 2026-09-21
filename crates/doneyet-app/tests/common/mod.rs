#![allow(dead_code)]

use doneyet_core::event::DomainEvent;
use doneyet_core::model::{Annotation, Outcome};
use doneyet_core::model::{
    Conclusion, Job, Phase, RepoRef, RunsPage, RunsQuery, WorkflowRun, World,
};
use doneyet_core::ports::{AnnotationSource, LogSource, ProviderError, RefreshHint, RunSource};
use jiff::Timestamp;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

pub fn ts(s: &str) -> Timestamp {
    s.parse().expect("valid RFC3339 timestamp")
}

pub fn repo() -> RepoRef {
    RepoRef {
        owner: "acme".to_string(),
        name: "api".to_string(),
    }
}

pub fn run(phase: Phase) -> WorkflowRun {
    WorkflowRun {
        id: 2841,
        run_number: 2841,
        name: "ci.yml".to_string(),
        display_title: "build & test".to_string(),
        head_branch: Some("main".to_string()),
        head_sha: "abc123".to_string(),
        event: "push".to_string(),
        phase,
        actor: "jonas".to_string(),
        html_url: "https://github.com/acme/api/actions/runs/2841".to_string(),
        created_at: ts("2026-09-21T10:00:00Z"),
        run_started_at: Some(ts("2026-09-21T10:00:01Z")),
        updated_at: ts("2026-09-21T10:03:00Z"),
    }
}

pub fn world(phase: Phase, jobs: Vec<Job>) -> World {
    World {
        repo: repo(),
        run: run(phase),
        jobs,
        annotations: Vec::new(),
        stats: None,
    }
}

pub fn job(id: u64, name: &str, phase: Phase) -> Job {
    Job {
        id,
        run_id: 2841,
        name: name.to_string(),
        phase,
        started_at: None,
        completed_at: None,
        runner_name: None,
        labels: Vec::new(),
        steps: Vec::new(),
    }
}

#[derive(Clone)]
#[allow(clippy::large_enum_variant)]
pub enum Step0 {
    NoRun,
    Fail(String),
    World(World),
}

pub struct FakeProvider {
    state: Arc<FakeState>,
}

pub struct FakeState {
    steps: Vec<Step0>,
    cursor: AtomicUsize,
    locate_calls: Mutex<Vec<tokio::time::Instant>>,
    history: Mutex<RunsPage>,
}

impl FakeProvider {
    pub fn new(steps: Vec<Step0>) -> Self {
        Self {
            state: Arc::new(FakeState {
                steps,
                cursor: AtomicUsize::new(0),
                locate_calls: Mutex::new(Vec::new()),
                history: Mutex::new(RunsPage {
                    total_count: 0,
                    runs: Vec::new(),
                }),
            }),
        }
    }

    pub fn with_history(steps: Vec<Step0>, history: Vec<WorkflowRun>) -> Self {
        let provider = Self::new(steps);
        *provider.state.history.lock().expect("history poisoned") = RunsPage {
            total_count: history.len() as u64,
            runs: history,
        };
        provider
    }

    pub fn handle(&self) -> Arc<FakeState> {
        self.state.clone()
    }
}

impl FakeState {
    pub fn fetches(&self) -> usize {
        self.locate_calls.lock().expect("log poisoned").len()
    }

    pub fn locate_calls(&self) -> Vec<tokio::time::Instant> {
        self.locate_calls.lock().expect("log poisoned").clone()
    }

    fn advance(&self) -> Step0 {
        let idx = self.cursor.fetch_add(1, Ordering::SeqCst);
        self.steps[idx.min(self.steps.len() - 1)].clone()
    }

    fn current(&self) -> Step0 {
        let idx = self.cursor.load(Ordering::SeqCst);
        let idx = idx.saturating_sub(1).min(self.steps.len() - 1);
        self.steps[idx].clone()
    }
}

#[async_trait::async_trait]
impl RunSource for FakeProvider {
    async fn list_runs(&self, query: &RunsQuery) -> Result<RunsPage, ProviderError> {
        if query.limit >= 20 {
            return Ok(self.state.history.lock().expect("history poisoned").clone());
        }
        self.state
            .locate_calls
            .lock()
            .expect("log poisoned")
            .push(tokio::time::Instant::now());
        match self.state.advance() {
            Step0::NoRun => Ok(RunsPage {
                total_count: 0,
                runs: Vec::new(),
            }),
            Step0::Fail(message) => Err(ProviderError::Other(message)),
            Step0::World(world) => Ok(RunsPage {
                total_count: 1,
                runs: vec![world.run],
            }),
        }
    }

    async fn get_run(&self, _run_id: u64) -> Result<WorkflowRun, ProviderError> {
        self.state
            .locate_calls
            .lock()
            .expect("log poisoned")
            .push(tokio::time::Instant::now());
        match self.state.advance() {
            Step0::NoRun => Err(ProviderError::NotFound("no run".to_string())),
            Step0::Fail(message) => Err(ProviderError::Other(message)),
            Step0::World(world) => Ok(world.run),
        }
    }

    async fn list_jobs(&self, _run_id: u64) -> Result<Vec<Job>, ProviderError> {
        match self.state.current() {
            Step0::World(world) => Ok(world.jobs),
            _ => Ok(Vec::new()),
        }
    }
}

#[async_trait::async_trait]
impl AnnotationSource for FakeProvider {
    async fn list_annotations(&self, _job_id: u64) -> Result<Vec<Annotation>, ProviderError> {
        Ok(Vec::new())
    }
}

#[async_trait::async_trait]
impl LogSource for FakeProvider {
    async fn job_logs(&self, _job_id: u64) -> Result<Vec<u8>, ProviderError> {
        Ok(Vec::new())
    }
}

#[derive(Clone, Default)]
pub struct RecordingRenderer {
    inner: Arc<RecorderInner>,
}

#[derive(Default)]
pub struct RecorderInner {
    frames: Mutex<Vec<(World, Vec<DomainEvent>)>>,
    finishes: Mutex<Vec<Outcome>>,
}

impl RecordingRenderer {
    pub fn frames(&self) -> Vec<(World, Vec<DomainEvent>)> {
        self.inner.frames.lock().expect("frames poisoned").clone()
    }

    pub fn finishes(&self) -> Vec<Outcome> {
        self.inner
            .finishes
            .lock()
            .expect("finishes poisoned")
            .clone()
    }
}

impl doneyet_core::ports::Renderer for RecordingRenderer {
    fn render(
        &mut self,
        world: &World,
        events: &[DomainEvent],
    ) -> Result<(), doneyet_core::ports::RenderError> {
        self.inner
            .frames
            .lock()
            .expect("frames poisoned")
            .push((world.clone(), events.to_vec()));
        Ok(())
    }

    fn finish(&mut self, outcome: &Outcome) -> Result<(), doneyet_core::ports::RenderError> {
        self.inner
            .finishes
            .lock()
            .expect("finishes poisoned")
            .push(outcome.clone());
        Ok(())
    }
}

pub struct ImmediatePush {
    hints: Mutex<VecDeque<RefreshHint>>,
}

impl ImmediatePush {
    pub fn with_hints(hints: Vec<RefreshHint>) -> Self {
        Self {
            hints: Mutex::new(hints.into_iter().collect()),
        }
    }
}

#[async_trait::async_trait]
impl doneyet_core::ports::PushSource for ImmediatePush {
    async fn wait(&mut self) -> Result<RefreshHint, ProviderError> {
        if let Some(hint) = self.hints.lock().expect("hints poisoned").pop_front() {
            return Ok(hint);
        }
        std::future::pending().await
    }
}

pub struct DyingPush;

#[async_trait::async_trait]
impl doneyet_core::ports::PushSource for DyingPush {
    async fn wait(&mut self) -> Result<RefreshHint, ProviderError> {
        Err(ProviderError::Other("push source died".to_string()))
    }
}

pub fn hint() -> RefreshHint {
    RefreshHint {
        repo: repo(),
        run_id: Some(2841),
    }
}

pub fn success_outcome() -> Outcome {
    Outcome {
        run: doneyet_core::model::RunRef {
            id: 2841,
            name: "ci.yml".to_string(),
        },
        conclusion: Conclusion::Success,
    }
}
