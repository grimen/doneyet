use doneyet_cli::{inject_default_subcommand, repo};
use doneyet_core::model::RepoRef;
use std::str::FromStr;

fn acme_api() -> RepoRef {
    RepoRef::from_str("acme/api").unwrap()
}

#[test]
fn bare_repo_arg_becomes_watch() {
    let out = inject_default_subcommand(vec!["doneyet".to_string(), "acme/api".to_string()]);
    assert_eq!(
        out,
        vec![
            "doneyet".to_string(),
            "watch".to_string(),
            "acme/api".to_string()
        ]
    );
}

#[test]
fn bare_repo_arg_with_flags_becomes_watch() {
    let out = inject_default_subcommand(vec![
        "doneyet".to_string(),
        "acme/api".to_string(),
        "-b".to_string(),
        "main".to_string(),
    ]);
    assert_eq!(out[1], "watch");
    assert_eq!(out[2], "acme/api");
}

#[test]
fn watch_logs_flag_defaults_to_twenty() {
    use clap::Parser;
    use doneyet_cli::{Cli, Command};

    let cli = Cli::parse_from(["doneyet", "watch", "acme/api", "--logs"]);
    match cli.command {
        Command::Watch { logs, .. } => assert_eq!(logs, Some(20)),
        other => panic!("expected watch, got {other:?}"),
    }
}

#[test]
fn watch_logs_flag_accepts_an_explicit_count() {
    use clap::Parser;
    use doneyet_cli::{Cli, Command};

    let cli = Cli::parse_from(["doneyet", "watch", "acme/api", "--logs", "5"]);
    match cli.command {
        Command::Watch { logs, .. } => assert_eq!(logs, Some(5)),
        other => panic!("expected watch, got {other:?}"),
    }
}

#[test]
fn grep_requires_logs() {
    use clap::Parser;
    use doneyet_cli::Cli;

    let err = Cli::try_parse_from(["doneyet", "watch", "acme/api", "--grep", "error"]);
    assert!(err.is_err(), "--grep must require --logs");
}

#[test]
fn grep_parses_alongside_logs() {
    use clap::Parser;
    use doneyet_cli::{Cli, Command};

    let cli = Cli::parse_from(["doneyet", "watch", "acme/api", "--logs", "--grep", "error"]);
    match cli.command {
        Command::Watch { logs, grep, .. } => {
            assert_eq!(logs, Some(20));
            assert_eq!(grep.as_deref(), Some("error"));
        }
        other => panic!("expected watch, got {other:?}"),
    }
}

#[test]
fn watch_run_id_parses() {
    use clap::Parser;
    use doneyet_cli::{Cli, Command};

    let cli = Cli::parse_from(["doneyet", "watch", "acme/api", "--run-id", "2841"]);
    match cli.command {
        Command::Watch { run_id, .. } => assert_eq!(run_id, Some(2841)),
        other => panic!("expected watch, got {other:?}"),
    }
}

#[test]
fn watch_run_id_conflicts_with_commit() {
    use clap::Parser;
    use doneyet_cli::Cli;

    let err = Cli::try_parse_from([
        "doneyet", "watch", "acme/api", "--run-id", "1", "--commit", "x",
    ]);
    assert!(err.is_err(), "--run-id must conflict with --commit");
}

#[test]
fn watch_pr_parses() {
    use clap::Parser;
    use doneyet_cli::{Cli, Command};

    let cli = Cli::parse_from(["doneyet", "watch", "acme/api", "--pr", "7"]);
    match cli.command {
        Command::Watch { pr, .. } => assert_eq!(pr, Some(7)),
        other => panic!("expected watch, got {other:?}"),
    }
}

#[test]
fn watch_pr_conflicts_with_commit() {
    use clap::Parser;
    use doneyet_cli::Cli;

    let err = Cli::try_parse_from(["doneyet", "watch", "acme/api", "--pr", "7", "--commit", "x"]);
    assert!(err.is_err(), "--pr must conflict with --commit");
}

#[test]
fn dash_is_a_subcommand_not_a_repo() {
    let out = inject_default_subcommand(vec!["doneyet".to_string(), "dash".to_string()]);
    assert_eq!(out, vec!["doneyet".to_string(), "dash".to_string()]);
}

