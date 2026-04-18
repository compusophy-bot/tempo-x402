//! Byte Pair Encoding tokenizer for Rust source code.
//!
//! Pure Rust. No external tokenizer crates. Trains from source text,
//! learns merge rules, encodes/decodes strings to/from token IDs.
//!
//! ## Design
//!
//! - Vocab size: configurable (default 8192 for Phase 3 code gen model)
//! - Base vocab: 256 byte values + special tokens
//! - Merge rules: learned from training corpus via frequency counting
//! - Serializable: merge rules + vocab stored as JSON for persistence
//!
//! ## Usage
//!
//! ```rust
//! let mut tokenizer = BpeTokenizer::new(8192);
//! tokenizer.train("fn main() { println!(\"hello\"); }");
//! let tokens = tokenizer.encode("fn main()");
//! let text = tokenizer.decode(&tokens);
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Special token IDs.
pub const PAD_TOKEN: u32 = 0;
pub const BOS_TOKEN: u32 = 1; // beginning of sequence
pub const EOS_TOKEN: u32 = 2; // end of sequence
pub const UNK_TOKEN: u32 = 3; // unknown

/// Base vocab starts after special tokens.
const BASE_VOCAB_START: u32 = 4;
/// 256 byte values as base vocab.
const BYTE_VOCAB_SIZE: u32 = 256;
/// First merge token ID.
const MERGE_START: u32 = BASE_VOCAB_START + BYTE_VOCAB_SIZE; // 260

/// A learned BPE tokenizer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BpeTokenizer {
    /// Maximum vocabulary size.
    pub vocab_size: u32,
    /// Merge rules in order of learning. Each rule: (token_a, token_b) → merged_token.
    pub merges: Vec<(u32, u32)>,
    /// Token ID → byte sequence mapping (for decoding).
    pub vocab: Vec<Vec<u8>>,
    /// Byte sequence → token ID (for encoding, built from vocab).
    #[serde(skip)]
    encode_cache: HashMap<Vec<u8>, u32>,
}

impl BpeTokenizer {
    /// Create a new tokenizer with the given max vocab size.
    pub fn new(vocab_size: u32) -> Self {
        let mut vocab = Vec::with_capacity(vocab_size as usize);

        // Special tokens
        vocab.push(vec![]); // PAD
        vocab.push(vec![1]); // BOS
        vocab.push(vec![2]); // EOS
        vocab.push(vec![3]); // UNK

        // Byte vocab: 0x00-0xFF
        for b in 0u8..=255 {
            vocab.push(vec![b]);
        }

        let mut tok = Self {
            vocab_size,
            merges: Vec::new(),
            vocab,
            encode_cache: HashMap::new(),
        };
        tok.rebuild_encode_cache();
        tok
    }

    /// Train the tokenizer on a corpus of text.
    /// Learns merge rules until vocab_size is reached.
    pub fn train(&mut self, text: &str) {
        let bytes = text.as_bytes();
        if bytes.is_empty() {
            return;
        }

        // Start with byte-level tokens
        let mut sequence: Vec<u32> = bytes.iter().map(|&b| BASE_VOCAB_START + b as u32).collect();

        let target_merges = self.vocab_size.saturating_sub(MERGE_START);

        for _ in 0..target_merges {
            // Count all adjacent pairs
            let mut pair_counts: HashMap<(u32, u32), usize> = HashMap::new();
            for window in sequence.windows(2) {
                let pair = (window[0], window[1]);
                *pair_counts.entry(pair).or_insert(0) += 1;
            }

            // Find most frequent pair
            let best = pair_counts
                .iter()
                .max_by_key(|(_, &count)| count)
                .map(|(&pair, &count)| (pair, count));

            let Some(((a, b), count)) = best else {
                break; // No more pairs
            };

            if count < 2 {
                break; // Not worth merging singletons
            }

            // Create new token for this merge
            let new_id = self.vocab.len() as u32;
            let mut new_bytes = self.vocab[a as usize].clone();
            new_bytes.extend_from_slice(&self.vocab[b as usize]);
            self.vocab.push(new_bytes);
            self.merges.push((a, b));

            // Apply merge to sequence
            let mut new_seq = Vec::with_capacity(sequence.len());
            let mut i = 0;
            while i < sequence.len() {
                if i + 1 < sequence.len() && sequence[i] == a && sequence[i + 1] == b {
                    new_seq.push(new_id);
                    i += 2;
                } else {
                    new_seq.push(sequence[i]);
                    i += 1;
                }
            }
            sequence = new_seq;

            if self.vocab.len() as u32 >= self.vocab_size {
                break;
            }
        }

        self.rebuild_encode_cache();
    }

