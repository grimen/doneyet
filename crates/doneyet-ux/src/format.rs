use doneyet_core::model::{Conclusion, Phase};
use unicode_width::UnicodeWidthStr;

pub fn format_duration(seconds: i64) -> String {
    let seconds = seconds.max(0);
    if seconds < 60 {
        format!("{seconds}s")
    } else if seconds < 3600 {
        let minutes = seconds / 60;
        let rest = seconds % 60;
        format!("{minutes}m{rest:02}s")
    } else {
        let hours = seconds / 3600;
        let rest = (seconds % 3600) / 60;
        format!("{hours}h{rest:02}m")
    }
}

pub fn phase_label(phase: &Phase) -> String {
    match phase {
        Phase::Queued => "queued".to_string(),
        Phase::Waiting => "waiting".to_string(),
        Phase::InProgress => "in_progress".to_string(),
        Phase::Done(conclusion) => conclusion_word(conclusion),
        Phase::Other(raw) => raw.clone(),
    }
}

pub fn conclusion_word(conclusion: &Conclusion) -> String {
    match conclusion {
        Conclusion::Success => "success".to_string(),
        Conclusion::Failure => "failure".to_string(),
        Conclusion::Cancelled => "cancelled".to_string(),
        Conclusion::Skipped => "skipped".to_string(),
        Conclusion::TimedOut => "timed_out".to_string(),
        Conclusion::StartupFailure => "startup_failure".to_string(),
        Conclusion::ActionRequired => "action_required".to_string(),
        Conclusion::Neutral => "neutral".to_string(),
        Conclusion::Stale => "stale".to_string(),
        Conclusion::Other(raw) => raw.clone(),
    }
}

pub fn bar_string(seconds: i64, max_seconds: i64, width: usize) -> String {
    bar_string_chars('█', '░', seconds, max_seconds, width)
}

pub fn bar_string_chars(
    filled: char,
    empty: char,
    seconds: i64,
    max_seconds: i64,
    width: usize,
) -> String {
    if width == 0 {
        return String::new();
    }
    let ratio = if max_seconds > 0 {
        (seconds.max(0) as f64) / (max_seconds.max(1) as f64)
    } else {
        1.0
    };
    let filled_count = ((ratio * width as f64).round() as usize).clamp(0, width);
    let mut bar = String::with_capacity(width * 4);
    for i in 0..width {
        bar.push(if i < filled_count { filled } else { empty });
    }
    bar
}

pub fn fit(text: &str, max_width: usize) -> String {
    if UnicodeWidthStr::width(text) <= max_width {
        return text.to_string();
    }
    if max_width == 0 {
        return String::new();
    }
    let mut width = 0;
    let mut out = String::new();
    for ch in text.chars() {
        let ch_width = UnicodeWidthStr::width(ch.to_string().as_str());
        if width + ch_width > max_width.saturating_sub(1) {
            break;
        }
        out.push(ch);
        width += ch_width;
    }
    out.push('…');
    out
}

pub fn pad_to(text: &str, column: usize) -> String {
    let text_width = UnicodeWidthStr::width(text);
    let mut out = text.to_string();
    for _ in text_width..column {
        out.push(' ');
    }
    out
}
