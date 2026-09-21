use std::path::Path;
use std::time::Duration;

use doneyet_core::event::DomainEvent;
use doneyet_core::model::{Outcome, World};
use serde_json::Value;

use crate::record::{FinishRecord, RECORD_VERSION, RenderRecord};

#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum ReplayEvent {
    Render {
        ts: Option<u64>,
        world: World,
        events: Vec<DomainEvent>,
    },
    Finish {
        ts: Option<u64>,
        outcome: Outcome,
    },
}

impl ReplayEvent {
    pub fn ts(&self) -> Option<u64> {
        match self {
            ReplayEvent::Render { ts, .. } | ReplayEvent::Finish { ts, .. } => *ts,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ReplayError {
    #[error("cannot read recording {path:?}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("malformed record at {path:?}:{line}: {source}")]
    Malformed {
        path: String,
        line: usize,
        #[source]
        source: serde_json::Error,
    },
    #[error("unsupported record version {found} at {path:?}:{line} (expected {expected})")]
    Version {
        path: String,
        line: usize,
        found: u8,
        expected: u8,
    },
    #[error("unknown record kind {found:?} at {path:?}:{line}")]
    Kind {
        path: String,
        line: usize,
        found: String,
    },
}

pub fn read_records(path: &Path) -> Result<Vec<ReplayEvent>, ReplayError> {
    let contents = std::fs::read_to_string(path).map_err(|source| ReplayError::Io {
        path: path.display().to_string(),
        source,
    })?;
    parse_records(&contents)
}

pub fn parse_records(contents: &str) -> Result<Vec<ReplayEvent>, ReplayError> {
    let mut events = Vec::new();
    for (index, line) in contents.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let line_no = index + 1;
        let value: Value = serde_json::from_str(line).map_err(|source| ReplayError::Malformed {
            path: "<recording>".to_string(),
            line: line_no,
            source,
        })?;
        let version = value.get("v").and_then(Value::as_u64).unwrap_or_default() as u8;
        if version != RECORD_VERSION {
            return Err(ReplayError::Version {
                path: "<recording>".to_string(),
                line: line_no,
                found: version,
                expected: RECORD_VERSION,
            });
        }
        match value.get("kind").and_then(Value::as_str) {
            Some("render") => {
                let record: RenderRecord =
                    serde_json::from_value(value).map_err(|source| ReplayError::Malformed {
                        path: "<recording>".to_string(),
                        line: line_no,
                        source,
                    })?;
                events.push(ReplayEvent::Render {
                    ts: record.ts,
                    world: record.world,
                    events: record.events,
                });
            }
            Some("finish") => {
                let record: FinishRecord =
                    serde_json::from_value(value).map_err(|source| ReplayError::Malformed {
                        path: "<recording>".to_string(),
                        line: line_no,
                        source,
                    })?;
                events.push(ReplayEvent::Finish {
                    ts: record.ts,
                    outcome: record.outcome,
                });
            }
            Some(other) => {
                return Err(ReplayError::Kind {
                    path: "<recording>".to_string(),
                    line: line_no,
                    found: other.to_string(),
                });
            }
            None => {
                return Err(ReplayError::Kind {
                    path: "<recording>".to_string(),
                    line: line_no,
                    found: "<missing>".to_string(),
                });
            }
        }
    }
    Ok(events)
}

pub fn frame_deltas(events: &[ReplayEvent]) -> Vec<Duration> {
    events
        .windows(2)
        .map(|pair| match (pair[0].ts(), pair[1].ts()) {
            (Some(earlier), Some(later)) => Duration::from_millis(later.saturating_sub(earlier)),
            _ => Duration::ZERO,
        })
        .collect()
}
