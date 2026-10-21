//! Bounded fake retrieval executor, used by native and deterministic application tests.
use crate::Product;
use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use view::resources::{Request, stream::Mailbox};
pub struct SourceService {
    queue: Mailbox<(Request, String), ()>,
    outstanding: Arc<AtomicUsize>,
}
impl Default for SourceService {
    fn default() -> Self {
        Self {
            queue: Mailbox::new(2),
            outstanding: Arc::new(AtomicUsize::new(0)),
        }
    }
}
impl SourceService {
    pub fn start(&self, request: Request, source: String) -> std::result::Result<(), Request> {
        if source.len() > 256 * 1024
            || self
                .outstanding
                .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                    (n < 2).then_some(n + 1)
                })
                .is_err()
        {
            return Err(request);
        }
        let queue = self.queue.clone();
        std::thread::spawn(move || {
            assert!(queue.send((request, source)).is_ok());
        });
        Ok(())
    }
    pub fn pending(&self) -> bool {
        self.outstanding.load(Ordering::Acquire) > 0
    }
    pub fn receive(&self) -> Option<(Request, String)> {
        let result = self.queue.receive()?;
        self.outstanding.fetch_sub(1, Ordering::AcqRel);
        Some(result)
    }
}
pub struct FakeService {
    queue: Mailbox<(Request, Vec<Product>), (Request, f32)>,
    outstanding: Arc<AtomicUsize>,
}
impl Default for FakeService {
    fn default() -> Self {
        Self {
            queue: Mailbox::new(4),
            outstanding: Arc::new(AtomicUsize::new(0)),
        }
    }
}
impl FakeService {
    /// Refuse excess work before creating a thread; caller retains the request.
    pub fn start(&self, request: Request, delay: Duration) -> std::result::Result<(), Request> {
        if self
            .outstanding
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                (n < 4).then_some(n + 1)
            })
            .is_err()
        {
            return Err(request);
        }
        let queue = self.queue.clone();
        std::thread::spawn(move || {
            std::thread::sleep(delay);
            queue.preview((request.clone(), 1.));
            let products = if request.key.is_empty() {
                vec![]
            } else {
                (0..100)
                    .map(|id| Product {
                        id,
                        name: format!("{} — product {id}", request.key),
                        image: "cover".into(),
                    })
                    .collect()
            };
            // At most four accepted requests exist, including queued replies.
            assert!(queue.send((request, products)).is_ok());
        });
        Ok(())
    }
    pub fn pending(&self) -> bool {
        self.outstanding.load(Ordering::Acquire) > 0
    }
    pub fn receive(&self) -> Option<(Request, Vec<Product>)> {
        let result = self.queue.receive()?;
        self.outstanding.fetch_sub(1, Ordering::AcqRel);
        Some(result)
    }
    pub fn progress(&self) -> Option<(Request, f32)> {
        self.queue.take_preview()
    }
}
