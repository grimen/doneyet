mod common;

use common::{
    FakeProvider, ImmediatePush, RecordingRenderer, Step0, hint, job, repo, success_outcome, world,
};
use doneyet_app::{NoopPushSource, WatchConfig, WatchEngine, WatchOutcome, WatchTarget};
use doneyet_core::event::DomainEvent;
use doneyet_core::model::{Conclusion, Phase, RunsQuery};
use std::time::Duration;
use tokio_util::sync::CancellationToken;

fn config() -> WatchConfig {
    WatchConfig {
        active_interval: Duration::from_secs(3),
        idle_interval: Duration::from_secs(20),
        log_tail: None,
        log_grep: None,
    }
}

fn query() -> RunsQuery {
    RunsQuery {
        repo: repo(),
        branch: None,
        head_sha: None,
        event: None,
        limit: 1,
    }
}

fn active_world() -> doneyet_core::model::World {
    world(Phase::InProgress, vec![job(1, "build", Phase::InProgress)])
}

fn terminal_world() -> doneyet_core::model::World {
    world(
        Phase::Done(Conclusion::Success),
        vec![job(1, "build", Phase::Done(Conclusion::Success))],
    )
}

#[tokio::test(start_paused = true)]
async fn renders_first_snapshot_then_completes() {
    let provider = FakeProvider::new(vec![
        Step0::World(active_world()),
        Step0::World(terminal_world()),
    ]);
    let renderer = RecordingRenderer::default();
    let mut engine = WatchEngine::new(
        repo(),
        Box::new(provider),
        Box::new(renderer.clone()),
        Box::new(NoopPushSource),
        config(),
        CancellationToken::new(),
    );
    let outcome = engine
        .watch(WatchTarget::Run(2841))
        .await
        .expect("watch should succeed");
    assert!(matches!(
        outcome,
        WatchOutcome::Completed(Conclusion::Success)
    ));
    let frames = renderer.frames();
    assert_eq!(frames.len(), 2, "one frame per fetch");
    assert_eq!(
        frames[0].1,
        vec![
            DomainEvent::RunStarted {
                run: frames[0].0.run.ref_of()
            },
            DomainEvent::JobAppeared {
                job: frames[0].0.jobs[0].ref_of()
            },
            DomainEvent::JobStarted {
                job: frames[0].0.jobs[0].ref_of()
            },
        ]
    );
    assert_eq!(
        frames[1].1,
        vec![
            DomainEvent::JobCompleted {
                job: frames[1].0.jobs[0].ref_of(),
                conclusion: Conclusion::Success,
            },
            DomainEvent::RunCompleted {
                run: frames[1].0.run.ref_of(),
                conclusion: Conclusion::Success,
            },
        ]
    );
    assert_eq!(renderer.finishes(), vec![success_outcome()]);
}

#[tokio::test(start_paused = true)]
async fn exits_on_first_snapshot_if_already_terminal() {
    let provider = FakeProvider::new(vec![Step0::World(terminal_world())]);
    let renderer = RecordingRenderer::default();
    let mut engine = WatchEngine::new(
        repo(),
        Box::new(provider),
        Box::new(renderer.clone()),
        Box::new(NoopPushSource),
        config(),
        CancellationToken::new(),
    );
    let outcome = engine
        .watch(WatchTarget::Run(2841))
        .await
        .expect("terminal watch should succeed");
    assert!(matches!(
        outcome,
        WatchOutcome::Completed(Conclusion::Success)
    ));
    assert_eq!(renderer.frames().len(), 1);
    assert_eq!(renderer.finishes().len(), 1);
}

