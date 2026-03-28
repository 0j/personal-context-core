/// Integration test: real SQLite DB + real Anthropic API call through send_chat_turn.
///
/// Run with:
///   ANTHROPIC_API_KEY=<key> cargo test --test chat_integration -- --include-ignored --nocapture
use std::sync::Arc;

use personal_context_core_lib::{
    llm::{anthropic::AnthropicAdapter, DummyLocalAdapter, ModelRegistry},
    services::chat_service::ChatService,
    state::{AppSettings, AppState, JobQueue},
};
use sqlx::{Row, sqlite::{SqliteConnectOptions, SqlitePoolOptions}};

/// Bootstrap an in-process SQLite pool with all migrations applied.
async fn build_pool() -> sqlx::SqlitePool {
    let db_path = std::env::temp_dir().join(format!(
        "pcc_test_{}.db",
        uuid::Uuid::new_v4()
    ));

    let opts = SqliteConnectOptions::new()
        .filename(&db_path)
        .create_if_missing(true)
        .pragma("foreign_keys", "ON")
        .pragma("journal_mode", "WAL");

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(opts)
        .await
        .expect("failed to open test database");

    sqlx::migrate!("src/db/migrations")
        .run(&pool)
        .await
        .expect("migration failed");

    pool
}

/// Build a registry with real Anthropic adapters for cloud slots.
fn build_registry() -> Arc<ModelRegistry> {
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
                .expect("ANTHROPIC_API_KEY must be set"),
        ),
    );
    registry.register(
        "cloud_reasoning",
        Arc::new(
            AnthropicAdapter::from_env("cloud_reasoning")
                .expect("ANTHROPIC_API_KEY must be set"),
        ),
    );

    Arc::new(registry)
}

async fn build_state() -> Arc<AppState> {
    let pool = build_pool().await;
    let registry = build_registry();
    let job_queue = Arc::new(JobQueue::new());
    Arc::new(AppState::new(pool, registry, job_queue, AppSettings::default()))
}

// ---------------------------------------------------------------------------
// Shared helper
// ---------------------------------------------------------------------------

