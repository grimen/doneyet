use std::time::Duration;

use doneyet_core::model::RepoRef;
use doneyet_core::ports::{ProviderError, RunWriteSource};
use doneyet_github::{GithubConfig, GithubProvider, RetryPolicy};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn provider(server: &MockServer, max_retries: u32) -> GithubProvider {
    GithubProvider::new(
        RepoRef {
            owner: "acme".to_string(),
            name: "api".to_string(),
        },
        GithubConfig {
            api_base: server.uri(),
            token: None,
            retry: RetryPolicy {
                max_retries,
                base_delay: Duration::from_millis(1),
                ..RetryPolicy::default()
            },
        },
    )
    .expect("provider builds")
}

#[tokio::test]
async fn rerun_posts_to_rerun_endpoint_and_succeeds() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/repos/acme/api/actions/runs/2841/rerun"))
        .respond_with(ResponseTemplate::new(201))
        .expect(1)
        .mount(&server)
        .await;
    let provider = provider(&server, 2);
    provider
        .rerun(2841, false)
        .await
        .expect("rerun of a full run is accepted");
}

#[tokio::test]
async fn rerun_failed_only_posts_to_failed_jobs_endpoint() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/repos/acme/api/actions/runs/2841/rerun-failed-jobs"))
        .respond_with(ResponseTemplate::new(201))
        .expect(1)
        .mount(&server)
        .await;
    let provider = provider(&server, 2);
    provider
        .rerun(2841, true)
        .await
        .expect("failed-only rerun is accepted");
}

#[tokio::test]
async fn cancel_posts_to_cancel_endpoint_and_succeeds() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/repos/acme/api/actions/runs/2841/cancel"))
        .respond_with(ResponseTemplate::new(202))
        .expect(1)
        .mount(&server)
        .await;
    let provider = provider(&server, 2);
    provider.cancel(2841).await.expect("cancel is accepted");
}

#[tokio::test]
async fn rerun_retries_transient_5xx_then_succeeds() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/repos/acme/api/actions/runs/2841/rerun"))
        .respond_with(ResponseTemplate::new(503))
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/repos/acme/api/actions/runs/2841/rerun"))
        .respond_with(ResponseTemplate::new(201))
        .expect(1)
        .mount(&server)
        .await;
    let provider = provider(&server, 2);
    provider
        .rerun(2841, false)
        .await
        .expect("a 503 followed by a retry must succeed");
}

#[tokio::test]
async fn rerun_surfaces_not_found() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/repos/acme/api/actions/runs/2841/rerun"))
        .respond_with(ResponseTemplate::new(404))
        .expect(1)
        .mount(&server)
        .await;
    let provider = provider(&server, 2);
    let err = provider
        .rerun(2841, false)
        .await
        .expect_err("an unknown run must be a not-found error");
    assert!(matches!(err, ProviderError::NotFound(_)), "{err}");
}
