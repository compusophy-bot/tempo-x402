//! LLM client with retry, backoff, and function calling support.
//!
//! Currently backed by the Gemini API. The public types (`LlmClient`, `LlmResult`)
//! are provider-agnostic so callers don't need to change when the backend changes.

use serde::{Deserialize, Serialize};

use crate::error::SoulError;

/// LLM client for the soul's thinking loop.
pub struct LlmClient {
    api_key: String,
    model_fast: String,
    #[allow(dead_code)]
    model_think: String,
    http: reqwest::Client,
}

// ── Gemini wire types (private) ─────────────────────────────────────────

#[derive(Serialize)]
struct GeminiRequest {
    contents: Vec<Content>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system_instruction: Option<Content>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<ToolDeclaration>>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct Content {
    #[serde(skip_serializing_if = "Option::is_none")]
    role: Option<String>,
    parts: Vec<Part>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct Part {
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    function_call: Option<FunctionCall>,
    #[serde(skip_serializing_if = "Option::is_none")]
    function_response: Option<FunctionResponse>,
    /// Gemini 3+ thought signature — must be preserved and passed back
    /// when sending function call history to avoid 400 errors.
    #[serde(skip_serializing_if = "Option::is_none")]
    thought_signature: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct ToolDeclaration {
    function_declarations: Vec<FunctionDeclaration>,
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

// ── Public types (provider-agnostic) ────────────────────────────────────

/// A function call returned by the LLM.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FunctionCall {
    pub name: String,
    pub args: serde_json::Value,
    /// Gemini 3+ thought signature — opaque, must be passed back as-is.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thought_signature: Option<String>,
}

/// A function response to send back to the LLM.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FunctionResponse {
    pub name: String,
    pub response: serde_json::Value,
}

/// A single function declaration describing a tool the LLM can call.
#[derive(Serialize, Clone, Debug)]
pub struct FunctionDeclaration {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// Result of an LLM call — either text or a function call request.
#[derive(Debug, Clone)]
pub enum LlmResult {
    Text(String),
    FunctionCall(FunctionCall),
}

/// A conversation message for multi-turn function calling.
#[derive(Clone, Debug)]
pub struct ConversationMessage {
    pub role: String,
    pub parts: Vec<ConversationPart>,
}

/// A part in a conversation message.
#[derive(Clone, Debug)]
pub enum ConversationPart {
    Text(String),
    FunctionCall(FunctionCall),
    FunctionResponse(FunctionResponse),
}

impl LlmClient {
    /// Create a new LLM client.
    pub fn new(api_key: String, model_fast: String, model_think: String) -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
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

    /// Send a simple prompt and return the response text.
    /// Retries up to 3 times with exponential backoff + jitter.
    pub async fn think(&self, system_prompt: &str, user_prompt: &str) -> Result<String, SoulError> {
        let request = GeminiRequest {
            contents: vec![Content {
                role: Some("user".to_string()),
                parts: vec![Part {
                    text: Some(user_prompt.to_string()),
                    function_call: None,
                    function_response: None,
                    thought_signature: None,
                }],
            }],
            system_instruction: Some(Content {
                role: None,
                parts: vec![Part {
                    text: Some(system_prompt.to_string()),
                    function_call: None,
                    function_response: None,
                    thought_signature: None,
                }],
            }),
            tools: None,
        };

        let result = self.send_request(&request).await?;
        match result {
            LlmResult::Text(t) => Ok(t),
            LlmResult::FunctionCall(_) => Ok(String::new()),
        }
    }

    /// Send a prompt with tools and conversation history. Returns Text or FunctionCall.
    pub async fn think_with_tools(
        &self,
        system_prompt: &str,
        conversation: &[ConversationMessage],
        tool_declarations: &[FunctionDeclaration],
    ) -> Result<LlmResult, SoulError> {
        let contents: Vec<Content> = conversation
            .iter()
            .map(|msg| Content {
                role: Some(msg.role.clone()),
                parts: msg
                    .parts
                    .iter()
                    .map(|p| match p {
                        ConversationPart::Text(t) => Part {
                            text: Some(t.clone()),
                            function_call: None,
                            function_response: None,
                            thought_signature: None,
                        },
                        ConversationPart::FunctionCall(fc) => {
                            // thought_signature must be at the Part level, NOT inside
                            // function_call — Gemini 3+ rejects unknown fields there.
                            let mut fc_clean = fc.clone();
                            let sig = fc_clean.thought_signature.take();
                            Part {
                                text: None,
                                function_call: Some(fc_clean),
                                function_response: None,
                                thought_signature: sig,
                            }
                        }
                        ConversationPart::FunctionResponse(fr) => Part {
                            text: None,
                            function_call: None,
                            function_response: Some(fr.clone()),
                            thought_signature: None,
                        },
                    })
                    .collect(),
            })
            .collect();

        let tools = if tool_declarations.is_empty() {
            None
        } else {
            Some(vec![ToolDeclaration {
                function_declarations: tool_declarations.to_vec(),
            }])
        };

        let request = GeminiRequest {
            contents,
            system_instruction: Some(Content {
                role: None,
                parts: vec![Part {
                    text: Some(system_prompt.to_string()),
                    function_call: None,
                    function_response: None,
                    thought_signature: None,
                }],
            }),
            tools,
        };

        self.send_request(&request).await
    }

    /// Low-level: send a request to the Gemini API and parse the response.
    async fn send_request(&self, request: &GeminiRequest) -> Result<LlmResult, SoulError> {
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            self.model_fast, self.api_key
        );

        let backoff_ms = [500u64, 1000, 2000];
        let mut last_err = None;

        for (attempt, base_delay) in backoff_ms.iter().enumerate() {
            match self.http.post(&url).json(request).send().await {
                Ok(resp) => {
                    let status = resp.status();
                    let body = resp.text().await.map_err(SoulError::Http)?;

                    if !status.is_success() {
                        last_err = Some(SoulError::Llm(format!(
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
                        .map_err(|e| SoulError::Llm(format!("failed to parse response: {e}")))?;

                    let parts = parsed
                        .candidates
                        .and_then(|c| c.into_iter().next())
                        .and_then(|c| c.content)
                        .and_then(|c| c.parts)
                        .unwrap_or_default();

                    // Check for function call first
                    for part in &parts {
                        if let Some(fc) = &part.function_call {
                            // Attach thought_signature from the Part to the FunctionCall
                            let mut fc = fc.clone();
                            if fc.thought_signature.is_none() {
                                fc.thought_signature = part.thought_signature.clone();
                            }
                            return Ok(LlmResult::FunctionCall(fc));
                        }
                    }

                    // Otherwise collect text
                    let text: String = parts
                        .iter()
                        .filter_map(|p| p.text.as_ref())
                        .cloned()
                        .collect::<Vec<_>>()
                        .join("");

                    return Ok(LlmResult::Text(text));
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

        Err(last_err.unwrap_or_else(|| SoulError::Llm("all retries exhausted".to_string())))
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
