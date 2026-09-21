use std::io::{IsTerminal, Write};

use doneyet_core::event::DomainEvent;
use doneyet_core::model::{Conclusion, Job, Outcome, Phase, RunsPage, Step, World};
use doneyet_core::ports::{RenderError, RenderResult, Renderer};
use jiff::Timestamp;
use unicode_width::UnicodeWidthStr;

use crate::format::{bar_string_chars, conclusion_word, fit, format_duration, pad_to, phase_label};
use crate::theme::{Pen, Theme};

const DURATION_COL: usize = 7;
const MAX_NAME_COL: usize = 32;

struct Palette {
    enabled: bool,
}

impl Palette {
    fn paint(&self, pen: &Pen, text: &str) -> String {
        if self.enabled {
            format!("{}{text}{}", pen.style().render(), anstyle::Reset.render())
        } else {
            text.to_string()
        }
    }
}

struct Layout {
    name_col: usize,
    bar_width: usize,
    max_seconds: i64,
    width: usize,
}

impl Layout {
    fn compute(world: &World, theme: &Theme, width: usize, now: Timestamp) -> Self {
        let name_col = name_column_width(world.jobs.iter().map(|j| j.name.as_str()), width);
        Self {
            name_col,
            bar_width: bar_width_for(width, name_col, theme.bar.width),
            max_seconds: max_job_seconds(world, now),
            width,
        }
    }
}

pub struct TermRenderer {
    writer: Box<dyn Write + Send>,
    theme: Theme,
    color: bool,
    width: usize,
    now: Box<dyn Fn() -> Timestamp + Send>,
    last_lines: usize,
}

impl TermRenderer {
    pub fn auto(width: usize) -> Self {
        Self::new(
            Box::new(std::io::stdout()),
            stdout_color(),
            width,
            Box::new(Timestamp::now),
        )
    }

    pub fn new(
        writer: Box<dyn Write + Send>,
        color: bool,
        width: usize,
        now: Box<dyn Fn() -> Timestamp + Send>,
    ) -> Self {
        Self::with_theme(writer, Theme::default(), color, width, now)
    }

    pub fn with_theme(
        writer: Box<dyn Write + Send>,
        theme: Theme,
        color: bool,
        width: usize,
        now: Box<dyn Fn() -> Timestamp + Send>,
    ) -> Self {
        Self {
            writer,
            theme,
            color,
            width,
            now,
            last_lines: 0,
        }
    }

    pub fn frame_text(world: &World, color: bool, width: usize, now: Timestamp) -> String {
        Self::frame_text_with(world, &Theme::default(), color, width, now)
    }

    pub fn frame_text_with(
        world: &World,
        theme: &Theme,
        color: bool,
        width: usize,
        now: Timestamp,
    ) -> String {
        let palette = Palette { enabled: color };
        let layout = Layout::compute(world, theme, width, now);
        let mut lines = Vec::new();
        lines.push(header_line(world, theme, &palette, width, now));
        for job in &world.jobs {
            lines.push(job_line(job, theme, &palette, &layout, now));
            for step in job.steps.iter().filter(|s| interesting_step(s)) {
                lines.push(step_line(step, theme, &palette, width));
            }
            if let Some(tail) = world.job_logs.iter().find(|log| log.job_id == job.id) {
                for line in &tail.lines {
                    lines.push(log_line(line, theme, width));
                }
            }
            if job_is_failed(&job.phase) {
                for annotation in world.annotations.iter().filter(|a| a.job_id == job.id) {
                    lines.push(annotation_line(annotation, theme, &palette, width));
                }
            }
        }
        lines.join("\n")
    }

    pub fn finish_text(outcome: &Outcome, color: bool) -> String {
        Self::finish_text_with(outcome, &Theme::default(), color)
    }

