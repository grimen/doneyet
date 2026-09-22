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

#[test]
fn config_path_prints_effective_default() {
    let home = std::env::temp_dir().join(format!("doneyet-home-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    std::fs::create_dir_all(&home).expect("create home");
    let output = Command::cargo_bin("doneyet")
        .expect("binary")
        .args(["config", "path"])
        .env("HOME", &home)
        .env_remove("DONEYET_CONFIG")
        .output()
        .expect("run binary");
    let _ = std::fs::remove_dir_all(&home);
    assert!(output.status.success(), "{output:?}");
    let expected = home.join(".config").join("doneyet").join("config.toml");
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    assert_eq!(stdout, expected.to_string_lossy().to_string(), "{output:?}");
}

#[test]
fn config_path_uses_doneyet_config_env() {
    let custom = std::env::temp_dir().join(format!("doneyet-custom-{}.toml", std::process::id()));
    std::fs::remove_file(&custom).ok();
    let output = Command::cargo_bin("doneyet")
        .expect("binary")
        .args(["config", "path"])
        .env("DONEYET_CONFIG", &custom)
        .output()
        .expect("run binary");
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    assert_eq!(stdout, custom.to_string_lossy().to_string(), "{output:?}");
}

#[test]
fn config_init_writes_default_file() {
    let path = std::env::temp_dir().join(format!("doneyet-init-new-{}.toml", std::process::id()));
    std::fs::remove_file(&path).ok();
    let output = Command::cargo_bin("doneyet")
        .expect("binary")
        .args(["config", "init"])
        .env("DONEYET_CONFIG", &path)
        .output()
        .expect("run binary");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("doneyet: wrote"), "{stdout}");
    assert!(
        stdout.contains(&path.to_string_lossy().to_string()),
        "{stdout}"
    );
    let contents = std::fs::read_to_string(&path).expect("config file written");
    for key in ["interval", "theme", "notify", "api_base"] {
        assert!(contents.contains(key), "missing key {key}: {contents}");
    }
    std::fs::remove_file(&path).ok();
}

#[test]
fn config_init_refuses_existing_without_force() {
    let path =
        std::env::temp_dir().join(format!("doneyet-init-refuse-{}.toml", std::process::id()));
    std::fs::write(&path, "sentinel = true\n").expect("write sentinel config");
    let output = Command::cargo_bin("doneyet")
        .expect("binary")
        .args(["config", "init"])
        .env("DONEYET_CONFIG", &path)
        .output()
        .expect("run binary");
    assert_eq!(output.status.code(), Some(4), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(&path.to_string_lossy().to_string()),
        "{stderr}"
    );
    assert!(stderr.contains("--force"), "{stderr}");
    let contents = std::fs::read_to_string(&path).expect("config file still readable");
    assert_eq!(
        contents, "sentinel = true\n",
        "existing config must be untouched"
    );
    std::fs::remove_file(&path).ok();
}

#[test]
fn config_init_force_overwrites() {
    let path = std::env::temp_dir().join(format!("doneyet-init-force-{}.toml", std::process::id()));
    std::fs::write(&path, "sentinel = true\n").expect("write sentinel config");
    let output = Command::cargo_bin("doneyet")
        .expect("binary")
        .args(["config", "init", "--force"])
        .env("DONEYET_CONFIG", &path)
        .output()
        .expect("run binary");
    assert!(output.status.success(), "{output:?}");
    let contents = std::fs::read_to_string(&path).expect("config file written");
    for key in ["interval", "theme", "notify", "api_base"] {
        assert!(contents.contains(key), "missing key {key}: {contents}");
    }
    assert!(!contents.contains("sentinel"), "{contents}");
    std::fs::remove_file(&path).ok();
}
