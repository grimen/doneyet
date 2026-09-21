use doneyet_github::token::resolve_token;
use std::collections::HashMap;

fn env(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

#[test]
fn prefers_doneyet_token_over_all_other_sources() {
    let e = env(&[
        ("DONEYET_TOKEN", " d1 "),
        ("GH_TOKEN", "gh"),
        ("GITHUB_TOKEN", "ghe"),
    ]);
    let got = resolve_token(&|k| e.get(k).cloned(), || Some("cli".to_string()));
    assert_eq!(got.as_deref(), Some("d1"));
}

#[test]
fn falls_back_to_gh_token_then_github_token() {
    let e = env(&[("GH_TOKEN", " gh "), ("GITHUB_TOKEN", "ghe")]);
    assert_eq!(
        resolve_token(&|k| e.get(k).cloned(), || None).as_deref(),
        Some("gh")
    );
    let e = env(&[("GITHUB_TOKEN", " ghe ")]);
    assert_eq!(
        resolve_token(&|k| e.get(k).cloned(), || None).as_deref(),
        Some("ghe")
    );
}

#[test]
fn gh_cli_is_the_last_resort() {
    let e = env(&[]);
    assert_eq!(
        resolve_token(&|k| e.get(k).cloned(), || Some(" cli ".to_string())).as_deref(),
        Some("cli")
    );
}

#[test]
fn blank_tokens_are_skipped() {
    let e = env(&[("DONEYET_TOKEN", "   "), ("GITHUB_TOKEN", " real ")]);
    assert_eq!(
        resolve_token(&|k| e.get(k).cloned(), || None).as_deref(),
        Some("real")
    );
}

#[test]
fn no_token_yields_none() {
    assert_eq!(resolve_token(&|_| None, || None), None);
}
