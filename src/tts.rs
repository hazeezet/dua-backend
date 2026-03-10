use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use reqwest::Client;
use serde::{Deserialize, Serialize};

// ─── Voice configuration ─────────────────────────────────────────────────────

/// Available voice genders with default speaker actors
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum VoiceGender {
    Male,
    Female,
}

impl Default for VoiceGender {
    fn default() -> Self {
        VoiceGender::Male
    }
}

impl VoiceGender {
    /// Default speaker name for each gender
    pub fn default_speaker(&self) -> &'static str {
        match self {
            VoiceGender::Male => "Charon",
            VoiceGender::Female => "Zephyr",
        }
    }
}

/// TTS synthesis configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtsConfig {
    /// Voice gender (default: Male)
    pub gender: VoiceGender,
    /// Custom speaker name - overrides gender default
    pub voice_name: Option<String>,
    /// Style prompt - controls how the text is spoken
    /// e.g. "Say the following reverently and clearly"
    pub prompt: Option<String>,
    /// Audio encoding: MP3, LINEAR16, OGG_OPUS (default: MP3)
    pub audio_encoding: Option<String>,
    /// Speaking rate/speed: [0.25, 4.0] (default: 1.0)
    pub speed: Option<f64>,
    /// Pitch: [-20.0, 20.0] (default: 0.0)
    pub pitch: Option<f64>,
}

impl Default for TtsConfig {
    fn default() -> Self {
        Self {
            gender: VoiceGender::default(),
            voice_name: None,
            prompt: None,
            audio_encoding: None,
            speed: None,
            pitch: None,
        }
    }
}

impl TtsConfig {
    /// Resolved speaker name
    pub fn resolved_speaker(&self) -> &str {
        self.voice_name
            .as_deref()
            .unwrap_or_else(|| self.gender.default_speaker())
    }

    /// Resolved style prompt
    pub fn resolved_prompt(&self) -> &str {
        self.prompt
            .as_deref()
            .unwrap_or("Say the following Arabic text clearly and reverently with proper tajweed pronunciation")
    }

    /// Resolved audio encoding
    pub fn resolved_encoding(&self) -> &str {
        self.audio_encoding.as_deref().unwrap_or("MP3")
    }
    
    /// Resolved speaking rate/speed
    pub fn resolved_speed(&self) -> f64 {
        self.speed.unwrap_or(1.0)
    }

    /// Resolved pitch
    pub fn resolved_pitch(&self) -> f64 {
        self.pitch.unwrap_or(0.0)
    }
}

// ─── Response type ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioResponse {
    pub audio: String,
    pub format: String,
}

// ─── Gemini TTS API request shapes ──────────────────────────────────────────
// Uses generativelanguage.googleapis.com which supports API key auth

#[derive(Debug, Serialize)]
struct GeminiTtsRequest {
    contents: Vec<GeminiContent>,
    #[serde(rename = "generationConfig")]
    generation_config: GeminiGenerationConfig,
}

#[derive(Debug, Serialize)]
struct GeminiContent {
    parts: Vec<GeminiPart>,
}

#[derive(Debug, Serialize)]
struct GeminiPart {
    text: String,
}

#[derive(Debug, Serialize)]
struct GeminiGenerationConfig {
    #[serde(rename = "responseModalities")]
    response_modalities: Vec<String>,
    #[serde(rename = "speechConfig")]
    speech_config: GeminiSpeechConfig,
}

#[derive(Debug, Serialize)]
struct GeminiSpeechConfig {
    #[serde(rename = "voiceConfig")]
    voice_config: GeminiVoiceConfig,
}

#[derive(Debug, Serialize)]
struct GeminiVoiceConfig {
    #[serde(rename = "prebuiltVoiceConfig")]
    prebuilt_voice_config: GeminiPrebuiltVoiceConfig,
}

#[derive(Debug, Serialize)]
struct GeminiPrebuiltVoiceConfig {
    #[serde(rename = "voiceName")]
    voice_name: String,
}

