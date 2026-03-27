use serde::{Deserialize, Serialize};

use super::{message::Message, routing::RouteDecision};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendChatTurnResponse {
    pub assistant_message: Message,
    pub route_decision: RouteDecision,
    pub model_used: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityDetailsResponse {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub aliases: Vec<String>,
    pub status: String,
    pub confidence: f64,
    pub related_memory_count: usize,
}
