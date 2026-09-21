use std::time::Duration;

use doneyet_core::diff::diff_worlds;
use doneyet_core::model::{Conclusion, Outcome, Phase, RepoRef, RunsQuery, WorkflowRun, World};
use doneyet_core::ports::{
    ProviderError, PushSource, RefreshHint, RenderError, Renderer, RunSource,
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
}

impl WatchOutcome {
    pub fn exit_code(&self) -> i32 {
        match self {
            WatchOutcome::Completed(conclusion) => conclusion.exit_code(),
            WatchOutcome::Interrupted => 130,
        }
    }
}

#[derive(Debug, Clone)]
pub struct WatchConfig {
    pub active_interval: Duration,
    pub idle_interval: Duration,
}

impl Default for WatchConfig {
    fn default() -> Self {
        Self {
            active_interval: Duration::from_secs(3),
            idle_interval: Duration::from_secs(20),
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

pub struct WatchEngine {
    repo: RepoRef,
    provider: Box<dyn RunSource>,
    renderer: Box<dyn Renderer>,
    push: Box<dyn PushSource>,
    push_dead: bool,
    config: WatchConfig,
    shutdown: CancellationToken,
}

impl WatchEngine {
    pub fn new(
        repo: RepoRef,
        provider: Box<dyn RunSource>,
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
        }
    }

    pub async fn watch(&mut self, target: WatchTarget) -> Result<WatchOutcome, EngineError> {
        let mut previous: Option<World> = None;
        loop {
            if self.shutdown.is_cancelled() {
                return Ok(WatchOutcome::Interrupted);
            }
            match self.locate_run(&target).await? {
                None => {
                    if let Some(outcome) = self.wait_tick(self.config.idle_interval).await {
                        return Ok(outcome);
                    }
                }
                Some(run) => {
                    let jobs = self.provider.list_jobs(run.id).await?;
                    let stats = match previous.as_ref().and_then(|world| world.stats.clone()) {
                        Some(stats) => Some(stats),
                        None => self.fetch_stats(&target, &run).await,
                    };
                    let world = World {
                        repo: self.repo.clone(),
                        run,
                        jobs,
                        annotations: Vec::new(),
                        stats,
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
                    if let Some(outcome) = self.wait_tick(self.config.active_interval).await {
                        return Ok(outcome);
                    }
                }
            }
        }
    }

    async fn wait_tick(&mut self, interval: Duration) -> Option<WatchOutcome> {
        tokio::select! {
            _ = tokio::time::sleep(interval) => {}
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
        None
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
