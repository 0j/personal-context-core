use anyhow::Context;
use sqlx::SqlitePool;

use crate::models::memory::{
    CandidateStatus, CandidateType, Memory, MemoryCandidate, MemoryType, Sensitivity, SourceKind,
    TemporalScope,
};

pub struct MemoryService {
    db: SqlitePool,
}

impl MemoryService {
    pub fn new(db: SqlitePool) -> Self {
        Self { db }
    }

    /// Retrieve memories whose content matches any of the provided keywords,
    /// ordered by salience_score descending.
    pub async fn retrieve_relevant(&self, keywords: &[&str], limit: u32) -> anyhow::Result<Vec<Memory>> {
        if keywords.is_empty() {
            return Ok(vec![]);
        }

        // Build a LIKE clause for each keyword joined with OR.
        let conditions: Vec<String> = keywords
            .iter()
            .map(|kw| format!("content LIKE '%{}%'", kw.replace('\'', "''")))
            .collect();
        let where_clause = conditions.join(" OR ");

        let sql = format!(
            "SELECT id, kind, scope, sensitivity, content, salience_score, \
                    source_kind, source_id, entity_id, created_at, updated_at, expires_at \
             FROM memories \
             WHERE {} \
             ORDER BY salience_score DESC \
             LIMIT {}",
            where_clause, limit
        );

        let rows = sqlx::query_as::<_, MemoryRow>(&sql)
            .fetch_all(&self.db)
            .await?;

        rows.into_iter().map(MemoryRow::into_memory).collect()
    }

    /// Retrieve all pending memory candidates.
    pub async fn list_candidates(&self) -> anyhow::Result<Vec<MemoryCandidate>> {
        let rows = sqlx::query_as::<_, CandidateRow>(
            "SELECT id, kind, content, source_id, confidence, status, reviewed_at, created_at \
             FROM memory_candidates \
             WHERE status = 'pending' \
             ORDER BY created_at DESC",
        )
        .fetch_all(&self.db)
        .await?;

        rows.into_iter().map(CandidateRow::into_candidate).collect()
    }

    /// Update the status of a memory candidate (approve / reject / defer).
    /// When approved, materializes the candidate into the `memories` table.
    pub async fn review_candidate(
        &self,
        candidate_id: &str,
        new_status: CandidateStatus,
    ) -> anyhow::Result<()> {
        if new_status == CandidateStatus::Approved {
            self.materialize_candidate(candidate_id).await?;
        }

        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE memory_candidates SET status = ?, reviewed_at = ? WHERE id = ?",
        )
        .bind(new_status.as_str())
        .bind(&now)
        .bind(candidate_id)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    /// Parse the candidate's JSON content and insert a confirmed `Memory` row.
    async fn materialize_candidate(&self, candidate_id: &str) -> anyhow::Result<()> {
        let row = sqlx::query_as::<_, CandidateRow>(
            "SELECT id, kind, content, source_id, confidence, status, reviewed_at, created_at \
             FROM memory_candidates WHERE id = ?",
        )
        .bind(candidate_id)
        .fetch_one(&self.db)
        .await
        .context("candidate not found")?;

        let candidate = row.into_candidate()?;

        // The content field holds a MemoryCandidateRaw JSON object written by the
        // extraction worker.  Fall back gracefully when individual fields are absent.
        let payload: serde_json::Value = serde_json::from_str(&candidate.content)
            .unwrap_or(serde_json::Value::Null);

        let statement = payload
            .get("statement")
            .and_then(|v| v.as_str())
            .unwrap_or(&candidate.content)
            .to_string();

        let kind = payload
            .get("memory_type")
            .and_then(|v| v.as_str())
            .and_then(|s| MemoryType::try_from(s).ok())
            .unwrap_or(MemoryType::Fact);

        let scope = payload
            .get("temporal_scope")
            .and_then(|v| v.as_str())
            .and_then(|s| TemporalScope::try_from(s).ok())
            .unwrap_or(TemporalScope::LongTerm);

        let sensitivity = payload
            .get("sensitivity")
            .and_then(|v| v.as_str())
            .and_then(|s| Sensitivity::try_from(s).ok())
            .unwrap_or(Sensitivity::Internal);

        let salience_score = payload
            .get("salience_score")
            .and_then(|v| v.as_f64())
            .unwrap_or(candidate.confidence);

        let now = chrono::Utc::now().to_rfc3339();
        let memory_id = uuid::Uuid::new_v4().to_string();

        sqlx::query(
            "INSERT INTO memories \
             (id, kind, scope, sensitivity, content, salience_score, \
              source_kind, source_id, entity_id, created_at, updated_at, expires_at) \
             VALUES (?, ?, ?, ?, ?, ?, 'conversation', NULL, NULL, ?, ?, NULL)",
        )
        .bind(&memory_id)
        .bind(kind.as_str())
        .bind(scope.as_str())
        .bind(sensitivity.as_str())
        .bind(&statement)
        .bind(salience_score)
        .bind(&now)
        .bind(&now)
        .execute(&self.db)
        .await
        .context("failed to insert materialized memory")?;

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Row types for sqlx mapping
// ---------------------------------------------------------------------------

#[derive(sqlx::FromRow)]
struct MemoryRow {
    id: String,
    kind: String,
    scope: String,
    sensitivity: String,
    content: String,
    salience_score: f64,
    source_kind: String,
    source_id: Option<String>,
    entity_id: Option<String>,
    created_at: String,
    updated_at: String,
    expires_at: Option<String>,
}

impl MemoryRow {
    fn into_memory(self) -> anyhow::Result<Memory> {
        Ok(Memory {
            id: self.id,
            kind: MemoryType::try_from(self.kind.as_str())?,
            scope: TemporalScope::try_from(self.scope.as_str())?,
            sensitivity: Sensitivity::try_from(self.sensitivity.as_str())?,
            content: self.content,
            salience_score: self.salience_score,
            source_kind: SourceKind::try_from(self.source_kind.as_str())?,
            source_id: self.source_id,
            entity_id: self.entity_id,
            created_at: self.created_at.parse()?,
            updated_at: self.updated_at.parse()?,
            expires_at: self.expires_at.map(|s| s.parse()).transpose()?,
        })
    }
}

#[derive(sqlx::FromRow)]
struct CandidateRow {
    id: String,
    kind: String,
    content: String,
    source_id: Option<String>,
    confidence: f64,
    status: String,
    reviewed_at: Option<String>,
    created_at: String,
}

impl CandidateRow {
    fn into_candidate(self) -> anyhow::Result<MemoryCandidate> {
        Ok(MemoryCandidate {
            id: self.id,
            kind: CandidateType::try_from(self.kind.as_str())?,
            content: self.content,
            source_id: self.source_id,
            confidence: self.confidence,
            status: CandidateStatus::try_from(self.status.as_str())?,
            reviewed_at: self.reviewed_at.map(|s| s.parse()).transpose()?,
            created_at: self.created_at.parse()?,
        })
    }
}
