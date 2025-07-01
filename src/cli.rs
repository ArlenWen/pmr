use clap::{Parser, Subcommand};
use std::collections::HashMap;

#[derive(Parser)]
#[command(name = "pmr")]
#[command(about = "A process management tool")]
#[command(version = "0.1.0")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Start a new process
    Start {
        /// Process name
        name: String,
        /// Environment variables (key=value format)
        #[arg(short, long)]
        env: Vec<String>,
        /// Working directory
        #[arg(short, long)]
        workdir: Option<String>,
        /// Log directory for this process (default: ./logs)
        #[arg(long)]
        log_dir: Option<String>,
        /// Command to execute
        command: String,
        /// Command arguments
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Stop a running process
    Stop {
        /// Process name
        name: String,
    },
    /// Restart a process
    Restart {
        /// Process name
        name: String,
    },
    /// Delete a process
    Delete {
        /// Process name
        name: String,
    },
    /// List all processes
    List,
    /// Show process status
    Status {
        /// Process name
        name: String,
    },
    /// Show process logs
    Logs {
        /// Process name
        name: String,
        /// Number of lines to show (default: all)
        #[arg(short = 'n', long)]
        lines: Option<usize>,
        /// Show rotated log files
        #[arg(long)]
        rotated: bool,
        /// Manually rotate log file
        #[arg(long)]
        rotate: bool,
    },
}

impl Commands {
    pub fn parse_env_vars(env_strings: Vec<String>) -> HashMap<String, String> {
        let mut env_vars = HashMap::new();
        for env_str in env_strings {
            if let Some((key, value)) = env_str.split_once('=') {
                env_vars.insert(key.to_string(), value.to_string());
            }
        }
        env_vars
    }
}
