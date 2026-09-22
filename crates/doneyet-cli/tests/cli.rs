use std::time::Duration;

use assert_cmd::Command;
use doneyet_core::model::Conclusion;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn doneyet() -> Command {
    Command::cargo_bin("doneyet").expect("binary")
}

const ACTIVE_RUN_PAGE: &str = r#"{"total_count":1,"workflow_runs":[{"id":2841,"run_number":2841,"name":"ci.yml","display_title":"build & test","head_branch":"main","head_sha":"abc","event":"push","status":"in_progress","conclusion":null,"actor":{"login":"jonas"},"html_url":"https://github.com/acme/api/actions/runs/2841","created_at":"2026-09-21T10:00:00Z","run_started_at":"2026-09-21T10:00:01Z","updated_at":"2026-09-21T10:00:30Z"}]}"#;

const COMPLETED_RUN_PAGE: &str = r#"{"total_count":1,"workflow_runs":[{"id":2841,"run_number":2841,"name":"ci.yml","display_title":"build & test","head_branch":"main","head_sha":"abc","event":"push","status":"completed","conclusion":"success","actor":{"login":"jonas"},"html_url":"https://github.com/acme/api/actions/runs/2841","created_at":"2026-09-21T10:00:00Z","run_started_at":"2026-09-21T10:00:01Z","updated_at":"2026-09-21T10:03:00Z"}]}"#;

const EMPTY_JOBS: &str = r#"{"total_count":0,"jobs":[]}"#;

const JOB_FILTER_JOBS: &str = r#"{"total_count":2,"jobs":[
  {"id":1,"run_id":2841,"status":"completed","conclusion":"success","name":"make (arm64)","started_at":"2026-09-21T10:00:02Z","completed_at":"2026-09-21T10:01:30Z","runner_name":"gh-runner-01","labels":["arm64"],"steps":[]},
  {"id":2,"run_id":2841,"status":"in_progress","conclusion":null,"name":"test (macos-latest)","started_at":"2026-09-21T10:01:31Z","completed_at":null,"runner_name":null,"labels":["macos-latest"],"steps":[]}
]}"#;

#[test]
fn completions_bash_exits_zero_and_mentions_doneyet() {
    let output = doneyet()
        .args(["completions", "bash"])
        .output()
        .expect("run binary");
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("doneyet"), "{stdout}");
}

#[test]
fn completions_zsh_exits_zero_and_mentions_doneyet() {
    let output = doneyet()
        .args(["completions", "zsh"])
        .output()
        .expect("run binary");
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("doneyet"), "{stdout}");
}

#[test]
fn completions_install_writes_file_and_prints_instruction() {
    let dir = std::env::temp_dir().join(format!("doneyet-install-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let output = doneyet()
        .args(["completions", "bash", "--install"])
        .env("DONEYET_COMPLETIONS_DIR", &dir)
        .output()
        .expect("run binary");
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let file = dir.join("doneyet.bash");
    let contents = std::fs::read_to_string(&file).expect("completion file exists");
    assert!(
        !contents.trim().is_empty(),
        "completion file must be non-empty"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(&dir.display().to_string()), "{stdout}");
    assert!(stdout.contains("source"), "{stdout}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn completions_without_install_prints_to_stdout() {
    let dir = std::env::temp_dir().join(format!("doneyet-uninstalled-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp completions dir");
    let output = doneyet()
        .args(["completions", "fish"])
        .env("DONEYET_COMPLETIONS_DIR", &dir)
        .output()
        .expect("run binary");
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.trim().is_empty(),
        "stdout must carry the completion script"
    );
    assert!(stdout.contains("doneyet"), "{stdout}");
    let mut entries = dir.read_dir().expect("dir readable");
    assert!(
        entries.next().is_none(),
        "--install is what writes files; the completions dir must stay empty"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn runs_command_lists_runs_and_exits_zero() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/api/actions/runs"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(doneyet_contract::fixtures::RUNS_PAGE),
        )
        .expect(1..)
        .mount(&server)
        .await;
    let output = doneyet()
        .args(["runs", "acme/api", "--api-base", &server.uri()])
        .output()
        .expect("run binary");
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("#2842"), "{stdout}");
    assert!(stdout.contains("#2841"), "{stdout}");
    assert!(stdout.contains("feat/retries"), "{stdout}");
}

#[tokio::test(flavor = "multi_thread")]
async fn runs_with_explicit_branch_still_queries_that_branch() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/api/actions/runs"))
        .and(wiremock::matchers::query_param("branch", "feat/explicit"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(doneyet_contract::fixtures::RUNS_PAGE),
        )
        .expect(1..)
        .mount(&server)
        .await;
    let output = doneyet()
        .args([
            "runs",
            "acme/api",
            "--branch",
            "feat/explicit",
            "--api-base",
            &server.uri(),
        ])
        .output()
        .expect("run binary");
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("#2841"), "{stdout}");
}

#[tokio::test(flavor = "multi_thread")]
async fn run_command_exits_zero_on_success_conclusion() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/api/actions/runs/2841"))
        .respond_with(ResponseTemplate::new(200).set_body_string(doneyet_contract::fixtures::RUN))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/api/actions/runs/2841/jobs"))
        .respond_with(ResponseTemplate::new(200).set_body_string(doneyet_contract::fixtures::JOBS))
        .expect(1)
        .mount(&server)
        .await;
    let output = doneyet()
        .args([
            "run",
            "2841",
            "--repo",
            "acme/api",
            "--api-base",
            &server.uri(),
        ])
        .output()
        .expect("run binary");
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("#2841"), "{stdout}");
    assert!(stdout.contains("success"), "{stdout}");
}

