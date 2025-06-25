use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessConfig {
    pub id: Uuid,
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub workdir: Option<PathBuf>,
    pub env_vars: HashMap<String, String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessState {
    pub config: ProcessConfig,
    pub pid: Option<u32>,
    pub status: ProcessStatus,
    pub started_at: Option<DateTime<Utc>>,
    pub stopped_at: Option<DateTime<Utc>>,
    pub restart_count: u32,
    pub stdout_path: PathBuf,
    pub stderr_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ProcessStatus {
    Stopped,
    Running,
    Failed,
    Unknown,
}

impl ProcessConfig {
    pub fn new(
        name: String,
        command: String,
        args: Vec<String>,
        workdir: Option<PathBuf>,
        env_vars: HashMap<String, String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            name,
            command,
            args,
            workdir,
            env_vars,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn update_env_vars(&mut self, new_vars: HashMap<String, String>) {
        self.env_vars.extend(new_vars);
        self.updated_at = Utc::now();
    }
}

impl ProcessState {
    pub fn new(config: ProcessConfig, data_dir: &Path) -> Self {
        let stdout_path = data_dir.join(format!("{}.stdout.log", config.name));
        let stderr_path = data_dir.join(format!("{}.stderr.log", config.name));

        Self {
            config,
            pid: None,
            status: ProcessStatus::Stopped,
            started_at: None,
            stopped_at: None,
            restart_count: 0,
            stdout_path,
            stderr_path,
        }
    }

    pub fn start(&mut self, pid: u32) {
        self.pid = Some(pid);
        self.status = ProcessStatus::Running;
        self.started_at = Some(Utc::now());
        self.stopped_at = None;
    }

    pub fn stop(&mut self) {
        self.pid = None;
        self.status = ProcessStatus::Stopped;
        self.stopped_at = Some(Utc::now());
    }

    pub fn fail(&mut self) {
        self.pid = None;
        self.status = ProcessStatus::Failed;
        self.stopped_at = Some(Utc::now());
    }

    #[allow(dead_code)]
    pub fn restart(&mut self, pid: u32) {
        self.restart_count += 1;
        self.start(pid);
    }

    pub fn is_running(&self) -> bool {
        self.status == ProcessStatus::Running && self.pid.is_some()
    }
}

impl std::fmt::Display for ProcessStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProcessStatus::Stopped => write!(f, "stopped"),
            ProcessStatus::Running => write!(f, "running"),
            ProcessStatus::Failed => write!(f, "failed"),
            ProcessStatus::Unknown => write!(f, "unknown"),
        }
    }
}