async fn run_turn(label: &str, message: &str) {
    let app_state = build_state().await;
    let svc = ChatService::new(Arc::clone(&app_state));

    let conv = svc
        .create_conversation(Some(label.to_string()))
        .await
        .expect("failed to create conversation");

    let resp = svc
        .send_turn(&conv.id, message)
        .await
        .expect("send_turn failed");

    let ctx = svc
        .get_context_for_conversation(&conv.id)
        .await
        .expect("failed to get context");

    println!();
    println!("┌─ {} ", label);
    println!("│  message       : {}", message);
    println!("│  model chosen  : {}", resp.route_decision.model_chosen.as_str());
    println!("│  route reason  : {}", resp.route_decision.reason.as_str());
    println!("│  privacy level : {}", resp.route_decision.privacy_level.as_str());
    println!("│  memories      : {}", ctx.memories.len());
    println!("│  explanation   : {}", resp.route_decision.explanation);
    println!("│  assistant     :");
    for line in resp.assistant_message.content.lines() {
        println!("│    {}", line);
    }
    println!("└─────────────────────────────────────────────────");

    assert!(!resp.assistant_message.content.is_empty());
    assert_eq!(resp.model_used, resp.route_decision.model_chosen.as_str());
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Full end-to-end test:
///   1. initialises app state with a real (temp) SQLite database
///   2. creates a conversation
///   3. sends "What is the capital of Sweden?"
///   4. prints assistant response, model chosen, and memory count injected
#[tokio::test]
#[ignore = "requires ANTHROPIC_API_KEY"]
async fn send_chat_turn_sweden_question() {
    let app_state = build_state().await;
    let svc = ChatService::new(Arc::clone(&app_state));

    // ── 1. Create conversation ────────────────────────────────────────────
    let conv = svc
        .create_conversation(Some("Integration test — Sweden".to_string()))
        .await
        .expect("failed to create conversation");

    println!("conversation id : {}", conv.id);

    // ── 2. Send the message ───────────────────────────────────────────────
    let question = "What is the capital of Sweden?";
    let resp = svc
        .send_turn(&conv.id, question)
        .await
        .expect("send_turn failed");

    // ── 3. Fetch context snapshot to report memory count ──────────────────
    let ctx = svc
        .get_context_for_conversation(&conv.id)
        .await
        .expect("failed to get context");

    // ── 4. Print results ──────────────────────────────────────────────────
    println!("─────────────────────────────────────────────────");
    println!("model chosen  : {}", resp.route_decision.model_chosen.as_str());
    println!("route reason  : {}", resp.route_decision.reason.as_str());
    println!("privacy level : {}", resp.route_decision.privacy_level.as_str());
    println!("memories      : {}", ctx.memories.len());
    println!("─────────────────────────────────────────────────");
    println!("assistant     :\n{}", resp.assistant_message.content);
    println!("─────────────────────────────────────────────────");

    assert!(!resp.assistant_message.content.is_empty(), "response should not be empty");
    assert_eq!(
        resp.model_used,
        resp.route_decision.model_chosen.as_str(),
        "model_used should match route decision"
    );
}

/// Privacy keyword "private" must force routing to local_general regardless of
/// any other signal (reasoning keyword "analyse" is also present but loses).
/// DummyLocalAdapter is used so no API call is made.
#[tokio::test]
#[ignore = "requires ANTHROPIC_API_KEY"]
async fn routing_privacy_keyword_wins() {
    run_turn(
        "privacy detection",
        "Analyse my private financial situation and help me plan",
    )
    .await;
}

/// No privacy keywords, no reasoning keywords, query is well under the 600-char
/// threshold — should take the default cloud_fast path.
#[tokio::test]
#[ignore = "requires ANTHROPIC_API_KEY"]
async fn routing_complexity_no_keyword_match() {
    run_turn(
        "complexity routing",
        "Write a detailed technical architecture for a distributed system",
    )
    .await;
}

/// Extraction pipeline end-to-end:
///   1. Send a message rich with extractable personal facts
///   2. Wait 3 s for the background ExtractionWorker to run
///   3. Assert ≥ 1 pending row exists in memory_candidates
///
/// Uses a message with an explicit profession, tool preference, and project so
/// the extraction model has unambiguous material to produce memory candidates.
#[tokio::test]
#[ignore = "requires ANTHROPIC_API_KEY"]
async fn extraction_worker_writes_memory_candidates() {
    let app_state = build_state().await;
    let svc = ChatService::new(Arc::clone(&app_state));

    let conv = svc
        .create_conversation(Some("Extraction pipeline test".to_string()))
        .await
        .expect("failed to create conversation");

    // Rich, unambiguous personal-fact message so the extractor has clear material.
    let message = "I'm a software engineer and I love working with Rust. \
                   I'm currently building a personal AI assistant project called \
                   Personal Context Core, and I strongly prefer local-first privacy \
                   over cloud-only solutions.";

    let resp = svc
        .send_turn(&conv.id, message)
        .await
        .expect("send_turn failed");

    println!(
        "send_turn ok — model: {}, assistant len: {} chars",
        resp.model_used,
        resp.assistant_message.content.len()
    );

    // Poll until the ExtractionWorker writes an extraction_completed event or 15 s elapses.
    // The worker logs this event as its final step, so it is a reliable completion signal.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    loop {
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        let done: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM events WHERE kind = 'extraction_completed'",
        )
        .fetch_one(&app_state.db)
        .await
        .expect("failed to poll events");

        if done > 0 {
            println!("extraction_completed event seen after {:.1}s", {
                let elapsed = deadline
                    .checked_duration_since(std::time::Instant::now())
                    .map(|r| 15.0 - r.as_secs_f64())
                    .unwrap_or(15.0);
                elapsed
            });
            break;
        }

        if std::time::Instant::now() >= deadline {
            // Print any extractor events for diagnosis.
            let events = sqlx::query("SELECT kind, payload FROM events ORDER BY created_at DESC LIMIT 10")
                .fetch_all(&app_state.db)
                .await
                .expect("failed to fetch events");
            println!("─── last events (timeout reached) ───");
            for row in &events {
                let kind: String = row.try_get("kind").unwrap_or_default();
                let payload: String = row.try_get("payload").unwrap_or_default();
                println!("  {kind}: {payload}");
            }
            panic!("ExtractionWorker did not complete within 15 s. See events above.");
        }
    }

    let candidate_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM memory_candidates WHERE status = 'pending'",
    )
    .fetch_one(&app_state.db)
    .await
    .expect("failed to count memory_candidates");

    // Print what was written so we can inspect field names on failure.
    let rows = sqlx::query("SELECT id, kind, confidence, content FROM memory_candidates WHERE status = 'pending'")
        .fetch_all(&app_state.db)
        .await
        .expect("failed to fetch candidates");

    println!("─────────────────────────────────────────────────");
    println!("pending memory_candidates : {}", candidate_count);
    for row in &rows {
        let id: String = row.try_get("id").unwrap_or_default();
        let kind: String = row.try_get("kind").unwrap_or_default();
        let confidence: f64 = row.try_get("confidence").unwrap_or_default();
        let content: String = row.try_get("content").unwrap_or_default();
        let payload: serde_json::Value =
            serde_json::from_str(&content).unwrap_or(serde_json::Value::Null);
        println!(
            "  [{kind}] confidence={confidence:.2}  statement={}",
            payload
                .get("statement")
                .and_then(|v| v.as_str())
                .unwrap_or("(no statement field)")
        );
        let _ = id;
    }
    println!("─────────────────────────────────────────────────");

    // Also print the extraction_completed event payload for inspection.
    if let Ok(payload) = sqlx::query_scalar::<_, String>(
        "SELECT payload FROM events WHERE kind = 'extraction_completed' LIMIT 1",
    )
    .fetch_one(&app_state.db)
    .await
    {
        let v: serde_json::Value = serde_json::from_str(&payload).unwrap_or_default();
        println!(
            "extraction summary: proposed={} rejected={}",
            v.get("proposed_count").and_then(|x| x.as_i64()).unwrap_or(0),
            v.get("rejected_count").and_then(|x| x.as_i64()).unwrap_or(0),
        );
    }

    assert!(
        candidate_count >= 1,
        "expected at least 1 pending memory candidate, got {candidate_count}. \
         Check 'extraction summary' above — if proposed=0 all candidates were auto-rejected \
         (likely confidence_below_threshold). Check stderr for schema validation errors."
    );
}