#[tokio::test(flavor = "multi_thread")]
async fn run_command_exits_one_on_failure_conclusion() {
    let failed_run = doneyet_contract::fixtures::RUN.replace("success", "failure");
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/api/actions/runs/2841"))
        .respond_with(ResponseTemplate::new(200).set_body_string(failed_run))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/api/actions/runs/2841/jobs"))
        .respond_with(ResponseTemplate::new(200).set_body_string(EMPTY_JOBS))
        .expect(1)
        .mount(&server)
        .await;
    let output = doneyet()
        .args([
            "run",
            "2841",
            "--repo",
            "acme/api",
            "--api-base",
            &server.uri(),
        ])
        .output()
        .expect("run binary");
    assert_eq!(output.status.code(), Some(1), "{output:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn watch_command_follows_run_until_terminal() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/api/actions/runs"))
        .respond_with(ResponseTemplate::new(200).set_body_string(ACTIVE_RUN_PAGE))
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/api/actions/runs"))
        .respond_with(ResponseTemplate::new(200).set_body_string(COMPLETED_RUN_PAGE))
        .expect(1..)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/api/actions/runs/2841/jobs"))
        .respond_with(ResponseTemplate::new(200).set_body_string(EMPTY_JOBS))
        .expect(1..)
        .mount(&server)
        .await;
    let output = doneyet()
        .args([
            "watch",
            "acme/api",
            "--interval",
            "1",
            "--api-base",
            &server.uri(),
        ])
        .timeout(Duration::from_secs(30))
        .output()
        .expect("run binary");
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("in_progress"), "{stdout}");
    assert!(stdout.contains("success"), "{stdout}");
}

#[tokio::test(flavor = "multi_thread")]
async fn watch_with_format_json_emits_jsonl_and_exits_zero() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/api/actions/runs"))
        .respond_with(ResponseTemplate::new(200).set_body_string(ACTIVE_RUN_PAGE))
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/api/actions/runs"))
        .respond_with(ResponseTemplate::new(200).set_body_string(COMPLETED_RUN_PAGE))
        .expect(1..)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/api/actions/runs/2841/jobs"))
        .respond_with(ResponseTemplate::new(200).set_body_string(EMPTY_JOBS))
        .expect(1..)
        .mount(&server)
        .await;
    let output = doneyet()
        .args([
            "watch",
            "acme/api",
            "--format",
            "json",
            "--interval",
            "1",
            "--api-base",
            &server.uri(),
        ])
        .timeout(Duration::from_secs(30))
        .output()
        .expect("run binary");
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.lines().collect();
    assert!(lines.len() >= 2, "expected render + finish lines: {stdout}");
    let first: doneyet_ux::record::RenderRecord =
        serde_json::from_str(lines[0]).expect("line 1 parses as a render record");
    assert_eq!(first.kind, "render", "{first:?}");
    assert_eq!(first.v, 1, "{first:?}");
    assert_eq!(first.world.run.id, 2841, "{first:?}");
    let last: doneyet_ux::record::FinishRecord =
        serde_json::from_str(lines[lines.len() - 1]).expect("last parses as a finish record");
    assert_eq!(last.kind, "finish", "{last:?}");
    assert_eq!(last.outcome.conclusion, Conclusion::Success, "{last:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn watch_format_json_rejects_commit_mode() {
    let output = doneyet()
        .args(["watch", "acme/api", "--commit", "abc", "--format", "json"])
        .output()
        .expect("run binary");
    assert_eq!(output.status.code(), Some(4), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("json"), "{stderr}");
}