#[tokio::test(start_paused = true)]
async fn keeps_polling_until_terminal() {
    let provider = FakeProvider::new(vec![
        Step0::World(active_world()),
        Step0::World(active_world()),
        Step0::World(terminal_world()),
    ]);
    let handle = provider.handle();
    let renderer = RecordingRenderer::default();
    let mut engine = WatchEngine::new(
        repo(),
        Box::new(provider),
        Box::new(renderer.clone()),
        Box::new(NoopPushSource),
        config(),
        CancellationToken::new(),
    );
    let outcome = engine
        .watch(WatchTarget::Run(2841))
        .await
        .expect("watch should succeed");
    assert!(matches!(
        outcome,
        WatchOutcome::Completed(Conclusion::Success)
    ));
    assert_eq!(
        handle.fetches(),
        3,
        "engine must poll once per interval while active"
    );
}

#[tokio::test(start_paused = true)]
async fn push_hint_preempts_poll_interval() {
    let provider = FakeProvider::new(vec![
        Step0::World(active_world()),
        Step0::World(active_world()),
        Step0::World(terminal_world()),
    ]);
    let handle = provider.handle();
    let renderer = RecordingRenderer::default();
    let mut engine = WatchEngine::new(
        repo(),
        Box::new(provider),
        Box::new(renderer),
        Box::new(ImmediatePush::with_hints(vec![hint()])),
        config(),
        CancellationToken::new(),
    );
    let started = tokio::time::Instant::now();
    let outcome = engine
        .watch(WatchTarget::Run(2841))
        .await
        .expect("watch should succeed");
    let elapsed = started.elapsed();
    assert!(matches!(
        outcome,
        WatchOutcome::Completed(Conclusion::Success)
    ));
    let calls = handle.locate_calls();
    assert_eq!(calls.len(), 3);
    let gap = calls[1] - calls[0];
    assert!(
        gap < Duration::from_secs(1),
        "push hint must preempt the 3s interval, gap was {gap:?}"
    );
    assert!(
        elapsed >= Duration::from_secs(3),
        "third fetch still waits for the interval, elapsed {elapsed:?}"
    );
}

#[tokio::test(start_paused = true)]
async fn cancellation_returns_interrupted() {
    let provider = FakeProvider::new(vec![
        Step0::World(active_world()),
        Step0::World(terminal_world()),
    ]);
    let handle = provider.handle();
    let renderer = RecordingRenderer::default();
    let shutdown = CancellationToken::new();
    let cancel = shutdown.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(1)).await;
        cancel.cancel();
    });
    let mut engine = WatchEngine::new(
        repo(),
        Box::new(provider),
        Box::new(renderer),
        Box::new(NoopPushSource),
        config(),
        shutdown,
    );
    let outcome = engine
        .watch(WatchTarget::Run(2841))
        .await
        .expect("cancelled watch is not an error");
    assert!(matches!(outcome, WatchOutcome::Interrupted));
    assert_eq!(handle.fetches(), 1, "no polling after cancellation");
}

#[tokio::test(start_paused = true)]
async fn latest_target_waits_until_run_appears() {
    let provider = FakeProvider::new(vec![
        Step0::NoRun,
        Step0::World(active_world()),
        Step0::World(terminal_world()),
    ]);
    let handle = provider.handle();
    let renderer = RecordingRenderer::default();
    let mut engine = WatchEngine::new(
        repo(),
        Box::new(provider),
        Box::new(renderer.clone()),
        Box::new(NoopPushSource),
        config(),
        CancellationToken::new(),
    );
    let outcome = engine
        .watch(WatchTarget::Latest(query()))
        .await
        .expect("watch should succeed");
    assert!(matches!(
        outcome,
        WatchOutcome::Completed(Conclusion::Success)
    ));
    assert_eq!(handle.fetches(), 3);
    assert_eq!(
        renderer.frames().len(),
        2,
        "no frame while no run exists yet"
    );
}

