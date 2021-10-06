use std::sync::{
    atomic::{AtomicBool, Ordering::Acquire, Ordering::Release},
    Arc,
};

#[derive(Debug)]
pub struct CancelToken {
    status: Arc<AtomicBool>,
}

impl CancelToken {
    pub fn new() -> Self {
        CancelToken {
            status: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn is_canceled(&self) -> bool {
        self.status.load(Acquire)
    }

    pub fn cancel(&mut self) {
        self.status.store(true, Release)
    }
}

impl Clone for CancelToken {
    fn clone(&self) -> Self {
        CancelToken {
            status: self.status.clone(),
        }
    }
}
