use doneyet_core::logtail::{LogCursor, append_log, append_log_matching, display_lines};
use doneyet_core::ports::LogChunk;

fn chunk(bytes: &str, next_offset: u64) -> LogChunk {
    LogChunk {
        bytes: bytes.as_bytes().to_vec(),
        next_offset,
    }
}

#[test]
fn holds_a_partial_line_until_newline() {
    let mut cursor = LogCursor::default();
    append_log(&mut cursor, &chunk("hel", 3), 20);
    assert!(cursor.lines.is_empty());
    assert_eq!(display_lines(&cursor, 20), vec!["hel".to_string()]);

    append_log(&mut cursor, &chunk("lo\nwor", 10), 20);
    assert_eq!(cursor.lines, vec!["hello".to_string()]);
    assert_eq!(cursor.pending, "wor");

    append_log(&mut cursor, &chunk("ld\n", 13), 20);
    assert_eq!(cursor.lines, vec!["hello".to_string(), "world".to_string()]);
    assert!(cursor.pending.is_empty());
}

#[test]
fn keeps_only_the_last_n_complete_lines() {
    let mut cursor = LogCursor::default();
    append_log(&mut cursor, &chunk("one\ntwo\nthree\n", 14), 2);
    assert_eq!(cursor.lines, vec!["two".to_string(), "three".to_string()]);
}

#[test]
fn replaced_log_resets_the_cursor() {
    let mut cursor = LogCursor {
        offset: 10,
        pending: "old".to_string(),
        lines: vec!["stale".to_string()],
    };
    append_log(&mut cursor, &chunk("new\n", 4), 20);
    assert_eq!(cursor.offset, 4);
    assert_eq!(cursor.lines, vec!["new".to_string()]);
    assert!(cursor.pending.is_empty());
}

#[test]
fn grep_keeps_only_matching_lines() {
    let mut cursor = LogCursor::default();
    append_log_matching(
        &mut cursor,
        &chunk("error: boom\nok\nerror: two\n", 24),
        20,
        "error",
    );
    assert_eq!(
        cursor.lines,
        vec!["error: boom".to_string(), "error: two".to_string()]
    );
}

#[test]
fn grep_cap_counts_only_matching_lines() {
    let mut cursor = LogCursor::default();
    append_log_matching(
        &mut cursor,
        &chunk("a error 1\nnoise\nb error 2\nnoise\nc error 3\n", 35),
        2,
        "error",
    );
    assert_eq!(
        cursor.lines,
        vec!["b error 2".to_string(), "c error 3".to_string()]
    );
}

#[test]
fn grep_without_matches_yields_no_lines() {
    let mut cursor = LogCursor::default();
    append_log_matching(&mut cursor, &chunk("all quiet\n", 10), 20, "error");
    assert!(cursor.lines.is_empty());
    assert!(display_lines(&cursor, 20).is_empty());
}
