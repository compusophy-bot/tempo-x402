use rusqlite::{params, Connection, OptionalExtension};
use std::sync::Mutex;

use crate::error::GatewayError;

/// Endpoint registration record
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Endpoint {
    pub id: i64,
    pub slug: String,
    pub owner_address: String,
    pub target_url: String,
    /// Human-readable price (e.g., "$0.01")
    pub price_usd: String,
    /// Token amount (e.g., "10000" for $0.01 with 6 decimals)
    pub price_amount: String,
    pub description: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub active: bool,
}

/// Request to create a new endpoint
#[derive(Debug, serde::Deserialize)]
pub struct CreateEndpoint {
    pub slug: String,
    pub target_url: String,
    #[serde(default = "default_price")]
    pub price: String,
    pub description: Option<String>,
}

fn default_price() -> String {
    "$0.01".to_string()
}

/// Request to update an endpoint
#[derive(Debug, serde::Deserialize)]
pub struct UpdateEndpoint {
    pub target_url: Option<String>,
    pub price: Option<String>,
    pub description: Option<String>,
}

/// SQLite database wrapper
pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub fn new(path: &str) -> Result<Self, GatewayError> {
        let conn = Connection::open(path)?;
        let db = Self {
            conn: Mutex::new(conn),
        };
        db.init_schema()?;
        Ok(db)
    }

    fn init_schema(&self) -> Result<(), GatewayError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| GatewayError::Internal("database lock poisoned".to_string()))?;
        conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS endpoints (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                slug TEXT UNIQUE NOT NULL,
                owner_address TEXT NOT NULL,
                target_url TEXT NOT NULL,
                price_usd TEXT NOT NULL DEFAULT '$0.01',
                price_amount TEXT NOT NULL,
                description TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                active INTEGER NOT NULL DEFAULT 1
            )
            "#,
            [],
        )?;

        // Create index on slug for fast lookups
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_endpoints_slug ON endpoints(slug)",
            [],
        )?;

        // Create index on owner_address for listing owned endpoints
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_endpoints_owner ON endpoints(owner_address)",
            [],
        )?;

        Ok(())
    }

    /// Insert a new endpoint
    pub fn create_endpoint(
        &self,
        slug: &str,
        owner_address: &str,
        target_url: &str,
        price_usd: &str,
        price_amount: &str,
        description: Option<&str>,
    ) -> Result<Endpoint, GatewayError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| GatewayError::Internal("database lock poisoned".to_string()))?;
        let now = chrono::Utc::now().timestamp();

        conn.execute(
            r#"
            INSERT INTO endpoints (slug, owner_address, target_url, price_usd, price_amount, description, created_at, updated_at, active)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1)
            "#,
            params![slug, owner_address, target_url, price_usd, price_amount, description, now, now],
        )?;

        let id = conn.last_insert_rowid();

        Ok(Endpoint {
            id,
            slug: slug.to_string(),
            owner_address: owner_address.to_string(),
            target_url: target_url.to_string(),
            price_usd: price_usd.to_string(),
            price_amount: price_amount.to_string(),
            description: description.map(String::from),
            created_at: now,
            updated_at: now,
            active: true,
        })
    }

    /// Get endpoint by slug
    pub fn get_endpoint(&self, slug: &str) -> Result<Option<Endpoint>, GatewayError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| GatewayError::Internal("database lock poisoned".to_string()))?;

        let endpoint = conn
            .query_row(
                r#"
                SELECT id, slug, owner_address, target_url, price_usd, price_amount, description, created_at, updated_at, active
                FROM endpoints
                WHERE slug = ?1 AND active = 1
                "#,
                params![slug],
                |row| {
                    Ok(Endpoint {
                        id: row.get(0)?,
                        slug: row.get(1)?,
                        owner_address: row.get(2)?,
                        target_url: row.get(3)?,
                        price_usd: row.get(4)?,
                        price_amount: row.get(5)?,
                        description: row.get(6)?,
                        created_at: row.get(7)?,
                        updated_at: row.get(8)?,
                        active: row.get::<_, i32>(9)? == 1,
                    })
                },
            )
            .optional()?;

        Ok(endpoint)
    }

    /// List active endpoints with pagination.
    /// `limit` defaults to 100, max 500. `offset` defaults to 0.
    pub fn list_endpoints(&self, limit: u32, offset: u32) -> Result<Vec<Endpoint>, GatewayError> {
        let limit = limit.clamp(1, 500);
        let conn = self
            .conn
            .lock()
            .map_err(|_| GatewayError::Internal("database lock poisoned".to_string()))?;

        let mut stmt = conn.prepare(
            r#"
            SELECT id, slug, owner_address, target_url, price_usd, price_amount, description, created_at, updated_at, active
            FROM endpoints
            WHERE active = 1
            ORDER BY created_at DESC
            LIMIT ?1 OFFSET ?2
            "#,
        )?;

        let endpoints = stmt
            .query_map(params![limit, offset], |row| {
                Ok(Endpoint {
                    id: row.get(0)?,
                    slug: row.get(1)?,
                    owner_address: row.get(2)?,
                    target_url: row.get(3)?,
                    price_usd: row.get(4)?,
                    price_amount: row.get(5)?,
                    description: row.get(6)?,
                    created_at: row.get(7)?,
                    updated_at: row.get(8)?,
                    active: row.get::<_, i32>(9)? == 1,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(endpoints)
    }

    /// Update an endpoint
    pub fn update_endpoint(
        &self,
        slug: &str,
        target_url: Option<&str>,
        price_usd: Option<&str>,
        price_amount: Option<&str>,
        description: Option<&str>,
    ) -> Result<Endpoint, GatewayError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| GatewayError::Internal("database lock poisoned".to_string()))?;
        let now = chrono::Utc::now().timestamp();

        // Build update query dynamically
        let mut updates = vec!["updated_at = ?1".to_string()];
        let mut param_idx = 2;

        if target_url.is_some() {
            updates.push(format!("target_url = ?{}", param_idx));
            param_idx += 1;
        }
        if price_usd.is_some() {
            updates.push(format!("price_usd = ?{}", param_idx));
            param_idx += 1;
        }
        if price_amount.is_some() {
            updates.push(format!("price_amount = ?{}", param_idx));
            param_idx += 1;
        }
        if description.is_some() {
            updates.push(format!("description = ?{}", param_idx));
            param_idx += 1;
        }

        let query = format!(
            "UPDATE endpoints SET {} WHERE slug = ?{} AND active = 1",
            updates.join(", "),
            param_idx
        );

        // Build params dynamically
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(now)];
        if let Some(v) = target_url {
            params_vec.push(Box::new(v.to_string()));
        }
        if let Some(v) = price_usd {
            params_vec.push(Box::new(v.to_string()));
        }
        if let Some(v) = price_amount {
            params_vec.push(Box::new(v.to_string()));
        }
        if let Some(v) = description {
            params_vec.push(Box::new(v.to_string()));
        }
        params_vec.push(Box::new(slug.to_string()));

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();

        let rows_affected = conn.execute(&query, params_refs.as_slice())?;

        if rows_affected == 0 {
            return Err(GatewayError::EndpointNotFound(slug.to_string()));
        }

        // Fetch updated endpoint using the already-held connection (avoids deadlock)
        let endpoint = conn
            .query_row(
                r#"
                SELECT id, slug, owner_address, target_url, price_usd, price_amount, description, created_at, updated_at, active
                FROM endpoints
                WHERE slug = ?1 AND active = 1
                "#,
                params![slug],
                |row| {
                    Ok(Endpoint {
                        id: row.get(0)?,
                        slug: row.get(1)?,
                        owner_address: row.get(2)?,
                        target_url: row.get(3)?,
                        price_usd: row.get(4)?,
                        price_amount: row.get(5)?,
                        description: row.get(6)?,
                        created_at: row.get(7)?,
                        updated_at: row.get(8)?,
                        active: row.get::<_, i32>(9)? == 1,
                    })
                },
            )
            .optional()?
            .ok_or_else(|| GatewayError::EndpointNotFound(slug.to_string()))?;

        Ok(endpoint)
    }

    /// Deactivate an endpoint (soft delete)
    pub fn delete_endpoint(&self, slug: &str) -> Result<(), GatewayError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| GatewayError::Internal("database lock poisoned".to_string()))?;
        let now = chrono::Utc::now().timestamp();

        let rows_affected = conn.execute(
            "UPDATE endpoints SET active = 0, updated_at = ?1 WHERE slug = ?2 AND active = 1",
            params![now, slug],
        )?;

        if rows_affected == 0 {
            return Err(GatewayError::EndpointNotFound(slug.to_string()));
        }

        Ok(())
    }

    /// Check if slug exists (only active endpoints, allowing reuse of deleted slugs)
    pub fn slug_exists(&self, slug: &str) -> Result<bool, GatewayError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| GatewayError::Internal("database lock poisoned".to_string()))?;

        let count: i32 = conn.query_row(
            "SELECT COUNT(*) FROM endpoints WHERE slug = ?1 AND active = 1",
            params![slug],
            |row| row.get(0),
        )?;

        Ok(count > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_get_endpoint() {
        let db = Database::new(":memory:").unwrap();

        let endpoint = db
            .create_endpoint(
                "test-api",
                "0x1234567890123456789012345678901234567890",
                "https://api.example.com",
                "$0.05",
                "50000",
                Some("Test API"),
            )
            .unwrap();

        assert_eq!(endpoint.slug, "test-api");
        assert_eq!(endpoint.price_usd, "$0.05");

        let fetched = db.get_endpoint("test-api").unwrap().unwrap();
        assert_eq!(fetched.slug, endpoint.slug);
    }

    #[test]
    fn test_list_endpoints() {
        let db = Database::new(":memory:").unwrap();

        db.create_endpoint(
            "api-1",
            "0x1111111111111111111111111111111111111111",
            "https://api1.example.com",
            "$0.01",
            "10000",
            None,
        )
        .unwrap();

        db.create_endpoint(
            "api-2",
            "0x2222222222222222222222222222222222222222",
            "https://api2.example.com",
            "$0.02",
            "20000",
            None,
        )
        .unwrap();

        let endpoints = db.list_endpoints(100, 0).unwrap();
        assert_eq!(endpoints.len(), 2);
    }

    #[test]
    fn test_delete_endpoint() {
        let db = Database::new(":memory:").unwrap();

        db.create_endpoint(
            "to-delete",
            "0x1234567890123456789012345678901234567890",
            "https://api.example.com",
            "$0.01",
            "10000",
            None,
        )
        .unwrap();

        db.delete_endpoint("to-delete").unwrap();

        let result = db.get_endpoint("to-delete").unwrap();
        assert!(result.is_none());
    }
}
