use doneyet_core::model::{Conclusion, Phase};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InkColor {
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
}

impl InkColor {
    fn ansi(self) -> anstyle::AnsiColor {
        match self {
            InkColor::Black => anstyle::AnsiColor::Black,
            InkColor::Red => anstyle::AnsiColor::Red,
            InkColor::Green => anstyle::AnsiColor::Green,
            InkColor::Yellow => anstyle::AnsiColor::Yellow,
            InkColor::Blue => anstyle::AnsiColor::Blue,
            InkColor::Magenta => anstyle::AnsiColor::Magenta,
            InkColor::Cyan => anstyle::AnsiColor::Cyan,
            InkColor::White => anstyle::AnsiColor::White,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Pen {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<InkColor>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub bold: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub dimmed: bool,
}

impl Pen {
    pub const fn plain() -> Self {
        Self {
            color: None,
            bold: false,
            dimmed: false,
        }
    }

    pub const fn colored(color: InkColor) -> Self {
        Self {
            color: Some(color),
            bold: false,
            dimmed: false,
        }
    }

    pub const fn bold() -> Self {
        Self {
            color: None,
            bold: true,
            dimmed: false,
        }
    }

    pub const fn dimmed() -> Self {
        Self {
            color: None,
            bold: false,
            dimmed: true,
        }
    }

    pub fn style(&self) -> anstyle::Style {
        let mut style = anstyle::Style::new();
        if let Some(color) = self.color {
            style = style.fg_color(Some(anstyle::Color::Ansi(color.ansi())));
        }
        if self.bold {
            style = style.effects(anstyle::Effects::BOLD);
        }
        if self.dimmed {
            style = style.effects(anstyle::Effects::DIMMED);
        }
        style
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct GlyphSet {
    pub queued: char,
    pub waiting: char,
    pub in_progress: char,
    pub success: char,
    pub failure: char,
    pub cancelled: char,
    pub skipped: char,
    pub timed_out: char,
    pub startup_failure: char,
    pub action_required: char,
    pub neutral: char,
    pub stale: char,
    pub unknown_conclusion: char,
    pub unknown_phase: char,
}

impl Default for GlyphSet {
    fn default() -> Self {
        Self {
            queued: '○',
            waiting: '◌',
            in_progress: '◐',
            success: '✔',
            failure: '✖',
            cancelled: '⊘',
            skipped: '↷',
            timed_out: '⏱',
            startup_failure: '✖',
            action_required: '⚠',
            neutral: '●',
            stale: '●',
            unknown_conclusion: '✳',
            unknown_phase: '◌',
        }
    }
}

impl GlyphSet {
    pub fn glyph(&self, phase: &Phase) -> char {
        match phase {
            Phase::Queued => self.queued,
            Phase::Waiting => self.waiting,
            Phase::InProgress => self.in_progress,
            Phase::Done(conclusion) => self.conclusion_glyph(conclusion),
            Phase::Other(_) => self.unknown_phase,
        }
    }

    pub fn conclusion_glyph(&self, conclusion: &Conclusion) -> char {
        match conclusion {
            Conclusion::Success => self.success,
            Conclusion::Failure => self.failure,
            Conclusion::Cancelled => self.cancelled,
            Conclusion::Skipped => self.skipped,
            Conclusion::TimedOut => self.timed_out,
            Conclusion::StartupFailure => self.startup_failure,
            Conclusion::ActionRequired => self.action_required,
            Conclusion::Neutral => self.neutral,
            Conclusion::Stale => self.stale,
            Conclusion::Other(_) => self.unknown_conclusion,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PhaseInk {
    pub queued: Pen,
    pub waiting: Pen,
    pub in_progress: Pen,
    pub success: Pen,
    pub failure: Pen,
    pub cancelled: Pen,
    pub skipped: Pen,
    pub timed_out: Pen,
    pub startup_failure: Pen,
    pub action_required: Pen,
    pub neutral: Pen,
    pub stale: Pen,
    pub unknown_conclusion: Pen,
    pub unknown_phase: Pen,
}

impl Default for PhaseInk {
    fn default() -> Self {
        Self {
            queued: Pen::colored(InkColor::Yellow),
            waiting: Pen::colored(InkColor::Yellow),
            in_progress: Pen::colored(InkColor::Cyan),
            success: Pen::colored(InkColor::Green),
            failure: Pen::colored(InkColor::Red),
            cancelled: Pen::colored(InkColor::Yellow),
            skipped: Pen::dimmed(),
            timed_out: Pen::colored(InkColor::Red),
            startup_failure: Pen::colored(InkColor::Red),
            action_required: Pen::colored(InkColor::Yellow),
            neutral: Pen::dimmed(),
            stale: Pen::dimmed(),
            unknown_conclusion: Pen::dimmed(),
            unknown_phase: Pen::colored(InkColor::Yellow),
        }
    }
}

impl PhaseInk {
    pub fn pen(&self, phase: &Phase) -> &Pen {
        match phase {
            Phase::Queued => &self.queued,
            Phase::Waiting => &self.waiting,
            Phase::InProgress => &self.in_progress,
            Phase::Done(conclusion) => self.conclusion_pen(conclusion),
            Phase::Other(_) => &self.unknown_phase,
        }
    }

    pub fn conclusion_pen(&self, conclusion: &Conclusion) -> &Pen {
        match conclusion {
            Conclusion::Success => &self.success,
            Conclusion::Failure => &self.failure,
            Conclusion::Cancelled => &self.cancelled,
            Conclusion::Skipped => &self.skipped,
            Conclusion::TimedOut => &self.timed_out,
            Conclusion::StartupFailure => &self.startup_failure,
            Conclusion::ActionRequired => &self.action_required,
            Conclusion::Neutral => &self.neutral,
            Conclusion::Stale => &self.stale,
            Conclusion::Other(_) => &self.unknown_conclusion,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct BarStyle {
    pub width: usize,
    pub filled: char,
    pub empty: char,
}

impl Default for BarStyle {
    fn default() -> Self {
        Self {
            width: 12,
            filled: '█',
            empty: '░',
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Theme {
    pub glyphs: GlyphSet,
    pub ink: PhaseInk,
    pub bar: BarStyle,
    pub connector: char,
    pub emphasis: Pen,
    pub title_separator: char,
    pub no_duration: char,
    pub annotation: Pen,
    pub annotation_glyph: char,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            glyphs: GlyphSet::default(),
            ink: PhaseInk::default(),
            bar: BarStyle::default(),
            connector: '│',
            emphasis: Pen::bold(),
            title_separator: '›',
            no_duration: '—',
            annotation: Pen::colored(InkColor::Yellow),
            annotation_glyph: '⚠',
        }
    }
}

impl Theme {
    pub fn ascii() -> Self {
        Self {
            glyphs: GlyphSet {
                queued: '.',
                waiting: '~',
                in_progress: '>',
                success: '+',
                failure: 'x',
                cancelled: '/',
                skipped: '-',
                timed_out: 'T',
                startup_failure: 'x',
                action_required: '!',
                neutral: '*',
                stale: '*',
                unknown_conclusion: '?',
                unknown_phase: '~',
            },
            bar: BarStyle {
                width: 12,
                filled: '#',
                empty: '.',
            },
            connector: '|',
            title_separator: '>',
            no_duration: '-',
            annotation_glyph: '!',
            ..Self::default()
        }
    }
}

pub fn theme_by_name(name: &str) -> Option<Theme> {
    match name {
        "default" => Some(Theme::default()),
        "ascii" => Some(Theme::ascii()),
        _ => None,
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ThemeLoadError {
    #[error("theme file {path:?} does not exist (built-in themes: default, ascii)")]
    FileNotFound { path: String },
    #[error("invalid theme in {path:?}: {source}")]
    Invalid {
        path: String,
        #[source]
        source: serde_json::Error,
    },
}

pub fn load_theme(spec: &str) -> Result<Theme, ThemeLoadError> {
    if let Some(theme) = theme_by_name(spec) {
        return Ok(theme);
    }
    let contents = std::fs::read_to_string(spec).map_err(|_| ThemeLoadError::FileNotFound {
        path: spec.to_string(),
    })?;
    serde_json::from_str(&contents).map_err(|source| ThemeLoadError::Invalid {
        path: spec.to_string(),
        source,
    })
}