// ─── Gemini TTS API response ─────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct GeminiTtsResponse {
    candidates: Vec<GeminiCandidate>,
}

#[derive(Debug, Deserialize)]
struct GeminiCandidate {
    content: GeminiResponseContent,
}

#[derive(Debug, Deserialize)]
struct GeminiResponseContent {
    parts: Vec<GeminiResponsePart>,
}

#[derive(Debug, Deserialize)]
struct GeminiResponsePart {
    #[serde(rename = "inlineData")]
    inline_data: GeminiInlineData,
}

#[derive(Debug, Deserialize)]
struct GeminiInlineData {
    data: String,
}



// ─── Error ───────────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum TtsError {
    #[error("Input text is not Arabic")]
    NotArabic,
    #[error("Input text is empty")]
    EmptyText,
    #[error("Input text too long (max 2000 characters)")]
    TextTooLong,
    #[error("HTTP request failed: {0}")]
    HttpError(#[from] reqwest::Error),
    #[error("TTS API error: {0}")]
    ApiError(String),
    #[error("Authentication error: {0}")]
    AuthError(String),
}

// ─── Arabic validation ───────────────────────────────────────────────────────

/// Check if a string contains predominantly Arabic characters.
pub fn is_arabic(text: &str) -> bool {
    if text.trim().is_empty() {
        return false;
    }

    let mut arabic_chars = 0u32;
    let mut total_chars = 0u32;

    for ch in text.chars() {
        if ch.is_whitespace() || ".,;:!?\"'()-".contains(ch) {
            continue;
        }

        total_chars += 1;

        let cp = ch as u32;
        if (0x0600..=0x06FF).contains(&cp)
            || (0x0750..=0x077F).contains(&cp)
            || (0x08A0..=0x08FF).contains(&cp)
            || (0xFB50..=0xFDFF).contains(&cp)
            || (0xFE70..=0xFEFF).contains(&cp)
        {
            arabic_chars += 1;
        }
    }

    if total_chars == 0 {
        return false;
    }

    (arabic_chars as f64 / total_chars as f64) >= 0.7
}

// ─── PCM → WAV conversion ─────────────────────────────────────────────────────

/// Gemini TTS returns raw 16-bit little-endian mono PCM at 24 kHz.
/// Browsers cannot play raw PCM, so we wrap it in a standard WAV container.
fn pcm_to_wav(pcm: &[u8]) -> Vec<u8> {
    let sample_rate: u32 = 24_000;
    let channels: u16 = 1;
    let bits_per_sample: u16 = 16;
    let byte_rate = sample_rate * channels as u32 * bits_per_sample as u32 / 8;
    let block_align = channels * bits_per_sample / 8;
    let data_size = pcm.len() as u32;
    let file_size = 36 + data_size; // everything after the first 8 RIFF bytes

    let mut wav: Vec<u8> = Vec::with_capacity(44 + pcm.len());

    // RIFF chunk
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&file_size.to_le_bytes());
    wav.extend_from_slice(b"WAVE");

    // fmt sub-chunk
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());          // sub-chunk size
    wav.extend_from_slice(&1u16.to_le_bytes());           // PCM = 1
    wav.extend_from_slice(&channels.to_le_bytes());
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    wav.extend_from_slice(&byte_rate.to_le_bytes());
    wav.extend_from_slice(&block_align.to_le_bytes());
    wav.extend_from_slice(&bits_per_sample.to_le_bytes());

    // data sub-chunk
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_size.to_le_bytes());
    wav.extend_from_slice(pcm);

    wav
}

// ─── Public API ──────────────────────────────────────────────────────────────

