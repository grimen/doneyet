use std::fmt;
use std::str::FromStr;

use jiff::Timestamp;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RepoRef {
    pub owner: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid repository reference {0:?}: expected OWNER/NAME")]
pub struct RepoParseError(String);

impl FromStr for RepoRef {
    type Err = RepoParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.splitn(2, '/');
        let owner = parts.next().unwrap_or("");
        let name = parts.next().unwrap_or("");
        if owner.is_empty() || name.is_empty() || name.contains('/') {
            return Err(RepoParseError(s.to_string()));
        }
        Ok(RepoRef {
            owner: owner.to_string(),
            name: name.to_string(),
        })
    }
}

impl fmt::Display for RepoRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.owner, self.name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Conclusion {
    Success,
    Failure,
    Cancelled,
    Skipped,
    TimedOut,
    StartupFailure,
    ActionRequired,
    Neutral,
    Stale,
    Other(String),
}

impl Conclusion {
    pub fn parse(raw: &str) -> Conclusion {
        match raw {
            "success" => Conclusion::Success,
            "failure" => Conclusion::Failure,
            "cancelled" => Conclusion::Cancelled,
            "skipped" => Conclusion::Skipped,
            "timed_out" => Conclusion::TimedOut,
            "startup_failure" => Conclusion::StartupFailure,
            "action_required" => Conclusion::ActionRequired,
            "neutral" => Conclusion::Neutral,
            "stale" => Conclusion::Stale,
            other => Conclusion::Other(other.to_string()),
        }
    }

    pub fn exit_code(&self) -> i32 {
        match self {
            Conclusion::Success | Conclusion::Skipped | Conclusion::Neutral => 0,
            Conclusion::Cancelled | Conclusion::TimedOut => 2,
            Conclusion::Failure
            | Conclusion::StartupFailure
            | Conclusion::ActionRequired
            | Conclusion::Stale
            | Conclusion::Other(_) => 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Phase {
    Queued,
    Waiting,
    InProgress,
    Done(Conclusion),
    Other(String),
}

impl Phase {
    pub fn from_parts(status: &str, conclusion: Option<&str>) -> Phase {
        match status {
            "queued" => Phase::Queued,
            "waiting" => Phase::Waiting,
            "in_progress" => Phase::InProgress,
            "completed" => match conclusion {
                Some(raw) => Phase::Done(Conclusion::parse(raw)),
                None => Phase::Done(Conclusion::Other("null".to_string())),
            },
            other => Phase::Other(other.to_string()),
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, Phase::Done(_))
    }

    pub fn is_failed(&self) -> bool {
        matches!(
            self,
            Phase::Done(Conclusion::Failure | Conclusion::TimedOut | Conclusion::StartupFailure)
        )
    }

    pub fn is_active(&self) -> bool {
        !self.is_terminal()
    }

    pub fn conclusion(&self) -> Option<&Conclusion> {
        match self {
            Phase::Done(c) => Some(c),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RunRef {
    pub id: u64,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct JobRef {
    pub id: u64,
    pub run_id: u64,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StepRef {
    pub job_id: u64,
    pub number: u64,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowRun {
    pub id: u64,
    pub run_number: u64,
    pub name: String,
    pub display_title: String,
    pub head_branch: Option<String>,
    pub head_sha: String,
    pub event: String,
    pub phase: Phase,
    pub actor: String,
    pub html_url: String,
    pub created_at: Timestamp,
    pub run_started_at: Option<Timestamp>,
    pub updated_at: Timestamp,
}

impl WorkflowRun {
    pub fn ref_of(&self) -> RunRef {
        RunRef {
            id: self.id,
            name: self.name.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Step {
    pub number: u64,
    pub name: String,
    pub phase: Phase,
    pub started_at: Option<Timestamp>,
    pub completed_at: Option<Timestamp>,
}

impl Step {
    pub fn ref_of(&self, job_id: u64) -> StepRef {
        StepRef {
            job_id,
            number: self.number,
            name: self.name.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Job {
    pub id: u64,
    pub run_id: u64,
    pub name: String,
    pub phase: Phase,
    pub started_at: Option<Timestamp>,
    pub completed_at: Option<Timestamp>,
    pub runner_name: Option<String>,
    pub labels: Vec<String>,
    pub steps: Vec<Step>,
}

impl Job {
    pub fn ref_of(&self) -> JobRef {
        JobRef {
            id: self.id,
            run_id: self.run_id,
            name: self.name.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Annotation {
    pub job_id: u64,
    pub path: Option<String>,
    pub start_line: Option<u64>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct World {
    pub repo: RepoRef,
    pub run: WorkflowRun,
    pub jobs: Vec<Job>,
    #[serde(default)]
    pub annotations: Vec<Annotation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stats: Option<crate::stats::WorkflowStats>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunsQuery {
    pub repo: RepoRef,
    pub branch: Option<String>,
    pub head_sha: Option<String>,
    pub event: Option<String>,
    pub limit: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunsPage {
    pub total_count: u64,
    pub runs: Vec<WorkflowRun>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Outcome {
    pub run: RunRef,
    pub conclusion: Conclusion,
}
