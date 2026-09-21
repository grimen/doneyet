use common::{SharedBuf, active_world};
use doneyet_core::event::DomainEvent;
use doneyet_core::model::{Conclusion, Outcome, Phase, RunRef, World};
use doneyet_core::ports::{RenderError, Renderer};
use doneyet_ux::record::TeeRenderer;
use doneyet_ux::replay::{ReplayError, ReplayEvent, frame_deltas, parse_records};
use std::time::Duration;

mod common;

struct NullRenderer;

impl Renderer for NullRenderer {
    fn render(&mut self, _world: &World, _events: &[DomainEvent]) -> Result<(), RenderError> {
        Ok(())
    }

    fn finish(&mut self, _outcome: &Outcome) -> Result<(), RenderError> {
        Ok(())
    }
}

fn expected_outcome() -> Outcome {
    Outcome {
        run: RunRef {
            id: 2841,
            name: "ci.yml".to_string(),
        },
        conclusion: Conclusion::Success,
    }
}

#[test]
fn tee_output_parses_back_event_for_event() {
    let sink = SharedBuf::new();
    let mut tee = TeeRenderer::with_writer(Box::new(NullRenderer), sink.clone());
    let world = active_world();
    let events = vec![DomainEvent::RunStarted {
        run: world.run.ref_of(),
    }];
    tee.render(&world, &events).expect("render");
    let mut terminal = world.clone();
    terminal.run.phase = Phase::Done(Conclusion::Success);
    tee.render(&terminal, &[]).expect("render");
    tee.finish(&expected_outcome()).expect("finish");
    drop(tee);

    let parsed = parse_records(&sink.take_string()).expect("records parse");
    assert_eq!(parsed.len(), 3);
    match &parsed[0] {
        ReplayEvent::Render {
            world: w,
            events: e,
            ts,
        } => {
            assert_eq!(w, &world);
            assert_eq!(e, &events);
            assert!(ts.is_some(), "tee stamps wall-clock timestamps");
        }
        other => panic!("expected render, got {other:?}"),
    }
    assert_eq!(
        parsed[1],
        ReplayEvent::Render {
            ts: parsed[1].ts(),
            world: terminal,
            events: Vec::new(),
        }
    );
    match &parsed[2] {
        ReplayEvent::Finish { outcome, ts } => {
            assert_eq!(outcome, &expected_outcome());
            assert!(ts.is_some());
        }
        other => panic!("expected finish, got {other:?}"),
    }
}

#[test]
fn legacy_records_without_ts_parse() {
    let record = doneyet_ux::record::RenderRecord {
        v: 1,
        kind: "render".to_string(),
        ts: None,
        world: active_world(),
        events: Vec::new(),
    };
    let jsonl = serde_json::to_string(&record).expect("serialize without ts");
    assert!(
        !jsonl.contains("\"ts\""),
        "ts must be omitted when absent: {jsonl}"
    );
    let parsed = parse_records(&jsonl).expect("legacy record parses");
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].ts(), None, "missing ts must default to None");
}

#[test]
fn malformed_line_reports_its_line_number() {
    let valid = serde_json::to_string(&doneyet_ux::record::RenderRecord {
        v: 1,
        kind: "render".to_string(),
        ts: None,
        world: active_world(),
        events: Vec::new(),
    })
    .expect("valid first line");
    let jsonl = format!("{valid}\nnot json\n");
    let err = parse_records(&jsonl).expect_err("malformed input");
    match err {
        ReplayError::Malformed { line, .. } => assert_eq!(line, 2),
        other => panic!("expected malformed-line error, got {other:?}"),
    }
}

#[test]
fn unsupported_version_is_rejected() {
    let jsonl = "{\"v\":2,\"kind\":\"render\"}\n";
    let err = parse_records(jsonl).expect_err("future version");
    assert!(
        matches!(err, ReplayError::Version { found: 2, .. }),
        "{err:?}"
    );
}

#[test]
fn unknown_kind_is_rejected() {
    let jsonl = "{\"v\":1,\"kind\":\"weird\"}\n";
    let err = parse_records(jsonl).expect_err("unknown kind");
    assert!(matches!(err, ReplayError::Kind { .. }), "{err:?}");
}

#[test]
fn missing_file_is_an_io_error() {
    let err = doneyet_ux::replay::read_records(std::path::Path::new(
        "/nonexistent/doneyet-recording.jsonl",
    ))
    .expect_err("missing file");
    assert!(matches!(err, ReplayError::Io { .. }), "{err:?}");
}

#[test]
fn frame_deltas_derive_pacing_from_timestamps() {
    let render = |ts: Option<u64>| ReplayEvent::Render {
        ts,
        world: active_world(),
        events: Vec::new(),
    };
    let events = vec![
        render(Some(1_000)),
        render(Some(3_500)),
        ReplayEvent::Finish {
            ts: Some(3_600),
            outcome: expected_outcome(),
        },
    ];
    assert_eq!(
        frame_deltas(&events),
        vec![Duration::from_millis(2_500), Duration::from_millis(100)]
    );
    let missing = vec![render(None), render(Some(9_999))];
    assert_eq!(frame_deltas(&missing), vec![Duration::ZERO]);
    assert!(frame_deltas(&events[..1]).is_empty());
}