    pub fn finish_text_with(outcome: &Outcome, theme: &Theme, color: bool) -> String {
        let palette = Palette { enabled: color };
        let pen = theme.ink.conclusion_pen(&outcome.conclusion);
        let glyph = theme.glyphs.conclusion_glyph(&outcome.conclusion);
        let glyph = palette.paint(pen, glyph.encode_utf8(&mut [0u8; 4]));
        let label = palette.paint(pen, &conclusion_word(&outcome.conclusion));
        format!(
            "{} {} #{} {}",
            glyph, outcome.run.name, outcome.run.id, label
        )
    }

    fn write_frame(&mut self, text: &str) -> RenderResult<()> {
        let (out, next) = inline_frame(self.last_lines, text);
        self.last_lines = next;
        self.writer
            .write_all(out.as_bytes())
            .and_then(|_| self.writer.flush())
            .map_err(|e| RenderError(e.to_string()))
    }
}

impl Renderer for TermRenderer {
    fn render(&mut self, world: &World, _events: &[DomainEvent]) -> RenderResult<()> {
        let text = Self::frame_text_with(world, &self.theme, self.color, self.width, (self.now)());
        self.write_frame(&text)
    }

    fn finish(&mut self, outcome: &Outcome) -> RenderResult<()> {
        let summary = Self::finish_text_with(outcome, &self.theme, self.color);
        self.last_lines = 0;
        self.writer
            .write_all(format!("\n{summary}\n").as_bytes())
            .and_then(|_| self.writer.flush())
            .map_err(|e| RenderError(e.to_string()))
    }
}

pub fn stdout_color() -> bool {
    if std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty()) {
        return false;
    }
    if std::env::var("CLICOLOR_FORCE").is_ok_and(|v| v != "0") {
        return true;
    }
    std::io::stdout().is_terminal()
}

pub fn board_frame(page: &RunsPage, color: bool, width: usize, now: Timestamp) -> String {
    board_frame_with(page, &Theme::default(), color, width, now)
}

pub fn board_frame_with(
    page: &RunsPage,
    theme: &Theme,
    color: bool,
    width: usize,
    now: Timestamp,
) -> String {
    let running = page.runs.iter().filter(|run| run.phase.is_active()).count();
    let shown = page.runs.len();
    let header = format!("{running} running · {shown} shown");
    let table = runs_table_with(page, theme, color, width, now);
    if table.is_empty() {
        header
    } else {
        format!("{header}\n{table}")
    }
}

pub fn inline_frame(previous_lines: usize, text: &str) -> (String, usize) {
    let lines: Vec<&str> = text.lines().collect();
    let redrawing = previous_lines > 0;
    let mut out = String::new();
    if redrawing {
        out.push_str(&format!("\x1b[{previous_lines}F"));
    }
    for line in &lines {
        out.push_str(line);
        if redrawing {
            out.push_str("\x1b[K");
        }
        out.push('\n');
    }
    if previous_lines > lines.len() {
        for _ in lines.len()..previous_lines {
            out.push_str("\x1b[K\n");
        }
    }
    (out, lines.len())
}

pub struct InlineRedraw<W: Write> {
    writer: W,
    last_lines: usize,
}

impl<W: Write> InlineRedraw<W> {
    pub fn new(writer: W) -> Self {
        Self {
            writer,
            last_lines: 0,
        }
    }

    pub fn write_frame(&mut self, text: &str) -> std::io::Result<()> {
        let (out, next) = inline_frame(self.last_lines, text);
        self.last_lines = next;
        self.writer.write_all(out.as_bytes())?;
        self.writer.flush()
    }
}

pub fn runs_table(page: &RunsPage, color: bool, width: usize, now: Timestamp) -> String {
    runs_table_with(page, &Theme::default(), color, width, now)
}

