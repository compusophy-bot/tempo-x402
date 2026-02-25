//! Gemini API client with retry and backoff.

use serde::{Deserialize, Serialize};

use crate::error::SoulError;

/// Gemini API client.
pub struct GeminiClient {
    api_key: String,
    model_fast: String,
    #[allow(dead_code)]
    model_think: String,
    http: reqwest::Client,
}

#[derive(Serialize)]
struct GeminiRequest {
    contents: Vec<Content>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system_instruction: Option<Content>,
}

#[derive(Serialize, Deserialize)]
struct Content {
    #[serde(skip_serializing_if = "Option::is_none")]
    role: Option<String>,
    parts: Vec<Part>,
}

#[derive(Serialize, Deserialize)]
struct Part {
    text: String,
}

#[derive(Deserialize)]
struct GeminiResponse {
    candidates: Option<Vec<Candidate>>,
}

#[derive(Deserialize)]
struct Candidate {
    content: Option<CandidateContent>,
}

#[derive(Deserialize)]
struct CandidateContent {
    parts: Option<Vec<Part>>,
}

impl GeminiClient {
    /// Create a new Gemini client.
    pub fn new(api_key: String, model_fast: String, model_think: String) -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap_or_default();

        Self {
            api_key,
            model_fast,
            model_think,
            http,
        }
    }

    /// Send a prompt to Gemini Flash and return the response text.
    /// Retries up to 3 times with exponential backoff + jitter.
    pub async fn think(&self, system_prompt: &str, user_prompt: &str) -> Result<String, SoulError> {
        let request = GeminiRequest {
            contents: vec![Content {
                role: Some("user".to_string()),
                parts: vec![Part {
                    text: user_prompt.to_string(),
                }],
            }],
            system_instruction: Some(Content {
                role: None,
                parts: vec![Part {
                    text: system_prompt.to_string(),
                }],
            }),
        };

        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            self.model_fast, self.api_key
        );

        let backoff_ms = [500u64, 1000, 2000];
        let mut last_err = None;

        for (attempt, base_delay) in backoff_ms.iter().enumerate() {
            match self.http.post(&url).json(&request).send().await {
                Ok(resp) => {
                    let status = resp.status();
                    let body = resp.text().await.map_err(SoulError::Http)?;

                    if !status.is_success() {
                        last_err = Some(SoulError::Gemini(format!(
                            "HTTP {}: {}",
                            status,
                            body.chars().take(200).collect::<String>()
                        )));
                        if attempt < backoff_ms.len() - 1 {
                            let jitter = jitter_ms(*base_delay);
                            tokio::time::sleep(std::time::Duration::from_millis(jitter)).await;
                            continue;
                        }
                        break;
                    }

                    let parsed: GeminiResponse = serde_json::from_str(&body)
                        .map_err(|e| SoulError::Gemini(format!("failed to parse response: {e}")))?;

                    let text = parsed
                        .candidates
                        .and_then(|c| c.into_iter().next())
                        .and_then(|c| c.content)
                        .and_then(|c| c.parts)
                        .and_then(|p| p.into_iter().next())
                        .map(|p| p.text)
                        .unwrap_or_default();

                    return Ok(text);
                }
                Err(e) => {
                    last_err = Some(SoulError::Http(e));
                    if attempt < backoff_ms.len() - 1 {
                        let jitter = jitter_ms(*base_delay);
                        tokio::time::sleep(std::time::Duration::from_millis(jitter)).await;
                    }
                }
            }
        }

        Err(last_err.unwrap_or_else(|| SoulError::Gemini("all retries exhausted".to_string())))
    }
}

/// Add ±25% jitter to a base delay.
fn jitter_ms(base: u64) -> u64 {
    let quarter = base / 4;
    let offset = simple_random() % (quarter * 2 + 1);
    base - quarter + offset
}

/// Simple pseudo-random using timestamp nanos (not cryptographic, just for jitter).
fn simple_random() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64)
        .unwrap_or(0)
}
