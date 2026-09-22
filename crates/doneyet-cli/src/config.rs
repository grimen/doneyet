use serde::Deserialize;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

const DEFAULT_CONFIG: &str = r#"interval = 3        # poll interval (watch and dash), seconds
theme = "ascii"     # built-in theme name or path to a JSON theme file
notify = true       # fire notify-send when a watched run finishes
api_base = "https://github.example/api/v3"   # GitHub API base URL
"#;

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct Config {
    #[serde(default)]
    pub interval: Option<u64>,
    #[serde(default)]
    pub theme: Option<String>,
    #[serde(default)]
    pub notify: Option<bool>,
    #[serde(default)]
    pub api_base: Option<String>,
}

impl Config {
    pub fn load() -> anyhow::Result<Config> {
        for path in candidate_paths() {
            let contents = match std::fs::read_to_string(&path) {
                Ok(contents) => contents,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => {
                    return Err(anyhow::anyhow!("cannot read {}: {error}", path.display()));
                }
            };
            let config = toml::from_str(&contents)
                .map_err(|error| anyhow::anyhow!("invalid config {}: {error}", path.display()))?;
            return Ok(config);
        }
        Ok(Config::default())
    }

    pub fn api_base_or(&self, cli: Option<String>) -> String {
        cli.or_else(|| self.api_base.clone())
            .unwrap_or_else(|| doneyet_github::DEFAULT_API_BASE.to_string())
    }

    pub fn theme_or(&self, cli: Option<String>) -> String {
        cli.or_else(|| self.theme.clone())
            .unwrap_or_else(|| "default".to_string())
    }

    pub fn interval_or(&self, cli: Option<u64>, default: u64) -> u64 {
        cli.or(self.interval).unwrap_or(default)
    }

    pub fn notify_or(&self, cli: bool) -> bool {
        cli || self.notify.unwrap_or(false)
    }
}

fn candidate_paths() -> Vec<PathBuf> {
    candidate_paths_with(
        || std::env::var_os("DONEYET_CONFIG"),
        || std::env::var_os("HOME"),
    )
}

fn candidate_paths_with(
    config_env: impl FnOnce() -> Option<OsString>,
    home: impl FnOnce() -> Option<OsString>,
) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(env_path) = config_env() {
        paths.push(PathBuf::from(env_path));
    }
    if let Some(home) = home() {
        paths.push(home_default_path(&home));
    }
    paths
}

fn home_default_path(home: &OsStr) -> PathBuf {
    PathBuf::from(home)
        .join(".config")
        .join("doneyet")
        .join("config.toml")
}

pub fn effective_path() -> anyhow::Result<PathBuf> {
    effective_path_with(
        || std::env::var_os("DONEYET_CONFIG"),
        || std::env::var_os("HOME"),
    )
}

pub fn effective_path_with(
    config_env: impl FnOnce() -> Option<OsString>,
    home: impl FnOnce() -> Option<OsString>,
) -> anyhow::Result<PathBuf> {
    if let Some(env_path) = config_env() {
        return Ok(PathBuf::from(env_path));
    }
    match home() {
        Some(home) => Ok(home_default_path(&home)),
        None => Err(anyhow::anyhow!(
            "cannot determine the config path: set DONEYET_CONFIG or HOME"
        )),
    }
}

pub fn write_default(path: &Path, force: bool) -> anyhow::Result<PathBuf> {
    if !force && path.exists() {
        anyhow::bail!(
            "config already exists at {} (use --force to overwrite)",
            path.display()
        );
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, DEFAULT_CONFIG)?;
    Ok(path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effective_path_prefers_doneyet_config() {
        let path = effective_path_with(
            || Some(OsString::from("/tmp/custom.toml")),
            || Some(OsString::from("/home/me")),
        )
        .expect("env path wins");
        assert_eq!(path, PathBuf::from("/tmp/custom.toml"));
    }

    #[test]
    fn effective_path_falls_back_to_home_default() {
        let path = effective_path_with(|| None, || Some(OsString::from("/home/me")))
            .expect("home default");
        assert_eq!(path, PathBuf::from("/home/me/.config/doneyet/config.toml"));
    }

    #[test]
    fn effective_path_errors_without_env_or_home() {
        let error = effective_path_with(|| None, || None).expect_err("must error");
        assert!(error.to_string().contains("DONEYET_CONFIG"), "{error}");
    }
}
