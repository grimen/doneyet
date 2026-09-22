use anyhow::{Context, Result};
use doneyet_core::model::RepoRef;
use std::str::FromStr;

pub fn resolve(arg: Option<&str>) -> Result<RepoRef> {
    resolve_with(arg, fetch_origin_remote)
}

pub fn resolve_with(
    arg: Option<&str>,
    fetch_remote: impl FnOnce() -> Option<String>,
) -> Result<RepoRef> {
    if let Some(raw) = arg {
        return RepoRef::from_str(raw).map_err(|e| anyhow::anyhow!("{e}"));
    }
    let url = fetch_remote()
        .context("no OWNER/NAME given and no origin remote found in the current directory")?;
    from_remote_url(&url)
        .with_context(|| format!("could not derive OWNER/NAME from remote {url:?}"))
}

fn fetch_origin_remote() -> Option<String> {
    std::process::Command::new("git")
        .args(["remote", "get-url", "origin"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|url| !url.is_empty())
}

pub fn detect_branch() -> Option<String> {
    detect_branch_in(std::env::current_dir().ok()?.as_path())
}

pub fn detect_branch_in(dir: &std::path::Path) -> Option<String> {
    std::process::Command::new("git")
        .args(["symbolic-ref", "--short", "HEAD"])
        .current_dir(dir)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|branch| !branch.is_empty())
}

pub fn from_remote_url(url: &str) -> Option<RepoRef> {
    let trimmed = url.trim().trim_end_matches(".git");
    let path = if let Some(rest) = trimmed.strip_prefix("https://github.com/") {
        rest
    } else if let Some(rest) = trimmed.strip_prefix("git@github.com:") {
        rest
    } else if let Some(rest) = trimmed.strip_prefix("ssh://git@github.com/") {
        rest
    } else {
        trimmed
    };
    RepoRef::from_str(path.trim_end_matches('/')).ok()
}
