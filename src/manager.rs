use crate::logger::Logger;
use crate::process::{ProcessConfig, ProcessState, ProcessStatus};
use crate::storage::Storage;
use anyhow::{Context, Result};
use nix::sys::signal::{self, Signal};
use nix::unistd::Pid;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::Command as TokioCommand;

pub struct ProcessManager {
    storage: Storage,
}

impl ProcessManager {
    pub fn new() -> Result<Self> {
        let storage = Storage::new()?;
        Ok(Self { storage })
    }

    pub async fn start_process(
        &self,
        name: String,
        command: String,
        args: Vec<String>,
        workdir: Option<PathBuf>,
        env_vars: HashMap<String, String>,
    ) -> Result<()> {
        // Check if process already exists
        if self.storage.process_exists(&name)? {
            return Err(anyhow::anyhow!("Process '{}' already exists", name));
        }

        // Create process configuration
        let config = ProcessConfig::new(name.clone(), command, args, workdir, env_vars);
        let mut process_state = ProcessState::new(config, self.storage.data_dir());

        // Create log files
        let (stdout_path, stderr_path) = self.storage.create_log_files(&name)?;

        // Start the process
        let mut cmd = TokioCommand::new(&process_state.config.command);
        cmd.args(&process_state.config.args);

        // Set working directory
        if let Some(ref workdir) = process_state.config.workdir {
            cmd.current_dir(workdir);
        }

        // Set environment variables
        for (key, value) in &process_state.config.env_vars {
            cmd.env(key, value);
        }

        // Configure stdio to redirect to files directly
        let stdout_file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&stdout_path)
            .context("Failed to create stdout log file")?;

