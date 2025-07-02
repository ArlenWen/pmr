use sqlx::{SqlitePool, Row, sqlite::SqlitePoolOptions};
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use crate::{Error, Result};

#[cfg(feature = "http-api")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiToken {
    pub id: String,
    pub token: String,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "http-api", derive(utoipa::ToSchema))]
pub struct ProcessRecord {
    pub id: String,
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub env_vars: HashMap<String, String>,
    pub working_dir: String,
    pub pid: Option<u32>,
    pub status: ProcessStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub log_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "http-api", derive(utoipa::ToSchema))]
pub enum ProcessStatus {
    Running,
    Stopped,
    Failed,
    Unknown,
}

impl std::fmt::Display for ProcessStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProcessStatus::Running => write!(f, "running"),
            ProcessStatus::Stopped => write!(f, "stopped"),
            ProcessStatus::Failed => write!(f, "failed"),
            ProcessStatus::Unknown => write!(f, "unknown"),
        }
    }
}

#[derive(Clone)]
pub struct Database {
    pool: SqlitePool,
}

impl Database {
    pub async fn new(database_url: &str) -> Result<Self> {
        // Add more detailed error context for database connection
        // Configure connection pool for better concurrent performance
        let pool = SqlitePoolOptions::new()
            .max_connections(100) // Increase max connections for concurrent access
            .min_connections(5)   // Keep some connections alive
            .acquire_timeout(std::time::Duration::from_secs(30)) // Longer timeout for high load
            .idle_timeout(std::time::Duration::from_secs(600))   // Keep connections alive longer
            .connect(database_url).await
            .map_err(|e| Error::Other(format!("Failed to connect to database at '{}': {}", database_url, e)))?;
        let db = Self { pool };
        db.configure_for_concurrency().await?;
        db.migrate().await?;
        Ok(db)
    }

    async fn configure_for_concurrency(&self) -> Result<()> {
        // Configure SQLite for better concurrent performance
        sqlx::query("PRAGMA journal_mode = WAL").execute(&self.pool).await?;
        sqlx::query("PRAGMA synchronous = NORMAL").execute(&self.pool).await?;
        sqlx::query("PRAGMA cache_size = 10000").execute(&self.pool).await?;
        sqlx::query("PRAGMA temp_store = memory").execute(&self.pool).await?;
        sqlx::query("PRAGMA mmap_size = 268435456").execute(&self.pool).await?; // 256MB
        sqlx::query("PRAGMA busy_timeout = 30000").execute(&self.pool).await?; // 30 seconds
        Ok(())
    }

    async fn migrate(&self) -> Result<()> {
        // Migrate processes table
        self.migrate_processes_table().await?;

        // Migrate API tokens table (if http-api feature is enabled)
        #[cfg(feature = "http-api")]
        self.migrate_api_tokens_table().await?;

        Ok(())
    }

    async fn migrate_processes_table(&self) -> Result<()> {
        // Check if the table exists and what columns it has
        let table_info = sqlx::query("PRAGMA table_info(processes)")
            .fetch_all(&self.pool)
            .await
            .unwrap_or_default();

        let has_old_columns = table_info.iter().any(|row| {
            let column_name: String = row.get("name");
            column_name == "stdout_path" || column_name == "stderr_path"
        });

        let has_new_column = table_info.iter().any(|row| {
            let column_name: String = row.get("name");
            column_name == "log_path"
        });

        if has_old_columns && !has_new_column {
            // Need to migrate from old schema to new schema
            // Create new table
            sqlx::query(
                r#"
                CREATE TABLE processes_new (
                    id TEXT PRIMARY KEY,
                    name TEXT UNIQUE NOT NULL,
                    command TEXT NOT NULL,
                    args TEXT NOT NULL,
                    env_vars TEXT NOT NULL,
                    working_dir TEXT NOT NULL,
                    pid INTEGER,
                    status TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    log_path TEXT NOT NULL
                )
                "#,
            )
            .execute(&self.pool)
            .await?;

            // Copy data from old table, using stdout_path as log_path
            sqlx::query(
                r#"
                INSERT INTO processes_new
                SELECT id, name, command, args, env_vars, working_dir, pid, status,
                       created_at, updated_at, stdout_path as log_path
                FROM processes
                "#,
            )
            .execute(&self.pool)
            .await?;

            // Drop old table and rename new one
            sqlx::query("DROP TABLE processes").execute(&self.pool).await?;
            sqlx::query("ALTER TABLE processes_new RENAME TO processes").execute(&self.pool).await?;
        } else if table_info.is_empty() {
            // Create new table from scratch
            sqlx::query(
                r#"
                CREATE TABLE processes (
                    id TEXT PRIMARY KEY,
                    name TEXT UNIQUE NOT NULL,
                    command TEXT NOT NULL,
                    args TEXT NOT NULL,
                    env_vars TEXT NOT NULL,
                    working_dir TEXT NOT NULL,
                    pid INTEGER,
                    status TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    log_path TEXT NOT NULL
                )
                "#,
            )
            .execute(&self.pool)
            .await?;
        }
        // If has_new_column is true, table is already in the correct format

        Ok(())
    }