#[tokio::test(flavor = "multi_thread")]
async fn watch_timeout_rejects_commit_mode() {
    let output = doneyet()
        .args(["watch", "acme/api", "--commit", "abc", "--timeout", "10"])
        .output()
        .expect("run binary");
    assert_eq!(output.status.code(), Some(4), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("timeout"), "{stderr}");
}

#[tokio::test(flavor = "multi_thread")]
async fn watch_with_job_filter_filters_rendered_jobs() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/api/actions/runs"))
        .respond_with(ResponseTemplate::new(200).set_body_string(ACTIVE_RUN_PAGE))
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/api/actions/runs"))
        .respond_with(ResponseTemplate::new(200).set_body_string(COMPLETED_RUN_PAGE))
        .expect(1..)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/api/actions/runs/2841/jobs"))
        .respond_with(ResponseTemplate::new(200).set_body_string(JOB_FILTER_JOBS))
        .expect(1..)
        .mount(&server)
        .await;
    let output = doneyet()
        .args([
            "watch",
            "acme/api",
            "--job",
            "test",
            "--interval",
            "1",
            "--api-base",
            &server.uri(),
        ])
        .timeout(Duration::from_secs(30))
        .output()
        .expect("run binary");
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("test (macos-latest)"), "{stdout}");
    assert!(!stdout.contains("make (arm64)"), "{stdout}");
}

#[tokio::test(flavor = "multi_thread")]
async fn watch_job_filter_rejects_commit_mode() {
    let output = doneyet()
        .args(["watch", "acme/api", "--commit", "abc", "--job", "test"])
        .output()
        .expect("run binary");
    assert_eq!(output.status.code(), Some(4), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("job"), "{stderr}");
}

#[tokio::test(flavor = "multi_thread")]
async fn watch_timeout_exits_two_when_run_never_finishes() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/api/actions/runs"))
        .respond_with(ResponseTemplate::new(200).set_body_string(ACTIVE_RUN_PAGE))
        .expect(1..)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/api/actions/runs/2841/jobs"))
        .respond_with(ResponseTemplate::new(200).set_body_string(EMPTY_JOBS))
        .expect(1..)
        .mount(&server)
        .await;
    let started = std::time::Instant::now();
    let output = doneyet()
        .args([
            "watch",
            "acme/api",
            "--timeout",
            "1",
            "--interval",
            "1",
            "--api-base",
            &server.uri(),
        ])
        .timeout(Duration::from_secs(30))
        .output()
        .expect("run binary");
    let elapsed = started.elapsed();
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(
        elapsed < Duration::from_secs(3),
        "timeout must exit promptly, took {elapsed:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn dash_timeout_exits_two_when_runs_never_finish() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/api/actions/runs"))
        .respond_with(ResponseTemplate::new(200).set_body_string(ACTIVE_RUN_PAGE))
        .expect(1..)
        .mount(&server)
        .await;
    let started = std::time::Instant::now();
    let output = doneyet()
        .args([
            "dash",
            "acme/api",
            "--timeout",
            "1",
            "--interval",
            "1",
            "--api-base",
            &server.uri(),
        ])
        .timeout(Duration::from_secs(30))
        .output()
        .expect("run binary");
    let elapsed = started.elapsed();
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(
        elapsed < Duration::from_secs(3),
        "timeout must exit promptly, took {elapsed:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn dash_json_emits_one_line_per_page() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/api/actions/runs"))
        .respond_with(ResponseTemplate::new(200).set_body_string(ACTIVE_RUN_PAGE))
        .expect(1..)
        .mount(&server)
        .await;
    let output = doneyet()
        .args([
            "dash",
            "acme/api",
            "--json",
            "--timeout",
            "1",
            "--interval",
            "1",
            "--api-base",
            &server.uri(),
        ])
        .timeout(Duration::from_secs(30))
        .output()
        .expect("run binary");
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.is_empty(), "json mode must emit lines: {stdout}");
    assert!(
        !stdout.contains('\x1b'),
        "no terminal escapes on stdout: {stdout}"
    );
    assert!(
        !stdout.contains("(q quit"),
        "no interactive hint on stdout: {stdout}"
    );
    let lines: Vec<&str> = stdout.lines().collect();
    assert!(
        !lines.is_empty(),
        "expected at least one page line: {stdout}"
    );
    let value: serde_json::Value =
        serde_json::from_str(lines[0]).expect("first line parses as JSON");
    assert_eq!(value["kind"], "page", "{value}");
    assert!(value["ts"].is_u64(), "{value}");
    assert_eq!(value["page"]["total_count"], 1, "{value}");
    assert_eq!(value["page"]["runs"][0]["id"], 2841, "{value}");
}

