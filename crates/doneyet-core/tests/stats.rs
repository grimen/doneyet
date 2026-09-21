use doneyet_core::model::{Conclusion, Phase, WorkflowRun};
use doneyet_core::stats::WorkflowStats;

fn run(id: u64, name: &str, conclusion: Conclusion, duration_secs: i64) -> WorkflowRun {
    let started = "2026-09-21T10:00:00Z".parse::<jiff::Timestamp>().unwrap();
    let completed = started
        .checked_add(jiff::Span::new().seconds(duration_secs))
        .unwrap();
    WorkflowRun {
        id,
        run_number: id,
        name: name.to_string(),
        display_title: String::new(),
        head_branch: None,
        head_sha: String::new(),
        event: "push".to_string(),
        phase: Phase::Done(conclusion),
        actor: String::new(),
        html_url: String::new(),
        created_at: started,
        run_started_at: Some(started),
        updated_at: completed,
    }
}

#[test]
fn empty_history_yields_no_median() {
    let stats = WorkflowStats::from_runs(&[]);
    assert_eq!(stats.median_duration_secs(), None);
    assert_eq!(stats.samples(), 0);
}

#[test]
fn median_over_odd_sample_count() {
    let runs = [
        run(1, "ci", Conclusion::Success, 100),
        run(2, "ci", Conclusion::Success, 300),
        run(3, "ci", Conclusion::Failure, 200),
    ];
    let stats = WorkflowStats::from_runs(&runs);
    assert_eq!(stats.samples(), 3);
    assert_eq!(stats.median_duration_secs(), Some(200));
}

#[test]
fn median_over_even_sample_count_rounds() {
    let runs = [
        run(1, "ci", Conclusion::Success, 100),
        run(2, "ci", Conclusion::Success, 201),
    ];
    let stats = WorkflowStats::from_runs(&runs);
    assert_eq!(stats.median_duration_secs(), Some(151));
}

#[test]
fn censored_and_unfinished_runs_are_excluded() {
    let runs = [
        run(1, "ci", Conclusion::Cancelled, 5),
        run(2, "ci", Conclusion::Skipped, 7),
        run(3, "ci", Conclusion::StartupFailure, 1),
        run(4, "ci", Conclusion::Success, 120),
        run(5, "ci", Conclusion::TimedOut, 240),
    ];
    let stats = WorkflowStats::from_runs(&runs);
    assert_eq!(stats.samples(), 2);
    assert_eq!(stats.median_duration_secs(), Some(180));
}

#[test]
fn in_progress_runs_are_ignored() {
    let mut active = run(9, "ci", Conclusion::Success, 0);
    active.phase = Phase::InProgress;
    let runs = [run(1, "ci", Conclusion::Success, 60), active];
    let stats = WorkflowStats::from_runs(&runs);
    assert_eq!(stats.samples(), 1);
    assert_eq!(stats.median_duration_secs(), Some(60));
}

#[test]
fn remaining_time_derives_from_elapsed() {
    let runs = [run(1, "ci", Conclusion::Success, 200)];
    let stats = WorkflowStats::from_runs(&runs);
    assert_eq!(stats.remaining_secs(40), Some(160));
    assert_eq!(
        stats.remaining_secs(200),
        Some(0),
        "exactly at median still reports zero"
    );
    assert_eq!(stats.remaining_secs(250), None, "past the median: no ETA");
    assert_eq!(WorkflowStats::from_runs(&[]).remaining_secs(10), None);
}

#[test]
fn stats_roundtrip_through_serde() {
    let stats = WorkflowStats::from_runs(&[run(1, "ci", Conclusion::Success, 42)]);
    let json = serde_json::to_string(&stats).unwrap();
    let back: WorkflowStats = serde_json::from_str(&json).unwrap();
    assert_eq!(back, stats);
}
