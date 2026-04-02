//! Children table extension for the gateway database.

use rusqlite::params;
use rusqlite::OptionalExtension;
use x402_gateway::db::Database;
use x402_gateway::error::GatewayError;

/// Child instance record
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChildInstance {
    pub id: i64,
    pub instance_id: String,
    pub address: String,
    pub url: Option<String>,
    pub railway_service_id: Option<String>,
    pub funded_amount: Option<String>,
    pub funding_tx: Option<String>,
    pub status: String,
    pub branch: Option<String>,
    /// Railway volume ID — MUST be deleted separately when service is deleted.
    pub volume_id: Option<String>,
    /// Borg-style ordinal designation: "one", "two", "three", etc.
    pub designation: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

const CHILDREN_SCHEMA: &str = r#"
    CREATE TABLE IF NOT EXISTS children (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        instance_id TEXT UNIQUE NOT NULL,
        address TEXT NOT NULL DEFAULT '',
        url TEXT,
        railway_service_id TEXT,
        funded_amount TEXT,
        funding_tx TEXT,
        status TEXT NOT NULL DEFAULT 'pending',
        branch TEXT,
        volume_id TEXT,
        created_at INTEGER NOT NULL,
        updated_at INTEGER NOT NULL
    );
    CREATE INDEX IF NOT EXISTS idx_children_instance_id ON children(instance_id);
"#;

/// Migrate: add `branch` column if it doesn't exist (backward-compatible).
fn migrate_add_branch_column(db: &Database) -> Result<(), GatewayError> {
    db.with_connection(|conn| {
        // Check if column exists
        let has_column: bool = conn
            .prepare("PRAGMA table_info(children)")?
            .query_map([], |row| row.get::<_, String>(1))?
            .filter_map(|r| r.ok())
            .any(|name| name == "branch");

        if !has_column {
            conn.execute_batch("ALTER TABLE children ADD COLUMN branch TEXT;")?;
            tracing::info!("Migrated children table: added `branch` column");
        }
        Ok(())
    })
}

/// Migrate: add `volume_id` column if it doesn't exist (backward-compatible).
fn migrate_add_volume_id_column(db: &Database) -> Result<(), GatewayError> {
    db.with_connection(|conn| {
        let has_column: bool = conn
            .prepare("PRAGMA table_info(children)")?
            .query_map([], |row| row.get::<_, String>(1))?
            .filter_map(|r| r.ok())
            .any(|name| name == "volume_id");

        if !has_column {
            conn.execute_batch("ALTER TABLE children ADD COLUMN volume_id TEXT;")?;
            tracing::info!("Migrated children table: added `volume_id` column");
        }
        Ok(())
    })
}

/// Migrate: add `designation` column if it doesn't exist.
fn migrate_add_designation_column(db: &Database) -> Result<(), GatewayError> {
    db.with_connection(|conn| {
        let has_column: bool = conn
            .prepare("PRAGMA table_info(children)")?
            .query_map([], |row| row.get::<_, String>(1))?
            .filter_map(|r| r.ok())
            .any(|name| name == "designation");

        if !has_column {
            conn.execute_batch("ALTER TABLE children ADD COLUMN designation TEXT;")?;
            tracing::info!("Migrated children table: added `designation` column");
        }
        Ok(())
    })
}

/// Initialize the children table on an existing gateway database.
pub fn init_children_schema(db: &Database) -> Result<(), GatewayError> {
    db.execute_schema(CHILDREN_SCHEMA)?;
    migrate_add_branch_column(db)?;
    migrate_add_volume_id_column(db)?;
    migrate_add_designation_column(db)?;
    Ok(())
}

/// Get the next lineage-based designation.
/// Parent is "borg-0", children are "borg-0-1", "borg-0-2", etc.
/// A child of "borg-0-1" would spawn "borg-0-1-1", "borg-0-1-2", etc.
pub fn next_designation(db: &Database) -> Result<String, GatewayError> {
    let parent_designation = std::env::var("DRONE_DESIGNATION")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "borg-0".to_string());

    db.with_connection(|conn| {
        let count: u32 = conn
            .query_row(
                "SELECT COUNT(*) FROM children WHERE status != 'failed'",
                [],
                |row| row.get(0),
            )
            .map_err(|e| GatewayError::Internal(format!("count query failed: {e}")))?;

        let child_number = count + 1;
        Ok(format!("{}-{}", parent_designation, child_number))
    })
}

