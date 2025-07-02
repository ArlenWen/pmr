use clap::Parser;
use pmr::{
    cli::{Cli, Commands},
    config::Config,
    formatter::Formatter,
    process::ProcessManager,
};

#[cfg(feature = "http-api")]
use pmr::{
    api::{ApiServer, AuthManager},
    cli::AuthCommands,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let formatter = Formatter::new(cli.format.clone());
    let config = Config::new();
    let process_manager = ProcessManager::new(config).await?;

    match cli.command {
        Commands::Start { name, command, args, env, workdir, log_dir } => {
            let env_vars = Commands::parse_env_vars(env);
            let message = process_manager.start_process(&name, &command, args, env_vars, workdir, log_dir).await?;
            println!("{}", formatter.format_success_message(&message));
        }
        Commands::Stop { name } => {
            let message = process_manager.stop_process(&name).await?;
            println!("{}", formatter.format_success_message(&message));
        }
        Commands::Restart { name } => {
            let message = process_manager.restart_process(&name).await?;
            println!("{}", formatter.format_success_message(&message));
        }
        Commands::Delete { name } => {
            let message = process_manager.delete_process(&name).await?;
            println!("{}", formatter.format_success_message(&message));
        }
        Commands::Clear { all } => {
            let result = process_manager.clear_processes(all).await?;
            println!("{}", formatter.format_clear_result(&result));
        }
        Commands::List => {
            let processes = process_manager.list_processes().await?;
            if processes.is_empty() {
                println!("{}", formatter.format_empty_list_message("No processes found."));
            } else {
                println!("{}", formatter.format_process_list(&processes));
            }
        }
        Commands::Status { name } => {
            let process = process_manager.get_process_status(&name).await?;
            println!("{}", formatter.format_process_status(&process));
        }
        Commands::Logs { name, lines, rotated, rotate } => {
            if rotate {
                let message = process_manager.rotate_process_logs(&name).await?;
                println!("{}", formatter.format_success_message(&message));
            } else if rotated {
                let rotated_logs = process_manager.get_rotated_logs(&name).await?;
                println!("{}", formatter.format_rotated_logs(&rotated_logs, &name));
            } else {
                let logs = process_manager.get_process_logs(&name, lines).await?;
                println!("{}", formatter.format_process_logs(&logs, &name));
            }
        }
        #[cfg(feature = "http-api")]
        Commands::Serve { port, daemon } => {
            if daemon {
                handle_serve_daemon(port, &process_manager, &formatter).await?;
            } else {
                let api_server = ApiServer::new(process_manager, port)?;
                println!("Starting PMR HTTP API server on port {}...", port);
                println!("Use 'pmr auth generate <name>' to create API tokens for authentication");
                api_server.start().await?;
            }
        }
        #[cfg(feature = "http-api")]
        Commands::ServeStatus => {
            handle_serve_status(&process_manager, &formatter).await?;
        }
        #[cfg(feature = "http-api")]
        Commands::ServeStop => {
            handle_serve_stop(&process_manager, &formatter).await?;
        }
        #[cfg(feature = "http-api")]
        Commands::ServeRestart { port } => {
            handle_serve_restart(port, &process_manager, &formatter).await?;
        }
        #[cfg(feature = "http-api")]
        Commands::Auth { command } => {
            handle_auth_command(command, &process_manager).await?;
        }
    }

    Ok(())
}

#[cfg(feature = "http-api")]
async fn handle_auth_command(command: AuthCommands, process_manager: &ProcessManager) -> Result<(), Box<dyn std::error::Error>> {
    let database = process_manager.get_database();
    let auth_manager = AuthManager::new(database);

    match command {
        AuthCommands::Generate { name, expires_in } => {
            let token = auth_manager.generate_token(name.clone(), expires_in).await?;
            println!("Generated new API token:");
            println!("Name: {}", token.name);
            println!("Token: {}", token.token);
            println!("Created: {}", token.created_at.format("%Y-%m-%d %H:%M:%S UTC"));
            if let Some(expires_at) = token.expires_at {
                println!("Expires: {}", expires_at.format("%Y-%m-%d %H:%M:%S UTC"));
            } else {
                println!("Expires: Never");
            }
            println!();
            println!("Use this token in API requests:");
            println!("Authorization: Bearer {}", token.token);
        }
        AuthCommands::List => {
            let tokens = auth_manager.list_tokens().await?;
            if tokens.is_empty() {
                println!("No API tokens found.");
            } else {
                println!("{:<20} {:<10} {:<20} {:<20}", "NAME", "STATUS", "CREATED", "EXPIRES");
                println!("{}", "-".repeat(80));
                for token in tokens {
                    let status = if token.is_active { "active" } else { "revoked" };
                    let expires = token.expires_at
                        .map(|e| e.format("%Y-%m-%d %H:%M:%S").to_string())
                        .unwrap_or_else(|| "Never".to_string());
                    println!("{:<20} {:<10} {:<20} {:<20}",
                        token.name,
                        status,
                        token.created_at.format("%Y-%m-%d %H:%M:%S"),
                        expires
                    );
                }
            }
        }
        AuthCommands::Revoke { token } => {
            match auth_manager.revoke_token(&token).await {
                Ok(_) => println!("Token revoked successfully"),
                Err(e) => println!("Error revoking token: {}", e),
            }
        }
    }

    Ok(())
}

