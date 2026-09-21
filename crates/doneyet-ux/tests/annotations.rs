use common::{failed_world, now};
use doneyet_ux::TermRenderer;
use unicode_width::UnicodeWidthStr;

mod common;

#[test]
fn failed_jobs_show_their_annotations_inline() {
    let frame = TermRenderer::frame_text(&failed_world(), false, 60, now());
    assert!(
        frame.contains("│  ⚠ src/lib.rs:42 unused variable: `x`"),
        "{frame}"
    );
    assert!(
        frame.contains("│  ⚠ process exited with code 1"),
        "path-less annotation renders message only: {frame}"
    );
}

#[test]
fn annotations_of_healthy_jobs_are_hidden() {
    let frame = TermRenderer::frame_text(&failed_world(), false, 60, now());
    let lines: Vec<&str> = frame.lines().collect();
    let build_index = lines
        .iter()
        .position(|l| l.contains("build (linux-x64)"))
        .expect("build job present");
    let test_index = lines
        .iter()
        .position(|l| l.contains("test (macos-arm64)"))
        .expect("test job present");
    let between = &lines[build_index + 1..test_index];
    assert!(
        between.is_empty(),
        "no annotation lines under the successful job: {between:?}"
    );
}

#[test]
fn ascii_theme_uses_ascii_annotation_glyph() {
    let frame = TermRenderer::frame_text_with(
        &failed_world(),
        &doneyet_ux::Theme::ascii(),
        false,
        60,
        now(),
    );
    assert!(frame.is_ascii(), "ascii theme stays 7-bit: {frame:?}");
    assert!(frame.contains("|  ! src/lib.rs:42"), "{frame}");
}

#[test]
fn annotation_lines_respect_width() {
    for width in [40usize, 60, 100] {
        let frame = TermRenderer::frame_text(&failed_world(), false, width, now());
        for line in frame.lines() {
            assert!(
                UnicodeWidthStr::width(line) <= width,
                "width {width}: {line:?}"
            );
        }
    }
}

#[test]
fn annotation_pen_is_themeable() {
    use doneyet_ux::theme::{InkColor, Pen, PhaseInk, Theme};
    let theme = Theme {
        ink: PhaseInk::default(),
        annotation: Pen::colored(InkColor::Magenta),
        ..Theme::default()
    };
    let frame = TermRenderer::frame_text_with(&failed_world(), &theme, true, 60, now());
    assert!(
        frame.contains("\x1b[35m⚠"),
        "annotation glyph must be magenta: {frame:?}"
    );
}
