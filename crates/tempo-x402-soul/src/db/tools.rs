// Dynamic tools CRUD operations.
use super::*;

impl SoulDatabase {
    /// Insert or update a dynamic tool.
    pub fn insert_tool(&self, tool: &DynamicTool) -> Result<(), SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;

        conn.execute(
            "INSERT INTO tools (name, description, parameters, handler_type, handler_config, enabled, mode_tags, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9) \
             ON CONFLICT(name) DO UPDATE SET description=?2, parameters=?3, handler_type=?4, handler_config=?5, enabled=?6, mode_tags=?7, updated_at=?9",
            params![
                tool.name,
                tool.description,
                tool.parameters,
                tool.handler_type,
                tool.handler_config,
                tool.enabled as i32,
                tool.mode_tags,
                tool.created_at,
                tool.updated_at,
            ],
        )?;
        Ok(())
    }

    /// Get a dynamic tool by name.
    pub fn get_tool(&self, name: &str) -> Result<Option<DynamicTool>, SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;

        let tool = conn
            .query_row(
                "SELECT name, description, parameters, handler_type, handler_config, enabled, mode_tags, created_at, updated_at \
                 FROM tools WHERE name = ?1",
                params![name],
                |row| {
                    let enabled: i32 = row.get(5)?;
                    Ok(DynamicTool {
                        name: row.get(0)?,
                        description: row.get(1)?,
                        parameters: row.get(2)?,
                        handler_type: row.get(3)?,
                        handler_config: row.get(4)?,
                        enabled: enabled != 0,
                        mode_tags: row.get(6)?,
                        created_at: row.get(7)?,
                        updated_at: row.get(8)?,
                    })
                },
            )
            .optional()?;

        Ok(tool)
    }

    /// List all dynamic tools (enabled only by default).
    pub fn list_tools(&self, enabled_only: bool) -> Result<Vec<DynamicTool>, SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;

        let query = if enabled_only {
            "SELECT name, description, parameters, handler_type, handler_config, enabled, mode_tags, created_at, updated_at \
             FROM tools WHERE enabled = 1 ORDER BY name"
        } else {
            "SELECT name, description, parameters, handler_type, handler_config, enabled, mode_tags, created_at, updated_at \
             FROM tools ORDER BY name"
        };

        let mut stmt = conn.prepare(query)?;
        let tools = stmt
            .query_map([], |row| {
                let enabled: i32 = row.get(5)?;
                Ok(DynamicTool {
                    name: row.get(0)?,
                    description: row.get(1)?,
                    parameters: row.get(2)?,
                    handler_type: row.get(3)?,
                    handler_config: row.get(4)?,
                    enabled: enabled != 0,
                    mode_tags: row.get(6)?,
                    created_at: row.get(7)?,
                    updated_at: row.get(8)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(tools)
    }

    /// Delete a dynamic tool by name. Returns true if a row was deleted.
    pub fn delete_tool(&self, name: &str) -> Result<bool, SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;

        let rows = conn.execute("DELETE FROM tools WHERE name = ?1", params![name])?;
        Ok(rows > 0)
    }

    /// Count enabled dynamic tools.
    pub fn count_tools(&self) -> Result<u32, SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;

        let count: u32 =
            conn.query_row("SELECT COUNT(*) FROM tools WHERE enabled = 1", [], |row| {
                row.get(0)
            })?;
        Ok(count)
    }
}
