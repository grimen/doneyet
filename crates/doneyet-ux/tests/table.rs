use doneyet_core::model::{Conclusion, Phase, RunsPage, WorkflowRun};
use jiff::Timestamp;

fn ts(s: &str) -> Timestamp {
    s.parse().expect("valid RFC3339 timestamp")
}

fn run(id: u64, number: u64, phase: Phase, branch: &str, title: &str) -> WorkflowRun {
    WorkflowRun {
        id,
        run_number: number,
        name: "ci.yml".to_string(),
        display_title: title.to_string(),
        head_branch: Some(branch.to_string()),
        head_sha: "abc".to_string(),
        event: "push".to_string(),
        phase,
        actor: "jonas".to_string(),
        html_url: format!("https://github.com/acme/api/actions/runs/{id}"),
        created_at: ts("2026-09-21T10:00:00Z"),
        run_started_at: Some(ts("2026-09-21T10:00:01Z")),
        updated_at: ts("2026-09-21T10:03:00Z"),
    }
}

#[test]
fn runs_table_lists_ids_and_branches_within_width() {
    let page = RunsPage {
        total_count: 3,
        runs: vec![
            run(
                2842,
                2842,
                Phase::InProgress,
                "feat/retries",
                "Add retry logic",
            ),
            run(
                2841,
                2841,
                Phase::Done(Conclusion::Success),
                "main",
                "build & test",
            ),
            run(
                2840,
                2840,
                Phase::Done(Conclusion::Failure),
                "main",
                "Fix flaky test",
            ),
        ],
    };
    let now = ts("2026-09-21T10:05:00Z");
    let table = doneyet_ux::runs_table(&page, false, 80, now);
    assert!(table.contains("#2842"), "{table}");
    assert!(table.contains("#2841"), "{table}");
    assert!(table.contains("feat/retries"), "{table}");
    for line in table.lines() {
        assert!(
            unicode_width::UnicodeWidthStr::width(line) <= 80,
            "line too long: {line:?}"
        );
    }
}

#[test]
fn board_frame_counts_in_progress_runs() {
    let page = RunsPage {
        total_count: 3,
        runs: vec![
            run(3, 3, Phase::InProgress, "main", "building"),
            run(2, 2, Phase::Queued, "main", "queued"),
            run(1, 1, Phase::Done(Conclusion::Success), "main", "done"),
        ],
    };
    let frame = doneyet_ux::board_frame(&page, false, 80, ts("2026-09-21T10:05:00Z"));
    let header = frame.lines().next().expect("header");
    assert_eq!(header, "2 running · 3 shown");
    assert!(frame.contains("#3"), "{frame}");
}

#[test]
fn runs_table_color_mode_emits_ansi() {
    let page = RunsPage {
        total_count: 1,
        runs: vec![run(1, 1, Phase::InProgress, "main", "t")],
    };
    let table = doneyet_ux::runs_table(&page, true, 80, ts("2026-09-21T10:05:00Z"));
    assert!(table.contains("\x1b["), "{table}");
}
