use google_ai_rs::{AsSchema, Client, genai::ResponseStream};
use serde::{Deserialize, Serialize};

// ─── Types ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, AsSchema)]
pub struct GeneratedDua {
    /// The Arabic text of the dua
    pub arabic: String,
    /// Phonetic transliteration for non-Arabic speakers
    pub transliteration: String,
    /// English translation of the dua
    pub translation: String,
    /// Source reference (e.g., "Quran 2:286", "Sahih Muslim 2735")
    pub reference: String,
    /// Why this dua is relevant to the user's situation
    pub context: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, AsSchema)]
pub struct GenerateDuaResponse {
    /// A personal, conversational response that matches the tone of the user's prompt.
    pub message: String,
    /// List of relevant duas
    pub duas: Vec<GeneratedDua>,
    /// Brief Islamic advice related to the user's situation (always sincere and scholarly)
    pub advice: String,
}

// ─── Error ───────────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum GeminiError {
    #[error("Gemini SDK error: {0}")]
    SdkError(#[from] google_ai_rs::Error),
    #[error("Failed to parse response: {0}")]
    ParseError(String),
}

// ─── System prompt ───────────────────────────────────────────────────────────

const SYSTEM_PROMPT: &str = r#"You are an Islamic dua expert and scholar with a warm, perceptive personality. Your role is to provide authentic, relevant duas (supplications) from the Quran and Sunnah based on a user's described needs.

You MUST respond with valid JSON in this exact structure:
{
  "message": "Your personal response to the user (2-4 sentences)",
  "duas": [
    {
      "arabic": "Full Arabic text with diacritics",
      "transliteration": "Phonetic transliteration",
      "translation": "English translation",
      "reference": "Source (e.g. Quran 2:286, Sahih Muslim 2735)",
      "context": "Why this dua is relevant (1-2 sentences)"
    }
  ],
  "advice": "Brief Islamic reminder (1-2 sentences)"
}

RULES:
1. Only return duas that are authentically sourced from the Quran or verified Hadith collections (Bukhari, Muslim, Tirmidhi, Abu Dawud, Nasa'i, Ibn Majah, Ahmad).
2. Each dua MUST include: accurate Arabic text with full diacritics, correct transliteration, faithful English translation, and precise source reference.
3. The "context" field should explain WHY this specific dua is relevant to the user's situation in 1-2 sentences.
4. Return 5-10 duas, prioritizing the most relevant ones first.
5. The "message" field is your personal response to the user. READ THEIR TONE CAREFULLY:
   - If they're being funny, light-hearted, or casual, respond with warmth and gentle humor while still being helpful.
   - If they're being serious, distressed, or emotional, respond with deep compassion and sincerity.
   - Match their energy. This is a human conversation, not a lecture.
   - Keep it to 2-4 sentences max. Never preachy.
6. The "advice" field should contain a brief, warm Islamic reminder (1-2 sentences) related to the user's situation.
7. Keep transliteration simple and readable for non-Arabic speakers.
8. Always include the full Arabic text with proper tashkeel (diacritical marks).
9. If the request is not related to duas, return an empty list of duas, a relevant message, and a gentle Islamic reminder in the "advice" field.
10. IMPORTANT: Return ONLY the JSON object, no markdown code fences, no extra text."#;

// ─── Public API ──────────────────────────────────────────────────────────────

/// Stream personalized duas based on user's described needs.
/// Uses `with_response_format("application/json")` instead of `as_response_schema`
/// so Gemini can stream partial JSON chunks immediately without waiting for
/// full schema validation.
pub async fn stream_generate_duas(
    api_key: &str,
    prompt: &str,
) -> Result<ResponseStream, GeminiError> {
    let t = std::time::Instant::now();
    tracing::error!("gemini: creating client");

    let client = Client::new(api_key).await?;
    tracing::error!(elapsed_ms = t.elapsed().as_millis() as u64, "gemini: client created");

    let model = client
        .generative_model("gemini-3-flash-preview")
        .with_system_instruction(SYSTEM_PROMPT)
        .with_response_format("application/json")
        .temperature(0.7);

    tracing::error!("gemini: calling stream_generate_content");
    let stream = model.stream_generate_content(prompt).await
        .map_err(|e| { tracing::error!(error = %e, "gemini: stream_generate_content failed"); GeminiError::SdkError(e) })?;
    tracing::error!(elapsed_ms = t.elapsed().as_millis() as u64, "gemini: stream ready");

    Ok(stream)
}
