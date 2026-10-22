//! Bounded worker handoff. Required events return ownership on saturation.
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

pub struct Mailbox<T, P> {
    inner: Arc<Mutex<Queue<T, P>>>,
}
struct Queue<T, P> {
    required: VecDeque<T>,
    preview: Option<P>,
    capacity: usize,
}
impl<T, P> Clone for Mailbox<T, P> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}
impl<T, P> Mailbox<T, P> {
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Queue {
                required: VecDeque::new(),
                preview: None,
                capacity,
            })),
        }
    }
    /// The producer retains/retries the returned event. No required event is discarded.
    pub fn send(&self, event: T) -> Result<(), T> {
        let mut q = self.inner.lock().expect("mailbox poisoned");
        if q.required.len() >= q.capacity {
            return Err(event);
        }
        q.required.push_back(event);
        Ok(())
    }
    /// Only replaceable progress/preview belongs in this single coalescing slot.
    pub fn preview(&self, preview: P) {
        self.inner.lock().expect("mailbox poisoned").preview = Some(preview);
    }
    pub fn receive(&self) -> Option<T> {
        self.inner
            .lock()
            .expect("mailbox poisoned")
            .required
            .pop_front()
    }
    pub fn take_preview(&self) -> Option<P> {
        self.inner.lock().expect("mailbox poisoned").preview.take()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SequenceResult {
    Apply,
    Duplicate,
    Resync,
}
/// Epoch is chosen by the application on reconnect; gaps suspend incremental delivery.
pub struct Sequence {
    epoch: u64,
    next: u64,
    resync: bool,
}
impl Sequence {
    pub fn new(epoch: u64, next: u64) -> Self {
        Self {
            epoch,
            next,
            resync: false,
        }
    }
    pub fn accept(&mut self, epoch: u64, sequence: u64) -> SequenceResult {
        if self.resync || epoch != self.epoch {
            self.resync = true;
            return SequenceResult::Resync;
        }
        if sequence < self.next {
            return SequenceResult::Duplicate;
        }
        if sequence != self.next || self.next == u64::MAX {
            self.resync = true;
            return SequenceResult::Resync;
        }
        self.next += 1;
        SequenceResult::Apply
    }
    /// Only an authoritative full snapshot may reset the stream cursor.
    pub fn resynchronize(&mut self, epoch: u64, next: u64) {
        *self = Self::new(epoch, next);
    }
}
