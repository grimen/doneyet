#![allow(dead_code)]

use doneyet_core::model::{Annotation, Conclusion, Job, Phase, RepoRef, Step, WorkflowRun, World};
use jiff::Timestamp;
use std::io::Write;
use std::sync::{Arc, Mutex};

pub fn ts(s: &str) -> Timestamp {
    s.parse().expect("valid RFC3339 timestamp")
}

pub fn now() -> Timestamp {
    ts("2026-09-21T10:03:13Z")
}

#[derive(Clone, Default)]
pub struct SharedBuf(Arc<Mutex<Vec<u8>>>);

impl SharedBuf {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn take_string(&self) -> String {
        let bytes = std::mem::take(&mut *self.0.lock().unwrap());
        String::from_utf8(bytes).expect("renderer wrote utf-8")
    }
}

impl Write for SharedBuf {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn step(number: u64, name: &str, phase: Phase) -> Step {
    Step {
        number,
        name: name.to_string(),
        phase,
        started_at: None,
        completed_at: None,
    }
}

fn job(
    id: u64,
    name: &str,
    phase: Phase,
    window: Option<(&str, Option<&str>)>,
    steps: Vec<Step>,
) -> Job {
    Job {
        id,
        run_id: 2841,
        name: name.to_string(),
        phase,
        started_at: window.map(|(s, _)| ts(s)),
        completed_at: window.and_then(|(_, e)| e).map(ts),
        runner_name: None,
        labels: Vec::new(),
        steps,
    }
}

pub fn active_world() -> World {
    let run = WorkflowRun {
        id: 2841,
        run_number: 2841,
        name: "ci.yml".to_string(),
        display_title: "build & test".to_string(),
        head_branch: Some("main".to_string()),
        head_sha: "abc123".to_string(),
        event: "push".to_string(),
        phase: Phase::InProgress,
        actor: "jonas".to_string(),
        html_url: "https://github.com/acme/api/actions/runs/2841".to_string(),
        created_at: ts("2026-09-21T10:00:00Z"),
        run_started_at: Some(ts("2026-09-21T10:00:01Z")),
        updated_at: ts("2026-09-21T10:03:12Z"),
    };
    let build = job(
        1,
        "build (linux-x64)",
        Phase::Done(Conclusion::Success),
        Some(("2026-09-21T10:00:02Z", Some("2026-09-21T10:02:03Z"))),
        vec![],
    );
    let test = job(
        2,
        "test (macos-arm64)",
        Phase::Done(Conclusion::Failure),
        Some(("2026-09-21T10:00:02Z", Some("2026-09-21T10:01:04Z"))),
        vec![
            step(1, "Set up job", Phase::Done(Conclusion::Success)),
            step(2, "Run cargo test", Phase::Done(Conclusion::Failure)),
            step(3, "Deploy", Phase::Done(Conclusion::Skipped)),
        ],
    );
    let deploy = job(
        3,
        "deploy (linux-x64)",
        Phase::InProgress,
        Some(("2026-09-21T10:02:32Z", None)),
        vec![step(1, "Roll out", Phase::InProgress)],
    );
    let notify = job(4, "notify (linux-x64)", Phase::Queued, None, vec![]);
    World {
        annotations: Vec::new(),
        stats: None,
        repo: RepoRef {
            owner: "acme".to_string(),
            name: "api".to_string(),
        },
        run,
        jobs: vec![build, test, deploy, notify],
        job_logs: Vec::new(),
    }
}

pub fn failed_world() -> World {
    let mut world = active_world();
    world.run.phase = Phase::Done(Conclusion::Failure);
    world.run.updated_at = now();
    let deploy = Job {
        phase: Phase::Done(Conclusion::Skipped),
        started_at: None,
        completed_at: None,
        steps: Vec::new(),
        ..world.jobs[2].clone()
    };
    let notify = Job {
        phase: Phase::Done(Conclusion::Skipped),
        ..world.jobs[3].clone()
    };
    world.jobs[2] = deploy;
    world.jobs[3] = notify;
    world.annotations = vec![
        Annotation {
            job_id: 2,
            path: Some("src/lib.rs".to_string()),
            start_line: Some(42),
            message: "unused variable: `x`".to_string(),
        },
        Annotation {
            job_id: 2,
            path: None,
            start_line: None,
            message: "process exited with code 1".to_string(),
        },
        Annotation {
            job_id: 1,
            path: Some("benches/odd.rs".to_string()),
            start_line: Some(7),
            message: "healthy jobs never show this".to_string(),
        },
    ];
    world
}
