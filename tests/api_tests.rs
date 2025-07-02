#[cfg(feature = "http-api")]
mod http_api_tests {
    use pmr::{
        api::{ApiServer, AuthManager},
        config::{Config, LogRotationConfig},
        process::ProcessManager,
        database::Database,
    };
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use tempfile::TempDir;
    use tokio::time::{sleep, Duration};

    /// Helper function to create test components
    async fn create_test_components() -> (Arc<ProcessManager>, Arc<Mutex<AuthManager>>, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let log_dir = temp_dir.path().join("logs");

        let config = Config::new()
            .with_database_path(db_path)
            .with_log_dir(log_dir)
            .with_log_rotation(LogRotationConfig {
                enabled: true,
                max_file_size: 1024,
                max_files: 3,
            });

        let process_manager = Arc::new(ProcessManager::new(config).await.unwrap());
        let database = process_manager.get_database();
        let auth_manager = Arc::new(Mutex::new(AuthManager::new(database)));

        (process_manager, auth_manager, temp_dir)
    }

    #[tokio::test]
    async fn test_api_server_creation() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let log_dir = temp_dir.path().join("logs");

        let config = Config::new()
            .with_database_path(db_path)
            .with_log_dir(log_dir)
            .with_log_rotation(LogRotationConfig {
                enabled: true,
                max_file_size: 1024,
                max_files: 3,
            });

