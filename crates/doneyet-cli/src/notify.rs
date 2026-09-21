use doneyet_core::model::Conclusion;

pub fn message(conclusion: &Conclusion) -> (&'static str, String) {
    let body = match conclusion {
        Conclusion::Success => "CI run succeeded".to_string(),
        Conclusion::Failure | Conclusion::StartupFailure => "CI run failed".to_string(),
        Conclusion::TimedOut => "CI run timed out".to_string(),
        Conclusion::Cancelled => "CI run was cancelled".to_string(),
        other => format!(
            "CI run ended: {}",
            doneyet_ux::format::conclusion_word(other)
        ),
    };
    ("doneyet", body)
}

pub fn send(conclusion: &Conclusion) {
    let (summary, body) = message(conclusion);
    let _ = std::process::Command::new("notify-send")
        .arg(summary)
        .arg(body)
        .status();
}
