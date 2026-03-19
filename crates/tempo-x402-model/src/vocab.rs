//! Vocabulary: maps plan step types to token IDs.
//!
//! Fixed vocabulary of ~35 tokens covering all PlanStep variants plus
//! special tokens (PAD, BOS, EOS, UNK) and context tokens (goal keywords).

use serde::{Deserialize, Serialize};

// ── Special Tokens ───────────────────────────────────────────────────

pub const PAD: u32 = 0;
pub const BOS: u32 = 1; // Beginning of sequence
pub const EOS: u32 = 2; // End of sequence
pub const UNK: u32 = 3; // Unknown token

// ── Plan Step Tokens ─────────────────────────────────────────────────

pub const TOK_READ_FILE: u32 = 4;
pub const TOK_SEARCH_CODE: u32 = 5;
pub const TOK_LIST_DIR: u32 = 6;
pub const TOK_RUN_SHELL: u32 = 7;
pub const TOK_COMMIT: u32 = 8;
pub const TOK_CHECK_SELF: u32 = 9;
pub const TOK_CREATE_ENDPOINT: u32 = 10;
pub const TOK_TEST_ENDPOINT: u32 = 11;
pub const TOK_CALL_PAID: u32 = 12;
pub const TOK_CARGO_CHECK: u32 = 13;
pub const TOK_GENERATE_CODE: u32 = 14;
pub const TOK_EDIT_CODE: u32 = 15;
pub const TOK_THINK: u32 = 16;
pub const TOK_DELETE_ENDPOINT: u32 = 17;
pub const TOK_DISCOVER_PEERS: u32 = 18;
pub const TOK_CALL_PEER: u32 = 19;
pub const TOK_CREATE_REPO: u32 = 20;
pub const TOK_FORK_REPO: u32 = 21;
pub const TOK_SCREENSHOT: u32 = 22;
pub const TOK_SCREEN_CLICK: u32 = 23;
pub const TOK_SCREEN_TYPE: u32 = 24;
pub const TOK_BROWSE_URL: u32 = 25;
pub const TOK_REVIEW_PR: u32 = 26;
pub const TOK_CLONE_SELF: u32 = 27;
pub const TOK_SPAWN_SPECIALIST: u32 = 28;
pub const TOK_DELEGATE_TASK: u32 = 29;

// ── Context Tokens (goal keywords mapped to IDs) ─────────────────────

pub const TOK_CTX_START: u32 = 30;
// Context tokens: 30-63 (34 slots for goal keywords)
pub const TOK_CTX_END: u32 = 63;

/// Total vocabulary size.
pub const VOCAB_SIZE: usize = 64;

/// Maximum sequence length (plan steps + context).
pub const MAX_SEQ_LEN: usize = 32;

/// Vocabulary for plan step tokenization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vocab {
    /// Maps keyword strings to context token IDs (30-63).
    pub keyword_map: std::collections::HashMap<String, u32>,
    next_keyword_id: u32,
}

impl Default for Vocab {
    fn default() -> Self {
        Self::new()
    }
}

impl Vocab {
    pub fn new() -> Self {
        Self {
            keyword_map: std::collections::HashMap::new(),
            next_keyword_id: TOK_CTX_START,
        }
    }

    /// Convert a plan step type name to a token ID.
    pub fn step_to_token(step_type: &str) -> u32 {
        match step_type {
            "read_file" => TOK_READ_FILE,
            "search_code" => TOK_SEARCH_CODE,
            "list_dir" => TOK_LIST_DIR,
            "run_shell" => TOK_RUN_SHELL,
            "commit" => TOK_COMMIT,
            "check_self" => TOK_CHECK_SELF,
            "create_script_endpoint" => TOK_CREATE_ENDPOINT,
            "test_script_endpoint" => TOK_TEST_ENDPOINT,
            "call_paid_endpoint" => TOK_CALL_PAID,
            "cargo_check" => TOK_CARGO_CHECK,
            "generate_code" => TOK_GENERATE_CODE,
            "edit_code" => TOK_EDIT_CODE,
            "think" => TOK_THINK,
            "delete_endpoint" => TOK_DELETE_ENDPOINT,
            "discover_peers" => TOK_DISCOVER_PEERS,
            "call_peer" => TOK_CALL_PEER,
            "create_github_repo" => TOK_CREATE_REPO,
            "fork_github_repo" => TOK_FORK_REPO,
            "screenshot" => TOK_SCREENSHOT,
            "screen_click" => TOK_SCREEN_CLICK,
            "screen_type" => TOK_SCREEN_TYPE,
            "browse_url" => TOK_BROWSE_URL,
            "review_peer_pr" => TOK_REVIEW_PR,
            "clone_self" => TOK_CLONE_SELF,
            "spawn_specialist" => TOK_SPAWN_SPECIALIST,
            "delegate_task" => TOK_DELEGATE_TASK,
            _ => UNK,
        }
    }

