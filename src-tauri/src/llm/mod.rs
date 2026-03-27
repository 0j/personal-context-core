pub mod traits;

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;

use traits::{LlmAdapter, ModelRequest, ModelResponse};

// ---------------------------------------------------------------------------
// DummyLocalAdapter
// ---------------------------------------------------------------------------

pub struct DummyLocalAdapter {
    pub model_name: String,
}

#[async_trait]
impl LlmAdapter for DummyLocalAdapter {
    async fn generate(&self, request: ModelRequest) -> anyhow::Result<ModelResponse> {
        let content = format!(
            "[LOCAL:{model}] stub response\n\
             --- system ---\n{system}\n\
             --- user ---\n{user}",
            model = self.model_name,
            system = request.system_prompt,
            user = request.user_prompt,
        );

        Ok(ModelResponse {
            model: self.model_name.clone(),
            content,
            input_tokens: (request.system_prompt.len() + request.user_prompt.len()) as u32 / 4,
            output_tokens: 32,
        })
    }
}

// ---------------------------------------------------------------------------
// DummyCloudAdapter
// ---------------------------------------------------------------------------

pub struct DummyCloudAdapter {
    pub model_name: String,
}

#[async_trait]
impl LlmAdapter for DummyCloudAdapter {
    async fn generate(&self, request: ModelRequest) -> anyhow::Result<ModelResponse> {
        let content = format!(
            "[CLOUD:{model}] stub response\n\
             --- system ---\n{system}\n\
             --- user ---\n{user}",
            model = self.model_name,
            system = request.system_prompt,
            user = request.user_prompt,
        );

        Ok(ModelResponse {
            model: self.model_name.clone(),
            content,
            input_tokens: (request.system_prompt.len() + request.user_prompt.len()) as u32 / 4,
            output_tokens: 32,
        })
    }
}

// ---------------------------------------------------------------------------
// ModelRegistry
// ---------------------------------------------------------------------------

pub struct ModelRegistry {
    adapters: HashMap<String, Arc<dyn LlmAdapter>>,
}

impl ModelRegistry {
    pub fn new() -> Self {
        Self {
            adapters: HashMap::new(),
        }
    }

    pub fn register(&mut self, name: impl Into<String>, adapter: Arc<dyn LlmAdapter>) {
        self.adapters.insert(name.into(), adapter);
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn LlmAdapter>> {
        self.adapters.get(name).cloned()
    }

    /// Returns the adapter or an error if the name is not registered.
    pub fn require(&self, name: &str) -> anyhow::Result<Arc<dyn LlmAdapter>> {
        self.get(name)
            .ok_or_else(|| anyhow::anyhow!("model '{}' not found in registry", name))
    }
}

impl Default for ModelRegistry {
    fn default() -> Self {
        Self::new()
    }
}
