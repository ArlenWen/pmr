use sqlx::{SqlitePool, Row};
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use crate::{Error, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
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

pub struct Database {
    pool: SqlitePool,
}

impl Database {
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = SqlitePool::connect(database_url).await?;
        let db = Self { pool };
        db.migrate().await?;
        Ok(db)
    }

    async fn migrate(&self) -> Result<()> {
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
}
