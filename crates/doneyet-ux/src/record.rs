use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::time::SystemTime;

use doneyet_core::event::DomainEvent;
use doneyet_core::model::{Outcome, World};
use doneyet_core::ports::{RenderResult, Renderer};
use serde::{Deserialize, Serialize};

pub const RECORD_VERSION: u8 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RenderRecord {
    pub v: u8,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ts: Option<u64>,
    pub world: World,
    pub events: Vec<DomainEvent>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FinishRecord {
    pub v: u8,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ts: Option<u64>,
    pub outcome: Outcome,
}

pub struct TeeRenderer<W: Write> {
    primary: Box<dyn Renderer>,
    sink: Option<BufWriter<W>>,
}

impl TeeRenderer<File> {
    pub fn new(primary: Box<dyn Renderer>, path: &Path) -> std::io::Result<Self> {
        Ok(Self {
            primary,
            sink: Some(BufWriter::new(File::create(path)?)),
        })
    }
}

impl<W: Write> TeeRenderer<W> {
    pub fn with_writer(primary: Box<dyn Renderer>, writer: W) -> Self {
        Self {
            primary,
            sink: Some(BufWriter::new(writer)),
        }
    }

    fn write_line<S: Serialize>(&mut self, record: S) {
        let Some(sink) = self.sink.as_mut() else {
            return;
        };
        let line = match serde_json::to_string(&record) {
            Ok(line) => line,
            Err(error) => {
                tracing::warn!("recording disabled: serialize failed: {error}");
                self.sink = None;
                return;
            }
        };
        if sink
            .write_all(line.as_bytes())
            .and_then(|_| sink.write_all(b"\n"))
            .is_err()
        {
            tracing::warn!("recording disabled: sink write failed");
            self.sink = None;
        }
    }
}

impl<W: Write + Send> Renderer for TeeRenderer<W> {
    fn render(&mut self, world: &World, events: &[DomainEvent]) -> RenderResult<()> {
        self.primary.render(world, events)?;
        self.write_line(RenderRecord {
            v: RECORD_VERSION,
            kind: "render".to_string(),
            ts: Some(now_epoch_millis()),
            world: world.clone(),
            events: events.to_vec(),
        });
        Ok(())
    }

    fn finish(&mut self, outcome: &Outcome) -> RenderResult<()> {
        self.primary.finish(outcome)?;
        self.write_line(FinishRecord {
            v: RECORD_VERSION,
            kind: "finish".to_string(),
            ts: Some(now_epoch_millis()),
            outcome: outcome.clone(),
        });
        if let Some(sink) = self.sink.as_mut() {
            let _ = sink.flush();
        }
        Ok(())
    }
}

fn now_epoch_millis() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as u64)
        .unwrap_or(0)
}
