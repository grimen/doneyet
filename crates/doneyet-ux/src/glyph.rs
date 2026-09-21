use crate::theme::Theme;
use doneyet_core::model::Phase;

pub fn phase_glyph(phase: &Phase) -> char {
    Theme::default().glyphs.glyph(phase)
}