#[tokio::test(start_paused = true)]
async fn push_failure_degrades_to_polling() {
    let provider = FakeProvider::new(vec![
        Step0::World(active_world()),
        Step0::World(active_world()),
        Step0::World(terminal_world()),
    ]);
    let handle = provider.handle();
    let renderer = RecordingRenderer::default();
    let started = tokio::time::Instant::now();
    let mut engine = WatchEngine::new(
        repo(),
        Box::new(provider),
        Box::new(renderer),
        Box::new(common::DyingPush),
        config(),
        CancellationToken::new(),
    );
    let outcome = engine
        .watch(WatchTarget::Run(2841))
        .await
        .expect("a dead push source must not kill the watch");
    assert!(matches!(
        outcome,
        WatchOutcome::Completed(Conclusion::Success)
    ));
    assert_eq!(
        handle.fetches(),
        3,
        "engine must finish via polling after push death"
    );
    assert!(
        started.elapsed() >= Duration::from_secs(3),
        "poll interval must still be honored after push death"
    );
}

#[tokio::test(start_paused = true)]
async fn provider_error_aborts_watch() {
    let provider = FakeProvider::new(vec![Step0::Fail("upstream exploded".to_string())]);
    let renderer = RecordingRenderer::default();
    let mut engine = WatchEngine::new(
        repo(),
        Box::new(provider),
        Box::new(renderer),
        Box::new(NoopPushSource),
        config(),
        CancellationToken::new(),
    );
    let err = engine
        .watch(WatchTarget::Run(2841))
        .await
        .expect_err("provider failure must abort");
    assert!(err.to_string().contains("upstream exploded"), "{err}");
}

#[tokio::test(start_paused = true)]
async fn terminal_run_with_failed_conclusion_maps_exit_code() {
    let failed_world = world(
        Phase::Done(Conclusion::Failure),
        vec![job(1, "test", Phase::Done(Conclusion::Failure))],
    );
    let provider = FakeProvider::new(vec![Step0::World(failed_world)]);
    let renderer = RecordingRenderer::default();
    let mut engine = WatchEngine::new(
        repo(),
        Box::new(provider),
        Box::new(renderer),
        Box::new(NoopPushSource),
        config(),
        CancellationToken::new(),
    );
    let outcome = engine
        .watch(WatchTarget::Latest(query()))
        .await
        .expect("watch should succeed");
    match outcome {
        WatchOutcome::Completed(conclusion) => {
            assert_eq!(conclusion, Conclusion::Failure);
            assert_eq!(conclusion.exit_code(), 1);
        }
        other => panic!("expected completion, got {other:?}"),
    }
}

#[test]
fn outcome_exit_codes() {
    assert_eq!(WatchOutcome::Completed(Conclusion::Success).exit_code(), 0);
    assert_eq!(
        WatchOutcome::Completed(Conclusion::Cancelled).exit_code(),
        2
    );
    assert_eq!(WatchOutcome::Interrupted.exit_code(), 130);
}

#[tokio::test(start_paused = true)]
async fn watch_enriches_frames_with_workflow_stats() {
    let history = vec![
        common::run(doneyet_core::model::Phase::Done(Conclusion::Success)),
        common::run(doneyet_core::model::Phase::Done(Conclusion::Success)),
    ];
    let provider = FakeProvider::with_history(
        vec![Step0::World(active_world()), Step0::World(terminal_world())],
        history,
    );
    let renderer = RecordingRenderer::default();
    let mut engine = WatchEngine::new(
        repo(),
        Box::new(provider),
        Box::new(renderer.clone()),
        Box::new(NoopPushSource),
        config(),
        CancellationToken::new(),
    );
    let outcome = engine
        .watch(WatchTarget::Run(2841))
        .await
        .expect("watch succeeds");
    assert!(matches!(
        outcome,
        WatchOutcome::Completed(Conclusion::Success)
    ));
    let frames = renderer.frames();
    assert_eq!(frames.len(), 2);
    let stats = frames[0].0.stats.as_ref().expect("stats on first frame");
    assert_eq!(stats.median_duration_secs(), Some(179));
    assert!(frames[1].0.stats.is_some(), "stats persist across frames");
}

