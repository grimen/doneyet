pub fn resolve_token(
    env: &dyn Fn(&str) -> Option<String>,
    gh_auth_token: impl FnOnce() -> Option<String>,
) -> Option<String> {
    ["DONEYET_TOKEN", "GH_TOKEN", "GITHUB_TOKEN"]
        .into_iter()
        .find_map(|key| non_empty(env(key)))
        .or_else(|| non_empty(gh_auth_token()))
}

fn non_empty(raw: Option<String>) -> Option<String> {
    raw.map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub fn resolve_from_environment() -> Option<String> {
    resolve_token(&|key| std::env::var(key).ok(), || {
        std::process::Command::new("gh")
            .args(["auth", "token"])
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
    })
}