    #[cfg(feature = "http-api")]
    async fn migrate_api_tokens_table(&self) -> Result<()> {
        // Check if the api_tokens table exists
        let table_exists = sqlx::query(
            "SELECT name FROM sqlite_master WHERE type='table' AND name='api_tokens'"
        )
        .fetch_optional(&self.pool)
        .await?
        .is_some();

        if !table_exists {
            // Create api_tokens table
            sqlx::query(
                r#"
                CREATE TABLE api_tokens (
                    id TEXT PRIMARY KEY,
                    token TEXT UNIQUE NOT NULL,
                    name TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    expires_at TEXT,
                    is_active INTEGER NOT NULL DEFAULT 1
                )
                "#,
            )
            .execute(&self.pool)
            .await?;

            // Create index on token for faster lookups
            sqlx::query("CREATE INDEX idx_api_tokens_token ON api_tokens(token)")
                .execute(&self.pool)
                .await?;
        }

        Ok(())
    }

    pub async fn insert_process(&self, process: &ProcessRecord) -> Result<()> {
        let args_json = serde_json::to_string(&process.args)?;
        let env_vars_json = serde_json::to_string(&process.env_vars)?;

        sqlx::query(
            r#"
            INSERT INTO processes (
                id, name, command, args, env_vars, working_dir, pid, status,
                created_at, updated_at, log_path
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&process.id)
        .bind(&process.name)
        .bind(&process.command)
        .bind(&args_json)
        .bind(&env_vars_json)
        .bind(&process.working_dir)
        .bind(process.pid.map(|p| p as i64))
        .bind(process.status.to_string())
        .bind(process.created_at.to_rfc3339())
        .bind(process.updated_at.to_rfc3339())
        .bind(&process.log_path)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_process_by_name(&self, name: &str) -> Result<Option<ProcessRecord>> {
        let row = sqlx::query("SELECT * FROM processes WHERE name = ?")
            .bind(name)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(row) = row {
            Ok(Some(self.row_to_process_record(row)?))
        } else {
            Ok(None)
        }
    }

    pub async fn get_all_processes(&self) -> Result<Vec<ProcessRecord>> {
        let rows = sqlx::query("SELECT * FROM processes ORDER BY created_at DESC")
            .fetch_all(&self.pool)
            .await?;

        let mut processes = Vec::new();
        for row in rows {
            processes.push(self.row_to_process_record(row)?);
        }
        Ok(processes)
    }

    pub async fn update_process_status(&self, name: &str, status: ProcessStatus, pid: Option<u32>) -> Result<()> {
        sqlx::query(
            "UPDATE processes SET status = ?, pid = ?, updated_at = ? WHERE name = ?"
        )
        .bind(status.to_string())
        .bind(pid.map(|p| p as i64))
        .bind(Utc::now().to_rfc3339())
        .bind(name)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn delete_process(&self, name: &str) -> Result<bool> {
        let result = sqlx::query("DELETE FROM processes WHERE name = ?")
            .bind(name)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn delete_process_by_id(&self, id: &str) -> Result<bool> {
        let result = sqlx::query("DELETE FROM processes WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn get_processes_by_status(&self, statuses: &[ProcessStatus]) -> Result<Vec<ProcessRecord>> {
        if statuses.is_empty() {
            return Ok(Vec::new());
        }

        let status_strings: Vec<String> = statuses.iter().map(|s| s.to_string()).collect();
        let placeholders = status_strings.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let query = format!("SELECT * FROM processes WHERE status IN ({}) ORDER BY created_at DESC", placeholders);

        let mut query_builder = sqlx::query(&query);
        for status in &status_strings {
            query_builder = query_builder.bind(status);
        }

        let rows = query_builder.fetch_all(&self.pool).await?;

        let mut processes = Vec::new();
        for row in rows {
            processes.push(self.row_to_process_record(row)?);
        }
        Ok(processes)
    }

    pub async fn delete_processes_by_names(&self, names: &[String]) -> Result<usize> {
        if names.is_empty() {
            return Ok(0);
        }

        let placeholders = names.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let query = format!("DELETE FROM processes WHERE name IN ({})", placeholders);

        let mut query_builder = sqlx::query(&query);
        for name in names {
            query_builder = query_builder.bind(name);
        }

        let result = query_builder.execute(&self.pool).await?;
        Ok(result.rows_affected() as usize)
    }

    fn row_to_process_record(&self, row: sqlx::sqlite::SqliteRow) -> Result<ProcessRecord> {
        let args_json: String = row.get("args");
        let env_vars_json: String = row.get("env_vars");
        let created_at_str: String = row.get("created_at");
        let updated_at_str: String = row.get("updated_at");
        let status_str: String = row.get("status");
        let pid_i64: Option<i64> = row.get("pid");

        let args: Vec<String> = serde_json::from_str(&args_json)?;
        let env_vars: HashMap<String, String> = serde_json::from_str(&env_vars_json)?;
        let created_at = DateTime::parse_from_rfc3339(&created_at_str)
            .map_err(|e| Error::Other(format!("Failed to parse created_at: {}", e)))?
            .with_timezone(&Utc);
        let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
            .map_err(|e| Error::Other(format!("Failed to parse updated_at: {}", e)))?
            .with_timezone(&Utc);

        let status = match status_str.as_str() {
            "running" => ProcessStatus::Running,
            "stopped" => ProcessStatus::Stopped,
            "failed" => ProcessStatus::Failed,
            _ => ProcessStatus::Unknown,
        };

        Ok(ProcessRecord {
            id: row.get("id"),
            name: row.get("name"),
            command: row.get("command"),
            args,
            env_vars,
            working_dir: row.get("working_dir"),
            pid: pid_i64.map(|p| p as u32),
            status,
            created_at,
            updated_at,
            log_path: row.get("log_path"),
        })
    }

    // API Token methods (only available with http-api feature)
    #[cfg(feature = "http-api")]
    pub async fn insert_api_token(&self, token: &ApiToken) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO api_tokens (id, token, name, created_at, expires_at, is_active)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&token.id)
        .bind(&token.token)
        .bind(&token.name)
        .bind(token.created_at.to_rfc3339())
        .bind(token.expires_at.map(|e| e.to_rfc3339()))
        .bind(if token.is_active { 1 } else { 0 })
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    #[cfg(feature = "http-api")]
    pub async fn get_api_token_by_token(&self, token: &str) -> Result<Option<ApiToken>> {
        let row = sqlx::query("SELECT * FROM api_tokens WHERE token = ?")
            .bind(token)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(row) = row {
            Ok(Some(self.row_to_api_token(row)?))
        } else {
            Ok(None)
        }
    }

    #[cfg(feature = "http-api")]
    pub async fn get_all_api_tokens(&self) -> Result<Vec<ApiToken>> {
        let rows = sqlx::query("SELECT * FROM api_tokens ORDER BY created_at DESC")
            .fetch_all(&self.pool)
            .await?;

        let mut tokens = Vec::new();
        for row in rows {
            tokens.push(self.row_to_api_token(row)?);
        }
        Ok(tokens)
    }

    #[cfg(feature = "http-api")]
    pub async fn update_api_token_status(&self, token: &str, is_active: bool) -> Result<bool> {
        let result = sqlx::query("UPDATE api_tokens SET is_active = ? WHERE token = ?")
            .bind(if is_active { 1 } else { 0 })
            .bind(token)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    #[cfg(feature = "http-api")]
    pub async fn delete_api_token(&self, token: &str) -> Result<bool> {
        let result = sqlx::query("DELETE FROM api_tokens WHERE token = ?")
            .bind(token)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    #[cfg(feature = "http-api")]
    fn row_to_api_token(&self, row: sqlx::sqlite::SqliteRow) -> Result<ApiToken> {
        let created_at_str: String = row.get("created_at");
        let expires_at_str: Option<String> = row.get("expires_at");
        let is_active_i64: i64 = row.get("is_active");

        let created_at = DateTime::parse_from_rfc3339(&created_at_str)
            .map_err(|e| Error::Other(format!("Failed to parse created_at: {}", e)))?
            .with_timezone(&Utc);

        let expires_at = if let Some(expires_str) = expires_at_str {
            Some(DateTime::parse_from_rfc3339(&expires_str)
                .map_err(|e| Error::Other(format!("Failed to parse expires_at: {}", e)))?
                .with_timezone(&Utc))
        } else {
            None
        };

        Ok(ApiToken {
            id: row.get("id"),
            token: row.get("token"),
            name: row.get("name"),
            created_at,
            expires_at,
            is_active: is_active_i64 != 0,
        })
    }
}
