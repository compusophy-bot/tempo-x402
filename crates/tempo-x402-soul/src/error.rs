//! Soul error types.

/// Errors that can occur in the soul crate.
#[derive(Debug, thiserror::Error)]
pub enum SoulError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("LLM API error: {0}")]
    Llm(String),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("observer error: {0}")]
    Observer(String),

    #[error("config error: {0}")]
    Config(String),

    #[error("guard error: {0}")]
    Guard(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
