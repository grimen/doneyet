use doneyet_core::model::{Conclusion, Phase, RepoRef, WorkflowRun};
use std::str::FromStr;

#[test]
fn repo_ref_parses_owner_slash_name() {
    let repo = RepoRef::from_str("acme/api").unwrap();
    assert_eq!(repo.owner, "acme");
    assert_eq!(repo.name, "api");
    assert_eq!(repo.to_string(), "acme/api");
}

#[test]
fn repo_ref_rejects_malformed_refs() {
    for bad in ["", "acme", "acme/", "/api", "a/b/c"] {
        assert!(
            RepoRef::from_str(bad).is_err(),
            "expected {bad:?} to be rejected"
        );
    }
}

#[test]
fn conclusion_parse_maps_known_github_strings() {
    let table = [
        ("success", Conclusion::Success),
        ("failure", Conclusion::Failure),
        ("cancelled", Conclusion::Cancelled),
        ("skipped", Conclusion::Skipped),
        ("timed_out", Conclusion::TimedOut),
        ("startup_failure", Conclusion::StartupFailure),
        ("action_required", Conclusion::ActionRequired),
        ("neutral", Conclusion::Neutral),
        ("stale", Conclusion::Stale),
    ];
    for (raw, expected) in table {
        assert_eq!(Conclusion::parse(raw), expected);
    }
}

#[test]
fn conclusion_parse_unknown_falls_back_to_other() {
    assert_eq!(
        Conclusion::parse("banana"),
        Conclusion::Other("banana".to_string())
    );
}

#[test]
fn conclusion_exit_codes_match_cli_contract() {
    let zero = [
        Conclusion::Success,
        Conclusion::Skipped,
        Conclusion::Neutral,
    ];
    let one = [
        Conclusion::Failure,
        Conclusion::StartupFailure,
        Conclusion::ActionRequired,
        Conclusion::Stale,
        Conclusion::Other("x".to_string()),
    ];
    let two = [Conclusion::Cancelled, Conclusion::TimedOut];
    for c in zero {
        assert_eq!(c.exit_code(), 0, "{c:?}");
    }
    for c in one {
        assert_eq!(c.exit_code(), 1, "{c:?}");
    }
    for c in two {
        assert_eq!(c.exit_code(), 2, "{c:?}");
    }
}

#[test]
fn phase_from_parts_maps_github_status_and_conclusion() {
    let table = [
        (("queued", None), Phase::Queued),
        (("waiting", None), Phase::Waiting),
        (("in_progress", None), Phase::InProgress),
        (
            ("completed", Some("success")),
            Phase::Done(Conclusion::Success),
        ),
        (
            ("completed", Some("timed_out")),
            Phase::Done(Conclusion::TimedOut),
        ),
    ];
    for ((status, conclusion), expected) in table {
        assert_eq!(Phase::from_parts(status, conclusion), expected);
    }
}

#[test]
fn phase_from_parts_handles_null_conclusion_and_unknown_status() {
    assert_eq!(
        Phase::from_parts("completed", None),
        Phase::Done(Conclusion::Other("null".to_string()))
    );
    assert_eq!(
        Phase::from_parts("pending", None),
        Phase::Other("pending".to_string())
    );
}

#[test]
fn phase_terminal_and_active_partitions() {
    assert!(!Phase::Queued.is_terminal());
    assert!(!Phase::Waiting.is_terminal());
    assert!(!Phase::InProgress.is_terminal());
    assert!(!Phase::Other("pending".to_string()).is_terminal());
    assert!(Phase::Done(Conclusion::Success).is_terminal());

    assert!(Phase::Queued.is_active());
    assert!(Phase::Waiting.is_active());
    assert!(Phase::InProgress.is_active());
    assert!(Phase::Other("pending".to_string()).is_active());
    assert!(!Phase::Done(Conclusion::Success).is_active());
}

#[test]
fn phase_conclusion_accessor() {
    assert_eq!(
        Phase::Done(Conclusion::Failure).conclusion(),
        Some(&Conclusion::Failure)
    );
    assert_eq!(Phase::InProgress.conclusion(), None);
}

#[test]
fn workflow_run_roundtrips_through_serde() {
    use doneyet_core::model::WorkflowRun;
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
        created_at: "2026-09-21T10:00:00Z".parse().unwrap(),
        run_started_at: Some("2026-09-21T10:00:01Z".parse().unwrap()),
        updated_at: "2026-09-21T10:03:00Z".parse().unwrap(),
    };
    let json = serde_json::to_string(&run).unwrap();
    let back: WorkflowRun = serde_json::from_str(&json).unwrap();
    assert_eq!(back, run);
}

#[test]
fn world_parses_legacy_recordings_without_annotations() {
    use doneyet_core::model::{Annotation, Job, Phase, World};
    let world = World {
        repo: RepoRef {
            owner: "acme".to_string(),
            name: "api".to_string(),
        },
        run: {
            let mut run = super_run();
            run.phase = Phase::Done(Conclusion::Failure);
            run
        },
        jobs: vec![Job {
            id: 7,
            run_id: run_id_of(&super_run()),
            name: "test".to_string(),
            phase: Phase::Done(Conclusion::Failure),
            started_at: None,
            completed_at: None,
            runner_name: None,
            labels: Vec::new(),
            steps: Vec::new(),
        }],
        stats: None,
        annotations: vec![Annotation {
            job_id: 7,
            path: Some("src/lib.rs".to_string()),
            start_line: Some(42),
            message: "unused variable".to_string(),
        }],
    };
    let mut value = serde_json::to_value(&world).unwrap();
    value.as_object_mut().unwrap().remove("annotations");
    let legacy = serde_json::to_string(&value).unwrap();
    let parsed: World = serde_json::from_str(&legacy).expect("legacy world parses");
    assert_eq!(
        parsed,
        World {
            annotations: Vec::new(),
            ..world
        }
    );
}

fn super_run() -> WorkflowRun {
    WorkflowRun {
        id: 2841,
        run_number: 2841,
        name: "ci.yml".to_string(),
        display_title: "build & test".to_string(),
        head_branch: Some("main".to_string()),
        head_sha: "abc".to_string(),
        event: "push".to_string(),
        phase: Phase::InProgress,
        actor: "jonas".to_string(),
        html_url: "https://github.com/acme/api/actions/runs/2841".to_string(),
        created_at: "2026-09-21T10:00:00Z".parse().unwrap(),
        run_started_at: Some("2026-09-21T10:00:01Z".parse().unwrap()),
        updated_at: "2026-09-21T10:03:00Z".parse().unwrap(),
    }
}

fn run_id_of(run: &WorkflowRun) -> u64 {
    run.id
}

#[test]
fn phase_is_failed_matches_failure_conclusions_only() {
    assert!(Phase::Done(Conclusion::Failure).is_failed());
    assert!(Phase::Done(Conclusion::TimedOut).is_failed());
    assert!(Phase::Done(Conclusion::StartupFailure).is_failed());
    assert!(!Phase::Done(Conclusion::Success).is_failed());
    assert!(!Phase::Done(Conclusion::Cancelled).is_failed());
    assert!(!Phase::Done(Conclusion::Skipped).is_failed());
    assert!(!Phase::InProgress.is_failed());
    assert!(!Phase::Queued.is_failed());
}