    /// Encode text to token IDs.
    pub fn encode(&self, text: &str) -> Vec<u32> {
        let bytes = text.as_bytes();
        if bytes.is_empty() {
            return vec![];
        }

        // Start with byte-level tokens
        let mut tokens: Vec<u32> = bytes.iter().map(|&b| BASE_VOCAB_START + b as u32).collect();

        // Apply merges in order
        for &(a, b) in &self.merges {
            let merged_id = self.merge_token_id(a, b);
            let mut new_tokens = Vec::with_capacity(tokens.len());
            let mut i = 0;
            while i < tokens.len() {
                if i + 1 < tokens.len() && tokens[i] == a && tokens[i + 1] == b {
                    new_tokens.push(merged_id);
                    i += 2;
                } else {
                    new_tokens.push(tokens[i]);
                    i += 1;
                }
            }
            tokens = new_tokens;
        }

        tokens
    }

    /// Decode token IDs back to text.
    pub fn decode(&self, tokens: &[u32]) -> String {
        let mut bytes = Vec::new();
        for &token in tokens {
            if (token as usize) < self.vocab.len() {
                bytes.extend_from_slice(&self.vocab[token as usize]);
            }
        }
        String::from_utf8_lossy(&bytes).to_string()
    }

    /// Get the current vocabulary size (including learned merges).
    pub fn current_vocab_size(&self) -> u32 {
        self.vocab.len() as u32
    }

    /// Get the token ID for a merge pair.
    fn merge_token_id(&self, a: u32, b: u32) -> u32 {
        for (i, &(ma, mb)) in self.merges.iter().enumerate() {
            if ma == a && mb == b {
                return MERGE_START + i as u32;
            }
        }
        UNK_TOKEN
    }

    /// Rebuild the encode cache from vocab.
    fn rebuild_encode_cache(&mut self) {
        self.encode_cache.clear();
        for (id, bytes) in self.vocab.iter().enumerate() {
            if !bytes.is_empty() {
                self.encode_cache.insert(bytes.clone(), id as u32);
            }
        }
    }

    /// Serialize to JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    /// Deserialize from JSON.
    pub fn from_json(json: &str) -> Option<Self> {
        let mut tok: Self = serde_json::from_str(json).ok()?;
        tok.rebuild_encode_cache();
        Some(tok)
    }

    /// Compression ratio: original bytes / encoded tokens.
    /// Higher = better compression = more efficient tokenization.
    pub fn compression_ratio(&self, text: &str) -> f64 {
        let tokens = self.encode(text);
        if tokens.is_empty() {
            return 1.0;
        }
        text.len() as f64 / tokens.len() as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_encode_decode() {
        let tok = BpeTokenizer::new(512);
        let text = "hello world";
        let tokens = tok.encode(text);
        let decoded = tok.decode(&tokens);
        assert_eq!(decoded, text);
    }

    #[test]
    fn test_train_reduces_tokens() {
        let mut tok = BpeTokenizer::new(512);
        let corpus = "fn main() { fn main() { fn main() { println!(\"hello\"); } } }";
        let before = tok.encode(corpus).len();
        tok.train(corpus);
        let after = tok.encode(corpus).len();
        // Training should reduce token count via merges
        assert!(
            after <= before,
            "after={after} should be <= before={before}"
        );
    }

    #[test]
    fn test_roundtrip_rust_code() {
        let mut tok = BpeTokenizer::new(1024);
        let code = r#"
pub fn fibonacci(n: u64) -> u64 {
    match n {
        0 => 0,
        1 => 1,
        _ => fibonacci(n - 1) + fibonacci(n - 2),
    }
}
"#;
        tok.train(code);
        let tokens = tok.encode(code);
        let decoded = tok.decode(&tokens);
        assert_eq!(decoded, code);
    }

    #[test]
    fn test_compression_ratio() {
        let mut tok = BpeTokenizer::new(1024);
        let repetitive = "struct Point { x: f64, y: f64 }\n".repeat(100);
        tok.train(&repetitive);
        let ratio = tok.compression_ratio(&repetitive);
        // Repetitive text should compress well
        assert!(
            ratio > 2.0,
            "ratio={ratio} should be > 2.0 for repetitive text"
        );
    }

    #[test]
    fn test_empty_input() {
        let tok = BpeTokenizer::new(512);
        assert!(tok.encode("").is_empty());
        assert_eq!(tok.decode(&[]), "");
    }

    #[test]
    fn test_special_tokens() {
        assert_eq!(PAD_TOKEN, 0);
        assert_eq!(BOS_TOKEN, 1);
        assert_eq!(EOS_TOKEN, 2);
        assert_eq!(UNK_TOKEN, 3);
        assert_eq!(MERGE_START, 260);
    }
}
