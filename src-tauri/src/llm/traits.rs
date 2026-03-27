use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRequest {
    pub model: String,
    pub system_prompt: String,
    pub user_prompt: String,
    /// Optional temperature in [0.0, 2.0]
    pub temperature: Option<f32>,
    /// Optional max tokens
    pub max_tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelResponse {
    pub model: String,
    pub content: String,
    pub input_tokens: u32,
    pub output_tokens: u32,
}

#[async_trait]
pub trait LlmAdapter: Send + Sync {
    async fn generate(&self, request: ModelRequest) -> anyhow::Result<ModelResponse>;
}
