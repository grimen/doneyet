use crate::ports::LogChunk;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LogCursor {
    pub offset: u64,
    pub pending: String,
    pub lines: Vec<String>,
}

pub fn append_log(cursor: &mut LogCursor, chunk: &LogChunk, limit: usize) {
    if chunk.next_offset < cursor.offset {
        cursor.lines.clear();
        cursor.pending.clear();
    }
    cursor.offset = chunk.next_offset;
    cursor
        .pending
        .push_str(&String::from_utf8_lossy(&chunk.bytes));
    while let Some(index) = cursor.pending.find('\n') {
        let mut line: String = cursor.pending.drain(..=index).collect();
        if line.ends_with('\n') {
            line.pop();
        }
        if line.ends_with('\r') {
            line.pop();
        }
        cursor.lines.push(line);
    }
    if cursor.lines.len() > limit {
        let drop_n = cursor.lines.len() - limit;
        cursor.lines.drain(0..drop_n);
    }
}

pub fn display_lines(cursor: &LogCursor, limit: usize) -> Vec<String> {
    let mut lines = cursor.lines.clone();
    if !cursor.pending.is_empty() {
        lines.push(cursor.pending.clone());
    }
    if lines.len() > limit {
        lines.split_off(lines.len() - limit)
    } else {
        lines
    }
}
