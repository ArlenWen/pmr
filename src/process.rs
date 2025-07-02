use crate::{
    config::Config,
    database::{Database, ProcessRecord, ProcessStatus},
    log_rotation::LogRotator,
    Error, Result,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "http-api", derive(utoipa::ToSchema))]
pub struct ClearResult {
    pub cleared_count: usize,
    pub cleared_processes: Vec<String>,
    pub failed_processes: Vec<String>,
    pub operation_type: String,
}

pub struct ProcessManager {
    db: Database,
    config: Config,
    log_rotator: LogRotator,
    // Track running processes to properly reap them
    running_processes: Arc<Mutex<HashMap<u32, tokio::process::Child>>>,
}

impl ProcessManager {
    pub async fn new(config: Config) -> Result<Self> {
        config.ensure_directories()?;
        // Add create_if_missing parameter to SQLite URL to automatically create the database file
        let database_url = format!("sqlite:{}?mode=rwc", config.database_path.display());
        let db = Database::new(&database_url).await?;
        let log_rotator = LogRotator::new(config.log_rotation.clone());
        let running_processes = Arc::new(Mutex::new(HashMap::new()));

        let process_manager = Self {
            db,
            config,
            log_rotator,
            running_processes: running_processes.clone()
        };

        // Start background task to reap zombie processes
        process_manager.start_process_reaper().await;

        Ok(process_manager)
    }

    #[cfg(any(test, feature = "http-api"))]
    pub fn get_database(&self) -> std::sync::Arc<Database> {
        std::sync::Arc::new(self.db.clone())
    }

