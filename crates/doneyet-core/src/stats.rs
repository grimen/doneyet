use crate::model::{Conclusion, Phase, WorkflowRun};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowStats {
    median_duration_secs: Option<i64>,
    samples: usize,
}

impl WorkflowStats {
    pub fn from_runs(runs: &[WorkflowRun]) -> Self {
        let mut durations: Vec<i64> = runs.iter().filter_map(duration_secs).collect();
        durations.sort_unstable();
        let samples = durations.len();
        let median_duration_secs = match samples {
            0 => None,
            n if n % 2 == 1 => Some(durations[n / 2]),
            n => {
                let (a, b) = (durations[n / 2 - 1], durations[n / 2]);
                Some((a + b + 1) / 2)
            }
        };
        Self {
            median_duration_secs,
            samples,
        }
    }

    pub fn median_duration_secs(&self) -> Option<i64> {
        self.median_duration_secs
    }

    pub fn samples(&self) -> usize {
        self.samples
    }

    pub fn remaining_secs(&self, elapsed_secs: i64) -> Option<i64> {
        let median = self.median_duration_secs?;
        let remaining = median - elapsed_secs;
        if remaining < 0 { None } else { Some(remaining) }
    }
}

fn duration_secs(run: &WorkflowRun) -> Option<i64> {
    let conclusion = match &run.phase {
        Phase::Done(conclusion) => conclusion,
        _ => return None,
    };
    if !matches!(
        conclusion,
        Conclusion::Success | Conclusion::Failure | Conclusion::TimedOut
    ) {
        return None;
    }
    let start = run.run_started_at.unwrap_or(run.created_at);
    Some((run.updated_at - start).get_seconds().max(0))
}
