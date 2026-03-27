use crate::models::routing::{ModelOption, PrivacyLevel, RouteDecision, RouteReason};

/// Keywords that indicate the query contains sensitive personal data and must
/// stay on-device.
const PRIVACY_KEYWORDS: &[&str] = &[
    "password", "secret", "private", "ssn", "credit card", "bank",
    "medical", "health", "diagnosis", "prescription", "salary", "income",
    "address", "passport", "license",
];

/// Queries longer than this character threshold are sent to the cloud reasoning
/// model — they likely require more compute than a local model can handle well.
const LONG_QUERY_THRESHOLD: usize = 600;

/// Keywords that suggest multi-step reasoning is needed.
const REASONING_KEYWORDS: &[&str] = &[
    "explain why", "compare", "analyse", "analyze", "pros and cons",
    "step by step", "reason", "evaluate", "argue", "synthesize",
];

pub struct RoutingService;

impl RoutingService {
    pub fn new() -> Self {
        Self
    }

    pub fn route(&self, query: &str) -> RouteDecision {
        let lower = query.to_lowercase();

        // 1. Privacy check — always wins; keep data local.
        if let Some(keyword) = PRIVACY_KEYWORDS.iter().find(|&&kw| lower.contains(kw)) {
            return RouteDecision::new(
                ModelOption::LocalGeneral,
                RouteReason::PrivacyKeywordDetected,
                PrivacyLevel::Sensitive,
                format!(
                    "Privacy keyword '{}' detected — routing to local model to keep data on-device.",
                    keyword
                ),
            );
        }

        // 2. Long query — send to cloud reasoning for better quality.
        if query.len() > LONG_QUERY_THRESHOLD {
            return RouteDecision::new(
                ModelOption::CloudReasoning,
                RouteReason::QueryTooLong,
                PrivacyLevel::Public,
                format!(
                    "Query length {} chars exceeds threshold {} — routing to cloud_reasoning.",
                    query.len(),
                    LONG_QUERY_THRESHOLD
                ),
            );
        }

        // 3. Reasoning keywords — cloud reasoning model.
        if let Some(keyword) = REASONING_KEYWORDS.iter().find(|&&kw| lower.contains(kw)) {
            return RouteDecision::new(
                ModelOption::CloudReasoning,
                RouteReason::ComplexReasoning,
                PrivacyLevel::Public,
                format!(
                    "Reasoning keyword '{}' detected — routing to cloud_reasoning.",
                    keyword
                ),
            );
        }

        // 4. Default — short, non-sensitive query goes to cloud_fast.
        RouteDecision::new(
            ModelOption::CloudFast,
            RouteReason::SimpleQuery,
            PrivacyLevel::Public,
            "Short, non-sensitive query — routing to cloud_fast for low latency.".to_string(),
        )
    }
}

impl Default for RoutingService {
    fn default() -> Self {
        Self::new()
    }
}
