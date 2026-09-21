use std::future::Future;
use std::time::Duration;

use doneyet_core::model::{Conclusion, Phase, RepoRef, RunsQuery};
use doneyet_core::ports::{PipelineProvider, ProviderError};
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::fixtures;

pub const CONTRACT_TOKEN: &str = "ghp_contract_token";
const RUNS_PATH: &str = "/repos/acme/api/actions/runs";

pub trait ProviderFactory {
    fn make(&self, base_url: String) -> impl Future<Output = Box<dyn PipelineProvider>>;
}

pub async fn run_suite<F: ProviderFactory>(factory: &F) {
    list_runs_maps_domain_fields(factory).await;
    list_runs_applies_query_filters(factory).await;
    list_runs_follows_link_pagination(factory).await;
    get_run_maps_single_run(factory).await;
    list_jobs_maps_jobs_and_steps(factory).await;
    list_annotations_maps_fields(factory).await;
    job_logs_returns_bytes(factory).await;
    job_logs_from_returns_the_unseen_suffix(factory).await;
    sends_bearer_token(factory).await;
    unauthorized_maps_to_auth(factory).await;
    not_found_maps_message(factory).await;
    rate_limited_maps_retry_after(factory).await;
    retries_on_server_errors(factory).await;
    etag_reuses_cached_body_on_304(factory).await;
}

fn repo() -> RepoRef {
    RepoRef {
        owner: "acme".to_string(),
        name: "api".to_string(),
    }
}

fn query(limit: u32) -> RunsQuery {
    RunsQuery {
        repo: repo(),
        branch: None,
        head_sha: None,
        event: None,
        limit,
    }
}

async fn setup<F: ProviderFactory>(factory: &F) -> (MockServer, Box<dyn PipelineProvider>) {
    let server = MockServer::start().await;
    let provider = factory.make(server.uri()).await;
    (server, provider)
}

async fn list_runs_maps_domain_fields<F: ProviderFactory>(factory: &F) {
    let (server, provider) = setup(factory).await;
    Mock::given(method("GET"))
        .and(path(RUNS_PATH))
        .respond_with(ResponseTemplate::new(200).set_body_string(fixtures::RUNS_PAGE))
        .expect(1)
        .mount(&server)
        .await;
    let page = provider
        .list_runs(&query(2))
        .await
        .expect("list_runs succeeds");
    assert_eq!(page.total_count, 2);
    assert_eq!(page.runs.len(), 2);
    let first = &page.runs[0];
    assert_eq!(first.id, 2842);
    assert_eq!(first.run_number, 2842);
    assert_eq!(first.name, "ci.yml");
    assert_eq!(first.display_title, "Add retry logic");
    assert_eq!(first.head_branch.as_deref(), Some("feat/retries"));
    assert_eq!(first.event, "pull_request");
    assert_eq!(first.phase, Phase::InProgress);
    assert_eq!(first.actor, "jonas");
    assert!(first.run_started_at.is_some());
    let second = &page.runs[1];
    assert_eq!(second.phase, Phase::Done(Conclusion::Success));
    server.verify().await;
}

async fn list_runs_applies_query_filters<F: ProviderFactory>(factory: &F) {
    let (server, provider) = setup(factory).await;
    Mock::given(method("GET"))
        .and(path(RUNS_PATH))
        .and(query_param("per_page", "7"))
        .and(query_param("branch", "feat/retries"))
        .and(query_param("event", "pull_request"))
        .and(query_param("head_sha", "abc123"))
        .respond_with(ResponseTemplate::new(200).set_body_string(fixtures::EMPTY_RUNS))
        .expect(1)
        .mount(&server)
        .await;
    let q = RunsQuery {
        repo: repo(),
        branch: Some("feat/retries".to_string()),
        head_sha: Some("abc123".to_string()),
        event: Some("pull_request".to_string()),
        limit: 7,
    };
    let page = provider
        .list_runs(&q)
        .await
        .expect("filtered list_runs succeeds");
    assert_eq!(page.total_count, 0);
    assert!(page.runs.is_empty());
    server.verify().await;
}

