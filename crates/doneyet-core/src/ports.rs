use crate::event::DomainEvent;
use crate::model::{Annotation, Job, Outcome, RepoRef, RunsPage, RunsQuery, WorkflowRun, World};
use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("unauthorized: token is missing, invalid, or expired")]
    Auth,
    #[error("not found: {0}")]
    NotFound(String),
    #[error("rate limited: retry after {retry_after:?}")]
    RateLimited { retry_after: Duration },
    #[error("transport error: {0}")]
    Transport(String),
    #[error("malformed provider response: {0}")]
    Parse(String),
    #[error("provider error: {0}")]
    Other(String),
}

#[async_trait::async_trait]
pub trait RunSource: Send + Sync {
    async fn list_runs(&self, query: &RunsQuery) -> Result<RunsPage, ProviderError>;
    async fn get_run(&self, run_id: u64) -> Result<WorkflowRun, ProviderError>;
    async fn list_jobs(&self, run_id: u64) -> Result<Vec<Job>, ProviderError>;
    async fn pr_head_sha(&self, number: u64) -> Result<String, ProviderError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogChunk {
    pub bytes: Vec<u8>,
    pub next_offset: u64,
}

#[async_trait::async_trait]
pub trait RunWriteSource: Send + Sync {
    async fn rerun(&self, run_id: u64, failed_only: bool) -> Result<(), ProviderError>;
    async fn cancel(&self, run_id: u64) -> Result<(), ProviderError>;
}

#[async_trait::async_trait]
pub trait LogSource: Send + Sync {
    async fn job_logs(&self, job_id: u64) -> Result<Vec<u8>, ProviderError>;

    async fn job_logs_from(&self, job_id: u64, offset: u64) -> Result<LogChunk, ProviderError> {
        let all = self.job_logs(job_id).await?;
        let start = usize::try_from(offset).unwrap_or(all.len()).min(all.len());
        Ok(LogChunk {
            bytes: all[start..].to_vec(),
            next_offset: all.len() as u64,
        })
    }
}

#[async_trait::async_trait]
pub trait AnnotationSource: Send + Sync {
    async fn list_annotations(&self, job_id: u64) -> Result<Vec<Annotation>, ProviderError>;
}

pub trait PipelineProvider: RunSource + LogSource + AnnotationSource {}

impl<T> PipelineProvider for T where T: RunSource + LogSource + AnnotationSource {}

#[derive(Debug, thiserror::Error)]
#[error("renderer error: {0}")]
pub struct RenderError(pub String);

pub type RenderResult<T> = Result<T, RenderError>;

pub trait Renderer: Send {
    fn render(&mut self, world: &World, events: &[DomainEvent]) -> RenderResult<()>;
    fn finish(&mut self, outcome: &Outcome) -> RenderResult<()>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefreshHint {
    pub repo: RepoRef,
    pub run_id: Option<u64>,
}

#[async_trait::async_trait]
pub trait PushSource: Send {
    async fn wait(&mut self) -> Result<RefreshHint, ProviderError>;
}
