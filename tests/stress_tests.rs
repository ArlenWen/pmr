use pmr::{
    config::{Config, LogRotationConfig},
    process::ProcessManager,
    database::ProcessStatus,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
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
            max_file_size: 512 * 1024, // 512KB for stress tests
            max_files: 5,
        });
    
    let pm = ProcessManager::new(config).await.unwrap();
    (pm, temp_dir)
}

#[tokio::test]
async fn test_high_volume_process_creation() {
    let (pm, _temp_dir) = create_test_process_manager().await;
    
    let num_processes = 100;
    let start_time = Instant::now();
    
    println!("Starting high volume process creation test with {} processes", num_processes);
    
    // Create many processes rapidly
    for i in 0..num_processes {
        let name = format!("stress_test_{}", i);
        let env_vars = HashMap::new();
        
        pm.start_process(
            &name,
            "echo",
            vec![format!("Stress test process {}", i)],
            env_vars,
            None,
            None,
        )
        .await
        .unwrap();
        
        // Small delay to avoid overwhelming the system
        if i % 10 == 0 {
            sleep(Duration::from_millis(10)).await;
        }
    }
    
    let creation_time = start_time.elapsed();
    println!("Created {} processes in {:?}", num_processes, creation_time);
    
    // Verify all processes were created
    let processes = pm.list_processes().await.unwrap();
    assert_eq!(processes.len(), num_processes);
    
    // Test rapid status queries
    let query_start = Instant::now();
    for i in 0..num_processes {
        let name = format!("stress_test_{}", i);
        let _status = pm.get_process_status(&name).await.unwrap();
    }
    let query_time = query_start.elapsed();
    println!("Queried {} process statuses in {:?}", num_processes, query_time);
    
    // Clean up all processes
    let cleanup_start = Instant::now();
    for i in 0..num_processes {
        let name = format!("stress_test_{}", i);
        pm.delete_process(&name).await.unwrap();
    }
    let cleanup_time = cleanup_start.elapsed();
    println!("Cleaned up {} processes in {:?}", num_processes, cleanup_time);
    
    // Verify cleanup
    let processes = pm.list_processes().await.unwrap();
    assert_eq!(processes.len(), 0);
    
    let total_time = start_time.elapsed();
    println!("Total stress test time: {:?}", total_time);
}

#[tokio::test]
async fn test_concurrent_heavy_load() {
    let (pm, _temp_dir) = create_test_process_manager().await;
    let pm = Arc::new(pm);
    
    let num_workers = 20;
    let processes_per_worker = 10;
    let total_processes = num_workers * processes_per_worker;
    
    println!("Starting concurrent heavy load test with {} workers, {} processes each", 
        num_workers, processes_per_worker);
    
    let start_time = Instant::now();
    let mut handles = vec![];
    
    // Create multiple concurrent workers
    for worker_id in 0..num_workers {
        let pm_clone = pm.clone();
        let handle = tokio::spawn(async move {
            let worker_start = Instant::now();
            
            // Each worker creates multiple processes
            for i in 0..processes_per_worker {
                let name = format!("worker_{}_process_{}", worker_id, i);
                let env_vars = HashMap::new();
                
                pm_clone
                    .start_process(
                        &name,
                        "echo",
                        vec![format!("Worker {} Process {}", worker_id, i)],
                        env_vars,
                        None,
                        None,
                    )
                    .await
                    .unwrap();
                
                // Small delay to simulate real workload
                sleep(Duration::from_millis(5)).await;
            }
            
            // Worker performs some operations
            for i in 0..processes_per_worker {
                let name = format!("worker_{}_process_{}", worker_id, i);
                let _status = pm_clone.get_process_status(&name).await.unwrap();
            }
            
            // Worker cleans up its processes
            for i in 0..processes_per_worker {
                let name = format!("worker_{}_process_{}", worker_id, i);
                pm_clone.delete_process(&name).await.unwrap();
            }
            
            let worker_time = worker_start.elapsed();
            println!("Worker {} completed in {:?}", worker_id, worker_time);
        });
        
        handles.push(handle);
    }
    
    // Wait for all workers to complete
    for handle in handles {
        handle.await.unwrap();
    }
    
    let total_time = start_time.elapsed();
    println!("Concurrent heavy load test completed in {:?}", total_time);
    println!("Processed {} total processes across {} workers", total_processes, num_workers);
    
    // Verify all processes are cleaned up
    let processes = pm.list_processes().await.unwrap();
    assert_eq!(processes.len(), 0);
}

#[tokio::test]
async fn test_rapid_start_stop_cycles() {
    let (pm, _temp_dir) = create_test_process_manager().await;
    
    let cycles = 50;
    let processes_per_cycle = 5;
    
    println!("Starting rapid start/stop cycles test: {} cycles, {} processes per cycle", 
        cycles, processes_per_cycle);
    
    let start_time = Instant::now();
    
    for cycle in 0..cycles {
        // Start processes
        for i in 0..processes_per_cycle {
            let name = format!("cycle_{}_process_{}", cycle, i);
            let env_vars = HashMap::new();
            
            pm.start_process(
                &name,
                "sleep",
                vec!["0.5".to_string()], // Short-lived process
                env_vars,
                None,
                None,
            )
            .await
            .unwrap();
        }
        
        // Verify processes exist
        let processes = pm.list_processes().await.unwrap();
        assert!(processes.len() >= processes_per_cycle);
        
        // Wait a bit for processes to start
        sleep(Duration::from_millis(100)).await;
        
        // Stop and delete processes
        for i in 0..processes_per_cycle {
            let name = format!("cycle_{}_process_{}", cycle, i);
            
            // Try to stop (might already be stopped)
            let _ = pm.stop_process(&name).await;
            
            // Delete
            pm.delete_process(&name).await.unwrap();
        }
        
        // Brief pause between cycles
        if cycle % 10 == 0 {
            sleep(Duration::from_millis(50)).await;
            println!("Completed cycle {}/{}", cycle + 1, cycles);
        }
    }
    
    let total_time = start_time.elapsed();
    println!("Rapid start/stop cycles completed in {:?}", total_time);
    
    // Verify cleanup
    let processes = pm.list_processes().await.unwrap();
    assert_eq!(processes.len(), 0);
}

