use clap::{Parser, Subcommand, ValueEnum};
use std::collections::HashMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, ValueEnum, Serialize, Deserialize)]
pub enum OutputFormat {
    /// Plain text output (default)
    Text,
    /// JSON formatted output
    Json,
}

impl Default for OutputFormat {
    fn default() -> Self {
        OutputFormat::Text
    }
}

#[cfg(feature = "http-api")]
#[derive(Subcommand)]
pub enum AuthCommands {
    /// Generate a new API token
    Generate {
        /// Token name/description
        name: String,
        /// Token expiration in days (optional)
        #[arg(long)]
        expires_in: Option<u32>,
    },
    /// List all API tokens
    List,
    /// Revoke an API token
    Revoke {
        /// Token to revoke
        token: String,
    },
}

#[derive(Parser)]
#[command(name = "pmr")]
#[command(about = "A process management tool")]
#[command(version = "0.2.0")]
pub struct Cli {
    /// Output format
    #[arg(long, value_enum, default_value_t = OutputFormat::default())]
    pub format: OutputFormat,

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
    /// Clear stopped/failed processes or all processes
    Clear {
        /// Clear all processes regardless of status
        #[arg(long)]
        all: bool,
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
    #[cfg(feature = "http-api")]
    /// Start HTTP API server
    Serve {
        /// Port to bind the API server (default: 8080)
        #[arg(short, long, default_value = "8080")]
        port: u16,
    },
    #[cfg(feature = "http-api")]
    /// Manage API authentication tokens
    Auth {
        #[command(subcommand)]
        command: AuthCommands,
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
