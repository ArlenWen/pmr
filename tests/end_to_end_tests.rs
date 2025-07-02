use pmr::{
    config::{Config, LogRotationConfig},
    process::ProcessManager,
    database::ProcessStatus,
};
use std::collections::HashMap;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::sleep;

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
            max_file_size: 1024,
            max_files: 3,
        });
    
    let pm = ProcessManager::new(config).await.unwrap();
    (pm, temp_dir)
}

#[tokio::test]
async fn test_complete_process_management_workflow() {
    let (pm, _temp_dir) = create_test_process_manager().await;
    
    println!("Starting complete process management workflow test");
    
    // Step 1: Verify empty state
    let processes = pm.list_processes().await.unwrap();
    assert_eq!(processes.len(), 0, "Should start with no processes");
    
    // Step 2: Create multiple processes with different configurations
    let test_processes = vec![
        ("web_server", "echo", vec!["Starting web server".to_string()]),
        ("database", "echo", vec!["Database initialized".to_string()]),
        ("worker", "echo", vec!["Worker process ready".to_string()]),
    ];
    
    for (name, command, args) in &test_processes {
        let mut env_vars = HashMap::new();
        env_vars.insert("PROCESS_NAME".to_string(), name.to_string());
        env_vars.insert("ENVIRONMENT".to_string(), "test".to_string());
        
        pm.start_process(name, command, args.clone(), env_vars, None, None)
            .await
            .unwrap();
        
        println!("Started process: {}", name);
    }
    
    // Step 3: Verify all processes were created
    let processes = pm.list_processes().await.unwrap();
    assert_eq!(processes.len(), test_processes.len(), "All processes should be created");
    
    // Step 4: Check individual process status
    for (name, _, _) in &test_processes {
        let status = pm.get_process_status(name).await.unwrap();
        assert_eq!(status.name, *name);
        assert!(matches!(status.status, ProcessStatus::Running | ProcessStatus::Stopped));
        println!("Process {} status: {:?}", name, status.status);
    }
    
    // Step 5: Wait for processes to complete and check logs
    sleep(Duration::from_millis(500)).await;
    
    for (name, _, args) in &test_processes {
        let logs = pm.get_process_logs(name, None).await.unwrap();
        assert!(logs.contains(&args[0]), "Logs should contain expected output");
        println!("Process {} logs verified", name);
    }
    
    // Step 6: Test restart functionality
    let restart_process = "web_server";
    pm.restart_process(restart_process).await.unwrap();
    println!("Restarted process: {}", restart_process);
    
    // Verify restart worked
    let status = pm.get_process_status(restart_process).await.unwrap();
    assert_eq!(status.name, restart_process);
    
    // Step 7: Test stop functionality (if process is still running)
    for (name, _, _) in &test_processes {
        let status = pm.get_process_status(name).await.unwrap();
        if status.status == ProcessStatus::Running {
            pm.stop_process(name).await.unwrap();
            println!("Stopped process: {}", name);
        }
    }
    
    // Step 8: Clean up all processes
    for (name, _, _) in &test_processes {
        pm.delete_process(name).await.unwrap();
        println!("Deleted process: {}", name);
    }
    
    // Step 9: Verify cleanup
    let processes = pm.list_processes().await.unwrap();
    assert_eq!(processes.len(), 0, "All processes should be cleaned up");
    
    println!("Complete process management workflow test passed!");
}

#[tokio::test]
async fn test_error_scenarios_and_recovery() {
    let (pm, _temp_dir) = create_test_process_manager().await;
    
    println!("Starting error scenarios and recovery test");
    
    // Test 1: Try to start process with invalid command
    let result = pm.start_process(
        "invalid_cmd",
        "nonexistent_command_12345",
        vec![],
        HashMap::new(),
        None,
        None,
    ).await;
    
    // Should succeed in creating the process record, but process will fail
    assert!(result.is_ok(), "Should create process record even for invalid commands");
    
    // Wait and check status
    sleep(Duration::from_millis(300)).await;
    let status = pm.get_process_status("invalid_cmd").await.unwrap();
    // Status should be Failed or Stopped (depending on how setsid handles it)
    assert!(matches!(status.status, ProcessStatus::Failed | ProcessStatus::Stopped | ProcessStatus::Running));
    
    // Test 2: Try to operate on non-existent process
    let result = pm.get_process_status("nonexistent").await;
    assert!(result.is_err(), "Should fail for non-existent process");
    
    let result = pm.stop_process("nonexistent").await;
    assert!(result.is_err(), "Should fail to stop non-existent process");
    
    let result = pm.delete_process("nonexistent").await;
    assert!(result.is_err(), "Should fail to delete non-existent process");
    
    // Test 3: Try to create duplicate process
    pm.start_process("duplicate", "echo", vec!["first".to_string()], HashMap::new(), None, None)
        .await
        .unwrap();
    
    let result = pm.start_process("duplicate", "echo", vec!["second".to_string()], HashMap::new(), None, None)
        .await;
    assert!(result.is_err(), "Should fail to create duplicate process");
    
    // Test 4: Recovery - system should still be functional
    pm.start_process("recovery_test", "echo", vec!["recovery".to_string()], HashMap::new(), None, None)
        .await
        .unwrap();
    
    let status = pm.get_process_status("recovery_test").await.unwrap();
    assert_eq!(status.name, "recovery_test");
    
    // Clean up
    pm.delete_process("invalid_cmd").await.unwrap();
    pm.delete_process("duplicate").await.unwrap();
    pm.delete_process("recovery_test").await.unwrap();
    
    println!("Error scenarios and recovery test passed!");
}