/// Store the volume_id for a child instance.
pub fn set_child_volume_id(
    db: &Database,
    instance_id: &str,
    volume_id: &str,
) -> Result<(), GatewayError> {
    let now = chrono::Utc::now().timestamp();
    db.with_connection(|conn| {
        conn.execute(
            "UPDATE children SET volume_id = ?1, updated_at = ?2 WHERE instance_id = ?3",
            params![volume_id, now, instance_id],
        )?;
        Ok(())
    })
}

/// Atomically check the children count is under the limit, then insert.
/// Returns `Ok(true)` if under limit (row inserted), `Ok(false)` if limit reached.
/// This prevents the TOCTOU race where two concurrent requests both pass the check.
#[allow(dead_code)]
pub fn create_child_if_under_limit(
    db: &Database,
    max_children: u32,
    instance_id: &str,
    url: Option<&str>,
    railway_service_id: Option<&str>,
) -> Result<bool, GatewayError> {
    let now = chrono::Utc::now().timestamp();

    db.with_connection(|conn| {
        // Use BEGIN IMMEDIATE to acquire a write lock atomically
        conn.execute_batch("BEGIN IMMEDIATE")?;

        let count: u32 = conn
            .query_row("SELECT COUNT(*) FROM children", [], |row| row.get(0))
            .map_err(|e| GatewayError::Internal(format!("count query failed: {e}")))?;

        if count >= max_children {
            conn.execute_batch("ROLLBACK")?;
            return Ok(false);
        }

        conn.execute(
            "INSERT INTO children (instance_id, url, railway_service_id, status, created_at, updated_at) \
             VALUES (?1, ?2, ?3, 'pending', ?4, ?5)",
            params![instance_id, url, railway_service_id, now, now],
        )?;

        conn.execute_batch("COMMIT")?;
        Ok(true)
    })
}

/// Reserve a child slot atomically before starting Railway deployment.
/// Inserts a row with status `'queued'` if under the limit.
/// Returns `Ok(true)` if reserved, `Ok(false)` if limit reached.
pub fn reserve_child_slot(
    db: &Database,
    max_children: u32,
    instance_id: &str,
) -> Result<bool, GatewayError> {
    let now = chrono::Utc::now().timestamp();

    db.with_connection(|conn| {
        conn.execute_batch("BEGIN IMMEDIATE")?;

        let count: u32 = conn
            .query_row(
                "SELECT COUNT(*) FROM children WHERE status != 'failed'",
                [],
                |row| row.get(0),
            )
            .map_err(|e| GatewayError::Internal(format!("count query failed: {e}")))?;

        if count >= max_children {
            conn.execute_batch("ROLLBACK")?;
            return Ok(false);
        }

        conn.execute(
            "INSERT INTO children (instance_id, status, created_at, updated_at) \
             VALUES (?1, 'queued', ?2, ?3)",
            params![instance_id, now, now],
        )?;

        conn.execute_batch("COMMIT")?;
        Ok(true)
    })
}

/// Update a child after Railway deployment succeeds.
/// Fills in the URL, service ID, branch, and transitions status to `'deploying'`.
pub fn update_child_deployment(
    db: &Database,
    instance_id: &str,
    url: &str,
    railway_service_id: &str,
    status: &str,
    branch: Option<&str>,
) -> Result<(), GatewayError> {
    let now = chrono::Utc::now().timestamp();

    db.with_connection(|conn| {
        conn.execute(
            "UPDATE children SET url = ?1, railway_service_id = ?2, status = ?3, \
             branch = COALESCE(?4, branch), updated_at = ?5 \
             WHERE instance_id = ?6",
            params![url, railway_service_id, status, branch, now, instance_id],
        )?;
        Ok(())
    })
}

/// Mark a child as failed after a deploy error or cleanup.
pub fn mark_child_failed(db: &Database, instance_id: &str) -> Result<(), GatewayError> {
    let now = chrono::Utc::now().timestamp();

    db.with_connection(|conn| {
        conn.execute(
            "UPDATE children SET status = 'failed', updated_at = ?1 WHERE instance_id = ?2",
            params![now, instance_id],
        )?;
        Ok(())
    })
}

