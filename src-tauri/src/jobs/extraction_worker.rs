use std::{
    collections::HashSet,
    sync::{Arc, OnceLock},
};

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use jsonschema::JSONSchema;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
    llm::traits::ModelRequest,
    models::{
        entity::{Entity, EntityStatus, EntityType},
        memory::{Memory, MemoryType, Sensitivity, SourceKind, TemporalScope},
        message::{Message, MessageRole},
    },
    services::chat_service::ChatService,
    state::AppState,
};

const EXTRACTOR_MODEL: &str = "cloud_fast";
const EXTRACTOR_PROMPT: &str = include_str!("../prompts/extractor_v1.txt");
const EXTRACTION_SCHEMA: &str =
    include_str!("../../../packages/prompt-contracts/extraction.schema.json");

// ---------------------------------------------------------------------------
// Public job descriptor
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionJob {
    pub conversation_id: String,
    pub message_ids: Vec<String>,
}

// ---------------------------------------------------------------------------
// Internal DTO types (shape the LLM JSON response)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExtractionOutput {
    entities_detected: Vec<EntityCandidate>,
    relationships_detected: Vec<RelationshipCandidate>,
    memory_candidates: Vec<MemoryCandidateRaw>,
    contradictions_found: Vec<ContradictionCandidate>,
    stale_candidates: Vec<StaleCandidate>,
    confidence_notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SourceRef {
    message_id: String,
    #[serde(default)]
    excerpt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EntityCandidate {
    entity_type: String,
    canonical_name: String,
    summary: String,
    confidence: f32,
    source_refs: Vec<SourceRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RelationshipCandidate {
    from_name: String,
    to_name: String,
    relation_type: String,
    confidence: f32,
    source_refs: Vec<SourceRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MemoryCandidateRaw {
    candidate_type: String,
    memory_type: String,
    statement: String,
    summary: String,
    temporal_scope: String,
    sensitivity: String,
    confidence: f32,
    salience_score: f32,
    related_entity_names: Vec<String>,
    source_refs: Vec<SourceRef>,
    rationale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ContradictionCandidate {
    existing_memory_id: String,
    new_claim: String,
    confidence: f32,
    source_refs: Vec<SourceRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StaleCandidate {
    memory_id: String,
    reason: String,
    confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ScoredCandidate {
    raw: MemoryCandidateRaw,
    rescored_confidence: f32,
    explicit_user_statement: bool,
    repeated_signal_count: u32,
    has_named_entities: bool,
    is_actionable: bool,
    is_sensitive: bool,
    auto_rejected: bool,
    auto_reject_reason: Option<String>,
}

// ---------------------------------------------------------------------------
// Worker
// ---------------------------------------------------------------------------

pub struct ExtractionWorker;

impl ExtractionWorker {
    pub async fn run_once(state: Arc<AppState>, job: ExtractionJob) -> Result<()> {
        let messages =
            ChatService::recent_messages(&state.db, &job.conversation_id, 8)
                .await
                .context("failed to load recent messages for extraction")?;

        if messages.is_empty() {
            Self::log_event(
                &state.db,
                "extraction_skipped",
                "extractor",
                "conversation",
                &job.conversation_id,
                json!({ "reason": "no_messages" }),
            )
            .await?;
            return Ok(());
        }

        let active_entities = Self::load_active_entities(&state.db, 24).await?;
        let recent_memories = Self::load_recently_approved_memories(&state.db, 24).await?;

        let system_prompt =
            Self::build_extractor_system_prompt(&active_entities, &recent_memories, &messages)?;
        let user_prompt = Self::build_extractor_user_prompt(&messages)?;

        let adapter = state
            .registry
            .get(EXTRACTOR_MODEL)
            .context("extractor model not registered")?;

        let response = adapter
            .generate(ModelRequest {
                model: EXTRACTOR_MODEL.to_string(),
                system_prompt,
                user_prompt,
                temperature: Some(0.0_f32),
                max_tokens: Some(4096),
            })
            .await
            .context("extractor model call failed")?;

        let raw_json = Self::extract_json_object(&response.content)
            .context("extractor did not return parseable JSON object")?;

        Self::validate_against_schema(&raw_json)?;

        let output: ExtractionOutput = serde_json::from_value(raw_json.clone())
            .context("schema-valid JSON did not deserialize")?;

        let scored = Self::score_candidates(&messages, &output.memory_candidates);

        let mut proposed = Vec::new();
        let mut rejected = Vec::new();

        for candidate in scored {
            if candidate.auto_rejected {
                let reject_reason = candidate
                    .auto_reject_reason
                    .clone()
                    .unwrap_or_else(|| "auto_rejected".to_string());

                Self::log_event(
                    &state.db,
                    "memory_candidate_auto_rejected",
                    "extractor",
                    "conversation",
                    &job.conversation_id,
                    json!({
                        "reason": reject_reason,
                        "candidate": candidate.raw,
                        "rescored_confidence": candidate.rescored_confidence,
                        "extraction_model": EXTRACTOR_MODEL
                    }),
                )
                .await?;

                rejected.push(json!({
                    "statement": candidate.raw.statement,
                    "reason": reject_reason,
                    "rescored_confidence": candidate.rescored_confidence
                }));
                continue;
            }

            let candidate_id = Uuid::new_v4().to_string();

            // Adapt to actual memory_candidates schema:
            //   kind       TEXT NOT NULL  ← candidate_type
            //   content    TEXT NOT NULL  ← full JSON payload (preserves all rich fields)
            //   source_id  TEXT           ← NULL (no single source)
            //   confidence REAL           ← rescored_confidence
            //   status     TEXT           ← 'pending'
            //   created_at TEXT
            //   reviewed_at TEXT          ← NULL
            sqlx::query(
                r#"
                INSERT INTO memory_candidates
                    (id, kind, content, source_id, confidence, status, created_at, reviewed_at)
                VALUES (?1, ?2, ?3, NULL, ?4, 'pending', ?5, NULL)
                "#,
            )
            .bind(&candidate_id)
            .bind(&candidate.raw.candidate_type)
            .bind(serde_json::to_string(&candidate.raw)?)
            .bind(candidate.rescored_confidence as f64)
            .bind(Utc::now().to_rfc3339())
            .execute(&state.db)
            .await
            .context("failed to insert memory candidate")?;

            Self::log_event(
                &state.db,
                "memory_candidate_proposed",
                "extractor",
                "memory_candidate",
                &candidate_id,
                json!({
                    "candidate": candidate.raw,
                    "rescored_confidence": candidate.rescored_confidence,
                    "signals": {
                        "explicit_user_statement": candidate.explicit_user_statement,
                        "repeated_signal_count": candidate.repeated_signal_count,
                        "has_named_entities": candidate.has_named_entities,
                        "is_actionable": candidate.is_actionable,
                        "is_sensitive": candidate.is_sensitive
                    },
                    "extraction_model": EXTRACTOR_MODEL
                }),
            )
            .await?;

            proposed.push(json!({
                "candidate_id": candidate_id,
                "statement": candidate.raw.statement,
                "rescored_confidence": candidate.rescored_confidence
            }));
        }

        Self::log_event(
            &state.db,
            "extraction_completed",
            "extractor",
            "conversation",
            &job.conversation_id,
            json!({
                "message_count": messages.len(),
                "model": EXTRACTOR_MODEL,
                "proposed_count": proposed.len(),
                "rejected_count": rejected.len(),
                "entities_detected_count": output.entities_detected.len(),
                "relationships_detected_count": output.relationships_detected.len(),
                "contradictions_found_count": output.contradictions_found.len(),
                "stale_candidates_count": output.stale_candidates.len(),
                "proposed": proposed,
                "rejected": rejected,
                "confidence_notes": output.confidence_notes,
            }),
        )
        .await?;

        Ok(())
    }

    // ---------------------------------------------------------------------------
    // Prompt builders
    // ---------------------------------------------------------------------------

    fn build_extractor_system_prompt(
        active_entities: &[Entity],
        recent_memories: &[Memory],
        messages: &[Message],
    ) -> Result<String> {
        let active_entities_json = serde_json::to_string_pretty(active_entities)?;
        let recent_memories_json = serde_json::to_string_pretty(recent_memories)?;
        let messages_json = serde_json::to_string_pretty(messages)?;

        Ok(EXTRACTOR_PROMPT
            .replace("{{ACTIVE_ENTITIES_JSON}}", &active_entities_json)
            .replace("{{RECENT_MEMORIES_JSON}}", &recent_memories_json)
            .replace("{{MESSAGES_JSON}}", &messages_json))
    }

    fn build_extractor_user_prompt(messages: &[Message]) -> Result<String> {
        Ok(format!(
            "Analyze the recent conversation and return only JSON.\n\nRecent messages:\n{}",
            serde_json::to_string_pretty(messages)?
        ))
    }

    // ---------------------------------------------------------------------------
    // Schema validation
    // ---------------------------------------------------------------------------

    fn validate_against_schema(raw_json: &Value) -> Result<()> {
        let schema_value: Value = serde_json::from_str(EXTRACTION_SCHEMA)
            .context("failed to parse extraction schema JSON")?;
        let compiled = JSONSchema::compile(&schema_value)
            .map_err(|e| anyhow!("failed to compile JSON schema: {e}"))?;

        if let Err(errors) = compiled.validate(raw_json) {
            let msgs: Vec<String> = errors.map(|e| e.to_string()).collect();
            return Err(anyhow!(
                "extractor JSON failed schema validation: {}",
                msgs.join("; ")
            ));
        }

        Ok(())
    }

    // ---------------------------------------------------------------------------
    // JSON extraction (handles LLM responses with surrounding prose)
    // ---------------------------------------------------------------------------

    fn extract_json_object(text: &str) -> Result<Value> {
        if let Ok(v) = serde_json::from_str::<Value>(text) {
            return Ok(v);
        }

        let start = text.find('{').ok_or_else(|| anyhow!("no opening brace found"))?;
        let end = text.rfind('}').ok_or_else(|| anyhow!("no closing brace found"))?;
        let slice = &text[start..=end];
        let parsed = serde_json::from_str::<Value>(slice)
            .with_context(|| format!("failed to parse extracted JSON slice: {}", slice))?;
        Ok(parsed)
    }

    // ---------------------------------------------------------------------------
    // DB loaders — adapted to actual schema
    // ---------------------------------------------------------------------------

    async fn load_active_entities(db: &sqlx::SqlitePool, limit: i64) -> Result<Vec<Entity>> {
        #[derive(sqlx::FromRow)]
        struct EntityRow {
            id: String,
            kind: String,
            name: String,
            aliases: Option<String>,
            status: String,
            confidence: f64,
            source_id: Option<String>,
            created_at: String,
            updated_at: String,
        }

        let rows = sqlx::query_as::<_, EntityRow>(
            r#"
            SELECT id, kind, name, aliases, status, confidence, source_id, created_at, updated_at
            FROM entities
            WHERE status = 'active'
            ORDER BY confidence DESC, updated_at DESC
            LIMIT ?1
            "#,
        )
        .bind(limit)
        .fetch_all(db)
        .await?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            let aliases: Vec<String> = row
                .aliases
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or_default();

            out.push(Entity {
                id: row.id,
                kind: EntityType::try_from(row.kind.as_str())
                    .unwrap_or(EntityType::Other),
                name: row.name,
                aliases,
                status: EntityStatus::try_from(row.status.as_str())
                    .unwrap_or(EntityStatus::Active),
                confidence: row.confidence,
                source_id: row.source_id,
                created_at: row.created_at.parse()?,
                updated_at: row.updated_at.parse()?,
            });
        }
        Ok(out)
    }

    async fn load_recently_approved_memories(
        db: &sqlx::SqlitePool,
        limit: i64,
    ) -> Result<Vec<Memory>> {
        #[derive(sqlx::FromRow)]
        struct MemRow {
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

        let rows = sqlx::query_as::<_, MemRow>(
            r#"
            SELECT id, kind, scope, sensitivity, content, salience_score,
                   source_kind, source_id, entity_id, created_at, updated_at, expires_at
            FROM memories
            ORDER BY updated_at DESC
            LIMIT ?1
            "#,
        )
        .bind(limit)
        .fetch_all(db)
        .await?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            out.push(Memory {
                id: row.id,
                kind: MemoryType::try_from(row.kind.as_str())
                    .unwrap_or(MemoryType::Fact),
                scope: TemporalScope::try_from(row.scope.as_str())
                    .unwrap_or(TemporalScope::LongTerm),
                sensitivity: Sensitivity::try_from(row.sensitivity.as_str())
                    .unwrap_or(Sensitivity::Internal),
                content: row.content,
                salience_score: row.salience_score,
                source_kind: SourceKind::try_from(row.source_kind.as_str())
                    .unwrap_or(SourceKind::AssistantInference),
                source_id: row.source_id,
                entity_id: row.entity_id,
                created_at: row.created_at.parse()?,
                updated_at: row.updated_at.parse()?,
                expires_at: row.expires_at.map(|s| s.parse()).transpose()?,
            });
        }
        Ok(out)
    }

    // ---------------------------------------------------------------------------
    // Scoring
    // ---------------------------------------------------------------------------

    fn score_candidates(
        messages: &[Message],
        candidates: &[MemoryCandidateRaw],
    ) -> Vec<ScoredCandidate> {
        let user_messages: Vec<&Message> = messages
            .iter()
            .filter(|m| matches!(m.role, MessageRole::User))
            .collect();

        let mut out = Vec::with_capacity(candidates.len());

        for raw in candidates {
            let explicit_user_statement =
                Self::explicit_user_statement(&user_messages, raw);
            let repeated_signal_count = Self::repeated_signal_count(messages, raw);
            let has_named_entities = !raw.related_entity_names.is_empty()
                || Self::contains_named_entity_shape(&raw.statement);
            let is_actionable = Self::is_actionable(raw);
            let is_sensitive =
                raw.sensitivity == "private" || raw.sensitivity == "highly_private";

            let rescored_confidence = Self::rescore_candidate(
                raw.confidence,
                explicit_user_statement,
                repeated_signal_count,
                has_named_entities,
                is_actionable,
                is_sensitive,
            );

            let (auto_rejected, auto_reject_reason) = if raw.source_refs.is_empty() {
                (true, Some("no_source_refs".to_string()))
            } else if rescored_confidence < 0.45 {
                (true, Some("confidence_below_threshold".to_string()))
            } else {
                (false, None)
            };


            out.push(ScoredCandidate {
                raw: raw.clone(),
                rescored_confidence,
                explicit_user_statement,
                repeated_signal_count,
                has_named_entities,
                is_actionable,
                is_sensitive,
                auto_rejected,
                auto_reject_reason,
            });
        }

        out
    }

    fn rescore_candidate(
        model_confidence: f32,
        explicit_user_statement: bool,
        repeated_signal_count: u32,
        has_named_entities: bool,
        is_actionable: bool,
        is_sensitive: bool,
    ) -> f32 {
        let mut score = model_confidence * 0.45;
        if explicit_user_statement {
            score += 0.20;
        }
        if repeated_signal_count > 1 {
            score += 0.15;
        }
        if has_named_entities {
            score += 0.10;
        }
        if is_actionable {
            score += 0.10;
        }
        if is_sensitive {
            score -= 0.10;
        }
        score.clamp(0.0, 1.0)
    }

    fn explicit_user_statement(user_messages: &[&Message], raw: &MemoryCandidateRaw) -> bool {
        let statement_lc = raw.statement.to_lowercase();
        let summary_lc = raw.summary.to_lowercase();

        user_messages.iter().any(|m| {
            let lc = m.content.to_lowercase();
            lc.contains(&statement_lc)
                || (!summary_lc.is_empty() && lc.contains(&summary_lc))
                || Self::fuzzy_overlap(&lc, &statement_lc) >= 0.72
        })
    }

    fn repeated_signal_count(messages: &[Message], raw: &MemoryCandidateRaw) -> u32 {
        let tokens = Self::salient_tokens(&raw.statement);
        if tokens.is_empty() {
            return 0;
        }

        let mut count = 0;
        for m in messages {
            let lc = m.content.to_lowercase();
            let hits = tokens.iter().filter(|t| lc.contains(t.as_str())).count();
            if hits >= 2 {
                count += 1;
            }
        }
        count
    }

    fn contains_named_entity_shape(s: &str) -> bool {
        static RE: OnceLock<Regex> = OnceLock::new();
        let re = RE.get_or_init(|| {
            Regex::new(r"\b[A-Z][a-zA-Z0-9]+(?:\s+[A-Z][a-zA-Z0-9]+)+\b").unwrap()
        });
        re.is_match(s)
    }

    fn is_actionable(raw: &MemoryCandidateRaw) -> bool {
        matches!(
            raw.memory_type.as_str(),
            "preference" | "ongoing_project" | "workflow_rule" | "constraint"
        ) || {
            let lc = format!(
                "{} {}",
                raw.statement.to_lowercase(),
                raw.summary.to_lowercase()
            );
            [
                "prefer", "wants", "needs", "should", "must", "working on", "project",
                "goal", "constraint",
            ]
            .iter()
            .any(|needle| lc.contains(needle))
        }
    }

    fn salient_tokens(s: &str) -> Vec<String> {
        let stop: HashSet<&str> = [
            "the", "and", "for", "with", "that", "this", "from", "have", "has", "user",
            "will", "would", "their", "they", "them", "into", "about", "your", "ours",
            "core", "statement", "summary",
        ]
        .into_iter()
        .collect();

        s.to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|t| t.len() > 3 && !stop.contains(*t))
            .map(|t| t.to_string())
            .collect()
    }

    fn fuzzy_overlap(a: &str, b: &str) -> f32 {
        let a_tokens: HashSet<String> = Self::salient_tokens(a).into_iter().collect();
        let b_tokens: HashSet<String> = Self::salient_tokens(b).into_iter().collect();

        if a_tokens.is_empty() || b_tokens.is_empty() {
            return 0.0;
        }

        let inter = a_tokens.intersection(&b_tokens).count() as f32;
        let union = a_tokens.union(&b_tokens).count() as f32;

        if union == 0.0 { 0.0 } else { inter / union }
    }

    // ---------------------------------------------------------------------------
    // Event log — adapted to actual events schema (kind, payload, created_at)
    // Actor and object tracking folded into payload JSON.
    // ---------------------------------------------------------------------------

    async fn log_event(
        db: &sqlx::SqlitePool,
        event_type: &str,
        actor: &str,
        object_type: &str,
        object_id: &str,
        mut payload: Value,
    ) -> Result<()> {
        if let Value::Object(ref mut map) = payload {
            map.insert("_actor".to_string(), Value::String(actor.to_string()));
            map.insert(
                "_object_type".to_string(),
                Value::String(object_type.to_string()),
            );
            map.insert(
                "_object_id".to_string(),
                Value::String(object_id.to_string()),
            );
        }

        sqlx::query(
            "INSERT INTO events (id, kind, payload, created_at) VALUES (?1, ?2, ?3, ?4)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(event_type)
        .bind(payload.to_string())
        .bind(Utc::now().to_rfc3339())
        .execute(db)
        .await?;

        Ok(())
    }
}
