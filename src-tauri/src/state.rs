use std::sync::Arc;

use hd_analyzer_core::LocalHdDriver;

#[derive(Clone, Default)]
pub struct AppState {
    pub fs_driver: Arc<LocalHdDriver>,
}
