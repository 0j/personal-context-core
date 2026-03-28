use std::sync::Arc;

use sqlx::SqlitePool;

use crate::{
    llm::traits::ModelRequest,
    models::{
        chat::SendChatTurnResponse,
        conversation::Conversation,
        message::{Message, MessageRole},
        prompt::PromptContext,
        routing::RouteDecision,
    },
    state::AppState,
};

use super::{
    memory_service::MemoryService,
    prompt_builder::PromptBuilder,
    routing_service::RoutingService,
};

pub struct ChatService {
    state: Arc<AppState>,
    router: RoutingService,
    memory_svc: MemoryService,
    prompt_builder: PromptBuilder,
}

impl ChatService {
    pub fn new(state: Arc<AppState>) -> Self {
        let memory_svc = MemoryService::new(state.db.clone());
        Self {
            state,
            router: RoutingService::new(),
            memory_svc,
            prompt_builder: PromptBuilder::new(),
        }
    }

    // ── Conversation ──────────────────────────────────────────────────────

    pub async fn create_conversation(&self, title: Option<String>) -> anyhow::Result<Conversation> {
        let conv = Conversation::new(title);
        let now = conv.created_at.to_rfc3339();
        sqlx::query(
            "INSERT INTO conversations (id, title, created_at, updated_at) VALUES (?, ?, ?, ?)",
        )
        .bind(&conv.id)
        .bind(&conv.title)
        .bind(&now)
        .bind(&now)
        .execute(&self.state.db)
        .await?;
        Ok(conv)
    }

    pub async fn list_conversations(&self) -> anyhow::Result<Vec<Conversation>> {
        let rows = sqlx::query_as::<_, ConversationRow>(
            "SELECT id, title, created_at, updated_at FROM conversations ORDER BY updated_at DESC",
        )
        .fetch_all(&self.state.db)
        .await?;

        rows.into_iter().map(ConversationRow::into_conversation).collect()
    }

    // ── Messages ──────────────────────────────────────────────────────────

