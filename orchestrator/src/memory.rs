use crossbeam::queue::ArrayQueue;
use std::sync::Arc;

/// HFT-Uyumlu (Lock-free) Ring Buffer
/// Mesaj kuyrukları için kullanılır.
#[derive(Clone)]
pub struct LockFreeBuffer {
    pub queue: Arc<ArrayQueue<Vec<u8>>>,
}

impl LockFreeBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            queue: Arc::new(ArrayQueue::new(capacity)),
        }
    }

    #[inline(always)]
    pub fn push(&self, data: Vec<u8>) -> Result<(), Vec<u8>> {
        self.queue.push(data)
    }

    #[inline(always)]
    pub fn pop(&self) -> Option<Vec<u8>> {
        self.queue.pop()
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}
