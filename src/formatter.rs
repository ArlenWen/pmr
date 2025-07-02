use crate::{
    cli::OutputFormat,
    database::ProcessRecord,
    process::ClearResult,
};
use serde::{Deserialize, Serialize};

/// Formatter for different output formats
pub struct Formatter {
    format: OutputFormat,
}

impl Formatter {
    pub fn new(format: OutputFormat) -> Self {
        Self { format }
    }

    /// Format process list output
    pub fn format_process_list(&self, processes: &[ProcessRecord]) -> String {
        match self.format {
            OutputFormat::Text => self.format_process_list_text(processes),
            OutputFormat::Json => self.format_process_list_json(processes),
        }
    }

    /// Format single process status output
    pub fn format_process_status(&self, process: &ProcessRecord) -> String {
        match self.format {
            OutputFormat::Text => self.format_process_status_text(process),
            OutputFormat::Json => self.format_process_status_json(process),
        }
    }

    /// Format process logs output
    pub fn format_process_logs(&self, logs: &str, process_name: &str) -> String {
        match self.format {
            OutputFormat::Text => logs.to_string(),
            OutputFormat::Json => {
                let log_output = LogOutput {
                    process_name: process_name.to_string(),
                    logs: logs.to_string(),
                };
                serde_json::to_string_pretty(&log_output).unwrap_or_else(|_| "{}".to_string())
            }
        }
    }

    /// Format rotated logs list output
    pub fn format_rotated_logs(&self, logs: &[String], process_name: &str) -> String {
        match self.format {
            OutputFormat::Text => {
                if logs.is_empty() {
                    format!("No rotated log files found for process '{}'", process_name)
                } else {
                    logs.join("\n")
                }
            }
            OutputFormat::Json => {
                let rotated_logs_output = RotatedLogsOutput {
                    process_name: process_name.to_string(),
                    rotated_logs: logs.to_vec(),
                };
                serde_json::to_string_pretty(&rotated_logs_output).unwrap_or_else(|_| "{}".to_string())
            }
        }
    }

    /// Format clear result output
    pub fn format_clear_result(&self, result: &ClearResult) -> String {
        match self.format {
            OutputFormat::Text => self.format_clear_result_text(result),
            OutputFormat::Json => self.format_clear_result_json(result),
        }
    }

    /// Format simple success message
    pub fn format_success_message(&self, message: &str) -> String {
        match self.format {
            OutputFormat::Text => message.to_string(),
            OutputFormat::Json => {
                let response = SimpleResponse {
                    success: true,
                    message: message.to_string(),
                };
                serde_json::to_string_pretty(&response).unwrap_or_else(|_| "{}".to_string())
            }
        }
    }

    /// Format error message
    pub fn format_error_message(&self, message: &str) -> String {
        match self.format {
            OutputFormat::Text => message.to_string(),
            OutputFormat::Json => {
                let response = SimpleResponse {
                    success: false,
                    message: message.to_string(),
                };
                serde_json::to_string_pretty(&response).unwrap_or_else(|_| "{}".to_string())
            }
        }
    }

    /// Format empty list message
    pub fn format_empty_list_message(&self, message: &str) -> String {
        match self.format {
            OutputFormat::Text => message.to_string(),
            OutputFormat::Json => {
                let response = EmptyListResponse {
                    processes: Vec::new(),
                    message: message.to_string(),
                };
                serde_json::to_string_pretty(&response).unwrap_or_else(|_| "{}".to_string())
            }
        }
    }

    // Private methods for text formatting
    fn format_process_list_text(&self, processes: &[ProcessRecord]) -> String {
        let mut output = String::new();
        output.push_str(&format!("{:<20} {:<10} {:<10} {:<30} {:<20}", "NAME", "STATUS", "PID", "COMMAND", "CREATED"));
        output.push('\n');
        output.push_str(&"-".repeat(90));
        output.push('\n');
        
        for process in processes {
            let pid_str = process.pid.map(|p| p.to_string()).unwrap_or_else(|| "-".to_string());
            let created_str = process.created_at.format("%Y-%m-%d %H:%M:%S").to_string();
            output.push_str(&format!(
                "{:<20} {:<10} {:<10} {:<30} {:<20}",
                process.name,
                process.status,
                pid_str,
                format!("{} {}", process.command, process.args.join(" ")),
                created_str
            ));
            output.push('\n');
        }
        
        output
    }

    fn format_process_list_json(&self, processes: &[ProcessRecord]) -> String {
        let process_list = ProcessListOutput {
            processes: processes.to_vec(),
        };
        serde_json::to_string_pretty(&process_list).unwrap_or_else(|_| "{}".to_string())
    }

    fn format_process_status_text(&self, process: &ProcessRecord) -> String {
        let mut output = String::new();
        output.push_str(&format!("Process: {}\n", process.name));
        output.push_str(&format!("Status: {}\n", process.status));
        output.push_str(&format!("PID: {}\n", process.pid.map(|p| p.to_string()).unwrap_or_else(|| "N/A".to_string())));
        output.push_str(&format!("Command: {} {}\n", process.command, process.args.join(" ")));
        output.push_str(&format!("Working Directory: {}\n", process.working_dir));
        output.push_str(&format!("Created: {}\n", process.created_at.format("%Y-%m-%d %H:%M:%S")));
        output.push_str(&format!("Updated: {}\n", process.updated_at.format("%Y-%m-%d %H:%M:%S")));
        output.push_str(&format!("Log File: {}\n", process.log_path));
        
        if !process.env_vars.is_empty() {
            output.push_str("Environment Variables:\n");
            for (key, value) in &process.env_vars {
                output.push_str(&format!("  {}={}\n", key, value));
            }
        }
        
        output
    }

    fn format_process_status_json(&self, process: &ProcessRecord) -> String {
        serde_json::to_string_pretty(process).unwrap_or_else(|_| "{}".to_string())
    }
}

// Helper structs for JSON output
#[derive(Serialize, Deserialize)]
struct ProcessListOutput {
    processes: Vec<ProcessRecord>,
}

#[derive(Serialize, Deserialize)]
struct LogOutput {
    process_name: String,
    logs: String,
}

#[derive(Serialize, Deserialize)]
struct RotatedLogsOutput {
    process_name: String,
    rotated_logs: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct SimpleResponse {
    success: bool,
    message: String,
}

#[derive(Serialize, Deserialize)]
struct EmptyListResponse {
    processes: Vec<ProcessRecord>,
    message: String,
}

impl Formatter {
    // Private methods for clear result formatting
    fn format_clear_result_text(&self, result: &ClearResult) -> String {
        let mut output = String::new();

        if result.cleared_count == 0 {
            output.push_str(&format!("No {} to clear.", result.operation_type));
        } else {
            output.push_str(&format!("Cleared {} {} ({} processes):",
                result.cleared_count,
                result.operation_type,
                result.cleared_count
            ));
            output.push('\n');

            for process_name in &result.cleared_processes {
                output.push_str(&format!("  - {}", process_name));
                output.push('\n');
            }
        }

        if !result.failed_processes.is_empty() {
            output.push('\n');
            output.push_str(&format!("Failed to clear {} processes:", result.failed_processes.len()));
            output.push('\n');
            for process_name in &result.failed_processes {
                output.push_str(&format!("  - {}", process_name));
                output.push('\n');
            }
        }

        output.trim_end().to_string()
    }

    fn format_clear_result_json(&self, result: &ClearResult) -> String {
        serde_json::to_string_pretty(result).unwrap_or_else(|_| "{}".to_string())
    }
}