    /// Start background task to reap zombie processes
    async fn start_process_reaper(&self) {
        let running_processes = self.running_processes.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(5));
            loop {
                interval.tick().await;
                let mut processes = running_processes.lock().await;
                let mut to_remove = Vec::new();

                for (pid, child) in processes.iter_mut() {
                    // Try to reap the process without blocking
                    match child.try_wait() {
                        Ok(Some(_exit_status)) => {
                            // Process has terminated, mark for removal
                            to_remove.push(*pid);
                        }
                        Ok(None) => {
                            // Process is still running, continue
                        }
                        Err(_) => {
                            // Error checking process status, assume it's dead
                            to_remove.push(*pid);
                        }
                    }
                }

                // Remove reaped processes
                for pid in to_remove {
                    processes.remove(&pid);
                }
            }
        });
    }

    pub async fn start_process(
        &self,
        name: &str,
        command: &str,
        args: Vec<String>,
        env_vars: HashMap<String, String>,
        working_dir: Option<String>,
        log_dir: Option<String>,
    ) -> Result<String> {
        // Check if process already exists
        if self.db.get_process_by_name(name).await?.is_some() {
            return Err(Error::ProcessAlreadyExists(name.to_string()));
        }

        let id = Uuid::new_v4().to_string();
        let working_dir = working_dir.unwrap_or_else(|| std::env::current_dir()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string());

        // Determine log directory - use custom log_dir if provided, otherwise use default
        let log_directory = if let Some(custom_log_dir) = log_dir {
            PathBuf::from(custom_log_dir)
        } else {
            self.config.default_log_dir.clone()
        };

        // Track resources created for potential rollback
        let mut created_log_dir = false;
        let mut created_log_file = false;
        let inserted_db_record = false;

        // Ensure the log directory exists
        let log_dir_existed = log_directory.exists();
        if let Err(e) = self.config.ensure_log_directory(&log_directory) {
            return Err(e);
        }
        if !log_dir_existed {
            created_log_dir = true;
        }

        let log_path = log_directory.join(format!("{}.log", name));

        // Check if log rotation is needed for existing log file
        if log_path.exists() {
            if let Err(e) = self.log_rotator.rotate_if_needed(&log_path).await {
                self.rollback_start_process(&id, &log_path, created_log_dir, created_log_file, inserted_db_record).await;
                return Err(e);
            }
        }

        // Create log file
        if let Err(e) = tokio::fs::File::create(&log_path).await {
            self.rollback_start_process(&id, &log_path, created_log_dir, created_log_file, inserted_db_record).await;
            return Err(e.into());
        }
        created_log_file = true;

        // Use setsid to create a new session and detach from terminal
        let mut cmd = tokio::process::Command::new("setsid");
        cmd.arg(command)
            .args(&args)
            .current_dir(&working_dir)
            .envs(&env_vars);

        // Set up stdio - redirect to log file
        let stdout_file = match std::fs::File::create(&log_path) {
            Ok(file) => file,
            Err(e) => {
                self.rollback_start_process(&id, &log_path, created_log_dir, created_log_file, inserted_db_record).await;
                return Err(e.into());
            }
        };

        let stderr_file = match std::fs::File::options().create(true).append(true).open(&log_path) {
            Ok(file) => file,
            Err(e) => {
                self.rollback_start_process(&id, &log_path, created_log_dir, created_log_file, inserted_db_record).await;
                return Err(e.into());
            }
        };

        cmd.stdout(Stdio::from(stdout_file))
            .stderr(Stdio::from(stderr_file))
            .stdin(Stdio::null());

        // Start the process
        let child = cmd.spawn();

        let (pid, initial_status) = match child {
            Ok(child) => {
                let pid = child.id().ok_or_else(|| {
                    Error::Other("Failed to get process ID".to_string())
                })?;

                // Store the child process for proper reaping
                {
                    let mut processes = self.running_processes.lock().await;
                    processes.insert(pid, child);
                }

                // Wait a moment to check if the process actually started successfully
                tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

                // Check if the process is still running
                let status = if self.is_process_running(pid).await {
                    ProcessStatus::Running
                } else {
                    // Process started but exited quickly - this could be either
                    // a failed command or a command that completed successfully
                    // We'll mark it as stopped for now, and let the user check logs
                    ProcessStatus::Stopped
                };

                (Some(pid), status)
            }
            Err(e) => {
                // Process failed to start at all - perform rollback
                self.rollback_start_process(&id, &log_path, created_log_dir, created_log_file, inserted_db_record).await;
                return Err(Error::Other(format!("Failed to start process '{}': {}", name, e)));
            }
        };

        // Create process record
        let process_record = ProcessRecord {
            id: id.clone(),
            name: name.to_string(),
            command: command.to_string(),
            args,
            env_vars,
            working_dir,
            pid,
            status: initial_status.clone(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            log_path: log_path.to_string_lossy().to_string(),
        };

        // Insert process record - if this fails, we need to rollback
        if let Err(e) = self.db.insert_process(&process_record).await {
            self.rollback_start_process(&id, &log_path, created_log_dir, created_log_file, inserted_db_record).await;
            return Err(e);
        }

        let message = match initial_status {
            ProcessStatus::Running => {
                if let Some(pid) = pid {
                    format!("Process '{}' started with PID {}", name, pid)
                } else {
                    format!("Process '{}' started", name)
                }
            }
            ProcessStatus::Stopped => {
                if let Some(pid) = pid {
                    format!("Process '{}' started with PID {} but exited quickly", name, pid)
                } else {
                    format!("Process '{}' started but exited quickly", name)
                }
            }
            ProcessStatus::Failed => {
                // This case should not happen anymore since we rollback on spawn failure
                format!("Process '{}' failed to start", name)
            }
            _ => format!("Process '{}' started with unknown status", name),
        };

        Ok(message)
    }

    /// Rollback resources created during a failed start_process operation
    async fn rollback_start_process(
        &self,
        process_id: &str,
        log_path: &PathBuf,
        created_log_dir: bool,
        created_log_file: bool,
        inserted_db_record: bool,
    ) {
        // Remove database record if it was inserted
        if inserted_db_record {
            if let Err(e) = self.db.delete_process_by_id(process_id).await {
                eprintln!("Warning: Failed to rollback database record for process ID {}: {}", process_id, e);
            }
        }

        // Remove log file if it was created
        if created_log_file && log_path.exists() {
            if let Err(e) = tokio::fs::remove_file(log_path).await {
                eprintln!("Warning: Failed to rollback log file {}: {}", log_path.display(), e);
            }
        }

        // Remove log directory if it was created and is now empty
        if created_log_dir {
            if let Some(log_dir) = log_path.parent() {
                // Only remove if directory is empty
                if let Ok(mut entries) = tokio::fs::read_dir(log_dir).await {
                    let mut is_empty = true;
                    if let Ok(Some(_)) = entries.next_entry().await {
                        is_empty = false;
                    }

                    if is_empty {
                        if let Err(e) = tokio::fs::remove_dir(log_dir).await {
                            eprintln!("Warning: Failed to rollback log directory {}: {}", log_dir.display(), e);
                        }
                    }
                }
            }
        }
    }

    pub async fn stop_process(&self, name: &str) -> Result<String> {
        let process = self.db.get_process_by_name(name).await?
            .ok_or_else(|| Error::ProcessNotFound(name.to_string()))?;

        if let Some(pid) = process.pid {
            // First try to get the child process from our tracking
            let mut child_opt = {
                let mut processes = self.running_processes.lock().await;
                processes.remove(&pid)
            };

            if let Some(ref mut child) = child_opt {
                // We have the child process, use tokio's kill method
                match child.kill().await {
                    Ok(_) => {
                        // Wait for the process to actually terminate
                        let _ = child.wait().await;
                        self.db.update_process_status(name, ProcessStatus::Stopped, Some(pid)).await?;
                        Ok(format!("Process '{}' stopped", name))
                    }
                    Err(e) => {
                        // Re-insert the child back if kill failed
                        let mut processes = self.running_processes.lock().await;
                        processes.insert(pid, child_opt.unwrap());
                        Err(Error::Other(format!("Failed to stop process '{}' with PID {}: {}", name, pid, e)))
                    }
                }
            } else {
                // Fallback to using libc::kill for processes not in our tracking
                let result = unsafe { libc::kill(pid as i32, libc::SIGTERM) };
                if result == 0 {
                    // Wait a bit for the process to terminate
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                    self.db.update_process_status(name, ProcessStatus::Stopped, Some(pid)).await?;
                    Ok(format!("Process '{}' stopped", name))
                } else {
                    Err(Error::Other(format!("Failed to stop process '{}' with PID {}", name, pid)))
                }
            }
        } else {
            Err(Error::InvalidProcessState(format!("Process '{}' has no PID", name)))
        }
    }

    pub async fn restart_process(&self, name: &str) -> Result<String> {
        let process = self.db.get_process_by_name(name).await?
            .ok_or_else(|| Error::ProcessNotFound(name.to_string()))?;

        // Stop the process if it's running
        if process.pid.is_some() && self.is_process_running(process.pid.unwrap()).await {
            self.stop_process(name).await?;
            // Wait a bit for the process to stop
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }

        // Extract log directory from the existing log path
        let log_dir = PathBuf::from(&process.log_path)
            .parent()
            .map(|p| p.to_string_lossy().to_string());

        // Delete the process record
        self.db.delete_process(name).await?;

        // Start the process again
        let start_message = self.start_process(
            name,
            &process.command,
            process.args,
            process.env_vars,
            Some(process.working_dir),
            log_dir,
        ).await?;

        Ok(format!("Process '{}' restarted. {}", name, start_message))
    }

    pub async fn delete_process(&self, name: &str) -> Result<String> {
        let process = self.db.get_process_by_name(name).await?
            .ok_or_else(|| Error::ProcessNotFound(name.to_string()))?;

        // Stop the process if it's running
        if let Some(pid) = process.pid {
            if self.is_process_running(pid).await {
                self.stop_process(name).await?;
            } else {
                // Process is not running, but remove it from tracking if present
                let mut processes = self.running_processes.lock().await;
                processes.remove(&pid);
            }
        }

        // Delete from database
        if self.db.delete_process(name).await? {
            // Optionally remove log file
            let _ = tokio::fs::remove_file(&process.log_path).await;
            Ok(format!("Process '{}' deleted", name))
        } else {
            Err(Error::ProcessNotFound(name.to_string()))
        }
    }

    pub async fn list_processes(&self) -> Result<Vec<ProcessRecord>> {
        let mut processes = self.db.get_all_processes().await?;

        // Update status for each process
        for process in &mut processes {
            if let Some(pid) = process.pid {
                let is_running = self.is_process_running(pid).await;
                let new_status = match process.status {
                    ProcessStatus::Failed => ProcessStatus::Failed, // Keep failed status
                    _ => {
                        if is_running {
                            ProcessStatus::Running
                        } else {
                            ProcessStatus::Stopped
                        }
                    }
                };

                if new_status != process.status {
                    self.db.update_process_status(&process.name, new_status.clone(), Some(pid)).await?;
                    process.status = new_status;
                }
            } else {
                // No PID means the process failed to start
                if process.status != ProcessStatus::Failed {
                    self.db.update_process_status(&process.name, ProcessStatus::Failed, None).await?;
                    process.status = ProcessStatus::Failed;
                }
            }
        }

        Ok(processes)
    }

    pub async fn clear_processes(&self, all: bool) -> Result<ClearResult> {
        let processes_to_clear = if all {
            // Get all processes
            self.db.get_all_processes().await?
        } else {
            // Get only stopped and failed processes
            self.db.get_processes_by_status(&[ProcessStatus::Stopped, ProcessStatus::Failed]).await?
        };

        let mut cleared_processes = Vec::new();
        let mut failed_processes = Vec::new();

        for process in processes_to_clear {
            match self.delete_single_process(&process).await {
                Ok(_) => cleared_processes.push(process.name),
                Err(_) => failed_processes.push(process.name),
            }
        }

        let operation_type = if all {
            "all processes".to_string()
        } else {
            "stopped/failed processes".to_string()
        };

        Ok(ClearResult {
            cleared_count: cleared_processes.len(),
            cleared_processes,
            failed_processes,
            operation_type,
        })
    }

    async fn delete_single_process(&self, process: &ProcessRecord) -> Result<()> {
        // Stop the process if it's running
        if let Some(pid) = process.pid {
            if self.is_process_running(pid).await {
                // Try to stop the process properly
                if let Err(_) = self.stop_process(&process.name).await {
                    // If proper stop fails, try direct kill
                    let result = unsafe { libc::kill(pid as i32, libc::SIGTERM) };
                    if result == 0 {
                        // Wait a bit for termination
                        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                    }
                }
            } else {
                // Process is not running, but remove it from tracking if present
                let mut processes = self.running_processes.lock().await;
                processes.remove(&pid);
            }
        }

        // Delete from database
        if !self.db.delete_process(&process.name).await? {
            return Err(Error::ProcessNotFound(process.name.clone()));
        }

        // Remove log file
        let _ = tokio::fs::remove_file(&process.log_path).await;

        Ok(())
    }

    pub async fn get_process_status(&self, name: &str) -> Result<ProcessRecord> {
        let mut process = self.db.get_process_by_name(name).await?
            .ok_or_else(|| Error::ProcessNotFound(name.to_string()))?;

        // Update status
        if let Some(pid) = process.pid {
            let is_running = self.is_process_running(pid).await;
            let new_status = match process.status {
                ProcessStatus::Failed => ProcessStatus::Failed, // Keep failed status
                _ => {
                    if is_running {
                        ProcessStatus::Running
                    } else {
                        ProcessStatus::Stopped
                    }
                }
            };

            if new_status != process.status {
                self.db.update_process_status(name, new_status.clone(), Some(pid)).await?;
                process.status = new_status;
            }
        } else {
            // No PID means the process failed to start
            if process.status != ProcessStatus::Failed {
                self.db.update_process_status(name, ProcessStatus::Failed, None).await?;
                process.status = ProcessStatus::Failed;
            }
        }

        Ok(process)
    }

    pub async fn get_process_logs(&self, name: &str, lines: Option<usize>) -> Result<String> {
        let process = self.db.get_process_by_name(name).await?
            .ok_or_else(|| Error::ProcessNotFound(name.to_string()))?;

        let content = match tokio::fs::read_to_string(&process.log_path).await {
            Ok(content) => content,
            Err(e) => {
                // Try to read as bytes and convert to string, replacing invalid UTF-8
                match tokio::fs::read(&process.log_path).await {
                    Ok(bytes) => String::from_utf8_lossy(&bytes).to_string(),
                    Err(_) => return Err(Error::Other(format!("Failed to read log file: {}", e))),
                }
            }
        };

        if let Some(lines) = lines {
            let lines_vec: Vec<&str> = content.lines().collect();
            let start = if lines_vec.len() > lines {
                lines_vec.len() - lines
            } else {
                0
            };
            Ok(lines_vec[start..].join("\n"))
        } else {
            Ok(content)
        }
    }

    async fn is_process_running(&self, pid: u32) -> bool {
        let result = unsafe { libc::kill(pid as i32, 0) };
        result == 0
    }

    /// Get rotated log files for a process
    pub async fn get_rotated_logs(&self, name: &str) -> Result<Vec<String>> {
        let process = self.db.get_process_by_name(name).await?
            .ok_or_else(|| Error::ProcessNotFound(name.to_string()))?;

        let log_path = PathBuf::from(&process.log_path);
        let rotated_files = self.log_rotator.get_rotated_files(&log_path)?;

        let mut logs = Vec::new();
        for file_path in rotated_files {
            let content = match tokio::fs::read_to_string(&file_path).await {
                Ok(content) => content,
                Err(_) => {
                    // Try to read as bytes and convert to string, replacing invalid UTF-8
                    match tokio::fs::read(&file_path).await {
                        Ok(bytes) => String::from_utf8_lossy(&bytes).to_string(),
                        Err(_) => continue,
                    }
                }
            };
            logs.push(format!("=== {} ===\n{}", file_path.display(), content));
        }

        Ok(logs)
    }

    /// Manually rotate log file for a process
    pub async fn rotate_process_logs(&self, name: &str) -> Result<String> {
        let process = self.db.get_process_by_name(name).await?
            .ok_or_else(|| Error::ProcessNotFound(name.to_string()))?;

        let log_path = PathBuf::from(&process.log_path);
        self.log_rotator.force_rotate(&log_path).await?;

        Ok(format!("Log rotation completed for process '{}'", name))
    }

    /// Get log rotation status for a process
    pub async fn get_log_rotation_status(&self, name: &str) -> Result<String> {
        let process = self.db.get_process_by_name(name).await?
            .ok_or_else(|| Error::ProcessNotFound(name.to_string()))?;

        let log_path = PathBuf::from(&process.log_path);
        let current_size = self.log_rotator.get_log_size(&log_path)?;
        let needs_rotation = self.log_rotator.needs_rotation(&log_path)?;
        let rotated_files = self.log_rotator.get_rotated_files(&log_path)?;

        let status = format!(
            "Log file: {}\nCurrent size: {} bytes\nNeeds rotation: {}\nRotated files: {}",
            log_path.display(),
            current_size,
            if needs_rotation { "Yes" } else { "No" },
            rotated_files.len()
        );

        Ok(status)
    }
}


