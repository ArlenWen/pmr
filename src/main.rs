use clap::Parser;
use pmr::{
    cli::{Cli, Commands},
    config::Config,
    process::ProcessManager,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let config = Config::new();
    let process_manager = ProcessManager::new(config).await?;

    match cli.command {
        Commands::Start { name, command, args, env, workdir, log_dir } => {
            let env_vars = Commands::parse_env_vars(env);
            process_manager.start_process(&name, &command, args, env_vars, workdir, log_dir).await?;
        }
        Commands::Stop { name } => {
            process_manager.stop_process(&name).await?;
        }
        Commands::Restart { name } => {
            process_manager.restart_process(&name).await?;
        }
        Commands::Delete { name } => {
            process_manager.delete_process(&name).await?;
        }
        Commands::List => {
            let processes = process_manager.list_processes().await?;
            if processes.is_empty() {
                println!("No processes found.");
            } else {
                println!("{:<20} {:<10} {:<10} {:<30} {:<20}", "NAME", "STATUS", "PID", "COMMAND", "CREATED");
                println!("{}", "-".repeat(90));
                for process in processes {
                    let pid_str = process.pid.map(|p| p.to_string()).unwrap_or_else(|| "-".to_string());
                    let created_str = process.created_at.format("%Y-%m-%d %H:%M:%S").to_string();
                    println!("{:<20} {:<10} {:<10} {:<30} {:<20}",
                        process.name,
                        process.status,
                        pid_str,
                        format!("{} {}", process.command, process.args.join(" ")),
                        created_str
                    );
                }
            }
        }
        Commands::Status { name } => {
            let process = process_manager.get_process_status(&name).await?;
            println!("Process: {}", process.name);
            println!("Status: {}", process.status);
            println!("PID: {}", process.pid.map(|p| p.to_string()).unwrap_or_else(|| "N/A".to_string()));
            println!("Command: {} {}", process.command, process.args.join(" "));
            println!("Working Directory: {}", process.working_dir);
            println!("Created: {}", process.created_at.format("%Y-%m-%d %H:%M:%S"));
            println!("Updated: {}", process.updated_at.format("%Y-%m-%d %H:%M:%S"));
            println!("Log File: {}", process.log_path);
            if !process.env_vars.is_empty() {
                println!("Environment Variables:");
                for (key, value) in &process.env_vars {
                    println!("  {}={}", key, value);
                }
            }
        }
        Commands::Logs { name, lines, rotated, rotate } => {
            if rotate {
                process_manager.rotate_process_logs(&name).await?;
            } else if rotated {
                let rotated_logs = process_manager.get_rotated_logs(&name).await?;
                if rotated_logs.is_empty() {
                    println!("No rotated log files found for process '{}'", name);
                } else {
                    for log in rotated_logs {
                        println!("{}", log);
                    }
                }
            } else {
                let logs = process_manager.get_process_logs(&name, lines).await?;
                println!("{}", logs);
            }
        }
    }

    Ok(())
}