use common::{active_world, failed_world, now};
use doneyet_core::stats::WorkflowStats;
use doneyet_ux::TermRenderer;
use unicode_width::UnicodeWidthStr;

mod common;

fn stats_with_median(seconds: i64) -> WorkflowStats {
    use doneyet_core::model::{Conclusion, Phase, WorkflowRun};
    use jiff::Timestamp;
    let started: Timestamp = "2026-09-21T10:00:00Z".parse().unwrap();
    let finished = started
        .checked_add(jiff::Span::new().seconds(seconds))
        .unwrap();
    let run = WorkflowRun {
        id: 1,
        run_number: 1,
        name: "ci.yml".to_string(),
        display_title: String::new(),
        head_branch: None,
        head_sha: String::new(),
        event: "push".to_string(),
        phase: Phase::Done(Conclusion::Success),
        actor: String::new(),
        html_url: String::new(),
        created_at: started,
        run_started_at: Some(started),
        updated_at: finished,
    };
    WorkflowStats::from_runs(&[run])
}

#[test]
fn running_run_with_stats_shows_eta() {
    let mut world = active_world();
    world.stats = Some(stats_with_median(200));
    let frame = TermRenderer::frame_text(&world, false, 60, now());
    assert!(
        frame.contains("3m12s (~8s left)"),
        "elapsed 3m12s of 3m20s median: {frame}"
    );
}

#[test]
fn no_stats_means_no_eta() {
    let frame = TermRenderer::frame_text(&active_world(), false, 60, now());
    assert!(!frame.contains("(~"), "{frame}");
}

#[test]
fn past_median_omits_eta() {
    let mut world = active_world();
    world.stats = Some(stats_with_median(60));
    let frame = TermRenderer::frame_text(&world, false, 60, now());
    assert!(!frame.contains("(~"), "192s elapsed vs 60s median: {frame}");
}

#[test]
fn finished_runs_never_show_eta() {
    let mut world = failed_world();
    world.stats = Some(stats_with_median(3600));
    let frame = TermRenderer::frame_text(&world, false, 60, now());
    assert!(!frame.contains("(~"), "{frame}");
}

#[test]
fn eta_respects_declared_width() {
    for width in [40usize, 60, 100] {
        let mut world = active_world();
        world.stats = Some(stats_with_median(200));
        let frame = TermRenderer::frame_text(&world, false, width, now());
        for line in frame.lines() {
            assert!(UnicodeWidthStr::width(line) <= width, "{width}: {line:?}");
        }
    }
}
