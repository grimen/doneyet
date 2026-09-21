use std::io::IsTerminal;

use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
use doneyet_core::model::RepoRef;
use doneyet_core::ports::RefreshHint;
use tokio_util::sync::CancellationToken;

pub struct RawModeGuard;

impl RawModeGuard {
    pub fn enable() -> Option<Self> {
        if std::io::stdin().is_terminal() && crossterm::terminal::enable_raw_mode().is_ok() {
            Some(Self)
        } else {
            None
        }
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let _ = crossterm::terminal::disable_raw_mode();
    }
}

pub fn spawn(
    repo: RepoRef,
    cancel: CancellationToken,
    hints: tokio::sync::mpsc::UnboundedSender<RefreshHint>,
) {
    std::thread::spawn(move || {
        while let Ok(event) = crossterm::event::read() {
            let Event::Key(key) = event else {
                continue;
            };
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => {
                    cancel.cancel();
                    break;
                }
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    cancel.cancel();
                    break;
                }
                KeyCode::Char('r') => {
                    let _ = hints.send(RefreshHint {
                        repo: repo.clone(),
                        run_id: None,
                    });
                }
                _ => {}
            }
        }
    });
}