        let process_manager = ProcessManager::new(config).await.unwrap();
        let server = ApiServer::new(process_manager, 0);
        assert!(server.is_ok());
    }

    #[tokio::test]
    async fn test_auth_manager_token_generation() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let database_url = format!("sqlite:{}?mode=rwc", db_path.display());
        let database = std::sync::Arc::new(Database::new(&database_url).await.unwrap());
        let auth_manager = AuthManager::new(database);

        // Generate a token
        let api_token = auth_manager.generate_token("test_user".to_string(), None).await.unwrap();
        assert!(!api_token.token.is_empty());

        // Validate the token
        assert!(auth_manager.validate_token(&api_token.token).await);

        // Invalid token should fail
        assert!(!auth_manager.validate_token("invalid_token").await);
    }

    #[tokio::test]
    async fn test_auth_manager_token_revocation() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let database_url = format!("sqlite:{}?mode=rwc", db_path.display());
        let database = std::sync::Arc::new(Database::new(&database_url).await.unwrap());
        let auth_manager = AuthManager::new(database);

        // Generate a token
        let api_token = auth_manager.generate_token("test_user".to_string(), None).await.unwrap();
        assert!(auth_manager.validate_token(&api_token.token).await);

        // Revoke the token
        auth_manager.revoke_token(&api_token.token).await.unwrap();

        // Token should no longer be valid
        assert!(!auth_manager.validate_token(&api_token.token).await);

        // Revoking non-existent token should return error
        let result = auth_manager.revoke_token("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_auth_manager_list_tokens() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let database_url = format!("sqlite:{}?mode=rwc", db_path.display());
        let database = std::sync::Arc::new(Database::new(&database_url).await.unwrap());
        let auth_manager = AuthManager::new(database);

        // Initially should be empty
        let initial_tokens = auth_manager.list_tokens().await.unwrap();
        let initial_count = initial_tokens.len();

        // Generate some tokens
        let api_token1 = auth_manager.generate_token("user1".to_string(), None).await.unwrap();
        let api_token2 = auth_manager.generate_token("user2".to_string(), None).await.unwrap();

        let tokens = auth_manager.list_tokens().await.unwrap();
        // Should have initial count + 2 new tokens
        assert_eq!(tokens.len(), initial_count + 2);

        // Check that our new tokens are in the list
        let token_values: Vec<String> = tokens.iter().map(|t| t.token.clone()).collect();
        assert!(token_values.contains(&api_token1.token));
        assert!(token_values.contains(&api_token2.token));
    }

    #[tokio::test]
    async fn test_auth_manager_persistence() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let database_url = format!("sqlite:{}?mode=rwc", db_path.display());
        let database = std::sync::Arc::new(Database::new(&database_url).await.unwrap());
        let auth_manager = AuthManager::new(database.clone());

        let api_token = auth_manager.generate_token("test_user".to_string(), None).await.unwrap();

        // Verify token is valid
        assert!(auth_manager.validate_token(&api_token.token).await);

        // Create a new AuthManager with the same database to test persistence
        let auth_manager2 = AuthManager::new(database);
        assert!(auth_manager2.validate_token(&api_token.token).await);
    }

    // Integration test with actual HTTP requests would require starting the server
    // and making HTTP requests, which is more complex and might be flaky in CI
    // For now, we focus on unit tests of the components
    
    #[tokio::test]
    async fn test_api_components_integration() {
        let (process_manager, auth_manager, _temp_dir) = create_test_components().await;
        
        // Generate an auth token
        let api_token = {
            let auth = auth_manager.lock().unwrap();
            auth.generate_token("test_user".to_string(), None).await.unwrap()
        };
        
        // Start a test process
        let env_vars = HashMap::new();
        process_manager
            .start_process("api_test", "echo", vec!["Hello API".to_string()], env_vars, None, None)
            .await
            .unwrap();
        
        // Wait for process to complete
        sleep(Duration::from_millis(300)).await;
        
        // Get process status
        let status = process_manager.get_process_status("api_test").await.unwrap();
        assert_eq!(status.name, "api_test");
        
        // Get process logs
        let logs = process_manager.get_process_logs("api_test", None).await.unwrap();
        assert!(logs.contains("Hello API"));
        
        // List processes
        let processes = process_manager.list_processes().await.unwrap();
        assert_eq!(processes.len(), 1);
        assert_eq!(processes[0].name, "api_test");
        
        // Validate the auth token
        let auth = auth_manager.lock().unwrap();
        let is_valid = auth.validate_token(&api_token.token).await;
        assert!(is_valid);
        
        // Clean up
        drop(auth);
        process_manager.delete_process("api_test").await.unwrap();
    }

    #[tokio::test]
    async fn test_api_error_handling() {
        let (process_manager, _auth_manager, _temp_dir) = create_test_components().await;
        
        // Test getting non-existent process
        let result = process_manager.get_process_status("nonexistent").await;
        assert!(result.is_err());
        
        // Test stopping non-existent process
        let result = process_manager.stop_process("nonexistent").await;
        assert!(result.is_err());
        
        // Test deleting non-existent process
        let result = process_manager.delete_process("nonexistent").await;
        assert!(result.is_err());
        
        // Test getting logs for non-existent process
        let result = process_manager.get_process_logs("nonexistent", None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_concurrent_api_operations() {
        let (process_manager, _auth_manager, _temp_dir) = create_test_components().await;
        
        // Start multiple processes concurrently
        let mut handles = vec![];
        
        for i in 0..5 {
            let pm = process_manager.clone();
            let handle = tokio::spawn(async move {
                let env_vars = HashMap::new();
                pm.start_process(
                    &format!("concurrent_{}", i),
                    "echo",
                    vec![format!("Process {}", i)],
                    env_vars,
                    None,
                    None,
                )
                .await
            });
            handles.push(handle);
        }
        
        // Wait for all processes to start
        for handle in handles {
            handle.await.unwrap().unwrap();
        }
        
        // Wait for processes to complete
        sleep(Duration::from_millis(500)).await;
        
        // List all processes
        let processes = process_manager.list_processes().await.unwrap();
        assert_eq!(processes.len(), 5);
        
        // Clean up all processes
        for i in 0..5 {
            process_manager
                .delete_process(&format!("concurrent_{}", i))
                .await
                .unwrap();
        }
        
        // Verify cleanup
        let processes = process_manager.list_processes().await.unwrap();
        assert_eq!(processes.len(), 0);
    }
}

// Placeholder test for when http-api feature is not enabled
#[cfg(not(feature = "http-api"))]
#[test]
fn test_http_api_not_enabled() {
    // This test just ensures the file compiles when http-api feature is disabled
    assert!(true);
}