#[tokio::test(flavor = "multi_thread")]
async fn dash_commit_filters_by_head_sha() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/api/actions/runs"))
        .and(wiremock::matchers::query_param("head_sha", "abc1234"))
        .respond_with(ResponseTemplate::new(200).set_body_string(ACTIVE_RUN_PAGE))
        .expect(1..)
        .mount(&server)
        .await;
    let output = doneyet()
        .args([
            "dash",
            "acme/api",
            "--commit",
            "abc1234",
            "--timeout",
            "1",
            "--interval",
            "1",
            "--api-base",
            &server.uri(),
        ])
        .timeout(Duration::from_secs(30))
        .output()
        .expect("run binary");
    assert_eq!(output.status.code(), Some(2), "{output:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn watch_with_webhook_accepts_push_and_completes() {
    use hmac::{Hmac, KeyInit, Mac};
    use sha2::Sha256;

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/api/actions/runs"))
        .respond_with(ResponseTemplate::new(200).set_body_string(ACTIVE_RUN_PAGE))
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/api/actions/runs"))
        .respond_with(ResponseTemplate::new(200).set_body_string(COMPLETED_RUN_PAGE))
        .expect(1..)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/api/actions/runs/2841/jobs"))
        .respond_with(ResponseTemplate::new(200).set_body_string(EMPTY_JOBS))
        .expect(1..)
        .mount(&server)
        .await;

    let webhook_port = 39471u16;
    let bin = env!("CARGO_BIN_EXE_doneyet");
    let child = std::process::Command::new(bin)
        .args([
            "watch",
            "acme/api",
            "--interval",
            "30",
            "--webhook",
            &format!("127.0.0.1:{webhook_port}"),
            "--webhook-secret",
            "e2e-secret",
            "--api-base",
            &server.uri(),
        ])
        .env("NO_COLOR", "1")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("spawn doneyet");

    let body = r#"{"action":"in_progress","workflow_run":{"id":2841},"repository":{"full_name":"acme/api"}}"#;
    let mut mac = <Hmac<Sha256>>::new_from_slice(b"e2e-secret").unwrap();
    mac.update(body.as_bytes());
    let signature = format!("sha256={}", hex::encode(mac.finalize().into_bytes()));

    let mut delivered = false;
    for _ in 0..100 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        let response = reqwest::Client::new()
            .post(format!("http://127.0.0.1:{webhook_port}/"))
            .header("x-github-event", "workflow_run")
            .header("x-hub-signature-256", &signature)
            .body(body)
            .send()
            .await;
        if let Ok(response) = response {
            assert_eq!(response.status(), 200);
            delivered = true;
            break;
        }
    }
    assert!(delivered, "webhook server never came up");

    let output = child.wait_with_output().expect("wait for doneyet");
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("in_progress"), "{stdout}");
    assert!(stdout.contains("success"), "{stdout}");
}

#[tokio::test(flavor = "multi_thread")]
async fn watch_with_record_writes_jsonl() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/api/actions/runs"))
        .respond_with(ResponseTemplate::new(200).set_body_string(ACTIVE_RUN_PAGE))
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/api/actions/runs"))
        .respond_with(ResponseTemplate::new(200).set_body_string(COMPLETED_RUN_PAGE))
        .expect(1..)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/api/actions/runs/2841/jobs"))
        .respond_with(ResponseTemplate::new(200).set_body_string(EMPTY_JOBS))
        .expect(1..)
        .mount(&server)
        .await;

    let record_path =
        std::env::temp_dir().join(format!("doneyet-e2e-record-{}.jsonl", std::process::id()));
    let record_arg = record_path.to_string_lossy().to_string();
    let output = doneyet()
        .args([
            "watch",
            "acme/api",
            "--interval",
            "1",
            "--record",
            &record_arg,
            "--api-base",
            &server.uri(),
        ])
        .output()
        .expect("run binary");
    assert_eq!(output.status.code(), Some(0), "{output:?}");

    let recorded = std::fs::read_to_string(&record_path).expect("recording file exists");
    std::fs::remove_file(&record_path).ok();
    let renders = recorded.matches("\"kind\":\"render\"").count();
    let finishes = recorded.matches("\"kind\":\"finish\"").count();
    assert!(renders >= 2, "expected >= 2 render records: {recorded}");
    assert_eq!(
        finishes, 1,
        "expected exactly one finish record: {recorded}"
    );
    assert!(recorded.contains("\"v\":1"), "{recorded}");
    assert!(
        recorded.contains("\"phase\":\"InProgress\""),
        "domain must roundtrip: {recorded}"
    );
}