#[tokio::test]
async fn test_database_stress() {
    let (pm, _temp_dir) = create_test_process_manager().await;
    
    let operations = 1000;
    println!("Starting database stress test with {} operations", operations);
    
    let start_time = Instant::now();
    
    // Perform many database operations rapidly
    for i in 0..operations {
        let name = format!("db_stress_{}", i);
        let env_vars = HashMap::new();
        
        // Create process
        pm.start_process(
            &name,
            "echo",
            vec![format!("DB stress {}", i)],
            env_vars,
            None,
            None,
        )
        .await
        .unwrap();
        
        // Query status
        let _status = pm.get_process_status(&name).await.unwrap();
        
        // List all processes (expensive operation)
        if i % 50 == 0 {
            let _processes = pm.list_processes().await.unwrap();
        }
        
        // Delete process
        pm.delete_process(&name).await.unwrap();
        
        // Progress indicator
        if i % 100 == 0 && i > 0 {
            println!("Completed {} database operations", i);
        }
    }
    
    let total_time = start_time.elapsed();
    println!("Database stress test completed in {:?}", total_time);
    println!("Average time per operation: {:?}", total_time / operations);
    
    // Verify database is clean
    let processes = pm.list_processes().await.unwrap();
    assert_eq!(processes.len(), 0);
}

#[tokio::test]
async fn test_log_generation_stress() {
    let (pm, _temp_dir) = create_test_process_manager().await;
    
    let num_processes = 10;
    let lines_per_process = 500;
    
    println!("Starting log generation stress test: {} processes, {} lines each", 
        num_processes, lines_per_process);
    
    let start_time = Instant::now();
    
    // Create processes that generate lots of logs
    for i in 0..num_processes {
        let name = format!("log_stress_{}", i);
        let env_vars = HashMap::new();
        
        pm.start_process(
            &name,
            "sh",
            vec![
                "-c".to_string(),
                format!("for j in $(seq 1 {}); do echo \"Process {} Line $j: $(date)\"; done", 
                    lines_per_process, i),
            ],
            env_vars,
            None,
            None,
        )
        .await
        .unwrap();
    }
    
    // Wait for processes to complete
    sleep(Duration::from_secs(3)).await;
    
    // Read logs from all processes
    let log_read_start = Instant::now();
    for i in 0..num_processes {
        let name = format!("log_stress_{}", i);
        let logs = pm.get_process_logs(&name, None).await.unwrap();
        
        // Verify logs contain expected content
        assert!(logs.contains(&format!("Process {}", i)));
        
        // Test limited log reading
        let limited_logs = pm.get_process_logs(&name, Some(10)).await.unwrap();
        let line_count = limited_logs.lines().count();
        assert!(line_count <= 10);
    }
    let log_read_time = log_read_start.elapsed();
    
    println!("Log reading completed in {:?}", log_read_time);
    
    // Clean up
    for i in 0..num_processes {
        let name = format!("log_stress_{}", i);
        pm.delete_process(&name).await.unwrap();
    }
    
    let total_time = start_time.elapsed();
    println!("Log generation stress test completed in {:?}", total_time);
}

#[tokio::test]
async fn test_error_handling_under_stress() {
    let (pm, _temp_dir) = create_test_process_manager().await;
    
    let operations = 200;
    println!("Starting error handling stress test with {} operations", operations);
    
    let start_time = Instant::now();
    let mut error_count = 0;
    
    for i in 0..operations {
        // Mix of valid and invalid operations
        if i % 3 == 0 {
            // Try to operate on non-existent process
            let result = pm.get_process_status("nonexistent_process").await;
            if result.is_err() {
                error_count += 1;
            }
        } else if i % 3 == 1 {
            // Try to create process with invalid command
            let name = format!("invalid_cmd_{}", i);
            let env_vars = HashMap::new();
            
            let result = pm.start_process(
                &name,
                "nonexistent_command_12345",
                vec![],
                env_vars,
                None,
                None,
            ).await;
            
            if result.is_ok() {
                // Clean up if somehow succeeded
                let _ = pm.delete_process(&name).await;
            }
        } else {
            // Valid operation
            let name = format!("valid_process_{}", i);
            let env_vars = HashMap::new();
            
            pm.start_process(
                &name,
                "echo",
                vec!["valid".to_string()],
                env_vars,
                None,
                None,
            )
            .await
            .unwrap();
            
            pm.delete_process(&name).await.unwrap();
        }
        
        if i % 50 == 0 && i > 0 {
            println!("Completed {} error handling operations", i);
        }
    }
    
    let total_time = start_time.elapsed();
    println!("Error handling stress test completed in {:?}", total_time);
    println!("Handled {} expected errors gracefully", error_count);
    
    // Verify system is still stable
    let processes = pm.list_processes().await.unwrap();
    println!("Final process count: {}", processes.len());
}
