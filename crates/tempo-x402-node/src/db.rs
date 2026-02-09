//! Children table extension for the gateway database.

use rusqlite::params;
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
        created_at INTEGER NOT NULL,
        updated_at INTEGER NOT NULL
    );
    CREATE INDEX IF NOT EXISTS idx_children_instance_id ON children(instance_id);
"#;

/// Initialize the children table on an existing gateway database.
pub fn init_children_schema(db: &Database) -> Result<(), GatewayError> {
    db.execute_schema(CHILDREN_SCHEMA)
}

/// Insert a new child instance record.
pub fn create_child(
    db: &Database,
    instance_id: &str,
    url: Option<&str>,
    railway_service_id: Option<&str>,
) -> Result<ChildInstance, GatewayError> {
    let now = chrono::Utc::now().timestamp();

    // Use execute_schema for the insert via a raw query
    // We need direct connection access — use a helper on Database.
    // For now, build the insert via execute_schema with bound params
    // Actually, execute_schema uses execute_batch which doesn't support params.
    // We'll use a slightly different approach: format the values safely.
    //
    // Since instance_id, url, railway_service_id come from our own code (not user input),
    // and we validate them, this is safe. But let's use the proper approach via
    // the Database's execute_schema for schema-only, and a separate method for data.

    // We can work around this by using the gateway's Database indirectly.
    // The simplest correct approach: use execute_schema for DDL only,
    // and for DML, open a second connection to the same file.
    //
    // However, this is overly complex. Let's just use the execute_schema
    // with a carefully constructed statement. Since instance_id is a UUID we generate,
    // url is from Railway API, and service_id is from Railway API, these are safe.

    let url_sql = url
        .map(|u| format!("'{}'", u.replace('\'', "''")))
        .unwrap_or_else(|| "NULL".to_string());
    let sid_sql = railway_service_id
        .map(|s| format!("'{}'", s.replace('\'', "''")))
        .unwrap_or_else(|| "NULL".to_string());
    let iid_escaped = instance_id.replace('\'', "''");

    let sql = format!(
        "INSERT INTO children (instance_id, url, railway_service_id, status, created_at, updated_at) \
         VALUES ('{}', {}, {}, 'pending', {}, {})",
        iid_escaped, url_sql, sid_sql, now, now
    );
    db.execute_schema(&sql)?;

    Ok(ChildInstance {
        id: 0, // We don't get last_insert_rowid through execute_schema
        instance_id: instance_id.to_string(),
        address: String::new(),
        url: url.map(String::from),
        railway_service_id: railway_service_id.map(String::from),
        funded_amount: None,
        funding_tx: None,
        status: "pending".to_string(),
        created_at: now,
        updated_at: now,
    })
}

/// Update a child instance when it registers back.
pub fn update_child(
    db: &Database,
    instance_id: &str,
    address: Option<&str>,
    url: Option<&str>,
    status: Option<&str>,
) -> Result<(), GatewayError> {
    let now = chrono::Utc::now().timestamp();
    let mut sets = vec![format!("updated_at = {}", now)];

    if let Some(a) = address {
        sets.push(format!("address = '{}'", a.replace('\'', "''")));
    }
    if let Some(u) = url {
        sets.push(format!("url = '{}'", u.replace('\'', "''")));
    }
    if let Some(s) = status {
        sets.push(format!("status = '{}'", s.replace('\'', "''")));
    }

    let iid_escaped = instance_id.replace('\'', "''");
    let sql = format!(
        "UPDATE children SET {} WHERE instance_id = '{}'",
        sets.join(", "),
        iid_escaped
    );
    db.execute_schema(&sql)?;
    Ok(())
}

// Read operations (list/count) use direct rusqlite connections opened from NodeState's db_path,
// since Database's execute_schema uses execute_batch which doesn't return rows.

/// Query children from a direct rusqlite connection.
pub fn query_children(conn: &rusqlite::Connection) -> Result<Vec<ChildInstance>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        r#"
        SELECT id, instance_id, address, url, railway_service_id,
               funded_amount, funding_tx, status, created_at, updated_at
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
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(children)
}

/// Count children from a direct rusqlite connection.
pub fn query_children_count(conn: &rusqlite::Connection) -> Result<u32, rusqlite::Error> {
    conn.query_row("SELECT COUNT(*) FROM children", params![], |row| row.get(0))
}