pub fn runs_table_with(
    page: &RunsPage,
    theme: &Theme,
    color: bool,
    width: usize,
    now: Timestamp,
) -> String {
    let palette = Palette { enabled: color };
    let mut lines = Vec::new();
    for run_model in &page.runs {
        let pen = theme.ink.pen(&run_model.phase);
        let glyph = palette.paint(
            pen,
            theme
                .glyphs
                .glyph(&run_model.phase)
                .encode_utf8(&mut [0u8; 4]),
        );
        let number = format!("#{}", run_model.run_number);
        let name = pad_to(&fit(&run_model.name, 14), 14);
        let branch = pad_to(
            &fit(run_model.head_branch.as_deref().unwrap_or("-"), 18),
            18,
        );
        let event = pad_to(&fit(&run_model.event, 12), 12);
        let age = format_duration((now - run_model.created_at).get_seconds().max(0));
        let fixed = 2 + 7 + 1 + 14 + 1 + 18 + 1 + 12 + 1 + UnicodeWidthStr::width(age.as_str());
        let title_max = width.saturating_sub(fixed);
        let title = if title_max >= 3 {
            pad_to(&fit(&run_model.display_title, title_max), title_max)
        } else {
            String::new()
        };
        let line = format!("{glyph} {number:<6} {name} {title} {branch} {event} {age}");
        lines.push(line.trim_end().to_string());
    }
    lines.join("\n")
}

fn interesting_step(step: &Step) -> bool {
    matches!(
        &step.phase,
        Phase::InProgress
            | Phase::Done(Conclusion::Failure | Conclusion::TimedOut | Conclusion::StartupFailure)
    )
}

fn name_column_width<'a>(names: impl Iterator<Item = &'a str>, width: usize) -> usize {
    let longest = names.map(UnicodeWidthStr::width).max().unwrap_or(0);
    longest
        .min(MAX_NAME_COL)
        .min(width.saturating_sub(DURATION_COL + 5).max(4))
}

fn bar_width_for(width: usize, name_col: usize, max_bar: usize) -> usize {
    let available = width.saturating_sub(name_col + DURATION_COL + 5);
    if available < 3 {
        0
    } else {
        available.min(max_bar)
    }
}

fn max_job_seconds(world: &World, now: Timestamp) -> i64 {
    world
        .jobs
        .iter()
        .filter_map(|job| job_seconds(job, now))
        .max()
        .unwrap_or(0)
}

fn job_seconds(job: &Job, now: Timestamp) -> Option<i64> {
    let start = job.started_at?;
    match &job.phase {
        Phase::Done(_) => job
            .completed_at
            .map(|end| (end - start).get_seconds().max(0)),
        phase if phase.is_active() => Some((now - start).get_seconds().max(0)),
        _ => None,
    }
}

fn header_line(
    world: &World,
    theme: &Theme,
    palette: &Palette,
    width: usize,
    now: Timestamp,
) -> String {
    let run = &world.run;
    let repo = world.repo.to_string();
    let glyph = theme.glyphs.glyph(&run.phase).to_string();
    let label = phase_label(&run.phase);
    let number = format!("#{}", run.run_number);
    let start = run.run_started_at.unwrap_or(run.created_at);
    let elapsed_secs = match &run.phase {
        Phase::Done(_) => (run.updated_at - start).get_seconds().max(0),
        _ => (now - start).get_seconds().max(0),
    };
    let elapsed = format_duration(elapsed_secs);
    let overhead_without_eta = UnicodeWidthStr::width(repo.as_str())
        + 2
        + 2
        + UnicodeWidthStr::width(number.as_str())
        + 2
        + UnicodeWidthStr::width(glyph.as_str())
        + 1
        + UnicodeWidthStr::width(label.as_str())
        + 2
        + UnicodeWidthStr::width(elapsed.as_str());
    let eta_text = if run.phase.is_active() {
        world
            .stats
            .as_ref()
            .and_then(|stats| stats.remaining_secs(elapsed_secs))
            .map(|remaining| format!(" (~{} left)", format_duration(remaining)))
            .filter(|eta| UnicodeWidthStr::width(eta.as_str()) + overhead_without_eta <= width)
            .unwrap_or_default()
    } else {
        String::new()
    };
    let overhead = overhead_without_eta + UnicodeWidthStr::width(eta_text.as_str());
    let title_max = width.saturating_sub(overhead);
    let title = if title_max >= 3 {
        let full = format!(
            "{} {} {}",
            run.name, theme.title_separator, run.display_title
        );
        format!("{}  ", fit(&full, title_max))
    } else {
        String::new()
    };

    let painted_repo = palette.paint(&theme.emphasis, &repo);
    let painted_glyph = palette.paint(theme.ink.pen(&run.phase), &glyph);
    let painted_label = palette.paint(theme.ink.pen(&run.phase), &label);
    let painted_eta = if eta_text.is_empty() {
        String::new()
    } else {
        palette.paint(&crate::theme::Pen::dimmed(), &eta_text)
    };
    format!(
        "{painted_repo}  {title}{number}  {painted_glyph} {painted_label}  {elapsed}{painted_eta}"
    )
}

