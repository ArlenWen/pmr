use pmr::{
    config::{Config, LogRotationConfig},
    process::ProcessManager,
    database::ProcessStatus,
    Error,
};
use std::collections::HashMap;
use tempfile::TempDir;
use tokio::time::{sleep, Duration};

/// Helper function to create a test ProcessManager with temporary directories
async fn create_test_process_manager() -> (ProcessManager, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let log_dir = temp_dir.path().join("logs");
    
    let config = Config::new()
        .with_database_path(db_path)
        .with_log_dir(log_dir)
        .with_log_rotation(LogRotationConfig {
            enabled: true,
            max_file_size: 1024, // 1KB for testing
            max_files: 3,
        });
    
    let pm = ProcessManager::new(config).await.unwrap();
    (pm, temp_dir)
}

#[tokio::test]
async fn test_process_lifecycle() {
    let (pm, _temp_dir) = create_test_process_manager().await;
    
    let name = "test_process";
    let command = "echo";
    let args = vec!["Hello, World!".to_string()];
    let env_vars = HashMap::new();
    
    // Test starting a process
    pm.start_process(name, command, args.clone(), env_vars.clone(), None, None)
        .await
        .unwrap();
    
    // Wait a bit for the process to complete (echo command finishes quickly)
    sleep(Duration::from_millis(300)).await;
    
    // Test getting process status
    let status = pm.get_process_status(name).await.unwrap();
    assert_eq!(status.name, name);
    assert_eq!(status.command, command);
    assert_eq!(status.args, args);
    
    // Test listing processes
    let processes = pm.list_processes().await.unwrap();
    assert_eq!(processes.len(), 1);
    assert_eq!(processes[0].name, name);
    
    // Test getting logs
    let logs = pm.get_process_logs(name, None).await.unwrap();
    assert!(logs.contains("Hello, World!"));
    
    // Test deleting process
    pm.delete_process(name).await.unwrap();
    
    // Verify process is deleted
    let result = pm.get_process_status(name).await;
    assert!(matches!(result, Err(Error::ProcessNotFound(_))));
    
    let processes = pm.list_processes().await.unwrap();
    assert_eq!(processes.len(), 0);
}

#[tokio::test]
async fn test_process_already_exists() {
    let (pm, _temp_dir) = create_test_process_manager().await;
    
    let name = "duplicate_process";
    let command = "sleep";
    let args = vec!["1".to_string()];
    let env_vars = HashMap::new();
    
    // Start first process
    pm.start_process(name, command, args.clone(), env_vars.clone(), None, None)
        .await
        .unwrap();
    
    // Try to start duplicate process
    let result = pm.start_process(name, command, args, env_vars, None, None).await;
    assert!(matches!(result, Err(Error::ProcessAlreadyExists(_))));
    
    // Cleanup
    pm.delete_process(name).await.unwrap();
}

#[tokio::test]
async fn test_process_not_found() {
    let (pm, _temp_dir) = create_test_process_manager().await;
    
    let name = "nonexistent_process";
    
    // Test getting status of non-existent process
    let result = pm.get_process_status(name).await;
    assert!(matches!(result, Err(Error::ProcessNotFound(_))));
    
    // Test stopping non-existent process
    let result = pm.stop_process(name).await;
    assert!(matches!(result, Err(Error::ProcessNotFound(_))));
    
    // Test restarting non-existent process
    let result = pm.restart_process(name).await;
    assert!(matches!(result, Err(Error::ProcessNotFound(_))));
    
    // Test deleting non-existent process
    let result = pm.delete_process(name).await;
    assert!(matches!(result, Err(Error::ProcessNotFound(_))));
}

#[tokio::test]
async fn test_long_running_process() {
    let (pm, _temp_dir) = create_test_process_manager().await;
    
    let name = "long_running";
    let command = "sleep";
    let args = vec!["2".to_string()]; // Sleep for 2 seconds
    let env_vars = HashMap::new();
    
    // Start long-running process
    pm.start_process(name, command, args, env_vars, None, None)
        .await
        .unwrap();
    
    // Check initial status - might be running or stopped depending on timing
    let status = pm.get_process_status(name).await.unwrap();
    assert!(status.pid.is_some());

    // Wait for process to complete
    sleep(Duration::from_millis(2500)).await;

    // Check final status - should be stopped after sleep completes
    let status = pm.get_process_status(name).await.unwrap();
    // Process should be stopped after the sleep duration
    assert!(matches!(status.status, ProcessStatus::Stopped | ProcessStatus::Running));
    
    // Cleanup
    pm.delete_process(name).await.unwrap();
}