    /// Convert a token ID back to a step type name.
    pub fn token_to_step(token: u32) -> &'static str {
        match token {
            TOK_READ_FILE => "read_file",
            TOK_SEARCH_CODE => "search_code",
            TOK_LIST_DIR => "list_dir",
            TOK_RUN_SHELL => "run_shell",
            TOK_COMMIT => "commit",
            TOK_CHECK_SELF => "check_self",
            TOK_CREATE_ENDPOINT => "create_script_endpoint",
            TOK_TEST_ENDPOINT => "test_script_endpoint",
            TOK_CALL_PAID => "call_paid_endpoint",
            TOK_CARGO_CHECK => "cargo_check",
            TOK_GENERATE_CODE => "generate_code",
            TOK_EDIT_CODE => "edit_code",
            TOK_THINK => "think",
            TOK_DELETE_ENDPOINT => "delete_endpoint",
            TOK_DISCOVER_PEERS => "discover_peers",
            TOK_CALL_PEER => "call_peer",
            TOK_CREATE_REPO => "create_github_repo",
            TOK_FORK_REPO => "fork_github_repo",
            TOK_SCREENSHOT => "screenshot",
            TOK_SCREEN_CLICK => "screen_click",
            TOK_SCREEN_TYPE => "screen_type",
            TOK_BROWSE_URL => "browse_url",
            TOK_REVIEW_PR => "review_peer_pr",
            TOK_CLONE_SELF => "clone_self",
            TOK_SPAWN_SPECIALIST => "spawn_specialist",
            TOK_DELEGATE_TASK => "delegate_task",
            _ => "unknown",
        }
    }

    /// Get or assign a context token for a goal keyword.
    pub fn keyword_token(&mut self, keyword: &str) -> u32 {
        if let Some(&id) = self.keyword_map.get(keyword) {
            return id;
        }
        if self.next_keyword_id > TOK_CTX_END {
            // Vocab full — hash to existing range
            let hash = keyword
                .bytes()
                .fold(0u32, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u32));
            return TOK_CTX_START + (hash % (TOK_CTX_END - TOK_CTX_START + 1));
        }
        let id = self.next_keyword_id;
        self.keyword_map.insert(keyword.to_string(), id);
        self.next_keyword_id += 1;
        id
    }

    /// Tokenize a plan step sequence into token IDs.
    pub fn tokenize_plan(&self, step_types: &[String]) -> Vec<u32> {
        let mut tokens = vec![BOS];
        for step in step_types.iter().take(MAX_SEQ_LEN - 2) {
            tokens.push(Self::step_to_token(step));
        }
        tokens.push(EOS);
        tokens
    }

    /// Tokenize goal keywords into context tokens.
    pub fn tokenize_context(&mut self, keywords: &[String]) -> Vec<u32> {
        keywords
            .iter()
            .take(8) // Max 8 context tokens
            .map(|kw| self.keyword_token(kw))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_step_tokens() {
        assert_eq!(Vocab::step_to_token("read_file"), TOK_READ_FILE);
        assert_eq!(Vocab::step_to_token("edit_code"), TOK_EDIT_CODE);
        assert_eq!(Vocab::step_to_token("nonexistent"), UNK);
        assert_eq!(Vocab::token_to_step(TOK_CARGO_CHECK), "cargo_check");
    }

    #[test]
    fn test_tokenize_plan() {
        let vocab = Vocab::new();
        let steps = vec![
            "read_file".to_string(),
            "edit_code".to_string(),
            "cargo_check".to_string(),
        ];
        let tokens = vocab.tokenize_plan(&steps);
        assert_eq!(
            tokens,
            vec![BOS, TOK_READ_FILE, TOK_EDIT_CODE, TOK_CARGO_CHECK, EOS]
        );
    }

    #[test]
    fn test_keyword_tokens() {
        let mut vocab = Vocab::new();
        let t1 = vocab.keyword_token("compile");
        let t2 = vocab.keyword_token("benchmark");
        let t3 = vocab.keyword_token("compile"); // Same keyword
        assert!(t1 >= TOK_CTX_START);
        assert_ne!(t1, t2);
        assert_eq!(t1, t3); // Deterministic
    }
}