async fn list_runs_follows_link_pagination<F: ProviderFactory>(factory: &F) {
    let (server, provider) = setup(factory).await;
    let page2_url = format!("{}{RUNS_PATH}?per_page=2&page=2", server.uri());
    Mock::given(method("GET"))
        .and(path(RUNS_PATH))
        .and(query_param("page", "2"))
        .respond_with(ResponseTemplate::new(200).set_body_string(fixtures::RUNS_PAGE2))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(RUNS_PATH))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(fixtures::RUNS_LINK_PAGE1)
                .append_header("Link", format!("<{page2_url}>; rel=\"next\"")),
        )
        .expect(1)
        .mount(&server)
        .await;
    let page = provider
        .list_runs(&query(2))
        .await
        .expect("paginated list_runs succeeds");
    let ids: Vec<u64> = page.runs.iter().map(|r| r.id).collect();
    assert_eq!(ids, vec![2842, 2840]);
    assert_eq!(page.total_count, 2);
    server.verify().await;
}

async fn get_run_maps_single_run<F: ProviderFactory>(factory: &F) {
    let (server, provider) = setup(factory).await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/api/actions/runs/2841"))
        .respond_with(ResponseTemplate::new(200).set_body_string(fixtures::RUN))
        .expect(1)
        .mount(&server)
        .await;
    let run = provider.get_run(2841).await.expect("get_run succeeds");
    assert_eq!(run.id, 2841);
    assert_eq!(run.run_number, 2841);
    assert_eq!(run.display_title, "build & test");
    assert_eq!(run.phase, Phase::Done(Conclusion::Success));
    assert_eq!(run.actor, "jonas");
    assert!(run.run_started_at.is_some());
    server.verify().await;
}

async fn list_jobs_maps_jobs_and_steps<F: ProviderFactory>(factory: &F) {
    let (server, provider) = setup(factory).await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/api/actions/runs/2841/jobs"))
        .respond_with(ResponseTemplate::new(200).set_body_string(fixtures::JOBS))
        .expect(1)
        .mount(&server)
        .await;
    let jobs = provider.list_jobs(2841).await.expect("list_jobs succeeds");
    assert_eq!(jobs.len(), 2);
    let build = &jobs[0];
    assert_eq!(build.id, 399444496);
    assert_eq!(build.run_id, 2841);
    assert_eq!(build.name, "build (ubuntu-latest)");
    assert_eq!(build.phase, Phase::Done(Conclusion::Success));
    assert_eq!(build.runner_name.as_deref(), Some("gh-runner-01"));
    assert_eq!(
        build.labels,
        vec!["ubuntu-latest".to_string(), "x64".to_string()]
    );
    assert_eq!(build.steps.len(), 3);
    assert_eq!(build.steps[0].phase, Phase::Done(Conclusion::Success));
    assert_eq!(build.steps[1].phase, Phase::Done(Conclusion::Failure));
    assert_eq!(build.steps[2].phase, Phase::Done(Conclusion::Skipped));
    assert!(build.steps[2].started_at.is_none());
    let test = &jobs[1];
    assert_eq!(test.phase, Phase::InProgress);
    assert!(test.completed_at.is_none());
    assert_eq!(test.steps[0].phase, Phase::InProgress);
    server.verify().await;
}

async fn list_annotations_maps_fields<F: ProviderFactory>(factory: &F) {
    let (server, provider) = setup(factory).await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/api/check-runs/399444496/annotations"))
        .respond_with(ResponseTemplate::new(200).set_body_string(fixtures::ANNOTATIONS))
        .expect(1)
        .mount(&server)
        .await;
    let annotations = provider
        .list_annotations(399444496)
        .await
        .expect("list_annotations succeeds");
    assert_eq!(annotations.len(), 2);
    assert_eq!(annotations[0].job_id, 399444496);
    assert_eq!(annotations[0].path.as_deref(), Some("src/lib.rs"));
    assert_eq!(annotations[0].start_line, Some(42));
    assert_eq!(annotations[0].message, "unused variable: `x`");
    assert_eq!(annotations[1].path, None);
    assert_eq!(annotations[1].start_line, None);
    server.verify().await;
}