async fn mount_flip_mocks(server: &MockServer, final_page: String) {
    use std::sync::Arc;
    let final_page = Arc::new(final_page);
    let final_page2 = final_page.clone();
    Mock::given(method("GET"))
        .and(path("/repos/acme/api/actions/runs"))
        .respond_with(ResponseTemplate::new(200).set_body_string(ACTIVE_RUN_PAGE))
        .up_to_n_times(1)
        .expect(1)
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/api/actions/runs"))
        .respond_with(ResponseTemplate::new(200).set_body_string(final_page2.as_str().to_string()))
        .expect(1..)
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/api/actions/runs/2841/jobs"))
        .respond_with(ResponseTemplate::new(200).set_body_string(EMPTY_JOBS))
        .expect(1..)
        .mount(server)
        .await;
    let _ = final_page;
}

#[tokio::test(flavor = "multi_thread")]
async fn replay_success_recording_renders_and_exits_zero() {
    let server = MockServer::start().await;
    mount_flip_mocks(&server, COMPLETED_RUN_PAGE.to_string()).await;
    let record_path =
        std::env::temp_dir().join(format!("doneyet-replay-ok-{}.jsonl", std::process::id()));
    let record_arg = record_path.to_string_lossy().to_string();
    let watch = doneyet()
        .args([
            "watch",
            "acme/api",
            "--interval",
            "1",
            "--record",
            &record_arg,
            "--api-base",
            &server.uri(),
        ])
        .output()
        .expect("record a watch");
    assert_eq!(watch.status.code(), Some(0), "{watch:?}");
    assert!(record_path.exists(), "recording file must exist");

    let replay = doneyet()
        .args(["replay", &record_arg, "--no-color"])
        .output()
        .expect("replay the recording");
    std::fs::remove_file(&record_path).ok();
    assert_eq!(replay.status.code(), Some(0), "{replay:?}");
    let stdout = String::from_utf8_lossy(&replay.stdout);
    assert!(stdout.contains("in_progress"), "{stdout}");
    assert!(stdout.contains("success"), "{stdout}");
}

#[tokio::test(flavor = "multi_thread")]
async fn replay_failure_recording_exits_one() {
    let server = MockServer::start().await;
    mount_flip_mocks(&server, COMPLETED_RUN_PAGE.replace("success", "failure")).await;
    let record_path =
        std::env::temp_dir().join(format!("doneyet-replay-fail-{}.jsonl", std::process::id()));
    let record_arg = record_path.to_string_lossy().to_string();
    let watch = doneyet()
        .args([
            "watch",
            "acme/api",
            "--interval",
            "1",
            "--record",
            &record_arg,
            "--api-base",
            &server.uri(),
        ])
        .output()
        .expect("record a failing watch");
    assert_eq!(watch.status.code(), Some(1), "{watch:?}");

    let replay = doneyet()
        .args(["replay", &record_arg, "--no-color"])
        .output()
        .expect("replay the failure");
    std::fs::remove_file(&record_path).ok();
    assert_eq!(replay.status.code(), Some(1), "{replay:?}");
    let stdout = String::from_utf8_lossy(&replay.stdout);
    assert!(stdout.contains("failure"), "{stdout}");
}

#[tokio::test(flavor = "multi_thread")]
async fn replay_malformed_file_errors_with_exit_four() {
    let path =
        std::env::temp_dir().join(format!("doneyet-replay-bad-{}.jsonl", std::process::id()));
    std::fs::write(&path, "definitely not jsonl\n").expect("write bad recording");
    let output = doneyet()
        .args(["replay", &path.to_string_lossy(), "--no-color"])
        .output()
        .expect("run replay");
    std::fs::remove_file(&path).ok();
    assert_eq!(output.status.code(), Some(4), "{output:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn bad_token_reports_auth_error_with_exit_four() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/api/actions/runs"))
        .respond_with(
            ResponseTemplate::new(401).set_body_string(r#"{"message": "Bad credentials"}"#),
        )
        .expect(1..)
        .mount(&server)
        .await;
    let output = doneyet()
        .env("DONEYET_TOKEN", "definitely-broken")
        .args(["runs", "acme/api", "--api-base", &server.uri()])
        .output()
        .expect("run binary");
    assert_eq!(output.status.code(), Some(4), "{output:?}");
}

const FAILED_RUN_JOBS: &str = r#"{"total_count":1,"jobs":[{"id":399444496,"run_id":2841,"status":"completed","conclusion":"failure","name":"test (macos-latest)","started_at":"2026-09-21T10:00:02Z","completed_at":"2026-09-21T10:01:04Z","runner_name":null,"labels":["macos-latest"],"steps":[{"name":"Run cargo test","number":2,"status":"completed","conclusion":"failure","started_at":"2026-09-21T10:00:06Z","completed_at":"2026-09-21T10:00:20Z"}]}]}"#;

