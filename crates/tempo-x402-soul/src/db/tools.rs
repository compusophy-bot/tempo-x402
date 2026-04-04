// Dynamic tools CRUD operations.
use super::*;

impl SoulDatabase {
    /// Insert or update a dynamic tool.
    pub fn insert_tool(&self, tool: &DynamicTool) -> Result<(), SoulError> {
        let value = serde_json::to_vec(tool)?;
        self.tools.insert(tool.name.as_bytes(), value)?;
        Ok(())
    }

    /// Get a dynamic tool by name.
    pub fn get_tool(&self, name: &str) -> Result<Option<DynamicTool>, SoulError> {
        match self.tools.get(name.as_bytes())? {
            Some(v) => {
                let tool: DynamicTool = serde_json::from_slice(&v)?;
                Ok(Some(tool))
            }
            None => Ok(None),
        }
    }

    /// List all dynamic tools (enabled only by default).
    pub fn list_tools(&self, enabled_only: bool) -> Result<Vec<DynamicTool>, SoulError> {
        let mut tools: Vec<DynamicTool> = self
            .tools
            .iter()
            .filter_map(|res| {
                let (_k, v) = res.ok()?;
                let t: DynamicTool = serde_json::from_slice(&v).ok()?;
                if enabled_only && !t.enabled {
                    None
                } else {
                    Some(t)
                }
            })
            .collect();
        tools.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(tools)
    }

    /// Delete a dynamic tool by name. Returns true if a row was deleted.
    pub fn delete_tool(&self, name: &str) -> Result<bool, SoulError> {
        let removed = self.tools.remove(name.as_bytes())?;
        Ok(removed.is_some())
    }

    /// Count enabled dynamic tools.
    pub fn count_tools(&self) -> Result<u32, SoulError> {
        let count = self
            .tools
            .iter()
            .filter_map(|res| {
                let (_k, v) = res.ok()?;
                let t: DynamicTool = serde_json::from_slice(&v).ok()?;
                if t.enabled { Some(()) } else { None }
            })
            .count();
        Ok(count as u32)
    }
}
