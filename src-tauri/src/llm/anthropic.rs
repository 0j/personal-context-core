use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::traits::{LlmAdapter, ModelRequest, ModelResponse};

const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";

fn model_id_for(slot: &str) -> &'static str {
    match slot {
        "cloud_fast" => "claude-haiku-4-5-20251001",
        "cloud_reasoning" => "claude-sonnet-4-6",
        _ => "claude-haiku-4-5-20251001",
    }
}

// ---------------------------------------------------------------------------
// Request / response shapes
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct ApiRequest<'a> {
    model: &'a str,
    max_tokens: u32,
    system: &'a str,
    messages: Vec<ApiMessage<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
}

#[derive(Serialize)]
struct ApiMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct ApiResponse {
    model: String,
    content: Vec<ContentBlock>,
    usage: Usage,
}

#[derive(Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    kind: String,
    text: Option<String>,
}

#[derive(Deserialize)]
struct Usage {
    input_tokens: u32,
    output_tokens: u32,
}

// ---------------------------------------------------------------------------
// Error response shape (for better diagnostics)
// ---------------------------------------------------------------------------

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
struct ApiError {
    #[serde(rename = "type")]
    kind: String,
    error: ApiErrorDetail,
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
struct ApiErrorDetail {
    #[serde(rename = "type")]
    kind: String,
    message: String,
}

// ---------------------------------------------------------------------------
// Adapter
// ---------------------------------------------------------------------------

pub struct AnthropicAdapter {
    /// Registry slot name — used to pick the right model ID.
    pub slot: String,
    client: reqwest::Client,
    api_key: String,
}

impl AnthropicAdapter {
    /// Reads `ANTHROPIC_API_KEY` from the environment.
    pub fn from_env(slot: impl Into<String>) -> anyhow::Result<Self> {
        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .map_err(|_| anyhow::anyhow!("ANTHROPIC_API_KEY env var is not set"))?;

        Ok(Self {
            slot: slot.into(),
            client: reqwest::Client::new(),
            api_key,
        })
    }
}

#[async_trait]
impl LlmAdapter for AnthropicAdapter {
    async fn generate(&self, request: ModelRequest) -> anyhow::Result<ModelResponse> {
        let model = model_id_for(&self.slot);
        let max_tokens = request.max_tokens.unwrap_or(1024);

        let body = ApiRequest {
            model,
            max_tokens,
            system: &request.system_prompt,
            messages: vec![ApiMessage {
                role: "user",
                content: &request.user_prompt,
            }],
            temperature: request.temperature,
        };

        let resp = self
            .client
            .post(ANTHROPIC_API_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("Anthropic API error {status}: {text}");
        }

        let api_resp: ApiResponse = resp.json().await?;

        let content = api_resp
            .content
            .into_iter()
            .filter(|b| b.kind == "text")
            .filter_map(|b| b.text)
            .collect::<Vec<_>>()
            .join("");

        Ok(ModelResponse {
            model: api_resp.model,
            content,
            input_tokens: api_resp.usage.input_tokens,
            output_tokens: api_resp.usage.output_tokens,
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::traits::ModelRequest;

    /// Sends a real message — requires ANTHROPIC_API_KEY in the environment.
    /// Run with: cargo test --lib -- anthropic::tests::send_chat_turn_live --nocapture
    #[tokio::test]
    #[ignore = "requires ANTHROPIC_API_KEY"]
    async fn send_chat_turn_live() {
        let adapter = AnthropicAdapter::from_env("cloud_fast")
            .expect("ANTHROPIC_API_KEY must be set");

        let request = ModelRequest {
            model: "cloud_fast".to_string(),
            system_prompt: "You are a helpful assistant. Be concise.".to_string(),
            user_prompt: "Say hello and tell me which model you are in one sentence.".to_string(),
            temperature: Some(0.0),
            max_tokens: Some(64),
        };

        let response = adapter.generate(request).await.expect("API call failed");

        println!("model   : {}", response.model);
        println!("content : {}", response.content);
        println!(
            "tokens  : {} in / {} out",
            response.input_tokens, response.output_tokens
        );

        assert!(!response.content.is_empty(), "response content should not be empty");
        assert!(response.output_tokens > 0, "should report output tokens");
    }
}