/// Synthesize Arabic text to speech using the Gemini TTS API.
/// Uses generativelanguage.googleapis.com which supports API key authentication.
///
/// `api_key` - Gemini API key string (from SSM)
pub async fn synthesize(
    api_key: &str,
    text: &str,
    config: &TtsConfig,
) -> Result<AudioResponse, TtsError> {
    let text = text.trim();

    if text.is_empty() {
        return Err(TtsError::EmptyText);
    }
    if text.len() > 2000 {
        return Err(TtsError::TextTooLong);
    }
    if !is_arabic(text) {
        return Err(TtsError::NotArabic);
    }

    // Combine prompt instruction with Arabic text
    let full_text = format!("{}: {}", config.resolved_prompt(), text);

    let request_body = GeminiTtsRequest {
        contents: vec![GeminiContent {
            parts: vec![GeminiPart { text: full_text }],
        }],
        generation_config: GeminiGenerationConfig {
            response_modalities: vec!["AUDIO".to_string()],
            speech_config: GeminiSpeechConfig {
                voice_config: GeminiVoiceConfig {
                    prebuilt_voice_config: GeminiPrebuiltVoiceConfig {
                        voice_name: config.resolved_speaker().to_string(),
                    },
                },
            },
        },
    };

    let client = Client::new();
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash-preview-tts:generateContent?key={}",
        api_key
    );

    let response = client
        .post(&url)
        .json(&request_body)
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        if status.as_u16() == 401 || status.as_u16() == 403 {
            return Err(TtsError::AuthError(format!("Authentication failed: {}", body)));
        }
        return Err(TtsError::ApiError(format!("API returned {}: {}", status, body)));
    }

    let gemini_response: GeminiTtsResponse = response
        .json()
        .await
        .map_err(|e| TtsError::ApiError(format!("Failed to parse response: {}", e)))?;

    let audio_content = gemini_response
        .candidates
        .into_iter()
        .next()
        .and_then(|c| c.content.parts.into_iter().next())
        .map(|p| p.inline_data.data)
        .ok_or_else(|| TtsError::ApiError("No audio data in response".to_string()))?;

    // Gemini TTS returns raw 16-bit PCM - decode it, wrap in WAV, re-encode
    let pcm_bytes = BASE64
        .decode(&audio_content)
        .map_err(|e| TtsError::ApiError(format!("Invalid base64 audio: {}", e)))?;

    let wav_bytes = pcm_to_wav(&pcm_bytes);
    let wav_base64 = BASE64.encode(&wav_bytes);

    Ok(AudioResponse {
        audio: wav_base64,
        format: "wav".to_string(),
    })
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arabic_text() {
        assert!(is_arabic("اللَّهُمَّ إِنِّي أَسْأَلُكَ"));
        assert!(is_arabic("بِسْمِ اللَّهِ الرَّحْمَٰنِ الرَّحِيمِ"));
    }

    #[test]
    fn test_non_arabic_text() {
        assert!(!is_arabic("Hello world"));
        assert!(!is_arabic("12345"));
    }

    #[test]
    fn test_empty_text() {
        assert!(!is_arabic(""));
        assert!(!is_arabic("   "));
    }

    #[test]
    fn test_mixed_text_mostly_arabic() {
        assert!(is_arabic("اللَّهُمَّ 123"));
    }

    #[test]
    fn test_mixed_text_mostly_english() {
        assert!(!is_arabic("Hello اللَّهُمَّ world this is english"));
    }

    #[test]
    fn test_default_config() {
        let config = TtsConfig::default();
        assert_eq!(config.gender, VoiceGender::Male);
        assert_eq!(config.resolved_speaker(), "Charon");
        assert_eq!(config.resolved_encoding(), "MP3");
    }

    #[test]
    fn test_female_config() {
        let config = TtsConfig {
            gender: VoiceGender::Female,
            voice_name: None,
            prompt: None,
            audio_encoding: None,
            speed: None,
            pitch: None,
        };
        assert_eq!(config.resolved_speaker(), "Zephyr");
    }

    #[test]
    fn test_custom_voice() {
        let config = TtsConfig {
            gender: VoiceGender::Male,
            voice_name: Some("Fenrir".to_string()),
            prompt: None,
            audio_encoding: None,
            speed: None,
            pitch: None,
        };
        assert_eq!(config.resolved_speaker(), "Fenrir");
    }
}
