//! Latest-frame ownership with a seek/reconfiguration generation barrier.
use std::sync::{
    atomic::{AtomicU64, Ordering},
    LockResult, Mutex, MutexGuard,
};

pub(crate) struct Mailbox<T> {
    frame: Mutex<Option<T>>,
    pub generation: AtomicU64,
}
impl<T> Default for Mailbox<T> {
    fn default() -> Self {
        Self {
            frame: Mutex::new(None),
            generation: AtomicU64::new(0),
        }
    }
}
impl<T> Mailbox<T> {
    pub fn lock(&self) -> LockResult<MutexGuard<'_, Option<T>>> {
        self.frame.lock()
    }
    pub fn invalidate(&self) {
        if let Ok(mut frame) = self.frame.lock() {
            self.generation.fetch_add(1, Ordering::AcqRel);
            *frame = None;
        }
    }
    /// Return displaced ownership for recycling, or reject stale work intact.
    pub fn publish(&self, frame: T, generation: u64) -> Result<Option<T>, T> {
        if let Ok(mut slot) = self.frame.lock() {
            if generation == self.generation.load(Ordering::Acquire) {
                return Ok(slot.replace(frame));
            }
        }
        Err(frame)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn seek_invalidates_queued_and_already_decoding_frames() {
        let mailbox = Mailbox::default();
        let old = mailbox.generation.load(Ordering::Acquire);
        assert_eq!(mailbox.publish(1, old), Ok(None));
        mailbox.invalidate();
        assert!(mailbox.lock().unwrap().is_none());
        assert_eq!(mailbox.publish(2, old), Err(2));
        assert!(mailbox.lock().unwrap().is_none());
        let new = mailbox.generation.load(Ordering::Acquire);
        assert_eq!(mailbox.publish(3, new), Ok(None));
        assert_eq!(*mailbox.lock().unwrap(), Some(3));
    }
    #[test]
    fn overwriting_and_invalidating_release_owned_payloads() {
        let mailbox = Mailbox::default();
        let picture = std::sync::Arc::new(());
        assert!(mailbox.publish(picture.clone(), 0).unwrap().is_none());
        let displaced = mailbox.publish(picture.clone(), 0).unwrap().unwrap();
        assert_eq!(std::sync::Arc::strong_count(&picture), 3);
        drop(displaced);
        mailbox.invalidate();
        assert_eq!(std::sync::Arc::strong_count(&picture), 1);
    }
    #[test]
    fn in_flight_worker_cannot_overwrite_the_new_seek_picture() {
        let mailbox = std::sync::Arc::new(Mailbox::default());
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let worker_barrier = barrier.clone();
        let producer = mailbox.clone();
        let worker = std::thread::spawn(move || {
            let generation = producer.generation.load(Ordering::Acquire);
            worker_barrier.wait();
            worker_barrier.wait();
            assert_eq!(producer.publish("stale", generation), Err("stale"));
        });
        barrier.wait();
        mailbox.invalidate();
        let current = mailbox.generation.load(Ordering::Acquire);
        assert_eq!(mailbox.publish("requested", current), Ok(None));
        barrier.wait();
        worker.join().unwrap();
        assert_eq!(*mailbox.lock().unwrap(), Some("requested"));
    }
}
