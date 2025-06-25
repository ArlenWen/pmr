use clap::{Parser, Subcommand};
use std::collections::HashMap;

#[derive(Parser)]
#[command(name = "pmr")]
#[command(about = "A process manager for Linux")]
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
        /// Command to execute
        command: String,
        /// Command arguments
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
        /// Working directory
        #[arg(short, long)]
        workdir: Option<String>,
        /// Environment variables (key=value format)
        #[arg(short, long)]
        env: Vec<String>,
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
    /// Delete a process configuration
    Delete {
        /// Process name
        name: String,
    },
    /// List all processes
    List,
    /// Show detailed process information
    Describe {
        /// Process name
        name: String,
    },
    /// Show process logs
    Logs {
        /// Process name
        name: String,
        /// Number of lines to show
        #[arg(short, long, default_value = "50")]
        lines: usize,
        /// Follow log output
        #[arg(short, long)]
        follow: bool,
    },
    /// Set environment variables for a process
    Env {
        /// Process name
        name: String,
        /// Environment variables to set (key=value format)
        vars: Vec<String>,
    },
}

impl Commands {
    pub fn parse_env_vars(env_strings: &[String]) -> anyhow::Result<HashMap<String, String>> {
        let mut env_vars = HashMap::new();
        for env_str in env_strings {
            if let Some((key, value)) = env_str.split_once('=') {
                env_vars.insert(key.to_string(), value.to_string());
            } else {
                return Err(anyhow::anyhow!("Invalid environment variable format: {}", env_str));
            }
        }
        Ok(env_vars)
    }
}
