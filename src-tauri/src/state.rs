use std::sync::Arc;

use sqlx::SqlitePool;
use tokio::sync::Mutex;

use crate::{
    jobs::extraction_worker::{ExtractionJob, ExtractionWorker},
    llm::ModelRegistry,
};

// ---------------------------------------------------------------------------
// AppSettings
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct AppSettings {
    pub default_local_model: String,
    pub default_cloud_model: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            default_local_model: "local_general".to_string(),
            default_cloud_model: "cloud_fast".to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// JobQueue
// ---------------------------------------------------------------------------

pub struct JobQueue;

impl JobQueue {
    pub fn new() -> Self {
        Self
    }

    /// Enqueue a background extraction job for the given message.
    pub fn enqueue_extraction(
        &self,
        message_id: &str,
        conversation_id: &str,
        state: Arc<AppState>,
    ) {
        let job = ExtractionJob {
            conversation_id: conversation_id.to_string(),
            message_ids: vec![message_id.to_string()],
        };
        tokio::spawn(async move {
            if let Err(e) = ExtractionWorker::run_once(state, job).await {
                eprintln!("[ExtractionWorker] error: {e:#}");
            }
        });
    }
}

impl Default for JobQueue {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// AppState
// ---------------------------------------------------------------------------

pub struct AppState {
    pub db: SqlitePool,
    pub registry: Arc<ModelRegistry>,
    pub job_queue: Arc<JobQueue>,
    pub settings: Arc<Mutex<AppSettings>>,
}

impl AppState {
    pub fn new(
        db: SqlitePool,
        registry: Arc<ModelRegistry>,
        job_queue: Arc<JobQueue>,
        settings: AppSettings,
    ) -> Self {
        Self {
            db,
            registry,
            job_queue,
            settings: Arc::new(Mutex::new(settings)),
        }
    }
}
