use crate::application::service::AppCore;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

pub struct AppHandleState {
    pub core: Arc<AppCore>,
    pub content_min_w: AtomicU32,
    pub content_min_h: AtomicU32,
}

impl AppHandleState {
    pub fn new(core: Arc<AppCore>) -> Self {
        Self {
            core,
            content_min_w: AtomicU32::new(240),
            content_min_h: AtomicU32::new(160),
        }
    }

    pub fn set_content_min(&self, w: u32, h: u32) {
        self.content_min_w.store(w, Ordering::SeqCst);
        self.content_min_h.store(h, Ordering::SeqCst);
    }

    pub fn content_min(&self) -> (u32, u32) {
        (
            self.content_min_w.load(Ordering::SeqCst),
            self.content_min_h.load(Ordering::SeqCst),
        )
    }

    pub fn content_min_logical(&self) -> (f64, f64) {
        let (w, h) = self.content_min();
        (w as f64, h as f64)
    }
}
