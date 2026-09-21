use doneyet_core::diff::diff_worlds;
use doneyet_core::event::DomainEvent;
use doneyet_core::model::{
    Conclusion, Job, JobRef, Phase, RepoRef, RunRef, Step, StepRef, WorkflowRun, World,
};
use jiff::Timestamp;

fn repo() -> RepoRef {
    RepoRef {
        owner: "acme".to_string(),
        name: "api".to_string(),
    }
}

fn run_ref() -> RunRef {
    RunRef {
        id: 2841,
        name: "ci.yml".to_string(),
    }
}

fn job_ref(id: u64, name: &str) -> JobRef {
    JobRef {
        id,
        run_id: 2841,
        name: name.to_string(),
    }
}

fn step_ref(job_id: u64, number: u64, name: &str) -> StepRef {
    StepRef {
        job_id,
        number,
        name: name.to_string(),
    }
}

fn run(phase: Phase) -> WorkflowRun {
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
        created_at: "2026-09-21T10:00:00Z".parse::<Timestamp>().unwrap(),
        run_started_at: None,
        updated_at: "2026-09-21T10:00:00Z".parse::<Timestamp>().unwrap(),
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

fn job(id: u64, name: &str, phase: Phase, steps: Vec<Step>) -> Job {
    Job {
        id,
        run_id: 2841,
        name: name.to_string(),
        phase,
        started_at: None,
        completed_at: None,
        runner_name: None,
        labels: Vec::new(),
        steps,
    }
}

fn world(run_phase: Phase, jobs: Vec<Job>) -> World {
    World {
        repo: repo(),
        run: run(run_phase),
        jobs,
        annotations: Vec::new(),
        stats: None,
    }
}

#[test]
fn first_snapshot_active_run_starts_then_jobs_appear() {
    let next = world(
        Phase::InProgress,
        vec![
            job(
                1,
                "build (linux)",
                Phase::InProgress,
                vec![step(1, "Compile", Phase::InProgress)],
            ),
            job(2, "test (linux)", Phase::Queued, vec![]),
        ],
    );
    let events = diff_worlds(None, &next);
    assert_eq!(
        events,
        vec![
            DomainEvent::RunStarted { run: run_ref() },
            DomainEvent::JobAppeared {
                job: job_ref(1, "build (linux)")
            },
            DomainEvent::JobStarted {
                job: job_ref(1, "build (linux)")
            },
            DomainEvent::JobAppeared {
                job: job_ref(2, "test (linux)")
            },
        ]
    );
}

#[test]
fn first_snapshot_completed_run_reports_completion_only() {
    let next = world(
        Phase::Done(Conclusion::Success),
        vec![job(
            1,
            "build (linux)",
            Phase::Done(Conclusion::Success),
            vec![],
        )],
    );
    let events = diff_worlds(None, &next);
    assert_eq!(
        events,
        vec![
            DomainEvent::RunCompleted {
                run: run_ref(),
                conclusion: Conclusion::Success,
            },
            DomainEvent::JobAppeared {
                job: job_ref(1, "build (linux)")
            },
        ]
    );
}

#[test]
fn unchanged_world_emits_nothing() {
    let w = world(
        Phase::InProgress,
        vec![job(1, "build", Phase::InProgress, vec![])],
    );
    assert_eq!(diff_worlds(Some(&w), &w), Vec::<DomainEvent>::new());
}

#[test]
fn job_queued_to_in_progress_starts() {
    let prev = world(
        Phase::InProgress,
        vec![job(1, "build", Phase::Queued, vec![])],
    );
    let next = world(
        Phase::InProgress,
        vec![job(1, "build", Phase::InProgress, vec![])],
    );
    assert_eq!(
        diff_worlds(Some(&prev), &next),
        vec![DomainEvent::JobStarted {
            job: job_ref(1, "build")
        }]
    );
}

#[test]
fn step_finishes_before_its_job_completes() {
    let prev = world(
        Phase::InProgress,
        vec![job(
            1,
            "test",
            Phase::InProgress,
            vec![step(2, "Run cargo test", Phase::InProgress)],
        )],
    );
    let next = world(
        Phase::InProgress,
        vec![job(
            1,
            "test",
            Phase::Done(Conclusion::Failure),
            vec![step(2, "Run cargo test", Phase::Done(Conclusion::Failure))],
        )],
    );
    assert_eq!(
        diff_worlds(Some(&prev), &next),
        vec![
            DomainEvent::StepFinished {
                job: job_ref(1, "test"),
                step: step_ref(1, 2, "Run cargo test"),
                conclusion: Conclusion::Failure,
            },
            DomainEvent::JobCompleted {
                job: job_ref(1, "test"),
                conclusion: Conclusion::Failure,
            },
        ]
    );
}

#[test]
fn run_completion_follows_job_completions() {
    let prev = world(
        Phase::InProgress,
        vec![job(1, "deploy", Phase::InProgress, vec![])],
    );
    let next = world(
        Phase::Done(Conclusion::Success),
        vec![job(1, "deploy", Phase::Done(Conclusion::Success), vec![])],
    );
    assert_eq!(
        diff_worlds(Some(&prev), &next),
        vec![
            DomainEvent::JobCompleted {
                job: job_ref(1, "deploy"),
                conclusion: Conclusion::Success,
            },
            DomainEvent::RunCompleted {
                run: run_ref(),
                conclusion: Conclusion::Success,
            },
        ]
    );
}

#[test]
fn step_queued_to_in_progress_starts() {
    let prev = world(
        Phase::InProgress,
        vec![job(
            1,
            "build",
            Phase::InProgress,
            vec![step(1, "Compile", Phase::Queued)],
        )],
    );
    let next = world(
        Phase::InProgress,
        vec![job(
            1,
            "build",
            Phase::InProgress,
            vec![step(1, "Compile", Phase::InProgress)],
        )],
    );
    assert_eq!(
        diff_worlds(Some(&prev), &next),
        vec![DomainEvent::StepStarted {
            job: job_ref(1, "build"),
            step: step_ref(1, 1, "Compile"),
        }]
    );
}

#[test]
fn matrix_job_appearing_and_sibling_completing_mid_run() {
    let prev = world(
        Phase::InProgress,
        vec![job(1, "build", Phase::InProgress, vec![])],
    );
    let next = world(
        Phase::InProgress,
        vec![
            job(1, "build", Phase::Done(Conclusion::Success), vec![]),
            job(2, "test (macos)", Phase::InProgress, vec![]),
        ],
    );
    assert_eq!(
        diff_worlds(Some(&prev), &next),
        vec![
            DomainEvent::JobCompleted {
                job: job_ref(1, "build"),
                conclusion: Conclusion::Success,
            },
            DomainEvent::JobAppeared {
                job: job_ref(2, "test (macos)")
            },
            DomainEvent::JobStarted {
                job: job_ref(2, "test (macos)")
            },
        ]
    );
}

#[test]
fn job_rerun_resets_to_requeued() {
    let prev = world(
        Phase::Done(Conclusion::Failure),
        vec![job(1, "test", Phase::Done(Conclusion::Failure), vec![])],
    );
    let next = world(Phase::Queued, vec![job(1, "test", Phase::Queued, vec![])]);
    assert_eq!(
        diff_worlds(Some(&prev), &next),
        vec![
            DomainEvent::JobRequeued {
                job: job_ref(1, "test")
            },
            DomainEvent::RunRequeued { run: run_ref() },
        ]
    );
}

#[test]
fn conclusion_change_between_terminal_states_recompletes() {
    let prev = world(
        Phase::InProgress,
        vec![job(1, "test", Phase::Done(Conclusion::Failure), vec![])],
    );
    let next = world(
        Phase::InProgress,
        vec![job(1, "test", Phase::Done(Conclusion::Success), vec![])],
    );
    assert_eq!(
        diff_worlds(Some(&prev), &next),
        vec![DomainEvent::JobCompleted {
            job: job_ref(1, "test"),
            conclusion: Conclusion::Success,
        }]
    );
}

#[test]
fn removed_job_is_ignored() {
    let prev = world(
        Phase::InProgress,
        vec![
            job(1, "build", Phase::Done(Conclusion::Success), vec![]),
            job(2, "old", Phase::Queued, vec![]),
        ],
    );
    let next = world(
        Phase::InProgress,
        vec![job(1, "build", Phase::Done(Conclusion::Success), vec![])],
    );
    assert_eq!(diff_worlds(Some(&prev), &next), Vec::<DomainEvent>::new());
}

#[test]
fn unknown_phase_is_active_and_can_complete() {
    let prev = world(Phase::Other("pending".to_string()), vec![]);
    let same = world(Phase::Other("pending".to_string()), vec![]);
    assert_eq!(diff_worlds(Some(&prev), &same), Vec::<DomainEvent>::new());

    let next = world(Phase::Done(Conclusion::Success), vec![]);
    assert_eq!(
        diff_worlds(Some(&prev), &next),
        vec![DomainEvent::RunCompleted {
            run: run_ref(),
            conclusion: Conclusion::Success,
        }]
    );
}

#[test]
fn direct_done_to_in_progress_after_rerun_starts_job() {
    let prev = world(
        Phase::Queued,
        vec![job(1, "test", Phase::Done(Conclusion::Failure), vec![])],
    );
    let next = world(
        Phase::Queued,
        vec![job(1, "test", Phase::InProgress, vec![])],
    );
    assert_eq!(
        diff_worlds(Some(&prev), &next),
        vec![DomainEvent::JobStarted {
            job: job_ref(1, "test")
        }]
    );
}
