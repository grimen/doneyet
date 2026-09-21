use crate::model::{Conclusion, JobRef, RunRef, StepRef};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DomainEvent {
    RunStarted {
        run: RunRef,
    },
    RunCompleted {
        run: RunRef,
        conclusion: Conclusion,
    },
    RunRequeued {
        run: RunRef,
    },
    JobAppeared {
        job: JobRef,
    },
    JobStarted {
        job: JobRef,
    },
    JobCompleted {
        job: JobRef,
        conclusion: Conclusion,
    },
    JobRequeued {
        job: JobRef,
    },
    StepStarted {
        job: JobRef,
        step: StepRef,
    },
    StepFinished {
        job: JobRef,
        step: StepRef,
        conclusion: Conclusion,
    },
}
