use crate::{
    config::Config,
    database::{Database, ProcessRecord, ProcessStatus},
    Error, Result,
};
use chrono::Utc;
use std::collections::HashMap;
use std::process::{Command, Stdio};
use uuid::Uuid;

pub struct ProcessManager {
    db: Database,
    config: Config,
}

impl ProcessManager {
    pub async fn new(config: Config) -> Result<Self> {
        config.ensure_directories()?;
        let database_url = format!("sqlite:{}", config.database_path.display());
        let db = Database::new(&database_url).await?;
        
        Ok(Self { db, config })
    }

    pub async fn start_process(
        &self,
        name: &str,
        command: &str,
        args: Vec<String>,
        env_vars: HashMap<String, String>,
        working_dir: Option<String>,
    ) -> Result<()> {
        // Check if process already exists
        if self.db.get_process_by_name(name).await?.is_some() {
            return Err(Error::ProcessAlreadyExists(name.to_string()));
        }

        let id = Uuid::new_v4().to_string();
        let working_dir = working_dir.unwrap_or_else(|| std::env::current_dir()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string());

        let log_path = self.config.log_dir.join(format!("{}.log", name));

        // Create log file
        tokio::fs::File::create(&log_path).await?;

        // Use setsid to create a new session and detach from terminal
        let mut cmd = Command::new("setsid");
        cmd.arg(command)
            .args(&args)
            .current_dir(&working_dir)
            .envs(&env_vars)
            .stdout(Stdio::from(std::fs::File::create(&log_path)?))
            .stderr(Stdio::from(std::fs::File::options().create(true).append(true).open(&log_path)?))
            .stdin(Stdio::null());

        // Start the process
        let child = cmd.spawn();

        let (pid, initial_status) = match child {
            Ok(child) => {
                let pid = child.id();
                // Detach the process - it will continue running independently
                std::mem::forget(child);

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
            Err(_) => {
                // Process failed to start at all
                (None, ProcessStatus::Failed)
            }
        };

        // Create process record
        let process_record = ProcessRecord {
            id,
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

        self.db.insert_process(&process_record).await?;

        match initial_status {
            ProcessStatus::Running => {
                if let Some(pid) = pid {
                    println!("Process '{}' started with PID {}", name, pid);
                }
            }
            ProcessStatus::Stopped => {
                if let Some(pid) = pid {
                    println!("Process '{}' started with PID {} but exited quickly", name, pid);
                }
            }
            ProcessStatus::Failed => {
                println!("Process '{}' failed to start", name);
            }
            _ => {}
        }

        Ok(())
    }

    pub async fn stop_process(&self, name: &str) -> Result<()> {
        let process = self.db.get_process_by_name(name).await?
            .ok_or_else(|| Error::ProcessNotFound(name.to_string()))?;

        if let Some(pid) = process.pid {
            // Send SIGTERM to the process
            let result = unsafe { libc::kill(pid as i32, libc::SIGTERM) };
            if result == 0 {
                self.db.update_process_status(name, ProcessStatus::Stopped, Some(pid)).await?;
                println!("Process '{}' stopped", name);
            } else {
                return Err(Error::Other(format!("Failed to stop process '{}' with PID {}", name, pid)));
            }
        } else {
            return Err(Error::InvalidProcessState(format!("Process '{}' has no PID", name)));
        }

        Ok(())
    }

    pub async fn restart_process(&self, name: &str) -> Result<()> {
        let process = self.db.get_process_by_name(name).await?
            .ok_or_else(|| Error::ProcessNotFound(name.to_string()))?;

        // Stop the process if it's running
        if process.pid.is_some() && self.is_process_running(process.pid.unwrap()).await {
            self.stop_process(name).await?;
            // Wait a bit for the process to stop
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }

        // Delete the process record
        self.db.delete_process(name).await?;

        // Start the process again
        self.start_process(
            name,
            &process.command,
            process.args,
            process.env_vars,
            Some(process.working_dir),
        ).await?;

        Ok(())
    }

    pub async fn delete_process(&self, name: &str) -> Result<()> {
        let process = self.db.get_process_by_name(name).await?
            .ok_or_else(|| Error::ProcessNotFound(name.to_string()))?;

        // Stop the process if it's running
        if let Some(pid) = process.pid {
            if self.is_process_running(pid).await {
                self.stop_process(name).await?;
            }
        }

        // Delete from database
        if self.db.delete_process(name).await? {
            // Optionally remove log file
            let _ = tokio::fs::remove_file(&process.log_path).await;
            println!("Process '{}' deleted", name);
        } else {
            return Err(Error::ProcessNotFound(name.to_string()));
        }

        Ok(())
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
}