#[cfg(feature = "http-api")]
const HTTP_SERVER_PROCESS_NAME: &str = "__pmr_http_server__";

#[cfg(feature = "http-api")]
async fn handle_serve_daemon(
    port: u16,
    process_manager: &ProcessManager,
    formatter: &Formatter,
) -> Result<(), Box<dyn std::error::Error>> {
    // Check if HTTP server is already running
    if let Ok(process) = process_manager.get_process_status(HTTP_SERVER_PROCESS_NAME).await {
        if process.status == pmr::database::ProcessStatus::Running {
            println!("{}", formatter.format_error_message("HTTP server is already running. Use 'pmr serve-status' to check status or 'pmr serve-stop' to stop it."));
            return Ok(());
        } else {
            // Process exists but is not running, delete it first
            let _ = process_manager.delete_process(HTTP_SERVER_PROCESS_NAME).await;
        }
    }

    // Get current executable path
    let current_exe = std::env::current_exe()?;
    let current_exe_str = current_exe.to_string_lossy().to_string();

    // Start HTTP server as a managed process
    let args = vec!["serve".to_string(), "--port".to_string(), port.to_string()];
    let env_vars = std::collections::HashMap::new();

    let message = process_manager
        .start_process(
            HTTP_SERVER_PROCESS_NAME,
            &current_exe_str,
            args,
            env_vars,
            None,
            None,
        )
        .await?;

    println!("{}", formatter.format_success_message(&message));
    println!("HTTP server started in daemon mode on port {}", port);
    println!("Use 'pmr serve-status' to check status");
    println!("Use 'pmr serve-stop' to stop the server");

    Ok(())
}

#[cfg(feature = "http-api")]
async fn handle_serve_status(
    process_manager: &ProcessManager,
    formatter: &Formatter,
) -> Result<(), Box<dyn std::error::Error>> {
    match process_manager.get_process_status(HTTP_SERVER_PROCESS_NAME).await {
        Ok(process) => {
            println!("{}", formatter.format_process_status(&process));
        }
        Err(_) => {
            println!("{}", formatter.format_error_message("HTTP server is not running"));
        }
    }
    Ok(())
}

#[cfg(feature = "http-api")]
async fn handle_serve_stop(
    process_manager: &ProcessManager,
    formatter: &Formatter,
) -> Result<(), Box<dyn std::error::Error>> {
    match process_manager.stop_process(HTTP_SERVER_PROCESS_NAME).await {
        Ok(message) => {
            println!("{}", formatter.format_success_message(&message));
        }
        Err(_) => {
            println!("{}", formatter.format_error_message("HTTP server is not running"));
        }
    }
    Ok(())
}

#[cfg(feature = "http-api")]
async fn handle_serve_restart(
    port: u16,
    process_manager: &ProcessManager,
    formatter: &Formatter,
) -> Result<(), Box<dyn std::error::Error>> {
    // Check if server exists and try to stop it
    match process_manager.get_process_status(HTTP_SERVER_PROCESS_NAME).await {
        Ok(process) => {
            if process.status == pmr::database::ProcessStatus::Running {
                println!("Stopping HTTP server...");
                let _ = process_manager.stop_process(HTTP_SERVER_PROCESS_NAME).await;
                // Wait a moment for the process to fully stop
                tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
            }
        }
        Err(_) => {
            // Server doesn't exist, that's fine
        }
    }

    // Delete the old process record if it exists
    let _ = process_manager.delete_process(HTTP_SERVER_PROCESS_NAME).await;

    // Start the server again
    println!("Starting HTTP server...");
    handle_serve_daemon(port, process_manager, formatter).await
}