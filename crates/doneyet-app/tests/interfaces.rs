mod common;

use common::{PageSink, RunsOnlyProvider, run, run_with};
use doneyet_app::{
    CommitWatchEngine, DashConfig, DashEngine, DashOutcome, NoopPushSource, WatchConfig,
    WatchOutcome,
};
use doneyet_core::model::{Conclusion, Phase, RunsPage, RunsQuery};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

fn dash_query() -> RunsQuery {
    RunsQuery {
        repo: common::repo(),
        branch: None,
        head_sha: None,
        event: None,
        limit: 5,
    }
}

fn dash_config() -> DashConfig {
    DashConfig {
        interval: Duration::from_secs(3),
        timeout: None,
    }
}

fn commit_query() -> RunsQuery {
    RunsQuery {
        repo: common::repo(),
        branch: None,
        head_sha: Some("abc123".to_string()),
        event: None,
        limit: 20,
    }
}

fn commit_config() -> WatchConfig {
    WatchConfig {
        active_interval: Duration::from_millis(100),
        idle_interval: Duration::from_millis(100),
        log_tail: None,
        log_grep: None,
        job: None,
        timeout: None,
    }
}

fn single_run_page() -> RunsPage {
    RunsPage {
        total_count: 1,
        runs: vec![run(Phase::InProgress)],
    }
}

#[tokio::test(start_paused = true)]
async fn runs_only_provider_powers_dash() {
    let provider = RunsOnlyProvider::new(single_run_page());
    let shutdown = CancellationToken::new();
    let mut sink = PageSink {
        pages: Arc::new(Mutex::new(Vec::new())),
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
    let rendered = sink.pages.lock().expect("pages poisoned");
    assert_eq!(rendered.len(), 2);
    assert_eq!(rendered[0].runs[0].id, 2841);
    assert_eq!(rendered[1].runs[0].id, 2841);
}

#[tokio::test(start_paused = true)]
async fn runs_only_provider_powers_commit_watch() {
    let provider = RunsOnlyProvider::new(RunsPage {
        total_count: 1,
        runs: vec![run_with(2841, Phase::Done(Conclusion::Success))],
    });
    let shutdown = CancellationToken::new();
    let mut sink = PageSink {
        pages: Arc::new(Mutex::new(Vec::new())),
        cancel: shutdown.clone(),
        stop_after: 1,
    };
    let mut engine = CommitWatchEngine::new(
        Box::new(provider),
        Box::new(NoopPushSource),
        commit_config(),
        shutdown,
    );
    let outcome = engine
        .run(commit_query(), &mut sink)
        .await
        .expect("watch succeeds");
    assert!(matches!(
        outcome,
        WatchOutcome::Completed(Conclusion::Success)
    ));
    assert_eq!(sink.pages.lock().expect("pages poisoned").len(), 1);
}
