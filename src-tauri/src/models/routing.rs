use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PrivacyLevel {
    /// Safe to send to any model including cloud
    Public,
    /// Send to local model only
    Sensitive,
    /// Never leave device; block cloud routing
    Restricted,
}

impl PrivacyLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            PrivacyLevel::Public => "public",
            PrivacyLevel::Sensitive => "sensitive",
            PrivacyLevel::Restricted => "restricted",
        }
    }
}

impl TryFrom<&str> for PrivacyLevel {
    type Error = anyhow::Error;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "public" => Ok(PrivacyLevel::Public),
            "sensitive" => Ok(PrivacyLevel::Sensitive),
            "restricted" => Ok(PrivacyLevel::Restricted),
            other => Err(anyhow::anyhow!("unknown privacy level: {}", other)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RouteReason {
    PrivacyKeywordDetected,
    QueryTooLong,
    SimpleQuery,
    ComplexReasoning,
    UserForced,
    PolicyOverride,
}

impl RouteReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            RouteReason::PrivacyKeywordDetected => "privacy_keyword_detected",
            RouteReason::QueryTooLong => "query_too_long",
            RouteReason::SimpleQuery => "simple_query",
            RouteReason::ComplexReasoning => "complex_reasoning",
            RouteReason::UserForced => "user_forced",
            RouteReason::PolicyOverride => "policy_override",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ModelOption {
    LocalGeneral,
    CloudFast,
    CloudReasoning,
}

impl ModelOption {
    pub fn as_str(&self) -> &'static str {
        match self {
            ModelOption::LocalGeneral => "local_general",
            ModelOption::CloudFast => "cloud_fast",
            ModelOption::CloudReasoning => "cloud_reasoning",
        }
    }
}

impl TryFrom<&str> for ModelOption {
    type Error = anyhow::Error;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "local_general" => Ok(ModelOption::LocalGeneral),
            "cloud_fast" => Ok(ModelOption::CloudFast),
            "cloud_reasoning" => Ok(ModelOption::CloudReasoning),
            other => Err(anyhow::anyhow!("unknown model option: {}", other)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteDecision {
    pub id: String,
    pub conversation_id: Option<String>,
    pub message_id: Option<String>,
    pub model_chosen: ModelOption,
    pub reason: RouteReason,
    pub privacy_level: PrivacyLevel,
    pub explanation: String,
    pub score: Option<f64>,
    pub created_at: DateTime<Utc>,
}

impl RouteDecision {
    pub fn new(
        model_chosen: ModelOption,
        reason: RouteReason,
        privacy_level: PrivacyLevel,
        explanation: String,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            conversation_id: None,
            message_id: None,
            model_chosen,
            reason,
            privacy_level,
            explanation,
            score: None,
            created_at: Utc::now(),
        }
    }
}
