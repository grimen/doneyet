use std::io::Write;

pub struct CrLfWriter<W: Write> {
    inner: W,
}

impl<W: Write> CrLfWriter<W> {
    pub fn new(inner: W) -> Self {
        Self { inner }
    }
}

impl<W: Write> Write for CrLfWriter<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut last = 0;
        for (index, byte) in buf.iter().enumerate() {
            if *byte == b'\n' {
                self.inner.write_all(&buf[last..index])?;
                self.inner.write_all(b"\r\n")?;
                last = index + 1;
            }
        }
        self.inner.write_all(&buf[last..])?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}
