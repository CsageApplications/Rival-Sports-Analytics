//! HTTP API server powering the Parlay Assistant chatbot.
//!
//! Exposes a small JSON API that the React dashboard talks to. Keeps the
//! Anthropic API key server-side only — the frontend never sees it.

mod config;
mod context;
mod prompt;

use axum::{
    extract::State,
    http::{Method, StatusCode},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tracing::{error, info};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

use config::Config;
use mlb_db::Database;
use mlb_llm::ClaudeClient;

struct AppState {
    db: Database,
    claude: ClaudeClient,
}

#[derive(Debug, Deserialize)]
struct ChatRequest {
    message: String,
    /// Optional risk preference hint from the UI's quick-select buttons.
    #[serde(default)]
    risk: Option<String>,
}

#[derive(Debug, Serialize)]
struct ChatResponse {
    reply: String,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env().add_directive("mlb_server=info".parse()?))
        .init();

    let config = Config::from_env()?;

    let db = Database::new(&config.db_path).await?;
    db.migrate().await?;

    let claude = ClaudeClient::new(config.anthropic_api_key.clone(), config.anthropic_model.clone());

    let state = Arc::new(AppState { db, claude });

    let cors = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST])
        .allow_headers(Any)
        .allow_origin(Any);

    let app = Router::new()
        .route("/api/health", get(health))
        .route("/api/chat", post(chat))
        .with_state(state)
        .layer(cors);

    let addr = format!("0.0.0.0:{}", config.port);
    info!("mlb-server listening on http://{addr}");

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn health() -> &'static str {
    "ok"
}

async fn chat(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ChatRequest>,
) -> Result<Json<ChatResponse>, (StatusCode, Json<ErrorResponse>)> {
    if req.message.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse { error: "message must not be empty".into() }),
        ));
    }

    let slate = context::build_slate_context(&state.db).await.map_err(|e| {
        error!("failed to build slate context: {e}");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: "failed to load current odds data".into() }),
        )
    })?;

    if slate.games_count == 0 {
        return Err((
            StatusCode::PRECONDITION_FAILED,
            Json(ErrorResponse {
                error: "No odds data loaded yet. Run `mlb fetch odds --with-props` first.".into(),
            }),
        ));
    }

    let system_prompt = prompt::build_system_prompt(&slate.text);

    let user_message = match &req.risk {
        Some(risk) if !risk.trim().is_empty() => {
            format!("[User-selected risk preference: {risk}]\n\n{}", req.message)
        }
        _ => req.message,
    };

    let reply = state
        .claude
        .ask(&system_prompt, &user_message)
        .await
        .map_err(|e| {
            error!("claude request failed: {e}");
            (
                StatusCode::BAD_GATEWAY,
                Json(ErrorResponse { error: format!("assistant request failed: {e}") }),
            )
        })?;

    Ok(Json(ChatResponse { reply }))
}
