use pmr::{
    config::{Config, LogRotationConfig},
    process::ProcessManager,
    database::ProcessStatus,
};
use std::collections::HashMap;
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
            max_file_size: 1024 * 1024, // 1MB for performance tests
            max_files: 3,
        });
    
    let pm = ProcessManager::new(config).await.unwrap();
    (pm, temp_dir)
}

#[tokio::test]
async fn test_multiple_process_creation_performance() {
    let (pm, _temp_dir) = create_test_process_manager().await;
    
    let num_processes = 50;
    let start_time = Instant::now();
    
    // Create multiple processes
    for i in 0..num_processes {
        let name = format!("perf_test_{}", i);
        let env_vars = HashMap::new();
        
        pm.start_process(
            &name,
            "echo",
            vec![format!("Process {}", i)],
            env_vars,
            None,
            None,
        )
        .await
        .unwrap();
    }
    
    let creation_time = start_time.elapsed();
    println!("Created {} processes in {:?}", num_processes, creation_time);
    
    // Verify all processes were created
    let processes = pm.list_processes().await.unwrap();
    assert_eq!(processes.len(), num_processes);
    
    // Performance assertion: should create processes reasonably quickly
    // Allow 15 seconds for 50 processes (300ms per process on average)
    // This accounts for system overhead and process startup time
    assert!(creation_time < Duration::from_secs(15),
        "Process creation took too long: {:?}", creation_time);
    
    // Clean up
    for i in 0..num_processes {
        let name = format!("perf_test_{}", i);
        pm.delete_process(&name).await.unwrap();
    }
}

#[tokio::test]
async fn test_database_query_performance() {
    let (pm, _temp_dir) = create_test_process_manager().await;
    
    // Create some test processes
    let num_processes = 20;
    for i in 0..num_processes {
        let name = format!("db_perf_test_{}", i);
        let env_vars = HashMap::new();
        
        pm.start_process(
            &name,
            "echo",
            vec![format!("Process {}", i)],
            env_vars,
            None,
            None,
        )
        .await
        .unwrap();
    }
    
    // Test list_processes performance
    let start_time = Instant::now();
    let iterations = 100;
    
    for _ in 0..iterations {
        let _processes = pm.list_processes().await.unwrap();
    }
    
    let query_time = start_time.elapsed();
    let avg_query_time = query_time / iterations;
    
    println!("Average list_processes query time: {:?}", avg_query_time);
    
    // Performance assertion: each query should be fast
    assert!(avg_query_time < Duration::from_millis(50), 
        "Database queries too slow: {:?}", avg_query_time);
    
    // Test individual process status queries
    let start_time = Instant::now();
    
    for i in 0..num_processes {
        let name = format!("db_perf_test_{}", i);
        let _status = pm.get_process_status(&name).await.unwrap();
    }
    
    let status_query_time = start_time.elapsed();
    let avg_status_query_time = status_query_time / num_processes;
    
    println!("Average get_process_status query time: {:?}", avg_status_query_time);
    
    // Performance assertion: status queries should be fast
    assert!(avg_status_query_time < Duration::from_millis(20), 
        "Status queries too slow: {:?}", avg_status_query_time);
    
    // Clean up
    for i in 0..num_processes {
        let name = format!("db_perf_test_{}", i);
        pm.delete_process(&name).await.unwrap();
    }
}

#[tokio::test]
async fn test_concurrent_operations_performance() {
    // For this test, we'll use sequential operations to simulate concurrent behavior
    // since ProcessManager doesn't implement Clone
    let (pm, _temp_dir) = create_test_process_manager().await;

    let num_operations = 10;
    let start_time = Instant::now();

    // Simulate concurrent-like operations by rapidly creating and managing processes
    for i in 0..num_operations {
        let name = format!("concurrent_test_{}", i);
        let env_vars = HashMap::new();

        // Start process
        pm.start_process(
            &name,
            "echo",
            vec![format!("Concurrent {}", i)],
            env_vars,
            None,
            None,
        )
        .await
        .unwrap();
    }

    // Wait a bit for processes to complete
    sleep(Duration::from_millis(200)).await;

    // Get status for all processes
    for i in 0..num_operations {
        let name = format!("concurrent_test_{}", i);
        let _status = pm.get_process_status(&name).await.unwrap();
    }

    // Delete all processes
    for i in 0..num_operations {
        let name = format!("concurrent_test_{}", i);
        pm.delete_process(&name).await.unwrap();
    }

    let concurrent_time = start_time.elapsed();
    println!("Completed {} rapid operations in {:?}", num_operations, concurrent_time);

    // Performance assertion: rapid operations should complete reasonably quickly
    assert!(concurrent_time < Duration::from_secs(3),
        "Rapid operations took too long: {:?}", concurrent_time);

    // Verify no processes remain
    let processes = pm.list_processes().await.unwrap();
    assert_eq!(processes.len(), 0);
}

