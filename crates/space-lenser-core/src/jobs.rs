use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    Queued,
    Running,
    Completed,
    Failed,
    Canceled,
    Superseded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressSnapshot {
    pub job_id: String,
    pub request_id: String,
    pub state: JobState,
    pub estimated_total_bytes: Option<u64>,
    pub scheduled_units: u64,
    pub discovered_units: u64,
    pub completed_units: u64,
    pub active_units: u64,
    pub skipped_units: u64,
    pub failed_units: u64,
    pub canceled_units: u64,
    pub bytes_measured: u64,
    pub active_paths: Vec<PathBuf>,
}

impl ProgressSnapshot {
    pub fn new(job_id: String, request_id: String) -> Self {
        Self {
            job_id,
            request_id,
            state: JobState::Queued,
            estimated_total_bytes: None,
            scheduled_units: 0,
            discovered_units: 0,
            completed_units: 0,
            active_units: 0,
            skipped_units: 0,
            failed_units: 0,
            canceled_units: 0,
            bytes_measured: 0,
            active_paths: Vec::new(),
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(
            self.state,
            JobState::Completed | JobState::Failed | JobState::Canceled | JobState::Superseded
        ) && self.active_units == 0
    }
}

#[derive(Debug, Clone)]
pub struct CancelToken {
    canceled: Arc<std::sync::atomic::AtomicBool>,
}

impl CancelToken {
    pub fn new() -> Self {
        Self {
            canceled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    pub fn cancel(&self) {
        self.canceled.store(true, Ordering::Release);
    }

    pub fn is_canceled(&self) -> bool {
        self.canceled.load(Ordering::Acquire)
    }
}

impl Default for CancelToken {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct ScanJobHandle {
    pub job_id: String,
    pub request_id: String,
    pub root_path: PathBuf,
    pub config_fingerprint: String,
    pub state: JobState,
    pub cancel_token: CancelToken,
}

#[derive(Debug, Default)]
pub struct JobRegistry {
    counter: AtomicU64,
    jobs: Mutex<HashMap<String, ScanJobHandle>>,
}

impl JobRegistry {
    pub fn create(&self, root_path: PathBuf, config_fingerprint: String) -> ScanJobHandle {
        let next = self.counter.fetch_add(1, Ordering::Relaxed) + 1;
        let handle = ScanJobHandle {
            job_id: format!("scan-{next}"),
            request_id: format!("req-{next}"),
            root_path,
            config_fingerprint,
            state: JobState::Queued,
            cancel_token: CancelToken::new(),
        };
        self.jobs
            .lock()
            .expect("job registry poisoned")
            .insert(handle.job_id.clone(), handle.clone());
        handle
    }

    pub fn set_state(&self, job_id: &str, state: JobState) {
        if let Some(job) = self
            .jobs
            .lock()
            .expect("job registry poisoned")
            .get_mut(job_id)
        {
            job.state = state;
        }
    }

    pub fn state(&self, job_id: &str) -> Option<JobState> {
        self.jobs
            .lock()
            .expect("job registry poisoned")
            .get(job_id)
            .map(|job| job.state)
    }

    pub fn active_exact(
        &self,
        path: &std::path::Path,
        config_fingerprint: &str,
    ) -> Option<ScanJobHandle> {
        self.jobs
            .lock()
            .expect("job registry poisoned")
            .values()
            .find(|job| {
                job.root_path == path
                    && job.config_fingerprint == config_fingerprint
                    && matches!(job.state, JobState::Queued | JobState::Running)
            })
            .cloned()
    }

    pub fn cancel(&self, job_id: &str) -> bool {
        let mut jobs = self.jobs.lock().expect("job registry poisoned");
        let Some(job) = jobs.get_mut(job_id) else {
            return false;
        };
        job.cancel_token.cancel();
        job.state = JobState::Canceled;
        true
    }

    pub fn supersede_path(&self, path: &std::path::Path) -> Vec<String> {
        let mut superseded = Vec::new();
        for job in self
            .jobs
            .lock()
            .expect("job registry poisoned")
            .values_mut()
        {
            if job.root_path.starts_with(path)
                && matches!(job.state, JobState::Queued | JobState::Running)
            {
                job.cancel_token.cancel();
                job.state = JobState::Superseded;
                superseded.push(job.job_id.clone());
            }
        }
        superseded
    }

    pub fn active_paths(&self) -> HashSet<PathBuf> {
        self.jobs
            .lock()
            .expect("job registry poisoned")
            .values()
            .filter(|job| matches!(job.state, JobState::Queued | JobState::Running))
            .map(|job| job.root_path.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_progress_requires_no_active_units() {
        let mut progress = ProgressSnapshot::new("job".to_string(), "req".to_string());
        progress.state = JobState::Completed;
        progress.active_units = 1;

        assert!(!progress.is_terminal());
    }

    #[test]
    fn supersede_marks_matching_active_jobs() {
        let registry = JobRegistry::default();
        registry.create(PathBuf::from("/tmp/a"), "config".to_string());

        let superseded = registry.supersede_path(std::path::Path::new("/tmp/a"));

        assert_eq!(superseded.len(), 1);
    }

    #[test]
    fn active_exact_reuses_matching_active_job() {
        let registry = JobRegistry::default();
        let created = registry.create(PathBuf::from("/tmp/a"), "config".to_string());

        let active = registry.active_exact(std::path::Path::new("/tmp/a"), "config");

        assert_eq!(active.unwrap().job_id, created.job_id);
    }
}
