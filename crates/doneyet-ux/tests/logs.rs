use common::{active_world, now};
use doneyet_core::model::JobLog;
use doneyet_ux::TermRenderer;

mod common;

#[test]
fn frame_appends_job_log_tail_within_width() {
    let mut world = active_world();
    world.job_logs = vec![JobLog {
        job_id: 3,
        lines: vec!["rolling out".to_string(), "x".repeat(80)],
    }];
    let text = TermRenderer::frame_text(&world, false, 40, now());
    assert!(text.contains("rolling out"), "{text}");
    assert!(text.contains("xxx"), "{text}");
    for line in text.lines() {
        assert!(
            unicode_width::UnicodeWidthStr::width(line) <= 40,
            "line too long: {line:?}"
        );
    }
}
