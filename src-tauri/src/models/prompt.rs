use serde::{Deserialize, Serialize};

use super::memory::Memory;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievedChunk {
    pub chunk_id: String,
    pub source_type: String,
    pub content: String,
    pub score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Policy {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub rule_json: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptContext {
    pub conversation_id: String,
    pub memories: Vec<Memory>,
    pub chunks: Vec<RetrievedChunk>,
    pub active_policies: Vec<Policy>,
    /// Raw entity names surfaced for the prompt
    pub entity_names: Vec<String>,
}

impl PromptContext {
    pub fn new(conversation_id: String) -> Self {
        Self {
            conversation_id,
            memories: vec![],
            chunks: vec![],
            active_policies: vec![],
            entity_names: vec![],
        }
    }
}