#[tokio::test(start_paused = true)]
async fn missing_history_still_watches_without_stats() {
    let provider = FakeProvider::new(vec![
        Step0::World(active_world()),
        Step0::World(terminal_world()),
    ]);
    let renderer = RecordingRenderer::default();
    let mut engine = WatchEngine::new(
        repo(),
        Box::new(provider),
        Box::new(renderer.clone()),
        Box::new(NoopPushSource),
        config(),
        CancellationToken::new(),
    );
    let outcome = engine
        .watch(WatchTarget::Latest(query()))
        .await
        .expect("watch succeeds without history");
    assert!(matches!(
        outcome,
        WatchOutcome::Completed(Conclusion::Success)
    ));
    let frames = renderer.frames();
    assert!(frames[0].0.stats.is_none());
}

#[tokio::test(start_paused = true)]
async fn channel_push_delivers_hints_and_preempts_polling() {
    use doneyet_app::ChannelPushSource;

    let provider = FakeProvider::new(vec![
        Step0::World(active_world()),
        Step0::World(active_world()),
        Step0::World(terminal_world()),
    ]);
    let handle = provider.handle();
    let renderer = RecordingRenderer::default();
    let (hints, push) = ChannelPushSource::channel();
    let started = tokio::time::Instant::now();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(1)).await;
        let _ = hints.send(hint());
        tokio::time::sleep(Duration::from_secs(60)).await;
        let _ = hints;
    });
    let mut engine = WatchEngine::new(
        repo(),
        Box::new(provider),
        Box::new(renderer),
        Box::new(push),
        config(),
        CancellationToken::new(),
    );
    let outcome = engine
        .watch(WatchTarget::Run(2841))
        .await
        .expect("watch succeeds");
    assert!(matches!(
        outcome,
        WatchOutcome::Completed(Conclusion::Success)
    ));
    let calls = handle.locate_calls();
    assert_eq!(calls.len(), 3);
    let gap = calls[1] - calls[0];
    assert!(
        gap < Duration::from_secs(1),
        "channel hint must preempt the interval, gap was {gap:?}"
    );
    assert!(started.elapsed() >= Duration::from_secs(3));
}

#[tokio::test(start_paused = true)]
async fn closed_channel_push_degrades_to_polling() {
    use doneyet_app::ChannelPushSource;

    let provider = FakeProvider::new(vec![
        Step0::World(active_world()),
        Step0::World(terminal_world()),
    ]);
    let renderer = RecordingRenderer::default();
    let (hints, push) = ChannelPushSource::channel();
    drop(hints);
    let mut engine = WatchEngine::new(
        repo(),
        Box::new(provider),
        Box::new(renderer),
        Box::new(push),
        config(),
        CancellationToken::new(),
    );
    let outcome = engine
        .watch(WatchTarget::Run(2841))
        .await
        .expect("a closed push channel must not kill the watch");
    assert!(matches!(
        outcome,
        WatchOutcome::Completed(Conclusion::Success)
    ));
}

struct PageSink {
    pages: std::sync::Arc<std::sync::Mutex<Vec<doneyet_core::model::RunsPage>>>,
    cancel: CancellationToken,
    stop_after: usize,
}

impl doneyet_app::BoardSink for PageSink {
    fn render_page(
        &mut self,
        page: &doneyet_core::model::RunsPage,
    ) -> Result<(), doneyet_core::ports::RenderError> {
        let mut pages = self.pages.lock().expect("pages poisoned");
        pages.push(page.clone());
        if pages.len() >= self.stop_after {
            self.cancel.cancel();
        }
        Ok(())
    }
}

fn dash_query() -> RunsQuery {
    RunsQuery {
        repo: repo(),
        branch: None,
        head_sha: None,
        event: None,
        limit: 5,
    }
}

fn dash_config() -> doneyet_app::DashConfig {
    doneyet_app::DashConfig {
        interval: Duration::from_secs(3),
    }
}

