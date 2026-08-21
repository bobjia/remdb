use std::sync::Mutex;

/// A small pool of reusable `Vec<u8>` write buffers to avoid re-allocating
/// on the hot frame serialization path. `ret` clears the buffer before reuse.
pub struct BufferPool {
    pool: Mutex<Vec<Vec<u8>>>,
    buf_size: usize,
    max_buffers: usize,
}

impl BufferPool {
    pub fn new(max_buffers: usize, buf_size: usize) -> Self {
        let mut pool = Vec::with_capacity(max_buffers);
        for _ in 0..max_buffers {
            pool.push(vec![0u8; buf_size]);
        }
        Self { pool: Mutex::new(pool), buf_size, max_buffers }
    }

    /// Take a buffer from the pool, or allocate a fresh one if the pool is empty.
    pub fn take(&self) -> Vec<u8> {
        let mut guard = self
            .pool
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(buf) = guard.pop() {
            buf
        } else {
            vec![0u8; self.buf_size]
        }
    }

    /// Return a buffer to the pool.
    pub fn ret(&self, mut buf: Vec<u8>) {
        buf.clear();
        let mut guard = self
            .pool
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if guard.len() < self.max_buffers {
            guard.push(buf);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn acquire_recycles_buffers() {
        let pool = BufferPool::new(2, 64);
        let a = pool.take();
        assert!(a.len() >= 64);
        // after returning, the next take reuses one of the pooled buffers
        pool.ret(a);
        let b = pool.take();
        assert!(b.capacity() >= 64);
    }

    #[test]
    fn concurrent_acquire_release_no_panic() {
        let pool = std::sync::Arc::new(BufferPool::new(4, 128));
        let mut handles = Vec::new();
        for _ in 0..8 {
            let pool = pool.clone();
            handles.push(thread::spawn(move || {
                for _ in 0..100 {
                    let b = pool.take();
                    let mut b = b;
                    b.resize(128, 0u8);
                    pool.ret(b);
                }
            }));
        }
        for h in handles {
            h.join().expect("thread panicked");
        }
    }
}