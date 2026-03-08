use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use crate::error::SoulError;

#[derive(Debug, Clone)]
pub struct Vault {
    base_path: PathBuf,
}

impl Vault {
    /// Create a new Vault with the specified base path for storage.
    pub fn new<P: AsRef<Path>>(base_path: P) -> Result<Self, SoulError> {
        let base_path = base_path.as_ref().to_path_buf();
        if !base_path.exists() {
            fs::create_dir_all(&base_path)?;
        }
        Ok(Self { base_path })
    }

    /// Retrieve JSON data by key.
    pub fn get<T>(&self, key: &str) -> Result<Option<T>, SoulError>
    where
        T: for<'de> Deserialize<'de>,
    {
        let file_path = self.key_to_path(key);
        if !file_path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&file_path)?;
        
        let data: T = serde_json::from_str(&content)?;
        
        Ok(Some(data))
    }

    /// Store JSON data by key.
    pub fn set<T>(&self, key: &str, value: &T) -> Result<(), SoulError>
    where
        T: Serialize,
    {
        let file_path = self.key_to_path(key);
        
        // Ensure parent directory exists for nested keys if we ever support them.
        // For now, let's just make sure the base path exists (already done in new()).
        
        let content = serde_json::to_string_pretty(value)
            .map_err(|e| SoulError::Config(format!("Failed to serialize vault entry '{}': {}", key, e)))?;
        
        fs::write(&file_path, content)?;
        
        Ok(())
    }

    /// Delete a vault entry by key.
    pub fn delete(&self, key: &str) -> Result<(), SoulError> {
        let file_path = self.key_to_path(key);
        if file_path.exists() {
            fs::remove_file(file_path)?;
        }
        Ok(())
    }

    /// Lists all available keys in the vault.
    pub fn list_keys(&self) -> Result<Vec<String>, SoulError> {
        let mut keys = Vec::new();
        let entries = fs::read_dir(&self.base_path)?;
        
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                if let Some(file_name) = path.file_stem() {
                    if let Some(key) = file_name.to_str() {
                        keys.push(key.to_string());
                    }
                }
            }
        }
        Ok(keys)
    }

    fn key_to_path(&self, key: &str) -> PathBuf {
        // Sanitize the key to prevent path traversal. 
        // Simple sanitization: remove any characters that aren't alphanumeric, underscores, or hyphens.
        let sanitized_key: String = key.chars()
            .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
            .collect();
        self.base_path.join(format!("{}.json", sanitized_key))
    }
}
