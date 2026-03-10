use aws_lambda_events::apigw::{ApiGatewayProxyRequest, ApiGatewayProxyResponse};
use dua_backend::{
    ErrorCode, ResponseError,
    ssm::{SsmConfig, SsmParameter},
    tts::{self, TtsConfig, VoiceGender},
    util,
};
use lambda_http::{Error, LambdaEvent};
use serde::Deserialize;
use std::env;

// ─── Request type ────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct GenerateAudioRequest {
    /// Arabic text to synthesize
    text: String,
    /// Voice gender: "male" or "female" (default: "male")
    gender: Option<VoiceGender>,
}

// ─── Handler ─────────────────────────────────────────────────────────────────

async fn handler(
    event: LambdaEvent<ApiGatewayProxyRequest>,
) -> Result<ApiGatewayProxyResponse, Error> {
    // Fetch GCP API Key from SSM
    let stage = env::var("STAGE").unwrap_or_else(|_| "dev".to_string());
    let ssm_path = format!("/hidayah/{}/gcp-api-key", stage);

    let ssm = SsmParameter::with_config(SsmConfig::default())
        .await
        .map_err(|e| Error::from(format!("SSM init error: {}", e)))?;

    let api_key = ssm
        .get_parameter_value(&ssm_path, Some(true))
        .await
        .map_err(|e| Error::from(format!("SSM fetch error: {}", e)))?;

    // Parse request body
    let body = match &event.payload.body {
        Some(b) => b,
        None => {
            return util::create_api_error_response(ResponseError::new(
                ErrorCode::BadRequest,
                Some("Request body is required"),
            ));
        }
    };

    let request: GenerateAudioRequest = match serde_json::from_str(body) {
        Ok(r) => r,
        Err(_) => {
            return util::create_api_error_response(ResponseError::new(
                ErrorCode::BadRequest,
                Some("Invalid request body. Expected JSON with 'text' field"),
            ));
        }
    };

    // Validate text
    let text = request.text.trim();

    if text.is_empty() {
        return util::create_api_error_response(ResponseError::new(
            ErrorCode::BadRequest,
            Some("Text cannot be empty"),
        ));
    }

    if !tts::is_arabic(text) {
        return util::create_api_error_response(ResponseError::new(
            ErrorCode::BadRequest,
            Some("Text must be in Arabic script"),
        ));
    }

    // Build TTS config from request
    let config = TtsConfig {
        gender: request.gender.unwrap_or_default(),
        // Keep defaults for everything else
        ..Default::default()
    };

    // Synthesize audio
    match tts::synthesize(&api_key, text, &config).await {
        Ok(audio) => util::create_api_success_response(audio, Some("Audio generated successfully")),
        Err(e) => {
            eprintln!("TTS error: {}", e);

            let message = match &e {
                tts::TtsError::NotArabic => "Text must be in Arabic script",
                tts::TtsError::EmptyText => "Text cannot be empty",
                tts::TtsError::TextTooLong => "Text is too long (max 2000 characters)",
                _ => "Failed to generate audio. Please try again",
            };

            util::create_api_error_response(ResponseError::new(
                ErrorCode::UnknownError,
                Some(message),
            ))
        }
    }
}

// ─── Main ────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<(), Error> {
    lambda_runtime::run(lambda_runtime::service_fn(handler)).await
}
