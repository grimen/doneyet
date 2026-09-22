use std::time::Duration;

use doneyet_core::diff::diff_worlds;
use std::collections::HashMap;

use doneyet_core::logtail::{LogCursor, append_log, append_log_matching, display_lines};
use doneyet_core::model::{
    Conclusion, Job, JobLog, Outcome, Phase, RepoRef, RunsPage, RunsQuery, WorkflowRun, World,
};
use doneyet_core::ports::{
    AnnotationSource, LogSource, ProviderError, PushSource, RefreshHint, RenderError, Renderer,
    RunSource,
};
use doneyet_core::stats::WorkflowStats;
use tokio_util::sync::CancellationToken;

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("provider failure: {0}")]
    Provider(#[from] ProviderError),
    #[error("renderer failure: {0}")]
    Render(#[from] RenderError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatchOutcome {
    Completed(Conclusion),
    Interrupted,
    TimedOut,
}

impl WatchOutcome {
    pub fn exit_code(&self) -> i32 {
        match self {
            WatchOutcome::Completed(conclusion) => conclusion.exit_code(),
            WatchOutcome::Interrupted => 130,
            WatchOutcome::TimedOut => 2,
        }
    }
}

#[derive(Debug, Clone)]
pub struct WatchConfig {
    pub active_interval: Duration,
    pub idle_interval: Duration,
    pub log_tail: Option<usize>,
    pub log_grep: Option<String>,
    pub job: Option<String>,
    pub timeout: Option<Duration>,
}

impl Default for WatchConfig {
    fn default() -> Self {
        Self {
            active_interval: Duration::from_secs(3),
            idle_interval: Duration::from_secs(20),
            log_tail: None,
            log_grep: None,
            job: None,
            timeout: None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum WatchTarget {
    Run(u64),
    Latest(RunsQuery),
}

pub struct NoopPushSource;

#[async_trait::async_trait]
impl PushSource for NoopPushSource {
    async fn wait(&mut self) -> Result<RefreshHint, ProviderError> {
        std::future::pending().await
    }
}

pub struct ChannelPushSource {
    receiver: tokio::sync::mpsc::UnboundedReceiver<RefreshHint>,
}

impl ChannelPushSource {
    pub fn channel() -> (tokio::sync::mpsc::UnboundedSender<RefreshHint>, Self) {
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
        (sender, Self { receiver })
    }
}

#[async_trait::async_trait]
impl PushSource for ChannelPushSource {
    async fn wait(&mut self) -> Result<RefreshHint, ProviderError> {
        self.receiver
            .recv()
            .await
            .ok_or_else(|| ProviderError::Other("push channel closed".to_string()))
    }
}

#[derive(Debug, Clone)]
pub struct DashConfig {
    pub interval: Duration,
    pub timeout: Option<Duration>,
}

impl Default for DashConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(5),
            timeout: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DashOutcome {
    Interrupted,
    TimedOut,
}

impl DashOutcome {
    pub fn exit_code(&self) -> i32 {
        match self {
            DashOutcome::Interrupted => 130,
            DashOutcome::TimedOut => 2,
        }
    }
}

pub trait BoardSink: Send {
    fn render_page(&mut self, page: &RunsPage) -> Result<(), RenderError>;
}

async fn push_or_tick(
    push: &mut Box<dyn PushSource>,
    push_dead: &mut bool,
    shutdown: &CancellationToken,
    interval: Duration,
) -> bool {
    tokio::select! {
        _ = tokio::time::sleep(interval) => false,
        hint = push.wait(), if !*push_dead => {
            match hint {
                Ok(_) => false,
                Err(error) => {
                    tracing::warn!("push source failed ({error}); continuing with polling only");
                    *push_dead = true;
                    false
                }
            }
        }
        _ = shutdown.cancelled() => true,
    }
}

fn rate_limit_backoff(retry_after: Duration, floor: Duration) -> Duration {
    retry_after.max(floor).min(Duration::from_secs(60))
}

pub fn aggregate_conclusion(runs: &[WorkflowRun]) -> Conclusion {
    if runs.iter().any(|run| {
        matches!(
            run.phase,
            Phase::Done(Conclusion::Failure | Conclusion::StartupFailure)
        )
    }) {
        Conclusion::Failure
    } else if runs.iter().any(|run| {
        matches!(
            run.phase,
            Phase::Done(Conclusion::TimedOut | Conclusion::Cancelled)
        )
    }) {
        Conclusion::Cancelled
    } else {
        Conclusion::Success
    }
}

fn filter_jobs(jobs: Vec<Job>, needle: Option<&str>) -> Vec<Job> {
    match needle {
        Some(needle) => jobs
            .into_iter()
            .filter(|job| job.name.contains(needle))
            .collect(),
        None => jobs,
    }
}

pub struct CommitWatchEngine {
    provider: Box<dyn RunSource>,
    push: Box<dyn PushSource>,
    push_dead: bool,
    config: WatchConfig,
    shutdown: CancellationToken,
}

impl CommitWatchEngine {
    pub fn new(
        provider: Box<dyn RunSource>,
        push: Box<dyn PushSource>,
        config: WatchConfig,
        shutdown: CancellationToken,
    ) -> Self {
        Self {
            provider,
            push,
            push_dead: false,
            config,
            shutdown,
        }
    }

    pub async fn run(
        &mut self,
        query: RunsQuery,
        sink: &mut dyn BoardSink,
    ) -> Result<WatchOutcome, EngineError> {
        loop {
            if self.shutdown.is_cancelled() {
                return Ok(WatchOutcome::Interrupted);
            }
            let page = match self.provider.list_runs(&query).await {
                Ok(page) => page,
                Err(ProviderError::RateLimited { retry_after }) => {
                    tracing::warn!("rate limited; backing off {:?}", retry_after);
                    let backoff = rate_limit_backoff(retry_after, self.config.active_interval);
                    if push_or_tick(&mut self.push, &mut self.push_dead, &self.shutdown, backoff)
                        .await
                    {
                        return Ok(WatchOutcome::Interrupted);
                    }
                    continue;
                }
                Err(ProviderError::Transport(message)) => {
                    tracing::warn!("transport error ({message}); continuing with polling");
                    if push_or_tick(
                        &mut self.push,
                        &mut self.push_dead,
                        &self.shutdown,
                        self.config.active_interval,
                    )
                    .await
                    {
                        return Ok(WatchOutcome::Interrupted);
                    }
                    continue;
                }
                Err(e) => return Err(e.into()),
            };
            sink.render_page(&page)?;
            let all_terminal =
                !page.runs.is_empty() && page.runs.iter().all(|run| run.phase.is_terminal());
            if all_terminal {
                return Ok(WatchOutcome::Completed(aggregate_conclusion(&page.runs)));
            }
            let interval = if page.runs.is_empty() {
                self.config.idle_interval
            } else {
                self.config.active_interval
            };
            if push_or_tick(
                &mut self.push,
                &mut self.push_dead,
                &self.shutdown,
                interval,
            )
            .await
            {
                return Ok(WatchOutcome::Interrupted);
            }
        }
    }
}

pub struct DashEngine {
    provider: Box<dyn RunSource>,
    push: Box<dyn PushSource>,
    push_dead: bool,
    config: DashConfig,
    shutdown: CancellationToken,
}

impl DashEngine {
    pub fn new(
        provider: Box<dyn RunSource>,
        push: Box<dyn PushSource>,
        config: DashConfig,
        shutdown: CancellationToken,
    ) -> Self {
        Self {
            provider,
            push,
            push_dead: false,
            config,
            shutdown,
        }
    }

    pub async fn run(
        &mut self,
        query: RunsQuery,
        sink: &mut dyn BoardSink,
    ) -> Result<DashOutcome, EngineError> {
        let deadline = self.config.timeout.map(|t| tokio::time::Instant::now() + t);
        loop {
            if self.shutdown.is_cancelled() {
                return Ok(DashOutcome::Interrupted);
            }
            if deadline.is_some_and(|deadline| tokio::time::Instant::now() >= deadline) {
                return Ok(DashOutcome::TimedOut);
            }
            let page = match self.provider.list_runs(&query).await {
                Ok(page) => page,
                Err(ProviderError::RateLimited { retry_after }) => {
                    tracing::warn!("rate limited; backing off {:?}", retry_after);
                    let backoff = rate_limit_backoff(retry_after, self.config.interval);
                    if push_or_tick(&mut self.push, &mut self.push_dead, &self.shutdown, backoff)
                        .await
                    {
                        return Ok(DashOutcome::Interrupted);
                    }
                    continue;
                }
                Err(ProviderError::Transport(message)) => {
                    tracing::warn!("transport error ({message}); continuing with polling");
                    if push_or_tick(
                        &mut self.push,
                        &mut self.push_dead,
                        &self.shutdown,
                        self.config.interval,
                    )
                    .await
                    {
                        return Ok(DashOutcome::Interrupted);
                    }
                    continue;
                }
                Err(e) => return Err(e.into()),
            };
            sink.render_page(&page)?;
            let interval = self.config.interval;
            if push_or_tick(
                &mut self.push,
                &mut self.push_dead,
                &self.shutdown,
                interval,
            )
            .await
            {
                return Ok(DashOutcome::Interrupted);
            }
        }
    }
}

pub trait WatchSource: RunSource + LogSource + AnnotationSource {}

impl<T> WatchSource for T where T: RunSource + LogSource + AnnotationSource {}

pub struct WatchEngine {
    repo: RepoRef,
    provider: Box<dyn WatchSource>,
    renderer: Box<dyn Renderer>,
    push: Box<dyn PushSource>,
    push_dead: bool,
    config: WatchConfig,
    shutdown: CancellationToken,
    cursors: HashMap<u64, LogCursor>,
}

impl WatchEngine {
    pub fn new(
        repo: RepoRef,
        provider: Box<dyn WatchSource>,
        renderer: Box<dyn Renderer>,
        push: Box<dyn PushSource>,
        config: WatchConfig,
        shutdown: CancellationToken,
    ) -> Self {
        Self {
            repo,
            provider,
            renderer,
            push,
            push_dead: false,
            config,
            shutdown,
            cursors: HashMap::new(),
        }
    }

    pub async fn watch(&mut self, target: WatchTarget) -> Result<WatchOutcome, EngineError> {
        let deadline = self.config.timeout.map(|t| tokio::time::Instant::now() + t);
        let mut previous: Option<World> = None;
        loop {
            if self.shutdown.is_cancelled() {
                return Ok(WatchOutcome::Interrupted);
            }
            if deadline.is_some_and(|deadline| tokio::time::Instant::now() >= deadline) {
                return Ok(WatchOutcome::TimedOut);
            }
            let run_opt = match self.locate_run(&target).await {
                Ok(None) => {
                    if let Some(outcome) = self.wait_tick(self.config.idle_interval, deadline).await
                    {
                        return Ok(outcome);
                    }
                    None
                }
                Ok(Some(run)) => Some(run),
                Err(ProviderError::RateLimited { retry_after }) => {
                    tracing::warn!("rate limited; backing off {:?}", retry_after);
                    let backoff = rate_limit_backoff(retry_after, self.config.active_interval);
                    if let Some(outcome) = self.wait_tick(backoff, deadline).await {
                        return Ok(outcome);
                    }
                    continue;
                }
                Err(ProviderError::Transport(message)) => {
                    tracing::warn!("transport error ({message}); continuing with polling");
                    if let Some(outcome) =
                        self.wait_tick(self.config.active_interval, deadline).await
                    {
                        return Ok(outcome);
                    }
                    continue;
                }
                Err(e) => return Err(e.into()),
            };
            let Some(run) = run_opt else {
                continue;
            };
            let jobs = match self.provider.list_jobs(run.id).await {
                Ok(jobs) => jobs,
                Err(ProviderError::RateLimited { retry_after }) => {
                    tracing::warn!("rate limited; backing off {:?}", retry_after);
                    let backoff = rate_limit_backoff(retry_after, self.config.active_interval);
                    if let Some(outcome) = self.wait_tick(backoff, deadline).await {
                        return Ok(outcome);
                    }
                    continue;
                }
                Err(ProviderError::Transport(message)) => {
                    tracing::warn!("transport error ({message}); continuing with polling");
                    if let Some(outcome) =
                        self.wait_tick(self.config.active_interval, deadline).await
                    {
                        return Ok(outcome);
                    }
                    continue;
                }
                Err(e) => return Err(e.into()),
            };
            let jobs = filter_jobs(jobs, self.config.job.as_deref());
            let job_logs = self.tail_logs(&jobs).await;
            let stats = match previous.as_ref().and_then(|world| world.stats.clone()) {
                Some(stats) => Some(stats),
                None => self.fetch_stats(&target, &run).await,
            };
            let mut annotations = Vec::new();
            for job in &jobs {
                if job.phase.is_failed() {
                    match self.provider.list_annotations(job.id).await {
                        Ok(mut ann) => annotations.append(&mut ann),
                        Err(ProviderError::RateLimited { retry_after }) => {
                            tracing::warn!(
                                "annotations rate limited; backing off {:?}",
                                retry_after
                            );
                            let backoff =
                                rate_limit_backoff(retry_after, self.config.active_interval);
                            if let Some(outcome) = self.wait_tick(backoff, deadline).await {
                                return Ok(outcome);
                            }
                            continue;
                        }
                        Err(e) => tracing::warn!("annotations fetch failed: {e}"),
                    }
                }
            }
            let world = World {
                repo: self.repo.clone(),
                run,
                jobs,
                annotations,
                stats,
                job_logs,
            };
            let events = diff_worlds(previous.as_ref(), &world);
            let conclusion = match &world.run.phase {
                Phase::Done(conclusion) => Some(conclusion.clone()),
                _ => None,
            };
            self.renderer.render(&world, &events)?;
            previous = Some(world);
            if let Some(conclusion) = conclusion {
                let outcome = Outcome {
                    run: previous
                        .as_ref()
                        .expect("world was just stored")
                        .run
                        .ref_of(),
                    conclusion: conclusion.clone(),
                };
                self.renderer.finish(&outcome)?;
                return Ok(WatchOutcome::Completed(conclusion));
            }
            if let Some(outcome) = self.wait_tick(self.config.active_interval, deadline).await {
                return Ok(outcome);
            }
        }
    }
    async fn wait_tick(
        &mut self,
        interval: Duration,
        deadline: Option<tokio::time::Instant>,
    ) -> Option<WatchOutcome> {
        let until = match deadline {
            Some(deadline) => std::cmp::min(tokio::time::Instant::now() + interval, deadline),
            None => tokio::time::Instant::now() + interval,
        };
        tokio::select! {
            _ = tokio::time::sleep_until(until) => {}
            hint = self.push.wait(), if !self.push_dead => {
                match hint {
                    Ok(_) => {}
                    Err(error) => {
                        tracing::warn!("push source failed ({error}); continuing with polling only");
                        self.push_dead = true;
                    }
                }
            }
            _ = self.shutdown.cancelled() => return Some(WatchOutcome::Interrupted),
        }
        if deadline.is_some_and(|deadline| tokio::time::Instant::now() >= deadline) {
            return Some(WatchOutcome::TimedOut);
        }
        None
    }

    async fn tail_logs(&mut self, jobs: &[Job]) -> Vec<JobLog> {
        let Some(limit) = self.config.log_tail else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for job in jobs {
            if !matches!(job.phase, Phase::InProgress) && !job.phase.is_failed() {
                continue;
            }
            let offset = self
                .cursors
                .get(&job.id)
                .map(|cursor| cursor.offset)
                .unwrap_or(0);
            let chunk = match self.provider.job_logs_from(job.id, offset).await {
                Ok(chunk) if chunk.next_offset < offset => {
                    if let Some(cursor) = self.cursors.get_mut(&job.id) {
                        cursor.lines.clear();
                        cursor.pending.clear();
                        cursor.offset = 0;
                    }
                    match self.provider.job_logs_from(job.id, 0).await {
                        Ok(chunk) => chunk,
                        Err(error) => {
                            tracing::warn!("job {} logs unavailable ({error})", job.id);
                            self.push_previous_tail(job.id, &mut out);
                            continue;
                        }
                    }
                }
                Ok(chunk) => chunk,
                Err(error) => {
                    tracing::warn!("job {} logs unavailable ({error})", job.id);
                    self.push_previous_tail(job.id, &mut out);
                    continue;
                }
            };
            let cursor = self.cursors.entry(job.id).or_default();
            match &self.config.log_grep {
                Some(grep) => append_log_matching(cursor, &chunk, limit, grep),
                None => append_log(cursor, &chunk, limit),
            }
            let lines = display_lines(cursor, limit);
            if !lines.is_empty() {
                out.push(JobLog {
                    job_id: job.id,
                    lines,
                });
            }
        }
        out
    }

    fn push_previous_tail(&self, job_id: u64, out: &mut Vec<JobLog>) {
        let Some(limit) = self.config.log_tail else {
            return;
        };
        let Some(cursor) = self.cursors.get(&job_id) else {
            return;
        };
        let lines = display_lines(cursor, limit);
        if !lines.is_empty() {
            out.push(JobLog { job_id, lines });
        }
    }

    async fn fetch_stats(&self, target: &WatchTarget, run: &WorkflowRun) -> Option<WorkflowStats> {
        let branch = match target {
            WatchTarget::Latest(query) => query.branch.clone(),
            WatchTarget::Run(_) => None,
        };
        let query = RunsQuery {
            repo: self.repo.clone(),
            branch,
            head_sha: None,
            event: None,
            limit: 20,
        };
        let page = match self.provider.list_runs(&query).await {
            Ok(page) => page,
            Err(error) => {
                tracing::warn!("workflow history unavailable ({error}); continuing without ETA");
                return None;
            }
        };
        let history: Vec<WorkflowRun> = page
            .runs
            .into_iter()
            .filter(|past| past.name == run.name)
            .collect();
        let stats = WorkflowStats::from_runs(&history);
        if stats.samples() == 0 {
            None
        } else {
            Some(stats)
        }
    }

    async fn locate_run(&self, target: &WatchTarget) -> Result<Option<WorkflowRun>, ProviderError> {
        match target {
            WatchTarget::Run(id) => Ok(Some(self.provider.get_run(*id).await?)),
            WatchTarget::Latest(query) => {
                let page = self.provider.list_runs(query).await?;
                Ok(page.runs.into_iter().next())
            }
        }
    }
}