#[tokio::test]
async fn test_log_file_operations_performance() {
    let (pm, _temp_dir) = create_test_process_manager().await;
    
    // Create a process that generates logs
    let name = "log_perf_test";
    let env_vars = HashMap::new();
    
    pm.start_process(
        name,
        "sh",
        vec![
            "-c".to_string(),
            "for i in $(seq 1 1000); do echo \"Log line $i\"; done".to_string(),
        ],
        env_vars,
        None,
        None,
    )
    .await
    .unwrap();
    
    // Wait for process to complete
    sleep(Duration::from_millis(1000)).await;
    
    // Test log reading performance
    let start_time = Instant::now();
    let iterations = 50;
    
    for _ in 0..iterations {
        let _logs = pm.get_process_logs(name, None).await.unwrap();
    }
    
    let log_read_time = start_time.elapsed();
    let avg_log_read_time = log_read_time / iterations;
    
    println!("Average log read time: {:?}", avg_log_read_time);
    
    // Performance assertion: log reading should be fast
    assert!(avg_log_read_time < Duration::from_millis(100), 
        "Log reading too slow: {:?}", avg_log_read_time);
    
    // Test log reading with line limits
    let start_time = Instant::now();
    
    for _ in 0..iterations {
        let _logs = pm.get_process_logs(name, Some(10)).await.unwrap();
    }
    
    let limited_log_read_time = start_time.elapsed();
    let avg_limited_log_read_time = limited_log_read_time / iterations;
    
    println!("Average limited log read time: {:?}", avg_limited_log_read_time);
    
    // Limited log reading should be faster than full log reading
    assert!(avg_limited_log_read_time <= avg_log_read_time);
    
    // Clean up
    pm.delete_process(name).await.unwrap();
}

#[tokio::test]
async fn test_memory_usage_stability() {
    let (pm, _temp_dir) = create_test_process_manager().await;
    
    // Create and delete many processes to test for memory leaks
    let cycles = 10;
    let processes_per_cycle = 20;
    
    for cycle in 0..cycles {
        println!("Memory test cycle {}/{}", cycle + 1, cycles);
        
        // Create processes
        for i in 0..processes_per_cycle {
            let name = format!("memory_test_{}_{}", cycle, i);
            let env_vars = HashMap::new();
            
            pm.start_process(
                &name,
                "echo",
                vec![format!("Memory test {} {}", cycle, i)],
                env_vars,
                None,
                None,
            )
            .await
            .unwrap();
        }
        
        // Wait a bit
        sleep(Duration::from_millis(200)).await;
        
        // Verify processes exist
        let processes = pm.list_processes().await.unwrap();
        assert_eq!(processes.len(), processes_per_cycle);
        
        // Delete all processes
        for i in 0..processes_per_cycle {
            let name = format!("memory_test_{}_{}", cycle, i);
            pm.delete_process(&name).await.unwrap();
        }
        
        // Verify cleanup
        let processes = pm.list_processes().await.unwrap();
        assert_eq!(processes.len(), 0);
    }
    
    println!("Memory stability test completed successfully");
}

#[tokio::test]
async fn test_process_status_update_performance() {
    let (pm, _temp_dir) = create_test_process_manager().await;
    
    // Create some long-running processes
    let num_processes = 15;
    for i in 0..num_processes {
        let name = format!("status_perf_test_{}", i);
        let env_vars = HashMap::new();
        
        pm.start_process(
            &name,
            "sleep",
            vec!["2".to_string()], // Sleep for 2 seconds
            env_vars,
            None,
            None,
        )
        .await
        .unwrap();
    }
    
    // Test status update performance during list_processes
    let start_time = Instant::now();
    let iterations = 20;
    
    for _ in 0..iterations {
        let processes = pm.list_processes().await.unwrap();
        assert_eq!(processes.len(), num_processes);
        
        // Check that status updates are working
        for process in &processes {
            assert!(matches!(process.status, ProcessStatus::Running | ProcessStatus::Stopped));
        }
    }
    
    let status_update_time = start_time.elapsed();
    let avg_status_update_time = status_update_time / iterations;
    
    println!("Average status update time for {} processes: {:?}", 
        num_processes, avg_status_update_time);
    
    // Performance assertion: status updates should be reasonable
    assert!(avg_status_update_time < Duration::from_millis(500), 
        "Status updates too slow: {:?}", avg_status_update_time);
    
    // Wait for processes to complete
    sleep(Duration::from_millis(2500)).await;
    
    // Clean up
    for i in 0..num_processes {
        let name = format!("status_perf_test_{}", i);
        pm.delete_process(&name).await.unwrap();
    }
}
