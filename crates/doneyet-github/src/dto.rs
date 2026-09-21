use doneyet_core::model::{Annotation, Job, Phase, Step, WorkflowRun};
use jiff::Timestamp;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct RawActor {
    #[serde(default)]
    pub login: String,
}

#[derive(Debug, Deserialize)]
pub struct RawRun {
    pub id: u64,
    #[serde(default)]
    pub run_number: u64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub display_title: String,
    pub head_branch: Option<String>,
    #[serde(default)]
    pub head_sha: String,
    #[serde(default)]
    pub event: String,
    #[serde(default)]
    pub status: String,
    pub conclusion: Option<String>,
    #[serde(default)]
    pub actor: Option<RawActor>,
    #[serde(default)]
    pub html_url: String,
    #[serde(default)]
    pub created_at: Option<Timestamp>,
    #[serde(default)]
    pub run_started_at: Option<Timestamp>,
    #[serde(default)]
    pub updated_at: Option<Timestamp>,
}

impl RawRun {
    pub fn into_model(self) -> WorkflowRun {
        WorkflowRun {
            id: self.id,
            run_number: self.run_number,
            name: self.name,
            display_title: self.display_title,
            head_branch: self.head_branch,
            head_sha: self.head_sha,
            event: self.event,
            phase: Phase::from_parts(&self.status, self.conclusion.as_deref()),
            actor: self.actor.map(|a| a.login).unwrap_or_default(),
            html_url: self.html_url,
            created_at: self.created_at.unwrap_or_else(epoch),
            run_started_at: self.run_started_at,
            updated_at: self.updated_at.unwrap_or_else(epoch),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct RawRunsPage {
    #[serde(default)]
    pub total_count: u64,
    #[serde(default)]
    pub workflow_runs: Vec<RawRun>,
}

#[derive(Debug, Deserialize)]
pub struct RawStep {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub number: u64,
    #[serde(default)]
    pub status: String,
    pub conclusion: Option<String>,
    #[serde(default)]
    pub started_at: Option<Timestamp>,
    #[serde(default)]
    pub completed_at: Option<Timestamp>,
}

impl RawStep {
    pub fn into_model(self) -> Step {
        Step {
            number: self.number,
            name: self.name,
            phase: Phase::from_parts(&self.status, self.conclusion.as_deref()),
            started_at: self.started_at,
            completed_at: self.completed_at,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct RawJob {
    pub id: u64,
    #[serde(default)]
    pub run_id: u64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub status: String,
    pub conclusion: Option<String>,
    #[serde(default)]
    pub started_at: Option<Timestamp>,
    #[serde(default)]
    pub completed_at: Option<Timestamp>,
    #[serde(default)]
    pub runner_name: Option<String>,
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default)]
    pub steps: Vec<RawStep>,
}

impl RawJob {
    pub fn into_model(self) -> Job {
        Job {
            id: self.id,
            run_id: self.run_id,
            name: self.name,
            phase: Phase::from_parts(&self.status, self.conclusion.as_deref()),
            started_at: self.started_at,
            completed_at: self.completed_at,
            runner_name: self.runner_name,
            labels: self.labels,
            steps: self.steps.into_iter().map(RawStep::into_model).collect(),
        }
    }
}

#[derive(Debug, Default, Deserialize)]
pub struct RawPullHead {
    #[serde(default)]
    pub sha: String,
}

#[derive(Debug, Deserialize)]
pub struct RawPull {
    #[serde(default)]
    pub number: u64,
    #[serde(default)]
    pub head: RawPullHead,
}

#[derive(Debug, Deserialize)]
pub struct RawJobsPage {
    #[serde(default)]
    pub total_count: u64,
    #[serde(default)]
    pub jobs: Vec<RawJob>,
}

#[derive(Debug, Deserialize)]
pub struct RawAnnotation {
    pub path: Option<String>,
    pub start_line: Option<u64>,
    #[serde(default)]
    pub message: String,
}

impl RawAnnotation {
    pub fn into_model(self, job_id: u64) -> Annotation {
        Annotation {
            job_id,
            path: self.path,
            start_line: self.start_line,
            message: self.message,
        }
    }
}

fn epoch() -> Timestamp {
    Timestamp::from_second(0).expect("unix epoch is representable")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completed_run_with_null_conclusion_maps_to_done_other() {
        let raw = RawRun {
            id: 1,
            run_number: 1,
            name: "ci".to_string(),
            display_title: String::new(),
            head_branch: None,
            head_sha: String::new(),
            event: String::new(),
            status: "completed".to_string(),
            conclusion: None,
            actor: None,
            html_url: String::new(),
            created_at: None,
            run_started_at: None,
            updated_at: None,
        };
        let model = raw.into_model();
        assert_eq!(
            model.phase,
            Phase::Done(doneyet_core::model::Conclusion::Other("null".to_string()))
        );
        assert_eq!(model.actor, "");
    }
}
