use assert_cmd::Command;
use std::io::Write;
use std::sync::Mutex;
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

static ENV_LOCK: Mutex<()> = Mutex::new(());

fn config_path(label: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "doneyet-config-{label}-{}.toml",
        std::process::id()
    ))
}

fn write_config(path: &std::path::Path, contents: &str) {
    std::fs::File::create(path)
        .expect("create config")
        .write_all(contents.as_bytes())
        .expect("write config");
}

fn remove_config(path: &std::path::Path) {
    std::fs::remove_file(path).ok();
}

#[test]
fn load_reads_all_four_keys() {
    let _guard = ENV_LOCK.lock().expect("env lock");
    let path = config_path("all");
    write_config(
        &path,
        r#"
interval = 7
theme = "ascii"
notify = true
api_base = "https://example.test/api"
"#,
    );
    unsafe {
        std::env::set_var("DONEYET_CONFIG", &path);
    }
    let config = doneyet_cli::config::Config::load().expect("valid config loads");
    assert_eq!(config.interval, Some(7));
    assert_eq!(config.theme.as_deref(), Some("ascii"));
    assert_eq!(config.notify, Some(true));
    assert_eq!(config.api_base.as_deref(), Some("https://example.test/api"));
    unsafe {
        std::env::remove_var("DONEYET_CONFIG");
    }
    remove_config(&path);
}

#[test]
fn load_missing_config_is_default() {
    let _guard = ENV_LOCK.lock().expect("env lock");
    let path = config_path("missing");
    unsafe {
        std::env::set_var("DONEYET_CONFIG", &path);
    }
    let config = doneyet_cli::config::Config::load().expect("missing config is not an error");
    assert_eq!(
        config,
        doneyet_cli::config::Config::default(),
        "absent file must produce built-in defaults"
    );
    unsafe {
        std::env::remove_var("DONEYET_CONFIG");
    }
}

#[test]
fn load_invalid_config_errors() {
    let _guard = ENV_LOCK.lock().expect("env lock");
    let path = config_path("bad");
    write_config(&path, "interval = not-a-number");
    unsafe {
        std::env::set_var("DONEYET_CONFIG", &path);
    }
    let error = doneyet_cli::config::Config::load().expect_err("garbage config must fail");
    assert!(error.to_string().contains("invalid config"), "{error}");
    unsafe {
        std::env::remove_var("DONEYET_CONFIG");
    }
    remove_config(&path);
}

#[tokio::test(flavor = "multi_thread")]
async fn config_api_base_is_used_without_cli_flag() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(wiremock::matchers::path("/repos/acme/api/actions/runs"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(doneyet_contract::fixtures::RUNS_PAGE),
        )
        .mount(&server)
        .await;
    let path = config_path("e2e-base");
    write_config(&path, &format!("api_base = \"{}\"\n", server.uri()));
    let output = Command::cargo_bin("doneyet")
        .expect("binary")
        .args(["runs", "acme/api"])
        .env("DONEYET_CONFIG", &path)
        .output()
        .expect("run binary");
    remove_config(&path);
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("#2842"), "{stdout}");
}

#[tokio::test(flavor = "multi_thread")]
async fn cli_flag_overrides_config_api_base() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(wiremock::matchers::path("/repos/acme/api/actions/runs"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(doneyet_contract::fixtures::RUNS_PAGE),
        )
        .mount(&server)
        .await;
    let path = config_path("e2e-override");
    write_config(&path, "api_base = \"https://127.0.0.1:1\"\n");
    let output = Command::cargo_bin("doneyet")
        .expect("binary")
        .args(["runs", "acme/api", "--api-base", &server.uri()])
        .env("DONEYET_CONFIG", &path)
        .output()
        .expect("run binary");
    remove_config(&path);
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("#2842"), "{stdout}");
}

#[test]
fn invalid_config_exits_with_error_code() {
    let path = config_path("e2e-invalid");
    write_config(&path, "[interval");
    let output = Command::cargo_bin("doneyet")
        .expect("binary")
        .args(["runs", "acme/api"])
        .env("DONEYET_CONFIG", &path)
        .output()
        .expect("run binary");
    remove_config(&path);
    assert_eq!(output.status.code(), Some(4), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("invalid config"), "{stderr}");
}
