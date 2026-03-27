use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EntityType {
    Person,
    Place,
    Project,
    Organization,
    Concept,
    Tool,
    Event,
    Other,
}

impl EntityType {
    pub fn as_str(&self) -> &'static str {
        match self {
            EntityType::Person => "person",
            EntityType::Place => "place",
            EntityType::Project => "project",
            EntityType::Organization => "organization",
            EntityType::Concept => "concept",
            EntityType::Tool => "tool",
            EntityType::Event => "event",
            EntityType::Other => "other",
        }
    }
}

impl TryFrom<&str> for EntityType {
    type Error = anyhow::Error;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "person" => Ok(EntityType::Person),
            "place" => Ok(EntityType::Place),
            "project" => Ok(EntityType::Project),
            "organization" => Ok(EntityType::Organization),
            "concept" => Ok(EntityType::Concept),
            "tool" => Ok(EntityType::Tool),
            "event" => Ok(EntityType::Event),
            "other" => Ok(EntityType::Other),
            other => Err(anyhow::anyhow!("unknown entity type: {}", other)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EntityStatus {
    Active,
    Archived,
    Merged,
    Deleted,
}

impl EntityStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            EntityStatus::Active => "active",
            EntityStatus::Archived => "archived",
            EntityStatus::Merged => "merged",
            EntityStatus::Deleted => "deleted",
        }
    }
}

impl TryFrom<&str> for EntityStatus {
    type Error = anyhow::Error;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "active" => Ok(EntityStatus::Active),
            "archived" => Ok(EntityStatus::Archived),
            "merged" => Ok(EntityStatus::Merged),
            "deleted" => Ok(EntityStatus::Deleted),
            other => Err(anyhow::anyhow!("unknown entity status: {}", other)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub id: String,
    pub kind: EntityType,
    pub name: String,
    pub aliases: Vec<String>,
    pub status: EntityStatus,
    pub confidence: f64,
    pub source_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Entity {
    pub fn new(kind: EntityType, name: String) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            kind,
            name,
            aliases: vec![],
            status: EntityStatus::Active,
            confidence: 1.0,
            source_id: None,
            created_at: now,
            updated_at: now,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relationship {
    pub id: String,
    pub from_entity: String,
    pub to_entity: String,
    pub relation_type: String,
    pub weight: f64,
    pub created_at: DateTime<Utc>,
}

impl Relationship {
    pub fn new(from_entity: String, to_entity: String, relation_type: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            from_entity,
            to_entity,
            relation_type,
            weight: 1.0,
            created_at: Utc::now(),
        }
    }
}
