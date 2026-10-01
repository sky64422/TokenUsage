use crate::application::service::AppCore;
use std::sync::Arc;

pub struct AppHandleState {
    pub core: Arc<AppCore>,
}
impl AppHandleState {
    pub fn new(core: Arc<AppCore>) -> Self {
        Self { core }
    }
}