#[tokio::test(flavor = "multi_thread")]
async fn run_shows_annotations_and_log_tail_for_failures() {
    let server = MockServer::start().await;
    let failed_run = doneyet_contract::fixtures::RUN.replace("success", "failure");
    Mock::given(method("GET"))
        .and(path("/repos/acme/api/actions/runs/2841"))
        .respond_with(ResponseTemplate::new(200).set_body_string(failed_run))
        .expect(1..)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/api/actions/runs/2841/jobs"))
        .respond_with(ResponseTemplate::new(200).set_body_string(FAILED_RUN_JOBS))
        .expect(1..)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/api/check-runs/399444496/annotations"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(doneyet_contract::fixtures::ANNOTATIONS),
        )
        .expect(1)
        .mount(&server)
        .await;
    let log_body = "2026-09-21T10:00:06Z cargo test\n2026-09-21T10:00:19Z thread panicked\n2026-09-21T10:00:20Z error: test failed\n";
    Mock::given(method("GET"))
        .and(path("/repos/acme/api/actions/jobs/399444496/logs"))
        .respond_with(ResponseTemplate::new(200).set_body_string(log_body))
        .expect(1)
        .mount(&server)
        .await;

    let output = doneyet()
        .args([
            "run",
            "2841",
            "--repo",
            "acme/api",
            "--logs-failed",
            "2",
            "--api-base",
            &server.uri(),
        ])
        .output()
        .expect("run binary");
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("src/lib.rs:42"),
        "annotation inline: {stdout}"
    );
    assert!(
        stdout.contains("unused variable"),
        "annotation message: {stdout}"
    );
    assert!(
        stdout.contains("logs: test (macos-latest)"),
        "log header: {stdout}"
    );
    assert!(
        stdout.contains("error: test failed"),
        "log tail shown: {stdout}"
    );
    assert!(
        !stdout.contains("cargo test\n2026"),
        "only the requested tail is printed: {stdout}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn run_without_logs_flag_shows_annotations_but_no_logs() {
    let server = MockServer::start().await;
    let failed_run = doneyet_contract::fixtures::RUN.replace("success", "failure");
    Mock::given(method("GET"))
        .and(path("/repos/acme/api/actions/runs/2841"))
        .respond_with(ResponseTemplate::new(200).set_body_string(failed_run))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/api/actions/runs/2841/jobs"))
        .respond_with(ResponseTemplate::new(200).set_body_string(FAILED_RUN_JOBS))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/api/check-runs/399444496/annotations"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(doneyet_contract::fixtures::ANNOTATIONS),
        )
        .expect(1)
        .mount(&server)
        .await;
    let output = doneyet()
        .args([
            "run",
            "2841",
            "--repo",
            "acme/api",
            "--api-base",
            &server.uri(),
        ])
        .output()
        .expect("run binary");
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("src/lib.rs:42"), "{stdout}");
    assert!(
        !stdout.contains("── logs:"),
        "no log section without the flag: {stdout}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn watch_notify_fires_desktop_notification_on_completion() {
    use std::os::unix::fs::PermissionsExt;

    let dir = std::env::temp_dir().join(format!("doneyet-notify-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create fake bin dir");
    let marker = dir.join("marker");
    let script = dir.join("notify-send");
    std::fs::write(
        &script,
        format!("#!/bin/sh\necho \"$@\" >> {}\n", marker.display()),
    )
    .expect("write fake notify-send");
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))
        .expect("make fake executable");

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/api/actions/runs"))
        .respond_with(ResponseTemplate::new(200).set_body_string(COMPLETED_RUN_PAGE))
        .expect(1..)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/api/actions/runs/2841/jobs"))
        .respond_with(ResponseTemplate::new(200).set_body_string(EMPTY_JOBS))
        .expect(1..)
        .mount(&server)
        .await;
    let path = std::env::var("PATH").unwrap_or_default();
    let output = doneyet()
        .args(["watch", "acme/api", "--notify", "--api-base", &server.uri()])
        .env("PATH", format!("{}:{path}", dir.display()))
        .timeout(Duration::from_secs(30))
        .output()
        .expect("run binary");
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let called = std::fs::read_to_string(&marker).expect("notify-send was invoked");
    assert!(called.contains("doneyet"), "{called}");
    assert!(called.contains("succeeded"), "{called}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn watch_run_id_follows_that_specific_run() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/api/actions/runs/2841"))
        .respond_with(ResponseTemplate::new(200).set_body_string(doneyet_contract::fixtures::RUN))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/api/actions/runs/2841/jobs"))
        .respond_with(ResponseTemplate::new(200).set_body_string(EMPTY_JOBS))
        .expect(1..)
        .mount(&server)
        .await;
    let output = doneyet()
        .args([
            "watch",
            "acme/api",
            "--run-id",
            "2841",
            "--api-base",
            &server.uri(),
        ])
        .timeout(Duration::from_secs(30))
        .output()
        .expect("run binary");
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("#2841"), "{stdout}");
}