fn job_line(
    job: &Job,
    theme: &Theme,
    palette: &Palette,
    layout: &Layout,
    now: Timestamp,
) -> String {
    let glyph = palette.paint(
        theme.ink.pen(&job.phase),
        theme.glyphs.glyph(&job.phase).encode_utf8(&mut [0u8; 4]),
    );
    let name = fit(
        &job.name,
        layout
            .name_col
            .min(layout.width.saturating_sub(DURATION_COL + 5).max(4)),
    );
    let name_pad = pad_to(&name, layout.name_col);
    let duration = match job_seconds(job, now) {
        Some(secs) => format_duration(secs),
        None => theme.no_duration.to_string(),
    };
    let duration_field = format!("{duration:>DURATION_COL$}");
    let bar = if layout.bar_width > 0 {
        if let Some(secs) = job_seconds(job, now) {
            if secs > 0 {
                palette.paint(
                    theme.ink.pen(&job.phase),
                    &bar_string_chars(
                        theme.bar.filled,
                        theme.bar.empty,
                        secs,
                        layout.max_seconds,
                        layout.bar_width,
                    ),
                )
            } else {
                String::new()
            }
        } else {
            String::new()
        }
    } else {
        String::new()
    };
    let line = format!("{glyph} {name_pad}  {duration_field} {bar}");
    line.trim_end().to_string()
}

fn step_line(step: &Step, theme: &Theme, palette: &Palette, width: usize) -> String {
    let glyph = palette.paint(
        theme.ink.pen(&step.phase),
        theme.glyphs.glyph(&step.phase).encode_utf8(&mut [0u8; 4]),
    );
    let name_max = width.saturating_sub(7);
    let name = fit(&step.name, name_max);
    format!("  {}  {glyph} {name}", theme.connector)
}

fn job_is_failed(phase: &Phase) -> bool {
    matches!(
        phase,
        Phase::Done(Conclusion::Failure | Conclusion::TimedOut | Conclusion::StartupFailure)
    )
}

fn log_line(line: &str, theme: &Theme, width: usize) -> String {
    let text = fit(line, width.saturating_sub(6));
    format!("  {}  {text}", theme.connector)
}

fn annotation_line(
    annotation: &doneyet_core::model::Annotation,
    theme: &Theme,
    palette: &Palette,
    width: usize,
) -> String {
    let location = match (&annotation.path, annotation.start_line) {
        (Some(path), Some(line)) => format!("{path}:{line} "),
        (Some(path), None) => format!("{path} "),
        (None, _) => String::new(),
    };
    let text = fit(
        &format!("{location}{}", annotation.message),
        width.saturating_sub(7),
    );
    let glyph = palette.paint(
        &theme.annotation,
        theme.annotation_glyph.encode_utf8(&mut [0u8; 4]),
    );
    format!("  {}  {glyph} {text}", theme.connector)
}