/// Look up a single child by instance_id.
pub fn get_child_by_instance_id(
    db: &Database,
    instance_id: &str,
) -> Result<Option<ChildInstance>, GatewayError> {
    db.with_connection(|conn| {
        let mut stmt = conn
            .prepare(
                "SELECT id, instance_id, address, url, railway_service_id, \
                 funded_amount, funding_tx, status, branch, volume_id, designation, created_at, updated_at \
                 FROM children WHERE instance_id = ?1",
            )
            .map_err(|e| GatewayError::Internal(format!("prepare failed: {e}")))?;

        let result = stmt
            .query_row(params![instance_id], |row| {
                Ok(ChildInstance {
                    id: row.get(0)?,
                    instance_id: row.get(1)?,
                    address: row.get(2)?,
                    url: row.get(3)?,
                    railway_service_id: row.get(4)?,
                    funded_amount: row.get(5)?,
                    funding_tx: row.get(6)?,
                    status: row.get(7)?,
                    branch: row.get(8)?,
                    volume_id: row.get(9)?,
                    designation: row.get(10)?,
                    created_at: row.get(11)?,
                    updated_at: row.get(12)?,
                })
            })
            .optional()
            .map_err(|e| GatewayError::Internal(format!("query failed: {e}")))?;

        Ok(result)
    })
}

/// Update a child instance when it registers back, using parameterized queries.
pub fn update_child(
    db: &Database,
    instance_id: &str,
    address: Option<&str>,
    url: Option<&str>,
    status: Option<&str>,
) -> Result<(), GatewayError> {
    let now = chrono::Utc::now().timestamp();

    db.with_connection(|conn| {
        conn.execute(
            "UPDATE children SET \
             address = COALESCE(?1, address), \
             url = COALESCE(?2, url), \
             status = COALESCE(?3, status), \
             updated_at = ?4 \
             WHERE instance_id = ?5",
            params![address, url, status, now, instance_id],
        )?;
        Ok(())
    })
}

// Read operations use direct rusqlite connections opened from NodeState's db_path,
// or with_connection for consistency.