#[tokio::test(start_paused = true)]
async fn dash_renders_each_page_until_cancel() {
    use doneyet_app::{DashEngine, DashOutcome};

    let provider = FakeProvider::new(vec![
        Step0::World(active_world()),
        Step0::World(terminal_world()),
    ]);
    let pages = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let shutdown = CancellationToken::new();
    let mut sink = PageSink {
        pages: pages.clone(),
        cancel: shutdown.clone(),
        stop_after: 2,
    };
    let mut engine = DashEngine::new(
        Box::new(provider),
        Box::new(NoopPushSource),
        dash_config(),
        shutdown,
    );
    let outcome = engine
        .run(dash_query(), &mut sink)
        .await
        .expect("cancel is not an error");
    assert!(matches!(outcome, DashOutcome::Interrupted));
    assert_eq!(outcome.exit_code(), 130);
    let rendered = pages.lock().expect("pages poisoned");
    assert_eq!(rendered.len(), 2);
    assert_eq!(rendered[0].runs[0].id, 2841);
    assert!(rendered[1].runs[0].phase.is_terminal());
}

#[tokio::test(start_paused = true)]
async fn dash_hint_preempts_interval() {
    use doneyet_app::{ChannelPushSource, DashEngine};

    let provider = FakeProvider::new(vec![
        Step0::World(active_world()),
        Step0::World(active_world()),
    ]);
    let handle = provider.handle();
    let (hints, push) = ChannelPushSource::channel();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(1)).await;
        let _ = hints.send(hint());
        tokio::time::sleep(Duration::from_secs(60)).await;
        let _ = hints;
    });
    let shutdown = CancellationToken::new();
    let mut sink = PageSink {
        pages: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        cancel: shutdown.clone(),
        stop_after: 2,
    };
    let mut engine = DashEngine::new(Box::new(provider), Box::new(push), dash_config(), shutdown);
    engine
        .run(dash_query(), &mut sink)
        .await
        .expect("cancel is not an error");
    let calls = handle.locate_calls();
    assert_eq!(calls.len(), 2);
    let gap = calls[1] - calls[0];
    assert!(
        gap < Duration::from_secs(1),
        "hint must preempt the interval, gap was {gap:?}"
    );
}

#[tokio::test(start_paused = true)]
async fn dash_closed_channel_falls_back_to_polling() {
    use doneyet_app::{ChannelPushSource, DashEngine, DashOutcome};

    let provider = FakeProvider::new(vec![
        Step0::World(active_world()),
        Step0::World(active_world()),
        Step0::World(active_world()),
    ]);
    let handle = provider.handle();
    let (hints, push) = ChannelPushSource::channel();
    drop(hints);
    let shutdown = CancellationToken::new();
    let mut sink = PageSink {
        pages: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        cancel: shutdown.clone(),
        stop_after: 3,
    };
    let mut engine = DashEngine::new(Box::new(provider), Box::new(push), dash_config(), shutdown);
    let outcome = engine
        .run(dash_query(), &mut sink)
        .await
        .expect("a closed push channel must not kill the board");
    assert!(matches!(outcome, DashOutcome::Interrupted));
    let calls = handle.locate_calls();
    assert_eq!(calls.len(), 3);
    assert!(
        calls[2] - calls[1] >= Duration::from_secs(3),
        "after the channel closes, the next refresh must wait out the interval"
    );
}

#[tokio::test(start_paused = true)]
async fn dash_provider_error_aborts() {
    use doneyet_app::DashEngine;

    let provider = FakeProvider::new(vec![Step0::Fail("boom".to_string())]);
    let shutdown = CancellationToken::new();
    let mut sink = PageSink {
        pages: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        cancel: shutdown.clone(),
        stop_after: 1,
    };
    let mut engine = DashEngine::new(
        Box::new(provider),
        Box::new(NoopPushSource),
        dash_config(),
        shutdown,
    );
    let error = engine
        .run(dash_query(), &mut sink)
        .await
        .expect_err("provider failure aborts");
    assert!(error.to_string().contains("boom"), "{error}");
    assert!(sink.pages.lock().expect("pages poisoned").is_empty());
}

