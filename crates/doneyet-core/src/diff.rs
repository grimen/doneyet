use crate::event::DomainEvent;
use crate::model::{Job, Phase, Step, World};

pub fn diff_worlds(prev: Option<&World>, next: &World) -> Vec<DomainEvent> {
    let mut events = Vec::new();
    match prev {
        None => first_snapshot(next, &mut events),
        Some(previous) => delta(previous, next, &mut events),
    }
    events
}

fn first_snapshot(next: &World, events: &mut Vec<DomainEvent>) {
    match &next.run.phase {
        Phase::Done(conclusion) => events.push(DomainEvent::RunCompleted {
            run: next.run.ref_of(),
            conclusion: conclusion.clone(),
        }),
        _ => events.push(DomainEvent::RunStarted {
            run: next.run.ref_of(),
        }),
    }
    for job in &next.jobs {
        events.push(DomainEvent::JobAppeared { job: job.ref_of() });
        if matches!(job.phase, Phase::InProgress) {
            events.push(DomainEvent::JobStarted { job: job.ref_of() });
        }
    }
}

fn delta(prev: &World, next: &World, events: &mut Vec<DomainEvent>) {
    for job in &next.jobs {
        match prev.jobs.iter().find(|p| p.id == job.id) {
            None => {
                events.push(DomainEvent::JobAppeared { job: job.ref_of() });
                if matches!(job.phase, Phase::InProgress) {
                    events.push(DomainEvent::JobStarted { job: job.ref_of() });
                }
            }
            Some(previous) => job_delta(previous, job, events),
        }
    }
    run_delta(prev, next, events);
}

fn run_delta(prev: &World, next: &World, events: &mut Vec<DomainEvent>) {
    if prev.run.phase == next.run.phase {
        return;
    }
    match &next.run.phase {
        Phase::Done(conclusion) => events.push(DomainEvent::RunCompleted {
            run: next.run.ref_of(),
            conclusion: conclusion.clone(),
        }),
        _ if prev.run.phase.is_terminal() => events.push(DomainEvent::RunRequeued {
            run: next.run.ref_of(),
        }),
        _ => {}
    }
}

fn job_delta(prev: &Job, next: &Job, events: &mut Vec<DomainEvent>) {
    for step in &next.steps {
        if let Some(previous) = prev.steps.iter().find(|p| p.number == step.number) {
            step_delta(previous, step, next, events);
        }
    }
    if prev.phase == next.phase {
        return;
    }
    match &next.phase {
        Phase::Done(conclusion) => events.push(DomainEvent::JobCompleted {
            job: next.ref_of(),
            conclusion: conclusion.clone(),
        }),
        Phase::InProgress => events.push(DomainEvent::JobStarted { job: next.ref_of() }),
        _ if prev.phase.is_terminal() => {
            events.push(DomainEvent::JobRequeued { job: next.ref_of() })
        }
        _ => {}
    }
}

fn step_delta(prev: &Step, next: &Step, job: &Job, events: &mut Vec<DomainEvent>) {
    if prev.phase == next.phase {
        return;
    }
    match &next.phase {
        Phase::Done(conclusion) => events.push(DomainEvent::StepFinished {
            job: job.ref_of(),
            step: next.ref_of(job.id),
            conclusion: conclusion.clone(),
        }),
        Phase::InProgress => events.push(DomainEvent::StepStarted {
            job: job.ref_of(),
            step: next.ref_of(job.id),
        }),
        _ => {}
    }
}