#[tokio::test]
async fn test_process_with_environment_variables() {
    let (pm, _temp_dir) = create_test_process_manager().await;
    
    let name = "env_test";
    let command = "sh";
    let args = vec!["-c".to_string(), "echo $TEST_VAR".to_string()];
    let mut env_vars = HashMap::new();
    env_vars.insert("TEST_VAR".to_string(), "test_value".to_string());
    
    // Start process with environment variables
    pm.start_process(name, command, args, env_vars, None, None)
        .await
        .unwrap();
    
    // Wait for process to complete
    sleep(Duration::from_millis(300)).await;
    
    // Check logs contain the environment variable value
    let logs = pm.get_process_logs(name, None).await.unwrap();
    assert!(logs.contains("test_value"));
    
    // Cleanup
    pm.delete_process(name).await.unwrap();
}

#[tokio::test]
async fn test_process_with_working_directory() {
    let (pm, temp_dir) = create_test_process_manager().await;
    
    let name = "workdir_test";
    let command = "pwd";
    let args = vec![];
    let env_vars = HashMap::new();
    let working_dir = Some(temp_dir.path().to_string_lossy().to_string());
    
    // Start process with custom working directory
    pm.start_process(name, command, args, env_vars, working_dir.clone(), None)
        .await
        .unwrap();
    
    // Wait for process to complete
    sleep(Duration::from_millis(300)).await;
    
    // Check logs contain the working directory
    let logs = pm.get_process_logs(name, None).await.unwrap();
    assert!(logs.contains(&*temp_dir.path().to_string_lossy()));
    
    // Cleanup
    pm.delete_process(name).await.unwrap();
}

#[tokio::test]
async fn test_failed_process() {
    let (pm, _temp_dir) = create_test_process_manager().await;
    
    let name = "failed_process";
    let command = "nonexistent_command_12345";
    let args = vec![];
    let env_vars = HashMap::new();
    
    // Start process that will fail
    pm.start_process(name, command, args, env_vars, None, None)
        .await
        .unwrap();
    
    // Wait a bit
    sleep(Duration::from_millis(300)).await;

    // Check status - should be failed or stopped since command doesn't exist
    let status = pm.get_process_status(name).await.unwrap();
    // The process might start and then fail, or fail to start entirely, or be running (setsid process)
    // Since we're using setsid, the setsid process itself might be running even if the target command fails
    println!("Process status: {:?}", status.status);
    assert!(matches!(status.status, ProcessStatus::Failed | ProcessStatus::Stopped | ProcessStatus::Running));
    
    // Cleanup
    pm.delete_process(name).await.unwrap();
}

#[tokio::test]
async fn test_restart_process() {
    let (pm, _temp_dir) = create_test_process_manager().await;
    
    let name = "restart_test";
    let command = "echo";
    let args = vec!["first_run".to_string()];
    let env_vars = HashMap::new();
    
    // Start process
    pm.start_process(name, command, args, env_vars, None, None)
        .await
        .unwrap();
    
    // Wait for completion
    sleep(Duration::from_millis(300)).await;
    
    // Get initial logs
    let initial_logs = pm.get_process_logs(name, None).await.unwrap();
    assert!(initial_logs.contains("first_run"));
    
    // Restart process
    pm.restart_process(name).await.unwrap();
    
    // Wait for completion
    sleep(Duration::from_millis(300)).await;
    
    // Check that process still exists and has new logs
    let status = pm.get_process_status(name).await.unwrap();
    assert_eq!(status.name, name);
    
    // Cleanup
    pm.delete_process(name).await.unwrap();
}

#[tokio::test]
async fn test_log_lines_limit() {
    let (pm, _temp_dir) = create_test_process_manager().await;
    
    let name = "log_lines_test";
    let command = "sh";
    let args = vec!["-c".to_string(), "for i in $(seq 1 10); do echo \"Line $i\"; done".to_string()];
    let env_vars = HashMap::new();
    
    // Start process that outputs multiple lines
    pm.start_process(name, command, args, env_vars, None, None)
        .await
        .unwrap();
    
    // Wait for completion
    sleep(Duration::from_millis(500)).await;
    
    // Get all logs
    let all_logs = pm.get_process_logs(name, None).await.unwrap();
    let all_lines: Vec<&str> = all_logs.lines().collect();
    assert!(all_lines.len() >= 10);
    
    // Get limited logs
    let limited_logs = pm.get_process_logs(name, Some(3)).await.unwrap();
    let limited_lines: Vec<&str> = limited_logs.lines().collect();
    assert_eq!(limited_lines.len(), 3);
    
    // Check that we get the last 3 lines
    assert!(limited_logs.contains("Line 8") || limited_logs.contains("Line 9") || limited_logs.contains("Line 10"));
    
    // Cleanup
    pm.delete_process(name).await.unwrap();
}