async fn job_logs_returns_bytes<F: ProviderFactory>(factory: &F) {
    let (server, provider) = setup(factory).await;
    let body = "2026-09-21T10:00:06Z Run cargo test\n2026-09-21T10:00:20Z error: test failed\n";
    Mock::given(method("GET"))
        .and(path("/repos/acme/api/actions/jobs/399444496/logs"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .expect(1)
        .mount(&server)
        .await;
    let logs = provider
        .job_logs(399444496)
        .await
        .expect("job_logs succeeds");
    assert_eq!(String::from_utf8_lossy(&logs), body);
    server.verify().await;
}

async fn job_logs_from_returns_the_unseen_suffix<F: ProviderFactory>(factory: &F) {
    let (server, provider) = setup(factory).await;
    let body = "alpha\nbeta\n";
    Mock::given(method("GET"))
        .and(path("/repos/acme/api/actions/jobs/399444496/logs"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .expect(1)
        .mount(&server)
        .await;
    let chunk = provider
        .job_logs_from(399444496, 6)
        .await
        .expect("job_logs_from succeeds");
    assert_eq!(chunk.bytes, b"beta\n");
    assert_eq!(chunk.next_offset, body.len() as u64);
    server.verify().await;
}

async fn sends_bearer_token<F: ProviderFactory>(factory: &F) {
    let (server, provider) = setup(factory).await;
    Mock::given(method("GET"))
        .and(path(RUNS_PATH))
        .and(header("authorization", format!("Bearer {CONTRACT_TOKEN}")))
        .respond_with(ResponseTemplate::new(200).set_body_string(fixtures::EMPTY_RUNS))
        .expect(1)
        .mount(&server)
        .await;
    provider
        .list_runs(&query(1))
        .await
        .expect("authorized list_runs succeeds");
    server.verify().await;
}

async fn unauthorized_maps_to_auth<F: ProviderFactory>(factory: &F) {
    let (server, provider) = setup(factory).await;
    Mock::given(method("GET"))
        .and(path(RUNS_PATH))
        .respond_with(
            ResponseTemplate::new(401).set_body_string(r#"{"message": "Bad credentials"}"#),
        )
        .expect(1)
        .mount(&server)
        .await;
    let err = provider
        .list_runs(&query(1))
        .await
        .expect_err("401 must fail");
    assert!(matches!(err, ProviderError::Auth), "got {err:?}");
    server.verify().await;
}

async fn not_found_maps_message<F: ProviderFactory>(factory: &F) {
    let (server, provider) = setup(factory).await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/api/actions/runs/999"))
        .respond_with(ResponseTemplate::new(404).set_body_string(r#"{"message": "Not Found"}"#))
        .expect(1)
        .mount(&server)
        .await;
    let err = provider.get_run(999).await.expect_err("404 must fail");
    match err {
        ProviderError::NotFound(message) => assert_eq!(message, "Not Found"),
        other => panic!("expected NotFound, got {other:?}"),
    }
    server.verify().await;
}

async fn rate_limited_maps_retry_after<F: ProviderFactory>(factory: &F) {
    let (server, provider) = setup(factory).await;
    Mock::given(method("GET"))
        .and(path(RUNS_PATH))
        .respond_with(
            ResponseTemplate::new(403)
                .append_header("retry-after", "42")
                .append_header("x-ratelimit-remaining", "0")
                .set_body_string(r#"{"message": "API rate limit exceeded"}"#),
        )
        .expect(1)
        .mount(&server)
        .await;
    let err = provider
        .list_runs(&query(1))
        .await
        .expect_err("403 rate limit must fail");
    match err {
        ProviderError::RateLimited { retry_after } => {
            assert_eq!(retry_after, Duration::from_secs(42))
        }
        other => panic!("expected RateLimited, got {other:?}"),
    }
    server.verify().await;
}

async fn retries_on_server_errors<F: ProviderFactory>(factory: &F) {
    let (server, provider) = setup(factory).await;
    Mock::given(method("GET"))
        .and(path(RUNS_PATH))
        .respond_with(ResponseTemplate::new(500))
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(RUNS_PATH))
        .respond_with(ResponseTemplate::new(200).set_body_string(fixtures::EMPTY_RUNS))
        .expect(1)
        .mount(&server)
        .await;
    let page = provider
        .list_runs(&query(1))
        .await
        .expect("must succeed after retrying a transient 500");
    assert!(page.runs.is_empty());
    server.verify().await;
}

async fn etag_reuses_cached_body_on_304<F: ProviderFactory>(factory: &F) {
    let (server, provider) = setup(factory).await;
    let etag = "\"d1e7f0\"";
    Mock::given(method("GET"))
        .and(path(RUNS_PATH))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(fixtures::RUNS_PAGE)
                .append_header("ETag", etag),
        )
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(RUNS_PATH))
        .and(header("if-none-match", etag))
        .respond_with(ResponseTemplate::new(304))
        .expect(1)
        .mount(&server)
        .await;
    let first = provider
        .list_runs(&query(2))
        .await
        .expect("first fetch succeeds");
    let second = provider
        .list_runs(&query(2))
        .await
        .expect("second fetch succeeds");
    assert_eq!(first, second);
    server.verify().await;
}
