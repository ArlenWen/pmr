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
        Commands::Serve { port } => {
            let api_server = ApiServer::new(process_manager, port)?;
            println!("Starting PMR HTTP API server on port {}...", port);
            println!("Use 'pmr auth generate <name>' to create API tokens for authentication");
            api_server.start().await?;
        }
        #[cfg(feature = "http-api")]
        Commands::Auth { command } => {
            handle_auth_command(command).await?;
        }
    }

    Ok(())
}

#[cfg(feature = "http-api")]
async fn handle_auth_command(command: AuthCommands) -> Result<(), Box<dyn std::error::Error>> {
    let mut auth_manager = AuthManager::new()?;

    match command {
        AuthCommands::Generate { name, expires_in } => {
            let token = auth_manager.generate_token(name.clone(), expires_in)?;
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
            let tokens = auth_manager.list_tokens();
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
            match auth_manager.revoke_token(&token) {
                Ok(_) => println!("Token revoked successfully"),
                Err(e) => println!("Error revoking token: {}", e),
            }
        }
    }

    Ok(())
}