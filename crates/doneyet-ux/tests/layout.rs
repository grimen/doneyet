use common::active_world;
use doneyet_core::model::{Conclusion, Phase};
use doneyet_ux::format::{bar_string, fit, format_duration, phase_label};
use doneyet_ux::glyph::phase_glyph;
use unicode_width::UnicodeWidthStr;

mod common;

#[test]
fn format_duration_uses_compact_units() {
    assert_eq!(format_duration(0), "0s");
    assert_eq!(format_duration(42), "42s");
    assert_eq!(format_duration(59), "59s");
    assert_eq!(format_duration(60), "1m00s");
    assert_eq!(format_duration(61), "1m01s");
    assert_eq!(format_duration(125), "2m05s");
    assert_eq!(format_duration(3599), "59m59s");
    assert_eq!(format_duration(3600), "1h00m");
    assert_eq!(format_duration(3720), "1h02m");
    assert_eq!(format_duration(-5), "0s");
}

#[test]
fn phase_glyphs_are_single_width() {
    let table = [
        Phase::Queued,
        Phase::Waiting,
        Phase::InProgress,
        Phase::Done(Conclusion::Success),
        Phase::Done(Conclusion::Failure),
        Phase::Done(Conclusion::Cancelled),
        Phase::Done(Conclusion::Skipped),
        Phase::Done(Conclusion::TimedOut),
        Phase::Done(Conclusion::StartupFailure),
        Phase::Done(Conclusion::ActionRequired),
        Phase::Done(Conclusion::Neutral),
        Phase::Done(Conclusion::Stale),
        Phase::Done(Conclusion::Other("weird".to_string())),
        Phase::Other("pending".to_string()),
    ];
    for phase in table {
        assert_eq!(
            UnicodeWidthStr::width(phase_glyph(&phase).to_string().as_str()),
            1,
            "{phase:?}"
        );
    }
}

#[test]
fn phase_labels_read_english() {
    assert_eq!(phase_label(&Phase::Queued), "queued");
    assert_eq!(phase_label(&Phase::Waiting), "waiting");
    assert_eq!(phase_label(&Phase::InProgress), "in_progress");
    assert_eq!(phase_label(&Phase::Done(Conclusion::Success)), "success");
    assert_eq!(phase_label(&Phase::Done(Conclusion::TimedOut)), "timed_out");
    assert_eq!(
        phase_label(&Phase::Done(Conclusion::Other("bizarre".to_string()))),
        "bizarre"
    );
    assert_eq!(phase_label(&Phase::Other("pending".to_string())), "pending");
}

#[test]
fn bar_string_fills_proportionally() {
    assert_eq!(bar_string(121, 121, 12), "████████████");
    assert_eq!(bar_string(62, 121, 12), "██████░░░░░░");
    assert_eq!(bar_string(41, 121, 12), "████░░░░░░░░");
    assert_eq!(bar_string(0, 121, 12), "░░░░░░░░░░░░");
    assert_eq!(bar_string(999, 121, 12), "████████████");
    assert_eq!(bar_string(10, 0, 12), "████████████");
    assert_eq!(bar_string(5, 10, 0), "");
}

#[test]
fn fit_truncates_with_ellipsis_and_respects_width() {
    assert_eq!(fit("hello", 10), "hello");
    assert_eq!(fit("hello", 5), "hello");
    assert_eq!(fit("hello world", 6), "hello…");
    let cjk = fit("日本語テキスト", 6);
    assert!(UnicodeWidthStr::width(cjk.as_str()) <= 6, "{cjk:?}");
    assert!(cjk.ends_with('…'), "{cjk:?}");
}

#[test]
fn every_plain_line_fits_declared_widths() {
    for width in [40usize, 60, 100, 120] {
        let frame =
            doneyet_ux::TermRenderer::frame_text(&active_world(), false, width, common::now());
        for line in frame.lines() {
            assert!(
                UnicodeWidthStr::width(line) <= width,
                "width {width}: line too long: {line:?}"
            );
        }
    }
}
