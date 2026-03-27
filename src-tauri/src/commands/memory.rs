use tauri::State;

use crate::{
    models::memory::{CandidateStatus, MemoryCandidate},
    services::memory_service::MemoryService,
    state::AppState,
};

fn make_memory_service(state: &AppState) -> MemoryService {
    MemoryService::new(state.db.clone())
}

#[tauri::command]
pub async fn list_memory_candidates(
    state: State<'_, AppState>,
) -> Result<Vec<MemoryCandidate>, String> {
    make_memory_service(&state)
        .list_candidates()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn review_memory_candidate(
    candidate_id: String,
    new_status: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let status = CandidateStatus::try_from(new_status.as_str()).map_err(|e| e.to_string())?;
    make_memory_service(&state)
        .review_candidate(&candidate_id, status)
        .await
        .map_err(|e| e.to_string())
}
