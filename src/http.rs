use crate::db::Db;
use anyhow::Result;
use askama::Template;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
    Form, Router,
};
use rand::Rng;
use serde::Deserialize;
use std::net::SocketAddr;
use tower_http::services::ServeDir;

#[derive(Clone)]
pub struct AppState {
    db: Db,
    domain: String,
}

pub async fn start_server(addr: SocketAddr, domain: String, db: Db) -> Result<()> {
    let state = AppState { db, domain };

    let app = Router::new()
        .route("/", get(index))
        .route("/create", post(create_mailbox))
        .route("/inbox/:local", get(view_inbox))
        .route("/inbox/:local/:id", get(view_message))
        .route("/api/check/:local", get(check_inbox))
        .nest_service("/static", ServeDir::new("static"))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("HTTP server listening on {}", addr);
    
    axum::serve(listener, app).await?;
    Ok(())
}

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate {
    domain: String,
}

async fn index(State(state): State<AppState>) -> impl IntoResponse {
    IndexTemplate {
        domain: state.domain,
    }
}

#[derive(Deserialize)]
struct CreateForm {
    custom: Option<String>,
}

async fn create_mailbox(
    State(state): State<AppState>,
    Form(form): Form<CreateForm>,
) -> Result<Redirect, Response> {
    let local = if let Some(custom) = form.custom {
        let custom = custom.trim().to_lowercase();
        if custom.is_empty() || !is_valid_local(&custom) {
            return Err((
                StatusCode::BAD_REQUEST,
                "Invalid email address. Use only letters, numbers, dots, and hyphens.",
            )
                .into_response());
        }
        custom
    } else {
        generate_random_local()
    };

    match state.db.create_mailbox(&local).await {
        Ok(_) => Ok(Redirect::to(&format!("/inbox/{}", local))),
        Err(_) => Err((
            StatusCode::CONFLICT,
            "Email address already exists. Try another one.",
        )
            .into_response()),
    }
}

fn generate_random_local() -> String {
    rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(12)
        .map(char::from)
        .collect::<String>()
        .to_lowercase()
}

fn is_valid_local(local: &str) -> bool {
    !local.is_empty()
        && local.len() <= 64
        && local
            .chars()
            .all(|c| c.is_alphanumeric() || c == '.' || c == '-' || c == '_')
        && !local.starts_with('.')
        && !local.ends_with('.')
}

#[derive(Template)]
#[template(path = "inbox.html")]
struct InboxTemplate {
    domain: String,
    local: String,
    email: String,
    messages: Vec<crate::db::Message>,
}

async fn view_inbox(
    Path(local): Path<String>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, Redirect> {
    let mailbox = match state.db.get_mailbox_by_local(&local).await {
        Ok(Some(mb)) => mb,
        _ => return Err(Redirect::to("/")),
    };

    let messages = state
        .db
        .get_messages_by_mailbox(mailbox.id)
        .await
        .unwrap_or_default();

    Ok(InboxTemplate {
        domain: state.domain.clone(),
        local: local.clone(),
        email: format!("{}@{}", local, state.domain),
        messages,
    })
}

#[derive(Template)]
#[template(path = "message.html")]
struct MessageTemplate {
    domain: String,
    local: String,
    message: crate::db::Message,
}

async fn view_message(
    Path((local, id)): Path<(String, String)>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, Redirect> {
    let message_id = uuid::Uuid::parse_str(&id)
        .map_err(|_| Redirect::to(&format!("/inbox/{}", local)))?;

    let message = match state.db.get_message_by_id(message_id).await {
        Ok(Some(msg)) => msg,
        _ => return Err(Redirect::to(&format!("/inbox/{}", local))),
    };

    Ok(MessageTemplate {
        domain: state.domain,
        local,
        message,
    })
}

#[derive(serde::Serialize)]
struct CheckResponse {
    count: usize,
    messages: Vec<MessageSummary>,
}

#[derive(serde::Serialize)]
struct MessageSummary {
    id: String,
    from: String,
    subject: String,
    received_at: String,
}

async fn check_inbox(
    Path(local): Path<String>,
    State(state): State<AppState>,
) -> Result<axum::Json<CheckResponse>, StatusCode> {
    let mailbox = match state.db.get_mailbox_by_local(&local).await {
        Ok(Some(mb)) => mb,
        _ => return Err(StatusCode::NOT_FOUND),
    };

    let messages = state
        .db
        .get_messages_by_mailbox(mailbox.id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let summaries: Vec<MessageSummary> = messages
        .iter()
        .map(|m| MessageSummary {
            id: m.id.to_string(),
            from: m.from_addr.clone(),
            subject: m.subject.clone(),
            received_at: m.received_at.to_rfc3339(),
        })
        .collect();

    Ok(axum::Json(CheckResponse {
        count: summaries.len(),
        messages: summaries,
    }))
}