#[tokio::test]
async fn test_log_management_features() {
    let (pm, _temp_dir) = create_test_process_manager().await;
    
    println!("Starting log management features test");
    
    // Create a process that generates multiple log lines
    pm.start_process(
        "log_test",
        "sh",
        vec![
            "-c".to_string(),
            "for i in $(seq 1 20); do echo \"Log line $i: $(date)\"; done".to_string(),
        ],
        HashMap::new(),
        None,
        None,
    )
    .await
    .unwrap();
    
    // Wait for process to complete
    sleep(Duration::from_millis(1000)).await;
    
    // Test 1: Get all logs
    let all_logs = pm.get_process_logs("log_test", None).await.unwrap();
    assert!(all_logs.contains("Log line 1"), "Should contain first log line");
    assert!(all_logs.contains("Log line 20"), "Should contain last log line");
    
    let line_count = all_logs.lines().count();
    assert!(line_count >= 20, "Should have at least 20 log lines");
    
    // Test 2: Get limited logs
    let limited_logs = pm.get_process_logs("log_test", Some(5)).await.unwrap();
    let limited_line_count = limited_logs.lines().count();
    assert!(limited_line_count <= 5, "Should have at most 5 lines");
    
    // Test 3: Check log rotation status
    let rotation_status = pm.get_log_rotation_status("log_test").await.unwrap();
    assert!(rotation_status.contains("Log file:"), "Should contain log file info");
    assert!(rotation_status.contains("Current size:"), "Should contain size info");
    
    // Test 4: Manual log rotation
    pm.rotate_process_logs("log_test").await.unwrap();
    
    // Test 5: Get rotated logs
    let rotated_logs = pm.get_rotated_logs("log_test").await.unwrap();
    // May or may not have rotated logs depending on file size
    println!("Rotated logs count: {}", rotated_logs.len());
    
    // Clean up
    pm.delete_process("log_test").await.unwrap();
    
    println!("Log management features test passed!");
}

#[tokio::test]
async fn test_environment_and_working_directory() {
    let (pm, temp_dir) = create_test_process_manager().await;
    
    println!("Starting environment and working directory test");
    
    // Test 1: Environment variables
    let mut env_vars = HashMap::new();
    env_vars.insert("TEST_VAR1".to_string(), "value1".to_string());
    env_vars.insert("TEST_VAR2".to_string(), "value2".to_string());
    env_vars.insert("CUSTOM_MESSAGE".to_string(), "Hello from PMR!".to_string());
    
    pm.start_process(
        "env_test",
        "sh",
        vec![
            "-c".to_string(),
            "echo \"VAR1: $TEST_VAR1\"; echo \"VAR2: $TEST_VAR2\"; echo \"MSG: $CUSTOM_MESSAGE\"".to_string(),
        ],
        env_vars,
        None,
        None,
    )
    .await
    .unwrap();
    
    // Test 2: Working directory
    let work_dir = temp_dir.path().to_string_lossy().to_string();
    pm.start_process(
        "workdir_test",
        "pwd",
        vec![],
        HashMap::new(),
        Some(work_dir.clone()),
        None,
    )
    .await
    .unwrap();
    
    // Wait for processes to complete
    sleep(Duration::from_millis(500)).await;
    
    // Verify environment variables
    let env_logs = pm.get_process_logs("env_test", None).await.unwrap();
    assert!(env_logs.contains("VAR1: value1"), "Should contain TEST_VAR1");
    assert!(env_logs.contains("VAR2: value2"), "Should contain TEST_VAR2");
    assert!(env_logs.contains("MSG: Hello from PMR!"), "Should contain CUSTOM_MESSAGE");
    
    // Verify working directory
    let workdir_logs = pm.get_process_logs("workdir_test", None).await.unwrap();
    assert!(workdir_logs.contains(&work_dir), "Should contain working directory path");
    
    // Clean up
    pm.delete_process("env_test").await.unwrap();
    pm.delete_process("workdir_test").await.unwrap();
    
    println!("Environment and working directory test passed!");
}

#[tokio::test]
async fn test_system_stability_under_load() {
    let (pm, _temp_dir) = create_test_process_manager().await;
    
    println!("Starting system stability under load test");
    
    let num_processes = 20;
    
    // Create multiple processes
    for i in 0..num_processes {
        let name = format!("stability_test_{}", i);
        pm.start_process(
            &name,
            "echo",
            vec![format!("Stability test {}", i)],
            HashMap::new(),
            None,
            None,
        )
        .await
        .unwrap();
    }
    
    // Perform multiple operations
    for _ in 0..5 {
        // List all processes
        let processes = pm.list_processes().await.unwrap();
        assert_eq!(processes.len(), num_processes);
        
        // Check status of random processes
        for i in (0..num_processes).step_by(5) {
            let name = format!("stability_test_{}", i);
            let _status = pm.get_process_status(&name).await.unwrap();
        }
        
        sleep(Duration::from_millis(100)).await;
    }
    
    // Clean up all processes
    for i in 0..num_processes {
        let name = format!("stability_test_{}", i);
        pm.delete_process(&name).await.unwrap();
    }
    
    // Verify cleanup
    let processes = pm.list_processes().await.unwrap();
    assert_eq!(processes.len(), 0);
    
    println!("System stability under load test passed!");
}
