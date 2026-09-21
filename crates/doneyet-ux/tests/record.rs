use common::{SharedBuf, active_world, now};
use doneyet_core::event::DomainEvent;
use doneyet_core::model::{Conclusion, Outcome, Phase, RunRef, World};
use doneyet_core::ports::Renderer;
use doneyet_ux::record::{JsonlRenderer, TeeRenderer};
use serde::Deserialize;
use std::io::Write;
use std::sync::{Arc, Mutex};

mod common;

type FrameLog = Vec<(World, Vec<DomainEvent>)>;

#[derive(Clone, Default)]
struct CountingRenderer {
    frames: Arc<Mutex<FrameLog>>,
    finishes: Arc<Mutex<Vec<Outcome>>>,
}

impl CountingRenderer {
    fn frames(&self) -> FrameLog {
        self.frames.lock().expect("frames poisoned").clone()
    }

    fn finishes(&self) -> Vec<Outcome> {
        self.finishes.lock().expect("finishes poisoned").clone()
    }
}

impl Renderer for CountingRenderer {
    fn render(
        &mut self,
        world: &World,
        events: &[DomainEvent],
    ) -> Result<(), doneyet_core::ports::RenderError> {
        self.frames
            .lock()
            .expect("frames poisoned")
            .push((world.clone(), events.to_vec()));
        Ok(())
    }

    fn finish(&mut self, outcome: &Outcome) -> Result<(), doneyet_core::ports::RenderError> {
        self.finishes
            .lock()
            .expect("finishes poisoned")
            .push(outcome.clone());
        Ok(())
    }
}

#[derive(Deserialize)]
struct RenderLine {
    v: u8,
    kind: String,
    world: World,
    events: Vec<DomainEvent>,
}

#[derive(Deserialize)]
struct FinishLine {
    v: u8,
    kind: String,
    outcome: Outcome,
}

fn outcome() -> Outcome {
    Outcome {
        run: RunRef {
            id: 2841,
            name: "ci.yml".to_string(),
        },
        conclusion: Conclusion::Success,
    }
}

struct FailingWriter;

impl Write for FailingWriter {
    fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
        Err(std::io::Error::other("disk full"))
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Err(std::io::Error::other("disk full"))
    }
}

#[test]
fn forwards_to_primary_and_records_jsonl() {
    let primary = CountingRenderer::default();
    let sink = SharedBuf::new();
    let mut tee = TeeRenderer::with_writer(Box::new(primary.clone()), sink.clone());

    let world = active_world();
    let first_events = vec![DomainEvent::RunStarted {
        run: world.run.ref_of(),
    }];
    tee.render(&world, &first_events).expect("render 1");

    let mut terminal = world.clone();
    terminal.run.phase = Phase::Done(Conclusion::Success);
    let final_events = vec![DomainEvent::RunCompleted {
        run: terminal.run.ref_of(),
        conclusion: Conclusion::Success,
    }];
    tee.render(&terminal, &final_events).expect("render 2");
    let final_outcome = outcome();
    tee.finish(&final_outcome).expect("finish");

    assert_eq!(primary.frames().len(), 2, "primary must see every frame");
    assert_eq!(primary.finishes(), vec![final_outcome.clone()]);

    let recorded = sink.take_string();
    let lines: Vec<&str> = recorded.lines().collect();
    assert_eq!(lines.len(), 3, "one JSONL line per call: {recorded:?}");

    let first: RenderLine = serde_json::from_str(lines[0]).expect("line 1 parses");
    assert_eq!(first.v, 1);
    assert_eq!(first.kind, "render");
    assert_eq!(first.world, world);
    assert_eq!(first.events, first_events);

    let second: RenderLine = serde_json::from_str(lines[1]).expect("line 2 parses");
    assert_eq!(second.world, terminal);
    assert_eq!(second.events, final_events);

    let last: FinishLine = serde_json::from_str(lines[2]).expect("line 3 parses");
    assert_eq!(last.v, 1);
    assert_eq!(last.kind, "finish");
    assert_eq!(last.outcome, final_outcome);
}

#[test]
fn sink_failure_disables_recording_but_not_rendering() {
    let primary = CountingRenderer::default();
    let mut tee = TeeRenderer::with_writer(Box::new(primary.clone()), FailingWriter);
    let world = active_world();
    tee.render(&world, &[])
        .expect("render with failing sink must succeed");
    tee.render(&world, &[]).expect("second render must succeed");
    tee.finish(&outcome()).expect("finish must succeed");
    assert_eq!(
        primary.frames().len(),
        2,
        "rendering continues after sink death"
    );
    assert_eq!(
        primary.finishes().len(),
        1,
        "finishing continues after sink death"
    );
}

#[test]
fn file_constructor_creates_readable_recording() {
    let path = std::env::temp_dir().join(format!("doneyet-record-{}.jsonl", std::process::id()));
    let primary = CountingRenderer::default();
    {
        let mut tee = TeeRenderer::new(Box::new(primary), &path).expect("open recording file");
        tee.render(&active_world(), &[]).expect("render");
        tee.finish(&outcome()).expect("finish");
    }
    let contents = std::fs::read_to_string(&path).expect("read recording back");
    assert!(contents.contains("\"kind\":\"render\""), "{contents}");
    assert!(contents.contains("\"kind\":\"finish\""), "{contents}");
    assert!(contents.contains("\"v\":1"), "{contents}");
    std::fs::remove_file(&path).ok();
}

#[test]
fn recorded_frames_ignore_wall_clock() {
    let primary = CountingRenderer::default();
    let sink = SharedBuf::new();
    let mut tee = TeeRenderer::with_writer(Box::new(primary), sink.clone());
    let world = active_world();
    tee.render(&world, &[]).expect("render");
    drop(tee);
    let recorded = sink.take_string();
    let first: RenderLine = serde_json::from_str(recorded.lines().next().expect("one line"))
        .expect("deterministic schema");
    assert_eq!(
        first.world.run.created_at,
        "2026-09-21T10:00:00Z".parse::<jiff::Timestamp>().unwrap()
    );
    let _ = now();
}

#[test]
fn jsonl_renderer_emits_same_schema_without_a_primary() {
    let sink = SharedBuf::new();
    let mut jsonl = JsonlRenderer::new(sink.clone());
    let world = active_world();
    let events = vec![DomainEvent::RunStarted {
        run: world.run.ref_of(),
    }];
    jsonl.render(&world, &events).expect("render");
    let final_outcome = outcome();
    jsonl.finish(&final_outcome).expect("finish");
    drop(jsonl);

    let recorded = sink.take_string();
    let lines: Vec<&str> = recorded.lines().collect();
    assert_eq!(lines.len(), 2, "one JSONL line per call: {recorded:?}");

    let render: RenderLine = serde_json::from_str(lines[0]).expect("line 1 parses");
    assert_eq!(render.v, 1);
    assert_eq!(render.kind, "render");
    assert_eq!(render.world, world);
    assert_eq!(render.events, events);

    let finish: FinishLine = serde_json::from_str(lines[1]).expect("line 2 parses");
    assert_eq!(finish.v, 1);
    assert_eq!(finish.kind, "finish");
    assert_eq!(finish.outcome, final_outcome);
}