    pub async fn create_message(&self, msg: &Message) -> anyhow::Result<()> {
        sqlx::query(
            "INSERT INTO messages (id, conversation_id, role, content, model_used, created_at) \
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&msg.id)
        .bind(&msg.conversation_id)
        .bind(msg.role.as_str())
        .bind(&msg.content)
        .bind(&msg.model_used)
        .bind(msg.created_at.to_rfc3339())
        .execute(&self.state.db)
        .await?;

        sqlx::query("UPDATE conversations SET updated_at = ? WHERE id = ?")
            .bind(msg.created_at.to_rfc3339())
            .bind(&msg.conversation_id)
            .execute(&self.state.db)
            .await?;

        Ok(())
    }

    /// Associated function so the extraction worker can call it without a
    /// full ChatService instance.
    pub async fn recent_messages(
        db: &SqlitePool,
        conversation_id: &str,
        limit: u32,
    ) -> anyhow::Result<Vec<Message>> {
        let rows = sqlx::query_as::<_, MessageRow>(
            "SELECT id, conversation_id, role, content, model_used, created_at \
             FROM messages \
             WHERE conversation_id = ? \
             ORDER BY created_at ASC \
             LIMIT ?",
        )
        .bind(conversation_id)
        .bind(limit)
        .fetch_all(db)
        .await?;

        rows.into_iter().map(MessageRow::into_message).collect()
    }

    // ── Core turn ─────────────────────────────────────────────────────────

    pub async fn send_turn(
        &self,
        conversation_id: &str,
        user_content: &str,
    ) -> anyhow::Result<SendChatTurnResponse> {
        // 1. Persist user message.
        let user_msg = Message::new(
            conversation_id.to_string(),
            MessageRole::User,
            user_content.to_string(),
        );
        self.create_message(&user_msg).await?;

        // 2. Route.
        let mut decision = self.router.route(user_content);
        decision.conversation_id = Some(conversation_id.to_string());
        decision.message_id = Some(user_msg.id.clone());
        let model_name = decision.model_chosen.as_str().to_string();

        // 3. Persist route decision.
        self.save_route_decision(&decision).await?;

        // 4. Retrieve relevant memories.
        let keywords: Vec<&str> = user_content
            .split_whitespace()
            .filter(|w| w.len() > 4)
            .take(8)
            .collect();
        let memories = self.memory_svc.retrieve_relevant(&keywords, 5).await?;

        // 5. Build prompt context.
        let mut ctx = PromptContext::new(conversation_id.to_string());
        ctx.memories = memories;
        let system_prompt = self.prompt_builder.build_system_prompt(&ctx);

        // 6. Call model adapter.
        let adapter = self.state.registry.require(&model_name)?;
        let request = ModelRequest {
            model: model_name.clone(),
            system_prompt,
            user_prompt: user_content.to_string(),
            temperature: None,
            max_tokens: None,
        };
        let response = adapter.generate(request).await?;

        // 7. Persist assistant message.
        let mut assistant_msg = Message::new(
            conversation_id.to_string(),
            MessageRole::Assistant,
            response.content.clone(),
        );
        assistant_msg.model_used = Some(model_name.clone());
        self.create_message(&assistant_msg).await?;

        // 8. Log audit event.
        self.log_event(
            "chat_turn",
            &serde_json::json!({
                "conversation_id": conversation_id,
                "user_message_id": user_msg.id,
                "assistant_message_id": assistant_msg.id,
                "model": model_name,
                "route_reason": decision.reason.as_str(),
            }),
        )
        .await?;

        // 9. Spawn background extraction job.
        self.state.job_queue.enqueue_extraction(
            &assistant_msg.id,
            conversation_id,
            Arc::clone(&self.state),
        );

        Ok(SendChatTurnResponse {
            assistant_message: assistant_msg,
            route_decision: decision,
            model_used: model_name,
        })
    }

    // ── Context snapshot ──────────────────────────────────────────────────

    pub async fn get_context_for_conversation(
        &self,
        conversation_id: &str,
    ) -> anyhow::Result<PromptContext> {
        let messages =
            Self::recent_messages(&self.state.db, conversation_id, 20).await?;
        let combined: String = messages
            .iter()
            .map(|m| m.content.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        let keywords: Vec<&str> = combined
            .split_whitespace()
            .filter(|w| w.len() > 4)
            .take(10)
            .collect();
        let memories = self.memory_svc.retrieve_relevant(&keywords, 10).await?;

        let mut ctx = PromptContext::new(conversation_id.to_string());
        ctx.memories = memories;
        Ok(ctx)
    }

    // ── Private helpers ───────────────────────────────────────────────────

    async fn save_route_decision(&self, d: &RouteDecision) -> anyhow::Result<()> {
        sqlx::query(
            "INSERT INTO route_decisions \
             (id, conversation_id, message_id, model_chosen, reason, privacy_level, score, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&d.id)
        .bind(&d.conversation_id)
        .bind(&d.message_id)
        .bind(d.model_chosen.as_str())
        .bind(d.reason.as_str())
        .bind(d.privacy_level.as_str())
        .bind(d.score)
        .bind(d.created_at.to_rfc3339())
        .execute(&self.state.db)
        .await?;
        Ok(())
    }

    async fn log_event(
        &self,
        kind: &str,
        payload: &serde_json::Value,
    ) -> anyhow::Result<()> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO events (id, kind, payload, created_at) VALUES (?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(kind)
        .bind(payload.to_string())
        .bind(&now)
        .execute(&self.state.db)
        .await?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Row types
// ---------------------------------------------------------------------------

#[derive(sqlx::FromRow)]
struct ConversationRow {
    id: String,
    title: Option<String>,
    created_at: String,
    updated_at: String,
}

impl ConversationRow {
    fn into_conversation(self) -> anyhow::Result<Conversation> {
        Ok(Conversation {
            id: self.id,
            title: self.title,
            created_at: self.created_at.parse()?,
            updated_at: self.updated_at.parse()?,
        })
    }
}

#[derive(sqlx::FromRow)]
struct MessageRow {
    id: String,
    conversation_id: String,
    role: String,
    content: String,
    model_used: Option<String>,
    created_at: String,
}

impl MessageRow {
    fn into_message(self) -> anyhow::Result<Message> {
        Ok(Message {
            id: self.id,
            conversation_id: self.conversation_id,
            role: MessageRole::try_from(self.role.as_str())?,
            content: self.content,
            model_used: self.model_used,
            created_at: self.created_at.parse()?,
        })
    }
}