        let stderr_file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&stderr_path)
            .context("Failed to create stderr log file")?;

        cmd.stdout(Stdio::from(stdout_file))
            .stderr(Stdio::from(stderr_file))
            .stdin(Stdio::null());

        // Spawn the process
        let spawn_result = cmd.spawn();

        match spawn_result {
            Ok(mut child) => {
                // Process started successfully
                let pid = child.id().context("Failed to get process ID")?;
                process_state.start(pid);

                // Save process state
                self.storage.save_process(&process_state)?;

                // Monitor process in background
                let storage = Storage::new()?;
                let process_name = name.clone();
                tokio::spawn(async move {
                    let status = child.wait().await;
                    let mut processes = storage.load_processes().unwrap_or_default();

                    if let Some(process) = processes.get_mut(&process_name) {
                        match status {
                            Ok(exit_status) => {
                                if exit_status.success() {
                                    process.stop();
                                } else {
                                    process.fail();
                                }
                            }
                            Err(_) => {
                                process.fail();
                            }
                        }
                        let _ = storage.save_processes(&processes);
                    }
                });

                println!("Process '{}' started with PID {}", name, pid);
                Ok(())
            }
            Err(spawn_error) => {
                // Process failed to start - save the failed state
                process_state.fail();
                self.storage.save_process(&process_state)?;

                // Write error message to stderr log
                if let Err(write_err) = std::fs::write(&stderr_path, format!("Failed to spawn process: {}\n", spawn_error)) {
                    eprintln!("Warning: Failed to write error to log file: {}", write_err);
                }

                Err(anyhow::anyhow!("Failed to spawn process: {}", spawn_error))
            }
        }
    }



    pub fn stop_process(&self, name: &str) -> Result<()> {
        let mut processes = self.storage.load_processes()?;
        
        let process = processes.get_mut(name)
            .ok_or_else(|| anyhow::anyhow!("Process '{}' not found", name))?;

        if !process.is_running() {
            return Err(anyhow::anyhow!("Process '{}' is not running", name));
        }

        let pid = process.pid.unwrap();
        
        // Send SIGTERM first
        if let Err(e) = signal::kill(Pid::from_raw(pid as i32), Signal::SIGTERM) {
            return Err(anyhow::anyhow!("Failed to send SIGTERM to process {}: {}", pid, e));
        }

        process.stop();
        self.storage.save_processes(&processes)?;

        println!("Process '{}' stopped", name);
        Ok(())
    }

    pub async fn restart_process(&self, name: &str) -> Result<()> {
        let process = self.storage.get_process(name)?
            .ok_or_else(|| anyhow::anyhow!("Process '{}' not found", name))?;

        // Stop if running
        if process.is_running() {
            self.stop_process(name)?;
            // Wait a bit for the process to stop
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }

        // Start again
        self.start_process(
            process.config.name,
            process.config.command,
            process.config.args,
            process.config.workdir,
            process.config.env_vars,
        ).await
    }

    pub fn delete_process(&self, name: &str) -> Result<()> {
        let processes = self.storage.load_processes()?;
        
        if let Some(process) = processes.get(name) {
            if process.is_running() {
                return Err(anyhow::anyhow!("Cannot delete running process '{}'. Stop it first.", name));
            }
        }

        let deleted = self.storage.delete_process(name)?;
        
        if deleted {
            println!("Process '{}' deleted", name);
        } else {
            return Err(anyhow::anyhow!("Process '{}' not found", name));
        }

        Ok(())
    }

    pub fn list_processes(&self) -> Result<()> {
        let mut processes = self.storage.load_processes()?;

        if processes.is_empty() {
            println!("No processes found");
            return Ok(());
        }

        // Check and update process statuses before displaying
        let mut status_updated = false;
        for (_name, process) in processes.iter_mut() {
            // Only check processes that are marked as running and have a PID
            if process.status == ProcessStatus::Running {
                if let Some(pid) = process.pid {
                    // Check if process is actually running using signal::kill with signal 0
                    match signal::kill(Pid::from_raw(pid as i32), None) {
                        Ok(_) => {
                            // Process is still running, status is correct
                        }
                        Err(_) => {
                            // Process is not running but status is still "Running"
                            // This means the process exited but the background monitor hasn't updated yet
                            // We mark as stopped since we can't determine the exit status
                            process.stop();
                            status_updated = true;
                        }
                    }
                } else {
                    // Status is Running but no PID - this shouldn't happen, but fix it
                    process.status = ProcessStatus::Unknown;
                    status_updated = true;
                }
            }
            // For processes with status Stopped, Failed, or Unknown, we don't need to check
            // as they have already been processed or are in a final state
        }

        // Save updated statuses if any changes were made
        if status_updated {
            self.storage.save_processes(&processes)?;
        }

        println!("{:<20} {:<10} {:<10} {:<20}", "NAME", "STATUS", "PID", "COMMAND");
        println!("{}", "-".repeat(70));

        for (name, process) in processes {
            let pid_str = process.pid.map_or("N/A".to_string(), |p| p.to_string());
            let command = format!("{} {}", process.config.command, process.config.args.join(" "));
            let command_display = if command.len() > 20 {
                format!("{}...", &command[..17])
            } else {
                command
            };

            println!(
                "{:<20} {:<10} {:<10} {:<20}",
                name,
                process.status,
                pid_str,
                command_display
            );
        }

        Ok(())
    }

    pub fn describe_process(&self, name: &str) -> Result<()> {
        let process = self.storage.get_process(name)?
            .ok_or_else(|| anyhow::anyhow!("Process '{}' not found", name))?;

        println!("Process: {}", process.config.name);
        println!("ID: {}", process.config.id);
        println!("Status: {}", process.status);
        println!("Command: {} {}", process.config.command, process.config.args.join(" "));

        if let Some(ref workdir) = process.config.workdir {
            println!("Working Directory: {}", workdir.display());
        }

        if let Some(pid) = process.pid {
            println!("PID: {}", pid);
        }

        println!("Restart Count: {}", process.restart_count);
        println!("Created: {}", process.config.created_at.format("%Y-%m-%d %H:%M:%S UTC"));
        println!("Updated: {}", process.config.updated_at.format("%Y-%m-%d %H:%M:%S UTC"));

        if let Some(started_at) = process.started_at {
            println!("Started: {}", started_at.format("%Y-%m-%d %H:%M:%S UTC"));
        }

        if let Some(stopped_at) = process.stopped_at {
            println!("Stopped: {}", stopped_at.format("%Y-%m-%d %H:%M:%S UTC"));
        }

        println!("Stdout Log: {}", process.stdout_path.display());
        println!("Stderr Log: {}", process.stderr_path.display());

        if !process.config.env_vars.is_empty() {
            println!("Environment Variables:");
            for (key, value) in &process.config.env_vars {
                println!("  {}={}", key, value);
            }
        }

        Ok(())
    }

    pub fn show_logs(&self, name: &str, lines: usize, follow: bool) -> Result<()> {
        let process = self.storage.get_process(name)?
            .ok_or_else(|| anyhow::anyhow!("Process '{}' not found", name))?;

        if follow {
            println!("Following logs for process '{}'...", name);
            println!("Press Ctrl+C to stop");

            // This will block, so we need to handle it in an async context
            return Err(anyhow::anyhow!("Follow mode should be handled in async context"));
        } else {
            println!("=== STDOUT ===");
            Logger::tail_logs(&process.stdout_path, lines)?;

            println!("\n=== STDERR ===");
            Logger::tail_logs(&process.stderr_path, lines)?;
        }

        Ok(())
    }

    pub async fn follow_logs(&self, name: &str) -> Result<()> {
        let process = self.storage.get_process(name)?
            .ok_or_else(|| anyhow::anyhow!("Process '{}' not found", name))?;

        println!("Following logs for process '{}'...", name);

        // For simplicity, we'll follow stdout. In a real implementation,
        // you might want to follow both stdout and stderr in parallel
        Logger::follow_logs(&process.stdout_path).await
    }

    pub fn set_env_vars(&self, name: &str, env_vars: HashMap<String, String>) -> Result<()> {
        let mut processes = self.storage.load_processes()?;

        let process = processes.get_mut(name)
            .ok_or_else(|| anyhow::anyhow!("Process '{}' not found", name))?;

        if process.is_running() {
            return Err(anyhow::anyhow!(
                "Cannot modify environment variables of running process '{}'. Stop it first.",
                name
            ));
        }

        process.config.update_env_vars(env_vars.clone());
        self.storage.save_processes(&processes)?;

        println!("Environment variables updated for process '{}':", name);
        for (key, value) in env_vars {
            println!("  {}={}", key, value);
        }

        Ok(())
    }

    pub fn check_process_status(&self, name: &str) -> Result<()> {
        let mut processes = self.storage.load_processes()?;

        let process = processes.get_mut(name)
            .ok_or_else(|| anyhow::anyhow!("Process '{}' not found", name))?;

        if let Some(pid) = process.pid {
            // Check if process is actually running
            match signal::kill(Pid::from_raw(pid as i32), None) {
                Ok(_) => {
                    // Process is running
                    if process.status != ProcessStatus::Running {
                        process.status = ProcessStatus::Running;
                        self.storage.save_processes(&processes)?;
                    }
                }
                Err(_) => {
                    // Process is not running
                    if process.status == ProcessStatus::Running {
                        // Process was running but now it's not
                        // Mark as stopped since we can't determine the exit reason
                        process.stop();
                        self.storage.save_processes(&processes)?;
                    }
                }
            }
        }

        Ok(())
    }
}
