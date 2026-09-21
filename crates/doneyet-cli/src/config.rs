use serde::Deserialize;
use std::path::PathBuf;

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
    let mut paths = Vec::new();
    if let Ok(env_path) = std::env::var("DONEYET_CONFIG") {
        paths.push(PathBuf::from(env_path));
    }
    if let Some(home) = std::env::var_os("HOME") {
        paths.push(
            PathBuf::from(home)
                .join(".config")
                .join("doneyet")
                .join("config.toml"),
        );
    }
    paths
}
