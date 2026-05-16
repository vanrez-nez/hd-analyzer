use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use hd_analyzer_core::ScanResult;

use crate::dto::SessionStatusDto;

#[derive(Clone, Default)]
pub struct AppState {
    pub sessions: Arc<Mutex<HashMap<String, StoredSession>>>,
    pub active_scan: Arc<Mutex<Option<String>>>,
}

#[derive(Clone)]
pub struct StoredSession {
    pub root: PathBuf,
    pub result: Option<ScanResult>,
    pub status: SessionStatusDto,
    pub error_message: Option<String>,
}