fn run_json(id: u64, status: &str, conclusion: Option<&str>) -> String {
    format!(
        r#"{{"id":{id},"run_number":{id},"name":"wf{id}.yml","display_title":"build & test","head_branch":"main","head_sha":"deadbeef","event":"push","status":"{status}","conclusion":{},"actor":{{"login":"jonas"}},"html_url":"https://github.com/acme/api/actions/runs/{id}","created_at":"2026-09-21T10:00:00Z","run_started_at":"2026-09-21T10:00:01Z","updated_at":"2026-09-21T10:03:00Z"}}"#,
        conclusion
            .map(|c| format!("\"{c}\""))
            .unwrap_or_else(|| "null".to_string())
    )
}

fn runs_page_json(runs: &[String]) -> String {
    format!(
        r#"{{"total_count":{},"workflow_runs":[{}]}}"#,
        runs.len(),
        runs.join(",")
    )
}

#[tokio::test(flavor = "multi_thread")]
async fn watch_commit_follows_all_workflows_and_exits_worst() {
    let active = runs_page_json(&[
        run_json(2841, "in_progress", None),
        run_json(2842, "in_progress", None),
    ]);
    let finished = runs_page_json(&[
        run_json(2841, "completed", Some("success")),
        run_json(2842, "completed", Some("failure")),
    ]);
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/api/actions/runs"))
        .and(wiremock::matchers::query_param("head_sha", "deadbeef"))
        .respond_with(ResponseTemplate::new(200).set_body_string(active))
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/api/actions/runs"))
        .and(wiremock::matchers::query_param("head_sha", "deadbeef"))
        .respond_with(ResponseTemplate::new(200).set_body_string(finished))
        .expect(1..)
        .mount(&server)
        .await;
    let output = doneyet()
        .args([
            "watch",
            "acme/api",
            "--commit",
            "deadbeef",
            "--interval",
            "1",
            "--api-base",
            &server.uri(),
        ])
        .timeout(Duration::from_secs(30))
        .output()
        .expect("run binary");
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("#2841"), "{stdout}");
    assert!(stdout.contains("#2842"), "{stdout}");
    assert!(stdout.contains("failure"), "{stdout}");
}

fn hook_marker(name: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("doneyet-hook-{name}-{}", std::process::id()));
    let _ = std::fs::remove_file(&path);
    path
}

#[tokio::test(flavor = "multi_thread")]
async fn watch_on_success_hook_runs_when_run_passes() {
    let server = MockServer::start().await;
    mount_flip_mocks(&server, COMPLETED_RUN_PAGE.to_string()).await;
    let marker = hook_marker("on-success");
    let hook = format!("echo ok >> {}", marker.display());
    let output = doneyet()
        .args([
            "watch",
            "acme/api",
            "--interval",
            "1",
            "--on-success",
            &hook,
            "--api-base",
            &server.uri(),
        ])
        .timeout(Duration::from_secs(30))
        .output()
        .expect("run binary");
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let contents = std::fs::read_to_string(&marker).expect("success hook must write the marker");
    assert_eq!(contents.trim(), "ok", "{contents}");
    let _ = std::fs::remove_file(&marker);
}

#[tokio::test(flavor = "multi_thread")]
async fn watch_on_failure_hook_runs_when_run_fails_and_not_success() {
    let server = MockServer::start().await;
    mount_flip_mocks(&server, COMPLETED_RUN_PAGE.replace("success", "failure")).await;
    let marker_ok = hook_marker("both-success");
    let marker_fail = hook_marker("both-failure");
    let hook_ok = format!("echo yes >> {}", marker_ok.display());
    let hook_fail = format!("echo no >> {}", marker_fail.display());
    let output = doneyet()
        .args([
            "watch",
            "acme/api",
            "--interval",
            "1",
            "--on-success",
            &hook_ok,
            "--on-failure",
            &hook_fail,
            "--api-base",
            &server.uri(),
        ])
        .timeout(Duration::from_secs(30))
        .output()
        .expect("run binary");
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let contents = std::fs::read_to_string(&marker_fail).expect("failure hook must write markerB");
    assert_eq!(contents.trim(), "no", "{contents}");
    assert!(
        !marker_ok.exists(),
        "success hook must not run on a failed run"
    );
    let _ = std::fs::remove_file(&marker_fail);
}

