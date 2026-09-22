use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

use doneyet_app::BoardSink;
use doneyet_core::model::RunsPage;
use doneyet_core::ports::RenderError;
use serde::Serialize;

pub struct DashJsonSink<W: Write + Send> {
    out: W,
}

impl<W: Write + Send> DashJsonSink<W> {
    pub fn new(out: W) -> Self {
        Self { out }
    }
}

impl<W: Write + Send> BoardSink for DashJsonSink<W> {
    fn render_page(&mut self, page: &RunsPage) -> Result<(), RenderError> {
        let record = PageRecord {
            kind: "page",
            ts: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|elapsed| elapsed.as_secs())
                .unwrap_or(0),
            page,
        };
        let line = serde_json::to_string(&record).map_err(|e| RenderError(e.to_string()))?;
        self.out
            .write_all(line.as_bytes())
            .and_then(|_| self.out.write_all(b"\n"))
            .map_err(|e| RenderError(e.to_string()))
    }
}

#[derive(Serialize)]
struct PageRecord<'a> {
    kind: &'a str,
    ts: u64,
    page: &'a RunsPage,
}

#[cfg(test)]
mod tests {
    use super::*;
    use doneyet_core::model::{Phase, WorkflowRun};

    fn sample_page() -> RunsPage {
        let run = WorkflowRun {
            id: 2841,
            run_number: 2841,
            name: "ci.yml".to_string(),
            display_title: "build & test".to_string(),
            head_branch: Some("main".to_string()),
            head_sha: "abc".to_string(),
            event: "push".to_string(),
            phase: Phase::InProgress,
            actor: "jonas".to_string(),
            html_url: "https://github.com/acme/api/actions/runs/2841".to_string(),
            created_at: "2026-09-21T10:00:00Z".parse().unwrap(),
            run_started_at: Some("2026-09-21T10:00:01Z".parse().unwrap()),
            updated_at: "2026-09-21T10:00:30Z".parse().unwrap(),
        };
        RunsPage {
            total_count: 1,
            runs: vec![run],
        }
    }

    #[test]
    fn writes_one_page_line() {
        let mut sink = DashJsonSink::new(Vec::new());
        sink.render_page(&sample_page()).expect("render succeeds");
        let output = String::from_utf8(sink.out).expect("utf8");
        assert!(output.ends_with('\n'), "{output}");
        assert_eq!(output.lines().count(), 1, "{output}");
        let value: serde_json::Value = serde_json::from_str(output.trim_end()).expect("parses");
        assert_eq!(value["kind"], "page", "{value}");
        assert!(value["ts"].is_u64(), "{value}");
        assert_eq!(value["page"]["total_count"], 1, "{value}");
        assert_eq!(value["page"]["runs"][0]["id"], 2841, "{value}");
        assert_eq!(value["page"]["runs"][0]["phase"], "InProgress", "{value}");
    }

    struct FailingWriter;

    impl Write for FailingWriter {
        fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("broken"))
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn write_failure_maps_to_render_error() {
        let mut sink = DashJsonSink::new(FailingWriter);
        let error = sink
            .render_page(&sample_page())
            .expect_err("write must fail");
        assert!(error.0.contains("broken"), "{error:?}");
    }
}
