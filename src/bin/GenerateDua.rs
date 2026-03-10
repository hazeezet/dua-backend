use dua_backend::{
    gemini,
    ssm::{SsmConfig, SsmParameter},
};
use http::{HeaderMap, HeaderValue};
use lambda_runtime::{
    service_fn,
    streaming::{channel, Body, Response},
    Error, LambdaEvent, MetadataPrelude,
};
use serde::Deserialize;
use serde_json::Value;
use std::env;

// ─── Request type ────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct GenerateDuaRequest {
    prompt: String,
}

// ─── Helper: build SSE headers ───────────────────────────────────────────────

fn sse_metadata(status: u16) -> MetadataPrelude {
    let mut headers = HeaderMap::new();
    headers.insert("Content-Type", HeaderValue::from_static("text/event-stream"));
    headers.insert("Cache-Control", HeaderValue::from_static("no-cache"));

    MetadataPrelude {
        status_code: http::StatusCode::from_u16(status).unwrap_or(http::StatusCode::OK),
        headers,
        cookies: vec![],
    }
}

// ─── Helper: send SSE event ──────────────────────────────────────────────────

async fn send_sse(tx: &mut lambda_runtime::streaming::Sender, data: &str) {
    let event = format!("data: {}\n\n", data);
    let _ = tx.send_data(event.into()).await;
}

// ─── Handler ─────────────────────────────────────────────────────────────────

async fn handler(event: LambdaEvent<Value>) -> Result<Response<Body>, Error> {
    let t_total = std::time::Instant::now();
    tracing::error!("handler invoked");

    // Parse request body from Lambda Function URL event
    let body_str = event
        .payload
        .get("body")
        .and_then(|b| b.as_str())
        .unwrap_or("");

    // Parse the request
    let request: GenerateDuaRequest = match serde_json::from_str(body_str) {
        Ok(r) => r,
        Err(_) => {
            let (mut tx, rx) = channel();
            tokio::spawn(async move {
                let msg = serde_json::json!({
                    "type": "error",
                    "message": "Invalid request body. Expected JSON with 'prompt' field"
                });
                send_sse(&mut tx, &msg.to_string()).await;
            });
            return Ok(Response {
                metadata_prelude: sse_metadata(400),
                stream: rx,
            });
        }
    };

    // Validate prompt
    let prompt = request.prompt.trim().to_string();

    if prompt.is_empty() {
        let (mut tx, rx) = channel();
        tokio::spawn(async move {
            let msg = serde_json::json!({
                "type": "error",
                "message": "Prompt cannot be empty"
            });
            send_sse(&mut tx, &msg.to_string()).await;
        });
        return Ok(Response {
            metadata_prelude: sse_metadata(400),
            stream: rx,
        });
    }

    // Fetch API key from SSM
    tracing::error!(elapsed_ms = t_total.elapsed().as_millis() as u64, "starting SSM fetch");
    let t_ssm = std::time::Instant::now();
    let stage = env::var("STAGE").unwrap_or_else(|_| "dev".to_string());
    let ssm_path = format!("/hidayah/{}/gcp-api-key", stage);

    let ssm = SsmParameter::with_config(SsmConfig::default())
        .await
        .map_err(|e| { tracing::error!(error = %e, "SSM init failed"); Error::from(format!("SSM init error: {}", e)) })?;
    tracing::error!(elapsed_ms = t_ssm.elapsed().as_millis() as u64, "SSM client ready");

    let api_key = ssm
        .get_parameter_value(&ssm_path, Some(true))
        .await
        .map_err(|e| { tracing::error!(error = %e, ssm_path = %ssm_path, "SSM fetch failed"); Error::from(format!("SSM fetch error: {}", e)) })?;
    tracing::error!(
        ssm_ms = t_ssm.elapsed().as_millis() as u64,
        total_ms = t_total.elapsed().as_millis() as u64,
        "SSM fetch done"
    );

    // Create streaming channel
    let (mut tx, rx) = channel();

    tracing::error!(total_ms = t_total.elapsed().as_millis() as u64, "spawning gemini task");

    // Spawn streaming task - pure pass-through from Gemini to client
    tokio::spawn(async move {
        let t_spawn = std::time::Instant::now();
        tracing::error!("gemini task started");

        match gemini::stream_generate_duas(&api_key, &prompt).await {
            Ok(mut stream) => {
                tracing::error!(elapsed_ms = t_spawn.elapsed().as_millis() as u64, "gemini stream ready");
                let mut chunk_count = 0usize;

                // Stream each chunk directly to the client
                while let Ok(Some(chunk)) = stream.next().await {
                    let text = chunk.text();
                    if !text.is_empty() {
                        if chunk_count == 0 {
                            tracing::error!(elapsed_ms = t_spawn.elapsed().as_millis() as u64, "FIRST CHUNK received");
                        }
                        chunk_count += 1;
                        let chunk_event = serde_json::json!({
                            "type": "chunk",
                            "text": text
                        });
                        send_sse(&mut tx, &chunk_event.to_string()).await;
                        if chunk_count <= 3 || chunk_count % 10 == 0 {
                            tracing::error!(chunk = chunk_count, elapsed_ms = t_spawn.elapsed().as_millis() as u64, "chunk sent");
                        }
                    }
                }

                tracing::error!(
                    chunks = chunk_count,
                    elapsed_ms = t_spawn.elapsed().as_millis() as u64,
                    "stream complete"
                );
                // Signal stream complete
                let done_event = serde_json::json!({ "type": "done" });
                send_sse(&mut tx, &done_event.to_string()).await;
            }
            Err(e) => {
                tracing::error!(
                    error = %e,
                    elapsed_ms = t_spawn.elapsed().as_millis() as u64,
                    "gemini stream failed"
                );
                let error_event = serde_json::json!({
                    "type": "error",
                    "message": "Failed to generate duas. Please try again"
                });
                send_sse(&mut tx, &error_event.to_string()).await;
            }
        }
    });

    tracing::error!(total_ms = t_total.elapsed().as_millis() as u64, "returning streaming response");
    // Return streaming response with SSE metadata
    Ok(Response {
        metadata_prelude: sse_metadata(200),
        stream: rx,
    })
}

// ─── Main ────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .json()
        .init();

    lambda_runtime::run(service_fn(handler)).await
}
