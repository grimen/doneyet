mod common;

use common::{FakeProvider, PageSink, Step0, run_with};
use doneyet_app::{CommitWatchEngine, NoopPushSource, WatchConfig, WatchOutcome};
use doneyet_core::model::{Conclusion, Phase, RunsPage, RunsQuery};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

fn query() -> RunsQuery {
    RunsQuery {
        repo: common::repo(),
        branch: None,
        head_sha: Some("abc123".to_string()),
        event: None,
        limit: 20,
    }
}

fn config() -> WatchConfig {
    WatchConfig {
        active_interval: Duration::from_secs(3),
        idle_interval: Duration::from_secs(20),
        log_tail: None,
        log_grep: None,
        job: None,
        timeout: None,
    }
}

fn page(runs: Vec<doneyet_core::model::WorkflowRun>) -> Step0 {
    Step0::Page(RunsPage {
        total_count: runs.len() as u64,
        runs,
    })
}

fn sink(stop_after: usize) -> (Arc<Mutex<Vec<RunsPage>>>, PageSink, CancellationToken) {
    let shutdown = CancellationToken::new();
    let sink = PageSink {
        pages: Arc::new(Mutex::new(Vec::new())),
        cancel: shutdown.clone(),
        stop_after,
    };
    (sink.pages.clone(), sink, shutdown)
}

#[tokio::test(start_paused = true)]
async fn commit_watch_aggregates_worst_conclusion() {
    let provider = FakeProvider::new(vec![
        page(vec![
            run_with(2841, Phase::InProgress),
            run_with(2842, Phase::InProgress),
        ]),
        page(vec![
            run_with(2841, Phase::Done(Conclusion::Success)),
            run_with(2842, Phase::Done(Conclusion::Failure)),
        ]),
    ]);
    let (pages, mut sink, shutdown) = sink(2);
    let mut engine = CommitWatchEngine::new(
        Box::new(provider),
        Box::new(NoopPushSource),
        config(),
        shutdown,
    );
    let outcome = engine
        .run(query(), &mut sink)
        .await
        .expect("watch succeeds");
    assert!(matches!(
        outcome,
        WatchOutcome::Completed(Conclusion::Failure)
    ));
    assert_eq!(outcome.exit_code(), 1);
    assert_eq!(pages.lock().expect("pages poisoned").len(), 2);
}

#[tokio::test(start_paused = true)]
async fn commit_watch_all_success_exits_zero() {
    let provider = FakeProvider::new(vec![page(vec![
        run_with(2841, Phase::Done(Conclusion::Success)),
        run_with(2842, Phase::Done(Conclusion::Success)),
    ])]);
    let (_, mut sink, shutdown) = sink(1);
    let mut engine = CommitWatchEngine::new(
        Box::new(provider),
        Box::new(NoopPushSource),
        config(),
        shutdown,
    );
    let outcome = engine
        .run(query(), &mut sink)
        .await
        .expect("watch succeeds");
    assert!(matches!(
        outcome,
        WatchOutcome::Completed(Conclusion::Success)
    ));
}

#[tokio::test(start_paused = true)]
async fn commit_watch_timeout_only_maps_to_cancelled() {
    let provider = FakeProvider::new(vec![page(vec![run_with(
        2841,
        Phase::Done(Conclusion::TimedOut),
    )])]);
    let (_, mut sink, shutdown) = sink(1);
    let mut engine = CommitWatchEngine::new(
        Box::new(provider),
        Box::new(NoopPushSource),
        config(),
        shutdown,
    );
    let outcome = engine
        .run(query(), &mut sink)
        .await
        .expect("watch succeeds");
    assert!(
        matches!(outcome, WatchOutcome::Completed(Conclusion::Cancelled)),
        "{outcome:?}"
    );
}

#[tokio::test(start_paused = true)]
async fn commit_watch_empty_page_keeps_polling() {
    let provider = FakeProvider::new(vec![
        Step0::NoRun,
        page(vec![run_with(2841, Phase::Done(Conclusion::Success))]),
    ]);
    let (_, mut sink, shutdown) = sink(2);
    let mut engine = CommitWatchEngine::new(
        Box::new(provider),
        Box::new(NoopPushSource),
        config(),
        shutdown,
    );
    let outcome = engine
        .run(query(), &mut sink)
        .await
        .expect("watch succeeds");
    assert!(matches!(
        outcome,
        WatchOutcome::Completed(Conclusion::Success)
    ));
}

#[tokio::test(start_paused = true)]
async fn commit_watch_cancel_returns_interrupted() {
    let provider = FakeProvider::new(vec![page(vec![run_with(2841, Phase::InProgress)])]);
    let (pages, mut sink, shutdown) = sink(1);
    let mut engine = CommitWatchEngine::new(
        Box::new(provider),
        Box::new(NoopPushSource),
        config(),
        shutdown,
    );
    let outcome = engine
        .run(query(), &mut sink)
        .await
        .expect("watch succeeds");
    assert!(matches!(outcome, WatchOutcome::Interrupted));
    assert_eq!(outcome.exit_code(), 130);
    assert_eq!(pages.lock().expect("pages poisoned").len(), 1);
}

#[tokio::test(start_paused = true)]
async fn commit_watch_survives_transport_error_then_completes() {
    let provider = FakeProvider::new(vec![
        Step0::Transport("connection reset".to_string()),
        page(vec![run_with(2841, Phase::Done(Conclusion::Success))]),
    ]);
    let (_, mut sink, shutdown) = sink(1);
    let mut engine = CommitWatchEngine::new(
        Box::new(provider),
        Box::new(NoopPushSource),
        config(),
        shutdown,
    );
    let outcome = engine
        .run(query(), &mut sink)
        .await
        .expect("a transport error must not kill the watch");
    assert!(matches!(
        outcome,
        WatchOutcome::Completed(Conclusion::Success)
    ));
}
