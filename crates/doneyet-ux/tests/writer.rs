use common::SharedBuf;
use doneyet_ux::writer::CrLfWriter;
use std::io::Write;

mod common;

#[test]
fn translates_lone_newlines_to_crlf() {
    let sink = SharedBuf::new();
    let mut writer = CrLfWriter::new(sink.clone());
    writer.write_all(b"a\nb\nc").expect("write");
    writer.flush().expect("flush");
    assert_eq!(sink.take_string(), "a\r\nb\r\nc");
}

#[test]
fn split_writes_translate_correctly() {
    let sink = SharedBuf::new();
    let mut writer = CrLfWriter::new(sink.clone());
    writer.write_all(b"hello \n").expect("write 1");
    writer.write_all(b"world\n").expect("write 2");
    assert_eq!(sink.take_string(), "hello \r\nworld\r\n");
}

#[test]
fn leaves_carriage_returns_untouched() {
    let sink = SharedBuf::new();
    let mut writer = CrLfWriter::new(sink.clone());
    writer.write_all(b"x\r\ny").expect("write");
    assert_eq!(sink.take_string(), "x\r\r\ny");
}
