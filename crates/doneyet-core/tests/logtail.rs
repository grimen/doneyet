use doneyet_core::logtail::{LogCursor, append_log, display_lines};
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
