use std::time::Duration;

use doneyet_core::model::{RepoRef, RunsQuery};
use doneyet_core::ports::{LogSource, ProviderError, RunSource};
use doneyet_github::{GithubConfig, GithubProvider, RetryPolicy};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const RUNS_PAGE: &str = r#"{"total_count":1,"workflow_runs":[{"id":2841,"run_number":2841,"name":"ci.yml","display_title":"build & test","head_branch":"main","head_sha":"deadbeef","event":"push","status":"completed","conclusion":"success","actor":{"login":"jonas"},"html_url":"https://github.com/acme/api/actions/runs/2841","created_at":"2026-09-21T10:00:00Z","run_started_at":"2026-09-21T10:00:01Z","updated_at":"2026-09-21T10:03:00Z"}]}"#;

const LOGS: &str = "line one\nline two\n";

fn repo() -> RepoRef {
    RepoRef {
        owner: "acme".to_string(),
        name: "api".to_string(),
    }
}

fn retry(max_retries: u32) -> RetryPolicy {
    RetryPolicy {
        max_retries,
        base_delay: Duration::from_millis(1),
        ..RetryPolicy::default()
    }
}

fn provider(server: &MockServer, max_retries: u32) -> GithubProvider {
    GithubProvider::new(
        repo(),
        GithubConfig {
            api_base: server.uri(),
            token: None,
            retry: retry(max_retries),
        },
    )
    .expect("provider builds")
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

#[tokio::test]
async fn list_runs_retries_five_hundred_then_succeeds() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/api/actions/runs"))
        .respond_with(ResponseTemplate::new(503))
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/api/actions/runs"))
        .respond_with(ResponseTemplate::new(200).set_body_string(RUNS_PAGE))
        .expect(1)
        .mount(&server)
        .await;
    let provider = provider(&server, 2);
    let page = provider
        .list_runs(&query())
        .await
        .expect("a 503 followed by retry must succeed");
    assert_eq!(page.runs.len(), 1);
    assert_eq!(page.runs[0].id, 2841);
}

#[tokio::test]
async fn list_runs_gives_up_after_max_retries() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/api/actions/runs"))
        .respond_with(ResponseTemplate::new(503))
        .expect(3)
        .mount(&server)
        .await;
    let provider = provider(&server, 2);
    let err = provider
        .list_runs(&query())
        .await
        .expect_err("must give up");
    assert!(err.to_string().contains("503"), "{err}");
}

#[tokio::test]
async fn job_logs_retries_five_hundred_then_succeeds() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/api/actions/jobs/1/logs"))
        .respond_with(ResponseTemplate::new(503))
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/api/actions/jobs/1/logs"))
        .respond_with(ResponseTemplate::new(200).set_body_string(LOGS))
        .expect(1)
        .mount(&server)
        .await;
    let provider = provider(&server, 2);
    let bytes = provider
        .job_logs(1)
        .await
        .expect("a 503 followed by retry must succeed");
    assert_eq!(bytes, LOGS.as_bytes());
}

#[tokio::test]
async fn job_logs_maps_rate_limit_to_rate_limited() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/api/actions/jobs/1/logs"))
        .respond_with(ResponseTemplate::new(429))
        .expect(1)
        .mount(&server)
        .await;
    let provider = provider(&server, 2);
    let err = provider
        .job_logs(1)
        .await
        .expect_err("must be rate limited");
    assert!(matches!(err, ProviderError::RateLimited { .. }), "{err}");
}
