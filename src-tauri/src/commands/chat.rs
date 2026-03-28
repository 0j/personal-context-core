use std::sync::Arc;

use tauri::State;

use crate::{
    models::{
        chat::SendChatTurnResponse,
        conversation::Conversation,
        message::{Message, MessageRole},
        prompt::PromptContext,
    },
    services::chat_service::ChatService,
    state::AppState,
};

fn make_chat_service(state: &Arc<AppState>) -> ChatService {
    ChatService::new(Arc::clone(state))
}

#[tauri::command]
pub async fn create_conversation(
    title: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<Conversation, String> {
    make_chat_service(&state)
        .create_conversation(title)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_conversations(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<Conversation>, String> {
    make_chat_service(&state)
        .list_conversations()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn append_message(
    conversation_id: String,
    role: String,
    content: String,
    state: State<'_, Arc<AppState>>,
) -> Result<Message, String> {
    let role = MessageRole::try_from(role.as_str()).map_err(|e| e.to_string())?;
    let msg = Message::new(conversation_id, role, content);
    make_chat_service(&state)
        .create_message(&msg)
        .await
        .map_err(|e| e.to_string())?;
    Ok(msg)
}

#[tauri::command]
pub async fn send_chat_turn(
    conversation_id: String,
    user_content: String,
    state: State<'_, Arc<AppState>>,
) -> Result<SendChatTurnResponse, String> {
    make_chat_service(&state)
        .send_turn(&conversation_id, &user_content)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_context_for_conversation(
    conversation_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<PromptContext, String> {
    make_chat_service(&state)
        .get_context_for_conversation(&conversation_id)
        .await
        .map_err(|e| e.to_string())
}
