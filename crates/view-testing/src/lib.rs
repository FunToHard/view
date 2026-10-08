//! Controlled headless time, inputs and service fixtures over public core APIs.
//! No native input, network, filesystem, GPU or wall-clock service is started.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::{collections::VecDeque, time::Duration};
use view_core::{CompletionToken, CoreError, Flush, NodeId, Runtime};

/// An action supplied by a fixture, not a native OS input event.
pub enum Input<A> {
    /// Direct typed action to a mounted component.
    Action(NodeId, A),
    /// Controlled worker completion, validated by the real runtime.
    Completion(CompletionToken, A),
}

/// Scheduling failure retains the input for retry.
pub struct ScheduleError<A> {
    /// Capacity or clock overflow reason.
    pub reason: ScheduleFailure,
    /// The input was not accepted.
    pub input: Input<A>,
}

/// Deterministic scheduling failures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScheduleFailure {
    /// Fixture input queue is full.
    QueueFull,
    /// Advancing time would exceed Duration's range.
    TimeExhausted,
}

/// One nonblocking driver turn.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Step {
    /// Inputs accepted by the runtime this turn.
    pub delivered: usize,
    /// Inputs rejected as stale/inactive, in deterministic order.
    pub rejected: Vec<CoreError>,
    /// Backpressure retained a due input for a later step.
    pub backpressure: bool,
    /// Actual runtime dispatch/commit outcome.
    pub flush: Flush,
}

struct Scheduled<A> {
    at: Duration,
    input: Input<A>,
}

/// Deterministic driver. Equal-time inputs preserve insertion order.
/// Calls never sleep. `advance` changes only virtual time; `step` delivers due
/// inputs and executes one runtime turn. Backpressure retains required input.
pub struct Harness<M, A> {
    runtime: Runtime<M, A>,
    model: M,
    now: Duration,
    inputs: VecDeque<Scheduled<A>>,
    capacity: usize,
}

impl<M, A> Harness<M, A> {
    /// Own a runtime/model fixture and start virtual time at zero.
    pub fn new(runtime: Runtime<M, A>, model: M, input_capacity: usize) -> Self {
        Self {
            runtime,
            model,
            now: Duration::ZERO,
            inputs: VecDeque::new(),
            capacity: input_capacity,
        }
    }
    /// Public runtime observation/setup interface.
    pub fn runtime(&self) -> &Runtime<M, A> {
        &self.runtime
    }
    /// Public runtime mutation/setup interface.
    pub fn runtime_mut(&mut self) -> &mut Runtime<M, A> {
        &mut self.runtime
    }
    /// Current application model fixture.
    pub fn model(&self) -> &M {
        &self.model
    }
    /// Current virtual timestamp.
    pub fn now(&self) -> Duration {
        self.now
    }
    /// Number of scheduled inputs, including future inputs.
    pub fn pending_inputs(&self) -> usize {
        self.inputs.len()
    }
    /// Advance without delivering input or invoking callbacks.
    pub fn advance(&mut self, delta: Duration) -> Result<(), ScheduleFailure> {
        self.now = self
            .now
            .checked_add(delta)
            .ok_or(ScheduleFailure::TimeExhausted)?;
        Ok(())
    }
    /// Schedule relative to current virtual time. Equal deadlines are FIFO.
    pub fn schedule(&mut self, after: Duration, input: Input<A>) -> Result<(), ScheduleError<A>> {
        if self.inputs.len() >= self.capacity {
            return Err(ScheduleError {
                reason: ScheduleFailure::QueueFull,
                input,
            });
        }
        let Some(at) = self.now.checked_add(after) else {
            return Err(ScheduleError {
                reason: ScheduleFailure::TimeExhausted,
                input,
            });
        };
        let index = self
            .inputs
            .iter()
            .position(|event| event.at > at)
            .unwrap_or(self.inputs.len());
        self.inputs.insert(index, Scheduled { at, input });
        Ok(())
    }
    /// Deliver due inputs in timestamp order, then flush once.
    pub fn step(&mut self) -> Result<Step, CoreError> {
        let mut delivered = 0;
        let mut rejected = Vec::new();
        let mut backpressure = false;
        while self
            .inputs
            .front()
            .is_some_and(|event| event.at <= self.now)
        {
            let event = self.inputs.pop_front().expect("due input exists");
            let (outcome, target) = match event.input {
                Input::Action(id, action) => (self.runtime.enqueue(id, action), Ok(id)),
                Input::Completion(token, action) => {
                    (self.runtime.complete(&token, action), Err(token))
                }
            };
            match outcome {
                Ok(_) => delivered += 1,
                Err(failure) if failure.error == CoreError::QueueFull => {
                    let input = match target {
                        Ok(id) => Input::Action(id, failure.action),
                        Err(token) => Input::Completion(token, failure.action),
                    };
                    self.inputs.push_front(Scheduled {
                        at: event.at,
                        input,
                    });
                    backpressure = true;
                    break;
                }
                Err(failure) => rejected.push(failure.error),
            }
        }
        Ok(Step {
            delivered,
            rejected,
            backpressure,
            flush: self.runtime.flush(&mut self.model)?,
        })
    }
}

/// Bounded FIFO replies supplied explicitly instead of live IO.
/// Store Result values to model successes and failures without a transport.
pub struct ServiceFixture<T> {
    replies: VecDeque<T>,
    capacity: usize,
}
impl<T> ServiceFixture<T> {
    /// Create an empty controlled service with a reply budget.
    pub fn new(capacity: usize) -> Self {
        Self {
            replies: VecDeque::new(),
            capacity,
        }
    }
    /// Supply a reply; full fixtures return it unchanged.
    pub fn supply(&mut self, reply: T) -> Result<(), T> {
        if self.replies.len() == self.capacity {
            return Err(reply);
        }
        self.replies.push_back(reply);
        Ok(())
    }
    /// Consume the next reply, or report that none has been supplied.
    pub fn take(&mut self) -> Option<T> {
        self.replies.pop_front()
    }
}