#[test]
fn known_subcommands_are_untouched() {
    for arg in [
        "watch",
        "runs",
        "run",
        "rerun",
        "cancel",
        "dash",
        "config",
        "completions",
        "help",
        "--version",
    ] {
        let out = inject_default_subcommand(vec!["doneyet".to_string(), arg.to_string()]);
        assert_eq!(out, vec!["doneyet".to_string(), arg.to_string()], "{arg}");
    }
}

#[test]
fn flags_are_untouched() {
    let out = inject_default_subcommand(vec!["doneyet".to_string(), "--version".to_string()]);
    assert_eq!(out.len(), 2);
}

#[test]
fn https_remote_urls_parse() {
    assert_eq!(
        repo::from_remote_url("https://github.com/acme/api.git"),
        Some(acme_api())
    );
    assert_eq!(
        repo::from_remote_url("https://github.com/acme/api"),
        Some(acme_api())
    );
}

#[test]
fn ssh_remote_urls_parse() {
    assert_eq!(
        repo::from_remote_url("git@github.com:acme/api.git"),
        Some(acme_api())
    );
    assert_eq!(
        repo::from_remote_url("ssh://git@github.com/acme/api.git"),
        Some(acme_api())
    );
}

#[test]
fn plain_owner_name_parses_and_garbage_fails() {
    assert_eq!(repo::from_remote_url("acme/api"), Some(acme_api()));
    assert_eq!(repo::from_remote_url("https://github.com/a/b/c"), None);
}

#[test]
fn explicit_arg_skips_the_remote_fetcher() {
    let called = std::cell::Cell::new(false);
    let resolved = repo::resolve_with(Some("acme/api"), || {
        called.set(true);
        Some("https://github.com/nope/nope.git".to_string())
    })
    .expect("arg parses");
    assert_eq!(resolved, acme_api());
    assert!(
        !called.get(),
        "remote fetcher must not run when OWNER/NAME is given"
    );
}

#[test]
fn missing_arg_falls_back_to_remote() {
    let resolved = repo::resolve_with(None, || Some("https://github.com/acme/api.git".to_string()))
        .expect("remote parses");
    assert_eq!(resolved, acme_api());
}

#[test]
fn missing_arg_without_remote_errors() {
    let err = repo::resolve_with(None, || None).expect_err("no remote");
    assert!(err.to_string().contains("origin remote"), "{err}");
}

#[test]
fn unparseable_remote_errors_with_context() {
    let err = repo::resolve_with(None, || Some("https://github.com/a/b/c".to_string()))
        .expect_err("garbage remote");
    assert!(err.to_string().contains("could not derive"), "{err}");
}

fn git(dir: &std::path::Path, args: &[&str]) {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("run git");
    assert!(output.status.success(), "git {args:?} failed: {output:?}");
}

#[test]
fn detect_branch_in_reads_the_current_git_branch() {
    let dir = std::env::temp_dir().join(format!("doneyet-detect-branch-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp repo dir");
    git(&dir, &["init"]);
    git(&dir, &["symbolic-ref", "HEAD", "refs/heads/feature/xyz"]);
    assert_eq!(
        repo::detect_branch_in(&dir),
        Some("feature/xyz".to_string())
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn detect_branch_in_returns_none_outside_a_git_checkout() {
    let dir = std::env::temp_dir().join(format!("doneyet-detect-plain-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let nested = dir.join("nested");
    std::fs::create_dir_all(&nested).expect("create plain dir");
    assert_eq!(repo::detect_branch_in(&nested), None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn detect_branch_in_returns_none_on_detached_head() {
    let dir = std::env::temp_dir().join(format!("doneyet-detect-detached-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp repo dir");
    git(&dir, &["init"]);
    git(
        &dir,
        &[
            "-c",
            "user.email=test@example.com",
            "-c",
            "user.name=test",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--allow-empty",
            "-m",
            "init",
        ],
    );
    git(&dir, &["checkout", "--detach"]);
    assert_eq!(repo::detect_branch_in(&dir), None);
    let _ = std::fs::remove_dir_all(&dir);
}
