use common::{SharedBuf, active_world, failed_world, now, ts};
use doneyet_core::model::{Conclusion, Outcome, Phase, RunRef};
use doneyet_core::ports::Renderer;
use doneyet_ux::TermRenderer;

mod common;

fn golden(name: &str, actual: &str) {
    let path = format!("{}/tests/snapshots/{name}.txt", env!("CARGO_MANIFEST_DIR"));
    if std::env::var("UPDATE_SNAPSHOTS").is_ok() {
        if let Some(parent) = std::path::Path::new(&path).parent() {
            std::fs::create_dir_all(parent).expect("create snapshot dir");
        }
        std::fs::write(&path, actual).expect("write golden snapshot");
        return;
    }
    let expected = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("missing golden {path}: {e}; run with UPDATE_SNAPSHOTS=1"));
    assert_eq!(actual, expected, "golden mismatch for {name}");
}

#[test]
fn golden_active_world_color() {
    let frame = TermRenderer::frame_text(&active_world(), true, 60, now());
    golden("active_color", &frame);
}

#[test]
fn golden_active_world_plain() {
    let frame = TermRenderer::frame_text(&active_world(), false, 60, now());
    golden("active_plain", &frame);
    assert!(!frame.contains("\x1b["));
}

#[test]
fn golden_failed_world_plain() {
    let frame = TermRenderer::frame_text(&failed_world(), false, 60, now());
    golden("failed_plain", &frame);
}

#[test]
fn golden_narrow_width_plain() {
    let frame = TermRenderer::frame_text(&active_world(), false, 40, now());
    golden("narrow_plain", &frame);
    for line in frame.lines() {
        assert!(
            unicode_width::UnicodeWidthStr::width(line) <= 40,
            "line exceeds width 40: {line:?}"
        );
    }
}

#[test]
fn second_frame_moves_cursor_up_and_clears_lines() {
    let buf = SharedBuf::new();
    let now = now();
    let mut renderer = TermRenderer::new(Box::new(buf.clone()), false, 60, Box::new(move || now));
    let world = active_world();
    renderer.render(&world, &[]).expect("first render");
    let first = buf.take_string();
    let first_lines = first.lines().count();
    assert!(
        !first.contains("\x1b["),
        "first frame must not move the cursor"
    );

    renderer.render(&world, &[]).expect("second render");
    let second = buf.take_string();
    let prefix = format!("\x1b[{first_lines}F");
    assert!(
        second.starts_with(&prefix),
        "second frame must start with {prefix:?}, got {second:?}"
    );
    assert!(
        second.contains("\x1b[K"),
        "second frame must clear line ends"
    );
}

#[test]
fn shrinked_frame_clears_stale_lines() {
    let buf = SharedBuf::new();
    let now = now();
    let mut renderer = TermRenderer::new(Box::new(buf.clone()), false, 60, Box::new(move || now));
    let big = active_world();
    renderer.render(&big, &[]).expect("big frame");
    let big_lines = buf.take_string().lines().count();

    let mut small = big.clone();
    small.jobs.clear();
    let small_run = small.run.clone();
    small.jobs = vec![doneyet_core::model::Job {
        id: 1,
        run_id: small_run.id,
        name: "only".to_string(),
        phase: Phase::Queued,
        started_at: None,
        completed_at: None,
        runner_name: None,
        labels: Vec::new(),
        steps: Vec::new(),
    }];
    renderer.render(&small, &[]).expect("small frame");
    let out = buf.take_string();
    let newline_count = out.bytes().filter(|b| *b == b'\n').count();
    assert_eq!(newline_count, big_lines, "stale lines must be cleared");
}

#[test]
fn finish_appends_outcome_summary() {
    let buf = SharedBuf::new();
    let now = now();
    let mut renderer = TermRenderer::new(Box::new(buf.clone()), false, 60, Box::new(move || now));
    let world = failed_world();
    renderer.render(&world, &[]).expect("render");
    buf.take_string();
    let outcome = Outcome {
        run: RunRef {
            id: 2841,
            name: "ci.yml".to_string(),
        },
        conclusion: Conclusion::Failure,
    };
    renderer.finish(&outcome).expect("finish");
    let out = buf.take_string();
    let last = out.trim_end();
    assert!(last.ends_with("#2841 failure"), "got {last:?}");
}

#[test]
fn frame_text_is_deterministic() {
    let a = TermRenderer::frame_text(&active_world(), true, 60, ts("2026-09-21T10:04:00Z"));
    let b = TermRenderer::frame_text(&active_world(), true, 60, ts("2026-09-21T10:04:00Z"));
    assert_eq!(a, b);
}
