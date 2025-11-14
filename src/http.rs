use axum::{
    extract::{Form, Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect},
    routing::{get, post},
    // Note: axum::Server is removed, we'll use axum::serve
    serve, // <-- New import
    Router,
};
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use serde::Deserialize;
use std::{net::SocketAddr, sync::Arc};
use tera::{Context, Tera};
use tokio::net::TcpListener; // <-- New import for server binding
use tower_http::services::ServeDir;
use tracing::error;
use uuid::Uuid; // <-- Added Uuid import for view_message Path

use crate::db::{Db, Message};

#[derive(Clone)]
pub struct AppState {
    pub db: Db,
    pub domain: String,
    pub templates: Arc<Tera>,
}

/// Start the HTTP server (called from main.rs)
pub async fn start_server(listen: SocketAddr, domain: String, db: Db) -> anyhow::Result<()> {
    // initialize tera: templates directory (templates/*)
    let mut tera = Tera::new("templates/**/*")?;
    // disable autoescape for now (we render raw text inside <pre>)
    tera.autoescape_on(vec![]);

    let state = AppState {
        db,
        domain,
        templates: Arc::new(tera),
    };

    let app = Router::new()
        .route("/", get(index))
        .route("/create", post(create_mailbox))
        .route("/inbox/:local", get(view_inbox))
        .route("/inbox/:local/:id", get(view_message))
        // serve static files from ./static on /static/*
        .nest_service("/static", ServeDir::new("static"))
        .with_state(state);

    tracing::info!("HTTP server listening on http://{}", listen);

    // FIX E0433: Use tokio::net::TcpListener and axum::serve
    let listener = TcpListener::bind(listen).await?;
    serve(listener, app.into_make_service()).await?;

    Ok(())
}

/* ---------- Handlers ---------- */

async fn index(State(state): State<AppState>) -> impl IntoResponse {
    let mut ctx = Context::new();
    ctx.insert("domain", &state.domain);
    let rendered = state
        .templates
        .render("index.html", &ctx)
        .unwrap_or_else(|e| {
            error!("template render error: {}", e);
            "<h1>Template render error</h1>".to_string()
        });
    Html(rendered)
}

#[derive(Deserialize)]
pub struct CreateForm {
    pub ttl_hours: Option<i64>,
}

async fn create_mailbox(
    State(state): State<AppState>,
    Form(form): Form<CreateForm>,
) -> impl IntoResponse {
    // generate a 10-char local part
    let local: String = thread_rng()
        .sample_iter(&Alphanumeric)
        .take(10)
        .map(char::from)
        .collect();

    let ttl_seconds = form.ttl_hours.map(|h| h * 3600);

    // FIX E0061: create_mailbox now accepts ttl_seconds
    if let Err(e) = state.db.create_mailbox(&local, ttl_seconds).await {
        error!("create_mailbox db error: {:?}", e);
        // redirect to home on error
        return (StatusCode::SEE_OTHER, Redirect::to("/")).into_response();
    }

    // FIX E0308: Must call .into_response() on the bare Redirect to match return type
    Redirect::to(&format!("/inbox/{}", local)).into_response()
}

async fn view_inbox(
    Path(local): Path<String>,
    State(state): State<AppState>,
) -> Result<Html<String>, Redirect> {
    // check mailbox exists (uses Db::mailbox_exists)
    match state.db.mailbox_exists(&local).await {
        Ok(true) => {}
        _ => return Err(Redirect::to("/")),
    }

    // List messages (uses Db::list_messages)
    let messages = match state.db.list_messages(&local).await {
        Ok(v) => v,
        Err(e) => {
            error!("db list_messages error: {:?}", e);
            vec![]
        }
    };

    // prepare context
    let mut ctx = Context::new();
    ctx.insert("domain", &state.domain);
    ctx.insert("local", &local);

    // convert messages into simple serializable objects for Tera
    let msgs_for_template: Vec<_> = messages
        .into_iter()
        .map(|m: Message| {
            let id = m.id.to_string();

            // FIX E0599 (unwrap_or_else for String) - Message::from_addr must be Option<String> in db.rs
            let from = m.from_addr.unwrap_or_else(|| "<unknown>".into());

            let received = {
                // FIX: Use .timestamp() which returns i64, not .timestamp_opt() which is not a method on DateTime<Utc>
                let ts = m.received_at.timestamp();
                chrono::NaiveDateTime::from_timestamp_opt(ts, 0)
                    .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                    .unwrap_or_else(|| "unknown".into())
            };

            serde_json::json!({
                "id": id,
                "from": from,
                "received": received
            })
        })
        .collect();

    ctx.insert("messages", &msgs_for_template);

    let rendered = state.templates.render("inbox.html", &ctx).map_err(|e| {
        error!("render inbox template: {:?}", e);
        Redirect::to("/")
    })?;

    Ok(Html(rendered))
}

async fn view_message(
    Path((local, id)): Path<(String, String)>,
    State(state): State<AppState>,
) -> Result<Html<String>, Redirect> {
    // parse uuid
    let uuid = match Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => return Err(Redirect::to(&format!("/inbox/{}", local))),
    };

    // Get message (uses Db::get_message)
    let opt = match state.db.get_message(&local, uuid).await {
        Ok(o) => o,
        Err(e) => {
            error!("db get_message error: {:?}", e);
            return Err(Redirect::to(&format!("/inbox/{}", local)));
        }
    };

    let message = match opt {
        Some(m) => m,
        None => return Err(Redirect::to(&format!("/inbox/{}", local))),
    };

    let mut ctx = Context::new();
    ctx.insert("domain", &state.domain);
    ctx.insert("local", &local);
    ctx.insert(
        "from",
        &message.from_addr.unwrap_or_else(|| "<unknown>".into()),
    );
    ctx.insert("raw", &message.raw);

    // FIX: Use .timestamp() which returns i64, not .timestamp_opt() which is not a method on DateTime<Utc>
    let ts = message.received_at.timestamp();
    let received = chrono::NaiveDateTime::from_timestamp_opt(ts, 0)
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|| "unknown".into());

    ctx.insert("received", &received);

    let rendered = state.templates.render("message.html", &ctx).map_err(|e| {
        error!("render message template: {:?}", e);
        Redirect::to(&format!("/inbox/{}", local))
    })?;

    Ok(Html(rendered))
}