#[tokio::test(flavor = "multi_thread")]
async fn watch_hook_failure_does_not_change_exit_code() {
    let server = MockServer::start().await;
    mount_flip_mocks(&server, COMPLETED_RUN_PAGE.to_string()).await;
    let output = doneyet()
        .args([
            "watch",
            "acme/api",
            "--interval",
            "1",
            "--on-success",
            "false",
            "--api-base",
            &server.uri(),
        ])
        .timeout(Duration::from_secs(30))
        .output()
        .expect("run binary");
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("hook failed"), "{stderr}");
}

#[tokio::test(flavor = "multi_thread")]
async fn watch_hooks_do_not_run_on_timeout() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/api/actions/runs"))
        .respond_with(ResponseTemplate::new(200).set_body_string(ACTIVE_RUN_PAGE))
        .expect(1..)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/api/actions/runs/2841/jobs"))
        .respond_with(ResponseTemplate::new(200).set_body_string(EMPTY_JOBS))
        .expect(1..)
        .mount(&server)
        .await;
    let marker = hook_marker("timeout");
    let hook = format!("touch {}", marker.display());
    let output = doneyet()
        .args([
            "watch",
            "acme/api",
            "--timeout",
            "1",
            "--interval",
            "1",
            "--on-success",
            &hook,
            "--api-base",
            &server.uri(),
        ])
        .timeout(Duration::from_secs(30))
        .output()
        .expect("run binary");
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(!marker.exists(), "hooks must not run on --timeout");
}

#[tokio::test(flavor = "multi_thread")]
async fn rerun_requests_rerun_and_exits_zero() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/repos/acme/api/actions/runs/2841/rerun"))
        .respond_with(ResponseTemplate::new(201))
        .expect(1)
        .mount(&server)
        .await;
    let output = doneyet()
        .args([
            "rerun",
            "2841",
            "--repo",
            "acme/api",
            "--api-base",
            &server.uri(),
        ])
        .output()
        .expect("run binary");
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("doneyet: rerun queued for run 2841"),
        "{stdout}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn rerun_failed_only_hits_failed_jobs_endpoint() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/repos/acme/api/actions/runs/2841/rerun-failed-jobs"))
        .respond_with(ResponseTemplate::new(201))
        .expect(1)
        .mount(&server)
        .await;
    let output = doneyet()
        .args([
            "rerun",
            "2841",
            "--failed-only",
            "--repo",
            "acme/api",
            "--api-base",
            &server.uri(),
        ])
        .output()
        .expect("run binary");
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("doneyet: rerun queued for run 2841"),
        "{stdout}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn cancel_requests_cancel_and_exits_zero() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/repos/acme/api/actions/runs/2841/cancel"))
        .respond_with(ResponseTemplate::new(202))
        .expect(1)
        .mount(&server)
        .await;
    let output = doneyet()
        .args([
            "cancel",
            "2841",
            "--repo",
            "acme/api",
            "--api-base",
            &server.uri(),
        ])
        .output()
        .expect("run binary");
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("doneyet: cancel requested for run 2841"),
        "{stdout}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn rerun_api_error_exits_four() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/repos/acme/api/actions/runs/2841/rerun"))
        .respond_with(ResponseTemplate::new(404))
        .expect(1)
        .mount(&server)
        .await;
    let output = doneyet()
        .args([
            "rerun",
            "2841",
            "--repo",
            "acme/api",
            "--api-base",
            &server.uri(),
        ])
        .output()
        .expect("run binary");
    assert_eq!(output.status.code(), Some(4), "{output:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn watch_pr_resolves_head_sha_and_follows_runs() {
    let finished = runs_page_json(&[run_json(2841, "completed", Some("success"))]);
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/api/pulls/7"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(r#"{"number":7,"head":{"sha":"deadbeef"}}"#),
        )
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/api/actions/runs"))
        .and(wiremock::matchers::query_param("head_sha", "deadbeef"))
        .respond_with(ResponseTemplate::new(200).set_body_string(finished))
        .expect(1..)
        .mount(&server)
        .await;
    let output = doneyet()
        .args([
            "watch",
            "acme/api",
            "--pr",
            "7",
            "--interval",
            "1",
            "--api-base",
            &server.uri(),
        ])
        .timeout(Duration::from_secs(30))
        .output()
        .expect("run binary");
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("#2841"), "{stdout}");
    assert!(
        stdout.contains("all runs for deadbeef finished"),
        "{stdout}"
    );
}
