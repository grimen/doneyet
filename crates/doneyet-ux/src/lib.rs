pub mod format;
pub mod glyph;
pub mod record;
pub mod renderer;
pub mod replay;
pub mod theme;
pub mod writer;

pub use record::TeeRenderer;
pub use renderer::{
    InlineRedraw, TermRenderer, board_frame, board_frame_with, inline_frame, runs_table,
    runs_table_with, stdout_color,
};
pub use replay::{ReplayError, ReplayEvent, frame_deltas, parse_records, read_records};
pub use theme::{
    BarStyle, GlyphSet, InkColor, Pen, PhaseInk, Theme, ThemeLoadError, load_theme, theme_by_name,
};
