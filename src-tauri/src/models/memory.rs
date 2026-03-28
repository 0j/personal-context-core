use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryType {
    Fact,
    Preference,
    Goal,
    Belief,
    Skill,
    Experience,
    Relationship,
    Routine,
}

impl MemoryType {
    pub fn as_str(&self) -> &'static str {
        match self {
            MemoryType::Fact => "fact",
            MemoryType::Preference => "preference",
            MemoryType::Goal => "goal",
            MemoryType::Belief => "belief",
            MemoryType::Skill => "skill",
            MemoryType::Experience => "experience",
            MemoryType::Relationship => "relationship",
            MemoryType::Routine => "routine",
        }
    }
}

impl TryFrom<&str> for MemoryType {
    type Error = anyhow::Error;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "fact" => Ok(MemoryType::Fact),
            "preference" => Ok(MemoryType::Preference),
            "goal" => Ok(MemoryType::Goal),
            "belief" => Ok(MemoryType::Belief),
            "skill" => Ok(MemoryType::Skill),
            "experience" => Ok(MemoryType::Experience),
            "relationship" => Ok(MemoryType::Relationship),
            "routine" => Ok(MemoryType::Routine),
            other => Err(anyhow::anyhow!("unknown memory type: {}", other)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TemporalScope {
    Permanent,
    LongTerm,
    ShortTerm,
    Session,
}

impl TemporalScope {
    pub fn as_str(&self) -> &'static str {
        match self {
            TemporalScope::Permanent => "permanent",
            TemporalScope::LongTerm => "long_term",
            TemporalScope::ShortTerm => "short_term",
            TemporalScope::Session => "session",
        }
    }
}

impl TryFrom<&str> for TemporalScope {
    type Error = anyhow::Error;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "permanent" => Ok(TemporalScope::Permanent),
            "long_term" => Ok(TemporalScope::LongTerm),
            "short_term" => Ok(TemporalScope::ShortTerm),
            "session" => Ok(TemporalScope::Session),
            other => Err(anyhow::anyhow!("unknown temporal scope: {}", other)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Sensitivity {
    Public,
    Internal,
    Private,
    Secret,
}

impl Sensitivity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Sensitivity::Public => "public",
            Sensitivity::Internal => "internal",
            Sensitivity::Private => "private",
            Sensitivity::Secret => "secret",
        }
    }
}

impl TryFrom<&str> for Sensitivity {
    type Error = anyhow::Error;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "public" => Ok(Sensitivity::Public),
            "internal" => Ok(Sensitivity::Internal),
            "private" => Ok(Sensitivity::Private),
            "secret" => Ok(Sensitivity::Secret),
            other => Err(anyhow::anyhow!("unknown sensitivity: {}", other)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    UserMessage,
    AssistantInference,
    UserEdit,
    Import,
    /// Memory materialized from an approved candidate extracted from a conversation.
    Conversation,
}

impl SourceKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            SourceKind::UserMessage => "user_message",
            SourceKind::AssistantInference => "assistant_inference",
            SourceKind::UserEdit => "user_edit",
            SourceKind::Import => "import",
            SourceKind::Conversation => "conversation",
        }
    }
}

impl TryFrom<&str> for SourceKind {
    type Error = anyhow::Error;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "user_message" => Ok(SourceKind::UserMessage),
            "assistant_inference" => Ok(SourceKind::AssistantInference),
            "user_edit" => Ok(SourceKind::UserEdit),
            "import" => Ok(SourceKind::Import),
            "conversation" => Ok(SourceKind::Conversation),
            other => Err(anyhow::anyhow!("unknown source kind: {}", other)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    pub id: String,
    pub kind: MemoryType,
    pub scope: TemporalScope,
    pub sensitivity: Sensitivity,
    pub content: String,
    pub salience_score: f64,
    pub source_kind: SourceKind,
    pub source_id: Option<String>,
    pub entity_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

impl Memory {
    pub fn new(
        kind: MemoryType,
        scope: TemporalScope,
        content: String,
        source_kind: SourceKind,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            kind,
            scope,
            sensitivity: Sensitivity::Internal,
            content,
            salience_score: 0.5,
            source_kind,
            source_id: None,
            entity_id: None,
            created_at: now,
            updated_at: now,
            expires_at: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CandidateType {
    Fact,
    Preference,
    Goal,
    Relationship,
    Other,
}

impl CandidateType {
    pub fn as_str(&self) -> &'static str {
        match self {
            CandidateType::Fact => "fact",
            CandidateType::Preference => "preference",
            CandidateType::Goal => "goal",
            CandidateType::Relationship => "relationship",
            CandidateType::Other => "other",
        }
    }
}

impl TryFrom<&str> for CandidateType {
    type Error = anyhow::Error;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "fact" => Ok(CandidateType::Fact),
            "preference" => Ok(CandidateType::Preference),
            "goal" => Ok(CandidateType::Goal),
            "relationship" => Ok(CandidateType::Relationship),
            "other" => Ok(CandidateType::Other),
            other => Err(anyhow::anyhow!("unknown candidate type: {}", other)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CandidateStatus {
    Pending,
    Approved,
    Rejected,
    Deferred,
}

impl CandidateStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            CandidateStatus::Pending => "pending",
            CandidateStatus::Approved => "approved",
            CandidateStatus::Rejected => "rejected",
            CandidateStatus::Deferred => "deferred",
        }
    }
}

impl TryFrom<&str> for CandidateStatus {
    type Error = anyhow::Error;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "pending" => Ok(CandidateStatus::Pending),
            "approved" => Ok(CandidateStatus::Approved),
            "rejected" => Ok(CandidateStatus::Rejected),
            "deferred" => Ok(CandidateStatus::Deferred),
            other => Err(anyhow::anyhow!("unknown candidate status: {}", other)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryCandidate {
    pub id: String,
    pub kind: CandidateType,
    pub content: String,
    pub source_id: Option<String>,
    pub confidence: f64,
    pub status: CandidateStatus,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

impl MemoryCandidate {
    pub fn new(kind: CandidateType, content: String, confidence: f64) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            kind,
            content,
            source_id: None,
            confidence,
            status: CandidateStatus::Pending,
            reviewed_at: None,
            created_at: Utc::now(),
        }
    }
}
