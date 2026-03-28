pub mod commands;
pub mod db;
pub mod jobs;
pub mod llm;
pub mod models;
pub mod services;
pub mod state;

use std::sync::Arc;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use tauri::Manager;

use commands::{
    chat::{
        append_message, create_conversation, get_context_for_conversation, list_conversations,
        send_chat_turn,
    },
    memory::{list_memory_candidates, review_memory_candidate},
};
use llm::{anthropic::AnthropicAdapter, DummyLocalAdapter, ModelRegistry};
use state::{AppSettings, AppState, JobQueue};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // ── Resolve data directory (Tauri v2 API) ────────────────────
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let db_path = data_dir.join("pcc.db");

            // ── Async bootstrap ───────────────────────────────────────────
            let app_state = tauri::async_runtime::block_on(async {
                let opts = SqliteConnectOptions::new()
                    .filename(&db_path)
                    .create_if_missing(true)
                    .pragma("foreign_keys", "ON")
                    .pragma("journal_mode", "WAL");

                let pool = SqlitePoolOptions::new()
                    .max_connections(5)
                    .connect_with(opts)
                    .await
                    .expect("failed to open SQLite database");

                // Run embedded migrations
                sqlx::migrate!("src/db/migrations")
                    .run(&pool)
                    .await
                    .expect("database migration failed");

                println!("[PCC] Database ready at {}", db_path.display());

                // ── Model registry ────────────────────────────────────────
                let mut registry = ModelRegistry::new();

                registry.register(
                    "local_general",
                    Arc::new(DummyLocalAdapter {
                        model_name: "local_general".to_string(),
                    }),
                );
                registry.register(
                    "cloud_fast",
                    Arc::new(
                        AnthropicAdapter::from_env("cloud_fast")
                            .expect("failed to initialise AnthropicAdapter for cloud_fast"),
                    ),
                );
                registry.register(
                    "cloud_reasoning",
                    Arc::new(
                        AnthropicAdapter::from_env("cloud_reasoning")
                            .expect("failed to initialise AnthropicAdapter for cloud_reasoning"),
                    ),
                );

                println!(
                    "[PCC] Model registry ready: local_general, cloud_fast, cloud_reasoning"
                );

                Arc::new(AppState::new(
                    pool,
                    Arc::new(registry),
                    Arc::new(JobQueue::new()),
                    AppSettings::default(),
                ))
            });

            // manage() must be called on app, not app_handle
            app.manage(app_state);
            println!("[PCC] App state initialised — ready.");

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            create_conversation,
            list_conversations,
            append_message,
            send_chat_turn,
            get_context_for_conversation,
            list_memory_candidates,
            review_memory_candidate,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
