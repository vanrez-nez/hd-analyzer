use std::sync::Arc;

use space_lenser_core::LocalHdDriver;

#[derive(Clone, Default)]
pub struct AppState {
    pub fs_driver: Arc<LocalHdDriver>,
}
