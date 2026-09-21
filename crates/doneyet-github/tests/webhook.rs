use doneyet_core::model::RepoRef;
use doneyet_core::ports::PushSource;
use doneyet_github::webhook::WebhookPush;
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;
use std::str::FromStr;
use std::time::Duration;

const SECRET: &str = "test-secret";

fn sign(secret: &str, body: &str) -> String {
    let mut mac = <Hmac<Sha256>>::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(body.as_bytes());
    format!("sha256={}", hex::encode(mac.finalize().into_bytes()))
}

async fn post(push: &WebhookPush, event: &str, body: &str, signature: &str) -> reqwest::Response {
    reqwest::Client::new()
        .post(format!("http://{}/", push.local_addr()))
        .header("x-github-event", event)
        .header("x-hub-signature-256", signature)
        .body(body.to_string())
        .send()
        .await
        .expect("post webhook")
}

const WORKFLOW_RUN_BODY: &str = r#"{"action":"in_progress","workflow_run":{"id":2841,"name":"ci.yml","status":"in_progress"},"repository":{"full_name":"acme/api"}}"#;

#[tokio::test]
async fn valid_workflow_run_emits_refresh_hint() {
    let mut push = WebhookPush::start("127.0.0.1:0", SECRET.to_string())
        .await
        .expect("start webhook server");
    let response = post(
        &push,
        "workflow_run",
        WORKFLOW_RUN_BODY,
        &sign(SECRET, WORKFLOW_RUN_BODY),
    )
    .await;
    assert_eq!(response.status(), 200);
    let hint = tokio::time::timeout(Duration::from_secs(5), push.wait())
        .await
        .expect("hint arrives")
        .expect("server alive");
    assert_eq!(hint.repo, RepoRef::from_str("acme/api").unwrap());
    assert_eq!(hint.run_id, Some(2841));
}

#[tokio::test]
async fn workflow_job_event_maps_run_id() {
    let mut push = WebhookPush::start("127.0.0.1:0", SECRET.to_string())
        .await
        .expect("start webhook server");
    let body = r#"{"action":"completed","workflow_job":{"id":99,"run_id":2842},"repository":{"full_name":"acme/api"}}"#;
    let response = post(&push, "workflow_job", body, &sign(SECRET, body)).await;
    assert_eq!(response.status(), 200);
    let hint = tokio::time::timeout(Duration::from_secs(5), push.wait())
        .await
        .expect("hint arrives")
        .expect("server alive");
    assert_eq!(hint.run_id, Some(2842));
}

#[tokio::test]
async fn invalid_signature_is_rejected_without_hint() {
    let mut push = WebhookPush::start("127.0.0.1:0", SECRET.to_string())
        .await
        .expect("start webhook server");
    let response = post(
        &push,
        "workflow_run",
        WORKFLOW_RUN_BODY,
        &sign("wrong-secret", WORKFLOW_RUN_BODY),
    )
    .await;
    assert_eq!(response.status(), 401);
    let no_hint = tokio::time::timeout(Duration::from_millis(200), push.wait()).await;
    assert!(
        no_hint.is_err(),
        "no hint may be emitted for bad signatures"
    );
}

#[tokio::test]
async fn missing_signature_header_is_rejected() {
    let push = WebhookPush::start("127.0.0.1:0", SECRET.to_string())
        .await
        .expect("start webhook server");
    let response = reqwest::Client::new()
        .post(format!("http://{}/", push.local_addr()))
        .header("x-github-event", "workflow_run")
        .body(WORKFLOW_RUN_BODY.to_string())
        .send()
        .await
        .expect("post webhook");
    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn ping_event_is_acknowledged_but_ignored() {
    let mut push = WebhookPush::start("127.0.0.1:0", SECRET.to_string())
        .await
        .expect("start webhook server");
    let body = r#"{"zen":"Keep it logically awesome.","repository":{"full_name":"acme/api"}}"#;
    let response = post(&push, "ping", body, &sign(SECRET, body)).await;
    assert_eq!(response.status(), 200);
    let no_hint = tokio::time::timeout(Duration::from_millis(200), push.wait()).await;
    assert!(no_hint.is_err(), "ping must not emit a hint");
}

#[tokio::test]
async fn webhook_path_also_accepted() {
    let push = WebhookPush::start("127.0.0.1:0", SECRET.to_string())
        .await
        .expect("start webhook server");
    let response = reqwest::Client::new()
        .post(format!("http://{}/webhook", push.local_addr()))
        .header("x-github-event", "ping")
        .header("x-hub-signature-256", sign(SECRET, "{}"))
        .body("{}")
        .send()
        .await
        .expect("post webhook");
    assert_eq!(response.status(), 200);
}
