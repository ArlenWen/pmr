mod cli;
mod lock;
mod logger;
mod manager;
mod process;
mod storage;

use clap::Parser;
use cli::{Cli, Commands};
use manager::ProcessManager;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let manager = ProcessManager::new()?;

    match cli.command {
        Commands::Start {
            name,
            command,
            args,
            workdir,
            env,
        } => {
            let workdir = workdir.map(PathBuf::from);
            let env_vars = Commands::parse_env_vars(&env)?;
            manager
                .start_process(name, command, args, workdir, env_vars)
                .await?;
        }
        Commands::Stop { name } => {
            manager.stop_process(&name).await?;
        }
        Commands::Restart { name } => {
            manager.restart_process(&name).await?;
        }
        Commands::Delete { name } => {
            manager.delete_process(&name).await?;
        }
        Commands::List => {
            manager.list_processes().await?;
        }
        Commands::Describe { name } => {
            manager.describe_process(&name)?;
        }
        Commands::Logs {
            name,
            lines,
            follow,
        } => {
            if follow {
                manager.follow_logs(&name).await?;
            } else {
                manager.show_logs(&name, lines, false)?;
            }
        }
        Commands::Env { name, vars } => {
            let env_vars = Commands::parse_env_vars(&vars)?;
            manager.set_env_vars(&name, env_vars).await?;
        }
    }

    Ok(())
}
