use std::str::FromStr;
use std::sync::Arc;

use axum::Router;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::post;
use doneyet_core::model::RepoRef;
use doneyet_core::ports::{ProviderError, PushSource, RefreshHint};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

type HmacSha256 = Hmac<Sha256>;
type SharedState = (Arc<Vec<u8>>, mpsc::UnboundedSender<RefreshHint>);

pub struct WebhookPush {
    local_addr: String,
    rx: mpsc::UnboundedReceiver<RefreshHint>,
    shutdown: CancellationToken,
}

impl WebhookPush {
    pub async fn start(addr: &str, secret: String) -> Result<Self, ProviderError> {
        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .map_err(|e| ProviderError::Transport(format!("bind {addr}: {e}")))?;
        let local_addr = listener
            .local_addr()
            .map_err(|e| ProviderError::Transport(e.to_string()))?
            .to_string();
        let (tx, rx) = mpsc::unbounded_channel();
        let shutdown = CancellationToken::new();
        let token = shutdown.clone();
        let app = Router::new()
            .route("/", post(hook))
            .route("/webhook", post(hook))
            .with_state((Arc::new(secret.into_bytes()), tx));
        tokio::spawn(async move {
            let server = axum::serve(listener, app).with_graceful_shutdown(token.cancelled_owned());
            if let Err(e) = server.await {
                tracing::warn!("webhook server stopped: {e}");
            }
        });
        Ok(Self {
            local_addr,
            rx,
            shutdown,
        })
    }

    pub fn local_addr(&self) -> &str {
        &self.local_addr
    }
}

impl Drop for WebhookPush {
    fn drop(&mut self) {
        self.shutdown.cancel();
    }
}

#[async_trait::async_trait]
impl PushSource for WebhookPush {
    async fn wait(&mut self) -> Result<RefreshHint, ProviderError> {
        self.rx
            .recv()
            .await
            .ok_or_else(|| ProviderError::Other("webhook server stopped".to_string()))
    }
}

async fn hook(
    State((secret, sender)): State<SharedState>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    if !signature_valid(&secret, &headers, &body) {
        return (StatusCode::UNAUTHORIZED, "invalid signature");
    }
    let event = headers
        .get("x-github-event")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    if let Some(hint) = parse_hint(event, &body) {
        tracing::debug!(event, run_id = ?hint.run_id, "webhook push received");
        let _ = sender.send(hint);
    }
    (StatusCode::OK, "")
}

fn signature_valid(secret: &[u8], headers: &HeaderMap, body: &[u8]) -> bool {
    let Some(received) = headers
        .get("x-hub-signature-256")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("sha256="))
        .and_then(|hex_value| hex::decode(hex_value).ok())
    else {
        return false;
    };
    let Ok(mut mac) = <HmacSha256 as Mac>::new_from_slice(secret) else {
        return false;
    };
    mac.update(body);
    mac.verify_slice(&received).is_ok()
}

fn parse_hint(event: &str, body: &[u8]) -> Option<RefreshHint> {
    let value: serde_json::Value = serde_json::from_slice(body).ok()?;
    let repo_raw = value.get("repository")?.get("full_name")?.as_str()?;
    let repo = RepoRef::from_str(repo_raw).ok()?;
    let run_id = match event {
        "workflow_run" => value.get("workflow_run")?.get("id")?.as_u64(),
        "workflow_job" => value.get("workflow_job")?.get("run_id")?.as_u64(),
        _ => return None,
    };
    Some(RefreshHint { repo, run_id })
}