fn logs_config(tail: usize) -> WatchConfig {
    WatchConfig {
        log_tail: Some(tail),
        ..config()
    }
}

#[tokio::test(start_paused = true)]
async fn watch_appends_log_tails_from_the_previous_offset() {
    use doneyet_core::ports::LogChunk;

    let provider = FakeProvider::new(vec![
        Step0::World(active_world()),
        Step0::World(active_world()),
        Step0::World(terminal_world()),
    ]);
    provider.script_log(
        1,
        Ok(LogChunk {
            bytes: b"one\n".to_vec(),
            next_offset: 4,
        }),
    );
    provider.script_log(
        1,
        Ok(LogChunk {
            bytes: b"two\nthree\n".to_vec(),
            next_offset: 14,
        }),
    );
    let handle = provider.handle();
    let renderer = RecordingRenderer::default();
    let mut engine = WatchEngine::new(
        repo(),
        Box::new(provider),
        Box::new(renderer.clone()),
        Box::new(NoopPushSource),
        logs_config(2),
        CancellationToken::new(),
    );
    engine
        .watch(WatchTarget::Run(2841))
        .await
        .expect("log tail must not fail the watch");
    let frames = renderer.frames();
    assert_eq!(frames[0].0.job_logs[0].lines, vec!["one".to_string()]);
    assert_eq!(
        frames[1].0.job_logs[0].lines,
        vec!["two".to_string(), "three".to_string()]
    );
    assert_eq!(handle.log_calls(), vec![(1, 0), (1, 4)]);
}

#[tokio::test(start_paused = true)]
async fn log_fetch_failure_does_not_abort_watch() {
    let provider = FakeProvider::new(vec![
        Step0::World(active_world()),
        Step0::World(terminal_world()),
    ]);
    provider.script_log(1, Err("logs down".to_string()));
    let renderer = RecordingRenderer::default();
    let mut engine = WatchEngine::new(
        repo(),
        Box::new(provider),
        Box::new(renderer.clone()),
        Box::new(NoopPushSource),
        logs_config(5),
        CancellationToken::new(),
    );
    let outcome = engine
        .watch(WatchTarget::Run(2841))
        .await
        .expect("a dead log fetch must not kill the watch");
    assert!(matches!(
        outcome,
        WatchOutcome::Completed(Conclusion::Success)
    ));
    assert!(renderer.frames()[0].0.job_logs.is_empty());
}

#[tokio::test(start_paused = true)]
async fn replaced_log_is_refetched_from_the_start() {
    use doneyet_core::ports::LogChunk;

    let provider = FakeProvider::new(vec![
        Step0::World(active_world()),
        Step0::World(active_world()),
        Step0::World(terminal_world()),
    ]);
    provider.script_log(
        1,
        Ok(LogChunk {
            bytes: b"hello\n".to_vec(),
            next_offset: 6,
        }),
    );
    provider.script_log(
        1,
        Ok(LogChunk {
            bytes: Vec::new(),
            next_offset: 0,
        }),
    );
    provider.script_log(
        1,
        Ok(LogChunk {
            bytes: b"new\n".to_vec(),
            next_offset: 4,
        }),
    );
    let handle = provider.handle();
    let renderer = RecordingRenderer::default();
    let mut engine = WatchEngine::new(
        repo(),
        Box::new(provider),
        Box::new(renderer.clone()),
        Box::new(NoopPushSource),
        logs_config(20),
        CancellationToken::new(),
    );
    engine
        .watch(WatchTarget::Run(2841))
        .await
        .expect("replaced log must not fail the watch");
    assert_eq!(
        renderer.frames()[1].0.job_logs[0].lines,
        vec!["new".to_string()]
    );
    assert_eq!(handle.log_calls(), vec![(1, 0), (1, 6), (1, 0)]);
}
