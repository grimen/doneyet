use common::{active_world, failed_world, now};
use doneyet_ux::TermRenderer;
use doneyet_ux::theme::{BarStyle, InkColor, Pen, PhaseInk, Theme, theme_by_name};

mod common;

#[test]
fn default_theme_reproduces_builtin_frame() {
    let world = active_world();
    assert_eq!(
        TermRenderer::frame_text(&world, false, 60, now()),
        TermRenderer::frame_text_with(&world, &Theme::default(), false, 60, now()),
        "default theme must be byte-identical to the built-in look"
    );
}

#[test]
fn ascii_theme_is_pure_ascii() {
    let frame = TermRenderer::frame_text_with(&active_world(), &Theme::ascii(), false, 60, now());
    assert!(
        frame.is_ascii(),
        "ascii theme must render 7-bit output: {frame:?}"
    );
    assert!(frame.contains("+ build (linux-x64)"), "{frame}");
    assert!(frame.contains("x test (macos-arm64)"), "{frame}");
    assert!(frame.contains("> deploy (linux-x64)"), "{frame}");
}

#[test]
fn custom_bar_chars_render() {
    let theme = Theme {
        bar: BarStyle {
            filled: '@',
            empty: '-',
            ..BarStyle::default()
        },
        ..Theme::default()
    };
    let frame = TermRenderer::frame_text_with(&failed_world(), &theme, false, 60, now());
    assert!(frame.contains("@@@@@@@@@@@@"), "full bar: {frame}");
    assert!(frame.contains("@@@@@@------"), "partial bar: {frame}");
}

#[test]
fn themed_pen_changes_sgr() {
    let theme = Theme {
        ink: PhaseInk {
            success: Pen::colored(InkColor::Yellow),
            ..PhaseInk::default()
        },
        ..Theme::default()
    };
    let frame = TermRenderer::frame_text_with(&active_world(), &theme, true, 60, now());
    assert!(
        frame.contains("\x1b[33m✔"),
        "success glyph must be yellow: {frame:?}"
    );
    assert!(!frame.contains("\x1b[32m✔"), "default green must be gone");
}

#[test]
fn custom_connector_and_width() {
    let theme = Theme {
        connector: ':',
        bar: BarStyle {
            width: 5,
            ..BarStyle::default()
        },
        ..Theme::default()
    };
    let frame = TermRenderer::frame_text_with(&active_world(), &theme, false, 60, now());
    assert!(frame.contains(":  ✖ Run cargo test"), "{frame}");
    assert!(frame.contains("█████"), "bar width 5: {frame}");
    assert!(!frame.contains("██████"), "no bar wider than 5: {frame}");
}

#[test]
fn partial_json_overrides_keep_defaults() {
    let theme: Theme = serde_json::from_str(r#"{"bar":{"width":5}}"#).expect("partial theme json");
    assert_eq!(theme.bar.width, 5);
    assert_eq!(theme.glyphs.success, '✔');
    assert_eq!(theme.connector, '│');
    assert_eq!(theme.ink.in_progress, Pen::colored(InkColor::Cyan));
}

#[test]
fn theme_roundtrips_through_json() {
    let theme = Theme::ascii();
    let json = serde_json::to_string(&theme).expect("serialize theme");
    let back: Theme = serde_json::from_str(&json).expect("deserialize theme");
    assert_eq!(back, theme);
}

#[test]
fn theme_by_name_maps_builtins() {
    assert_eq!(theme_by_name("default"), Some(Theme::default()));
    assert_eq!(theme_by_name("ascii"), Some(Theme::ascii()));
    assert_eq!(theme_by_name("bogus"), None);
}

#[test]
fn load_theme_resolves_builtins() {
    assert_eq!(
        doneyet_ux::theme::load_theme("default").expect("default"),
        Theme::default()
    );
    assert_eq!(
        doneyet_ux::theme::load_theme("ascii").expect("ascii"),
        Theme::ascii()
    );
}

#[test]
fn load_theme_reads_partial_json_files() {
    let path = std::env::temp_dir().join(format!("doneyet-theme-{}.json", std::process::id()));
    std::fs::write(&path, r#"{"bar":{"width":5}}"#).expect("write theme file");
    let theme = doneyet_ux::theme::load_theme(path.to_str().unwrap()).expect("theme file loads");
    std::fs::remove_file(&path).ok();
    assert_eq!(theme.bar.width, 5);
    assert_eq!(
        theme.glyphs.success, '✔',
        "unset fields fall back to defaults"
    );
}

#[test]
fn load_theme_invalid_json_reports_the_file() {
    let path = std::env::temp_dir().join(format!("doneyet-theme-bad-{}.json", std::process::id()));
    std::fs::write(&path, "not json").expect("write theme file");
    let err = doneyet_ux::theme::load_theme(path.to_str().unwrap()).expect_err("invalid json");
    std::fs::remove_file(&path).ok();
    assert!(err.to_string().contains("invalid theme"), "{err}");
}

#[test]
fn load_theme_unknown_spec_names_the_builtins() {
    let err =
        doneyet_ux::theme::load_theme("definitely-not-a-theme-spec").expect_err("unknown spec");
    let message = err.to_string();
    assert!(message.contains("default"), "{message}");
    assert!(message.contains("ascii"), "{message}");
}