/// Query all children (including failed) from a direct rusqlite connection.
#[allow(dead_code)]
pub fn query_children(conn: &rusqlite::Connection) -> Result<Vec<ChildInstance>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        r#"
        SELECT id, instance_id, address, url, railway_service_id,
               funded_amount, funding_tx, status, branch, volume_id, designation, created_at, updated_at
        FROM children
        ORDER BY created_at DESC
        "#,
    )?;

    let children = stmt
        .query_map([], |row| {
            Ok(ChildInstance {
                id: row.get(0)?,
                instance_id: row.get(1)?,
                address: row.get(2)?,
                url: row.get(3)?,
                railway_service_id: row.get(4)?,
                funded_amount: row.get(5)?,
                funding_tx: row.get(6)?,
                status: row.get(7)?,
                branch: row.get(8)?,
                volume_id: row.get(9)?,
                designation: row.get(10)?,
                created_at: row.get(11)?,
                updated_at: row.get(12)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(children)
}

/// Query only active (non-failed) children from a direct rusqlite connection.
pub fn query_children_active(
    conn: &rusqlite::Connection,
) -> Result<Vec<ChildInstance>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        r#"
        SELECT id, instance_id, address, url, railway_service_id,
               funded_amount, funding_tx, status, branch, volume_id, designation, created_at, updated_at
        FROM children
        WHERE status != 'failed'
        ORDER BY created_at DESC
        "#,
    )?;

    let children = stmt
        .query_map([], |row| {
            Ok(ChildInstance {
                id: row.get(0)?,
                instance_id: row.get(1)?,
                address: row.get(2)?,
                url: row.get(3)?,
                railway_service_id: row.get(4)?,
                funded_amount: row.get(5)?,
                funding_tx: row.get(6)?,
                status: row.get(7)?,
                branch: row.get(8)?,
                volume_id: row.get(9)?,
                designation: row.get(10)?,
                created_at: row.get(11)?,
                updated_at: row.get(12)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(children)
}

/// List active (non-failed) children via the Database wrapper (consistent with writes).
pub fn list_children_active(db: &Database) -> Result<Vec<ChildInstance>, GatewayError> {
    db.with_connection(|conn| {
        let mut stmt = conn.prepare(
            r#"
            SELECT id, instance_id, address, url, railway_service_id,
                   funded_amount, funding_tx, status, branch, volume_id, designation, created_at, updated_at
            FROM children
            WHERE status NOT IN ('failed', 'unreachable')
            ORDER BY created_at DESC
            "#,
        )?;

        let children = stmt
            .query_map([], |row| {
                Ok(ChildInstance {
                    id: row.get(0)?,
                    instance_id: row.get(1)?,
                    address: row.get(2)?,
                    url: row.get(3)?,
                    railway_service_id: row.get(4)?,
                    funded_amount: row.get(5)?,
                    funding_tx: row.get(6)?,
                    status: row.get(7)?,
                    branch: row.get(8)?,
                    volume_id: row.get(9)?,
                    designation: row.get(10)?,
                    created_at: row.get(11)?,
                    updated_at: row.get(12)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(children)
    })
}

/// Delete a child record only if its status is 'failed'.
/// Returns `Ok(true)` if a row was deleted, `Ok(false)` if not found or not failed.
pub fn delete_failed_child(db: &Database, instance_id: &str) -> Result<bool, GatewayError> {
    db.with_connection(|conn| {
        let rows = conn.execute(
            "DELETE FROM children WHERE instance_id = ?1 AND status = 'failed'",
            params![instance_id],
        )?;
        Ok(rows > 0)
    })
}

/// Force-delete a child record regardless of status.
/// Returns `Ok(true)` if a row was deleted, `Ok(false)` if not found.
pub fn delete_child(db: &Database, instance_id: &str) -> Result<bool, GatewayError> {
    db.with_connection(|conn| {
        let rows = conn.execute(
            "DELETE FROM children WHERE instance_id = ?1",
            params![instance_id],
        )?;
        Ok(rows > 0)
    })
}

/// Update a child's status.
pub fn update_child_status(
    db: &Database,
    instance_id: &str,
    status: &str,
) -> Result<(), GatewayError> {
    let now = chrono::Utc::now().timestamp();

    db.with_connection(|conn| {
        conn.execute(
            "UPDATE children SET status = ?1, updated_at = ?2 WHERE instance_id = ?3",
            params![status, now, instance_id],
        )?;
        Ok(())
    })
}

/// Insert or update a linked peer (manually linked, not cloned).
/// Uses INSERT OR REPLACE to handle re-linking gracefully.
pub fn link_peer(
    db: &Database,
    instance_id: &str,
    address: &str,
    url: &str,
) -> Result<(), GatewayError> {
    let now = chrono::Utc::now().timestamp();

    db.with_connection(|conn| {
        conn.execute(
            "INSERT INTO children (instance_id, address, url, status, created_at, updated_at) \
             VALUES (?1, ?2, ?3, 'running', ?4, ?5) \
             ON CONFLICT(instance_id) DO UPDATE SET \
             address = excluded.address, url = excluded.url, \
             status = 'running', updated_at = excluded.updated_at",
            params![instance_id, address, url, now, now],
        )?;
        Ok(())
    })
}

/// Mark a child as unreachable (soft failure — may recover on next probe).
pub fn mark_child_unreachable(db: &Database, instance_id: &str) -> Result<(), GatewayError> {
    let now = chrono::Utc::now().timestamp();

    db.with_connection(|conn| {
        conn.execute(
            "UPDATE children SET status = 'unreachable', updated_at = ?1 WHERE instance_id = ?2",
            params![now, instance_id],
        )?;
        Ok(())
    })
}

/// Delete children that have been unreachable for longer than `max_age_secs`.
/// Returns the number of rows deleted.
pub fn prune_unreachable_children(db: &Database, max_age_secs: i64) -> Result<u32, GatewayError> {
    let cutoff = chrono::Utc::now().timestamp() - max_age_secs;

    db.with_connection(|conn| {
        let rows = conn.execute(
            "DELETE FROM children WHERE status = 'unreachable' AND updated_at < ?1",
            params![cutoff],
        )?;
        Ok(rows as u32)
    })
}

/// Count children from a direct rusqlite connection.
#[allow(dead_code)]
pub fn query_children_count(conn: &rusqlite::Connection) -> Result<u32, rusqlite::Error> {
    conn.query_row("SELECT COUNT(*) FROM children", params![], |row| row.get(0))
}

// ── Cartridges ──────────────────────────────────────────────────────

const CARTRIDGES_SCHEMA: &str = r#"
    CREATE TABLE IF NOT EXISTS cartridges (
        slug TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        description TEXT,
        version TEXT NOT NULL DEFAULT '0.1.0',
        price_usd TEXT NOT NULL DEFAULT '$0.001',
        price_amount TEXT NOT NULL DEFAULT '1000',
        owner_address TEXT NOT NULL DEFAULT '',
        source_repo TEXT,
        wasm_path TEXT NOT NULL,
        wasm_hash TEXT NOT NULL,
        active INTEGER NOT NULL DEFAULT 1,
        created_at INTEGER NOT NULL,
        updated_at INTEGER NOT NULL
    );
    CREATE INDEX IF NOT EXISTS idx_cartridges_active ON cartridges(active);

    CREATE TABLE IF NOT EXISTS cartridge_kv (
        slug TEXT NOT NULL,
        key TEXT NOT NULL,
        value TEXT,
        updated_at INTEGER NOT NULL,
        PRIMARY KEY (slug, key)
    );
"#;

/// Initialize the cartridges tables on an existing gateway database.
pub fn init_cartridges_schema(db: &Database) -> Result<(), GatewayError> {
    db.execute_schema(CARTRIDGES_SCHEMA)?;
    // Migration: add cartridge_type column if missing
    db.with_connection(|conn| {
        let has_col = conn
            .prepare("SELECT cartridge_type FROM cartridges LIMIT 0")
            .is_ok();
        if !has_col {
            conn.execute(
                "ALTER TABLE cartridges ADD COLUMN cartridge_type TEXT NOT NULL DEFAULT 'backend'",
                [],
            )
            .map_err(|e| GatewayError::Internal(format!("migration: {e}")))?;
        }
        Ok(())
    })
}

/// A registered WASM cartridge.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CartridgeRecord {
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub version: String,
    pub price_usd: String,
    pub price_amount: String,
    pub owner_address: String,
    pub source_repo: Option<String>,
    pub wasm_path: String,
    pub wasm_hash: String,
    pub active: bool,
    pub created_at: i64,
    pub updated_at: i64,
    /// "backend", "interactive", or "frontend"
    #[serde(default = "default_cartridge_type")]
    pub cartridge_type: String,
}

fn default_cartridge_type() -> String {
    "backend".to_string()
}

/// Register or update a cartridge in the database.
pub fn upsert_cartridge(db: &Database, record: &CartridgeRecord) -> Result<(), GatewayError> {
    db.with_connection(|conn| {
        conn.execute(
            "INSERT INTO cartridges (slug, name, description, version, price_usd, price_amount, \
             owner_address, source_repo, wasm_path, wasm_hash, active, created_at, updated_at, cartridge_type) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14) \
             ON CONFLICT(slug) DO UPDATE SET \
             name=?2, description=?3, version=?4, price_usd=?5, price_amount=?6, \
             wasm_path=?9, wasm_hash=?10, active=?11, updated_at=?13, cartridge_type=?14",
            params![
                record.slug,
                record.name,
                record.description,
                record.version,
                record.price_usd,
                record.price_amount,
                record.owner_address,
                record.source_repo,
                record.wasm_path,
                record.wasm_hash,
                record.active as i32,
                record.created_at,
                record.updated_at,
                record.cartridge_type,
            ],
        )
        .map_err(|e| GatewayError::Internal(format!("upsert cartridge: {e}")))?;
        Ok(())
    })
}

/// Check if a cartridge slug exists in DB (active or not). Used by auto-register
/// to avoid resurrecting soft-deleted cartridges.
pub fn get_cartridge_any(db: &Database, slug: &str) -> Result<Option<String>, GatewayError> {
    db.with_connection(|conn| {
        conn.query_row(
            "SELECT slug FROM cartridges WHERE slug = ?1",
            params![slug],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| GatewayError::Internal(format!("get cartridge any: {e}")))
    })
}

/// Get a cartridge by slug.
pub fn get_cartridge(db: &Database, slug: &str) -> Result<Option<CartridgeRecord>, GatewayError> {
    db.with_connection(|conn| {
        conn.query_row(
            "SELECT slug, name, description, version, price_usd, price_amount, \
             owner_address, source_repo, wasm_path, wasm_hash, active, created_at, updated_at, \
             COALESCE(cartridge_type, 'backend') \
             FROM cartridges WHERE slug = ?1 AND active = 1",
            params![slug],
            |row| {
                Ok(CartridgeRecord {
                    slug: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    version: row.get(3)?,
                    price_usd: row.get(4)?,
                    price_amount: row.get(5)?,
                    owner_address: row.get(6)?,
                    source_repo: row.get(7)?,
                    wasm_path: row.get(8)?,
                    wasm_hash: row.get(9)?,
                    active: row.get::<_, i32>(10)? != 0,
                    created_at: row.get(11)?,
                    updated_at: row.get(12)?,
                    cartridge_type: row.get::<_, String>(13).unwrap_or_else(|_| "backend".to_string()),
                })
            },
        )
        .optional()
        .map_err(|e| GatewayError::Internal(format!("get cartridge: {e}")))
    })
}

/// List all active cartridges.
pub fn list_cartridges(db: &Database) -> Result<Vec<CartridgeRecord>, GatewayError> {
    db.with_connection(|conn| {
        let mut stmt = conn
            .prepare(
                "SELECT slug, name, description, version, price_usd, price_amount, \
                 owner_address, source_repo, wasm_path, wasm_hash, active, created_at, updated_at, \
                 COALESCE(cartridge_type, 'backend') \
                 FROM cartridges WHERE active = 1 ORDER BY created_at DESC",
            )
            .map_err(|e| GatewayError::Internal(format!("list cartridges: {e}")))?;

        let rows = stmt
            .query_map([], |row| {
                Ok(CartridgeRecord {
                    slug: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    version: row.get(3)?,
                    price_usd: row.get(4)?,
                    price_amount: row.get(5)?,
                    owner_address: row.get(6)?,
                    source_repo: row.get(7)?,
                    wasm_path: row.get(8)?,
                    wasm_hash: row.get(9)?,
                    active: row.get::<_, i32>(10)? != 0,
                    created_at: row.get(11)?,
                    updated_at: row.get(12)?,
                    cartridge_type: row.get::<_, String>(13).unwrap_or_else(|_| "backend".to_string()),
                })
            })
            .map_err(|e| GatewayError::Internal(format!("list cartridges query: {e}")))?;

        Ok(rows.filter_map(|r| r.ok()).collect())
    })
}

/// Deactivate a cartridge by slug.
pub fn delete_cartridge(db: &Database, slug: &str) -> Result<bool, GatewayError> {
    db.with_connection(|conn| {
        let rows = conn
            .execute(
                "UPDATE cartridges SET active = 0, updated_at = ?1 WHERE slug = ?2",
                params![chrono::Utc::now().timestamp(), slug],
            )
            .map_err(|e| GatewayError::Internal(format!("delete cartridge: {e}")))?;
        Ok(rows > 0)
    })
}

/// Deactivate all cartridges.
pub fn delete_all_cartridges(db: &Database) -> Result<u64, GatewayError> {
    db.with_connection(|conn| {
        let rows = conn
            .execute(
                "UPDATE cartridges SET active = 0, updated_at = ?1 WHERE active = 1",
                params![chrono::Utc::now().timestamp()],
            )
            .map_err(|e| GatewayError::Internal(format!("delete all cartridges: {e}")))?;
        Ok(rows as u64)
    })
}

/// Get a KV value for a cartridge.
pub fn cartridge_kv_get(
    db: &Database,
    slug: &str,
    key: &str,
) -> Result<Option<String>, GatewayError> {
    db.with_connection(|conn| {
        conn.query_row(
            "SELECT value FROM cartridge_kv WHERE slug = ?1 AND key = ?2",
            params![slug, key],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| GatewayError::Internal(format!("kv get: {e}")))
    })
}

/// Set a KV value for a cartridge.
pub fn cartridge_kv_set(
    db: &Database,
    slug: &str,
    key: &str,
    value: &str,
) -> Result<(), GatewayError> {
    db.with_connection(|conn| {
        conn.execute(
            "INSERT INTO cartridge_kv (slug, key, value, updated_at) VALUES (?1, ?2, ?3, ?4) \
             ON CONFLICT(slug, key) DO UPDATE SET value = ?3, updated_at = ?4",
            params![slug, key, value, chrono::Utc::now().timestamp()],
        )
        .map_err(|e| GatewayError::Internal(format!("kv set: {e}")))?;
        Ok(())
    })
}

/// Load all KV pairs for a cartridge (for pre-loading into WASM state).
pub fn cartridge_kv_load(
    db: &Database,
    slug: &str,
) -> Result<std::collections::HashMap<String, String>, GatewayError> {
    db.with_connection(|conn| {
        let mut stmt = conn
            .prepare("SELECT key, value FROM cartridge_kv WHERE slug = ?1")
            .map_err(|e| GatewayError::Internal(format!("kv load: {e}")))?;
        let rows = stmt
            .query_map(params![slug], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| GatewayError::Internal(format!("kv load query: {e}")))?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    })
}
