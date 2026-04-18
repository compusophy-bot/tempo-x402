//! Soul state key-value CRUD methods (sled-backed, lock-free).
use super::*;

impl SoulDatabase {
    /// Get a soul state value by key.
    pub fn get_state(&self, key: &str) -> Result<Option<String>, SoulError> {
        match self.state.get(key.as_bytes())? {
            Some(bytes) => Ok(Some(
                String::from_utf8(bytes.to_vec())
                    .unwrap_or_else(|_| String::from("<invalid utf8>")),
            )),
            None => Ok(None),
        }
    }

    /// Get all soul state key-value pairs.
    pub fn get_all_state(&self) -> Result<Vec<(String, String)>, SoulError> {
        let mut pairs = Vec::new();
        for entry in self.state.iter() {
            let (key, value) = entry?;
            let key_str = String::from_utf8(key.to_vec()).unwrap_or_default();
            let value_str = String::from_utf8(value.to_vec()).unwrap_or_default();
            pairs.push((key_str, value_str));
        }
        Ok(pairs)
    }

    /// Set a soul state value (upsert). Lock-free — safe from any thread.
    pub fn set_state(&self, key: &str, value: &str) -> Result<(), SoulError> {
        self.state.insert(key.as_bytes(), value.as_bytes())?;
        Ok(())
    }
}
