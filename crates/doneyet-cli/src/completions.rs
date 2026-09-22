use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

pub fn install_dir() -> Result<PathBuf> {
    install_dir_with(|key| std::env::var(key).ok()).context(
        "could not determine the completions directory (set DONEYET_COMPLETIONS_DIR, XDG_DATA_HOME, or HOME)",
    )
}

pub fn install_dir_with(get_env: impl Fn(&str) -> Option<String>) -> Option<PathBuf> {
    if let Some(dir) = get_env("DONEYET_COMPLETIONS_DIR").filter(|value| !value.is_empty()) {
        return Some(PathBuf::from(dir));
    }
    if let Some(xdg) = get_env("XDG_DATA_HOME").filter(|value| !value.is_empty()) {
        return Some(PathBuf::from(xdg).join("doneyet").join("completions"));
    }
    let home = get_env("HOME").filter(|value| !value.is_empty())?;
    Some(
        PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("doneyet")
            .join("completions"),
    )
}

pub fn shell_file_name(shell: &clap_complete::Shell) -> &'static str {
    match shell {
        clap_complete::Shell::Bash => "doneyet.bash",
        clap_complete::Shell::Elvish => "doneyet.elv",
        clap_complete::Shell::Fish => "doneyet.fish",
        clap_complete::Shell::PowerShell => "_doneyet.ps1",
        clap_complete::Shell::Zsh => "_doneyet",
        _ => unreachable!("every known clap_complete::Shell variant is matched above"),
    }
}

pub fn install(shell: clap_complete::Shell, dir: &Path) -> Result<PathBuf> {
    std::fs::create_dir_all(dir).with_context(|| {
        format!(
            "could not create the completions directory {}",
            dir.display()
        )
    })?;
    let file = dir.join(shell_file_name(&shell));
    let mut script = Vec::new();
    clap_complete::generate(
        shell,
        &mut <crate::Cli as clap::CommandFactory>::command(),
        "doneyet",
        &mut script,
    );
    std::fs::write(&file, script)
        .with_context(|| format!("could not write the completion file {}", file.display()))?;
    Ok(file)
}

pub fn enable_instruction(shell: &clap_complete::Shell, file: &Path) -> String {
    match shell {
        clap_complete::Shell::Bash => format!("source {}", file.display()),
        clap_complete::Shell::Fish => format!("source {}", file.display()),
        clap_complete::Shell::Zsh => format!("source {}", file.display()),
        clap_complete::Shell::PowerShell => format!(". {}", file.display()),
        clap_complete::Shell::Elvish => format!("eval (cat {})", file.display()),
        _ => unreachable!("every known clap_complete::Shell variant is matched above"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_file_names_cover_every_variant() {
        assert_eq!(shell_file_name(&clap_complete::Shell::Bash), "doneyet.bash");
        assert_eq!(
            shell_file_name(&clap_complete::Shell::Elvish),
            "doneyet.elv"
        );
        assert_eq!(shell_file_name(&clap_complete::Shell::Fish), "doneyet.fish");
        assert_eq!(
            shell_file_name(&clap_complete::Shell::PowerShell),
            "_doneyet.ps1"
        );
        assert_eq!(shell_file_name(&clap_complete::Shell::Zsh), "_doneyet");
    }

    #[test]
    fn install_dir_prefers_the_explicit_env_var() {
        let dir = install_dir_with(|key| match key {
            "DONEYET_COMPLETIONS_DIR" => Some("/env/dir".to_string()),
            "XDG_DATA_HOME" => Some("/xdg".to_string()),
            "HOME" => Some("/home/u".to_string()),
            _ => None,
        });
        assert_eq!(dir, Some(std::path::PathBuf::from("/env/dir")));
    }

    #[test]
    fn install_dir_falls_back_to_xdg_data_home() {
        let dir = install_dir_with(|key| match key {
            "DONEYET_COMPLETIONS_DIR" => None,
            "XDG_DATA_HOME" => Some("/xdg".to_string()),
            "HOME" => Some("/home/u".to_string()),
            _ => None,
        });
        assert_eq!(
            dir,
            Some(std::path::PathBuf::from("/xdg/doneyet/completions"))
        );
    }

    #[test]
    fn install_dir_falls_back_to_home() {
        let dir = install_dir_with(|key| match key {
            "DONEYET_COMPLETIONS_DIR" => None,
            "XDG_DATA_HOME" => None,
            "HOME" => Some("/home/u".to_string()),
            _ => None,
        });
        assert_eq!(
            dir,
            Some(std::path::PathBuf::from(
                "/home/u/.local/share/doneyet/completions"
            ))
        );
    }

    #[test]
    fn install_dir_is_none_without_any_anchor() {
        let dir = install_dir_with(|_| None);
        assert_eq!(dir, None);
    }

    #[test]
    fn install_dir_ignores_empty_env_values() {
        let dir = install_dir_with(|key| match key {
            "DONEYET_COMPLETIONS_DIR" => Some(String::new()),
            "XDG_DATA_HOME" => Some("/xdg".to_string()),
            "HOME" => Some("/home/u".to_string()),
            _ => None,
        });
        assert_eq!(
            dir,
            Some(std::path::PathBuf::from("/xdg/doneyet/completions"))
        );
    }
}
