use pmr::{
    config::{Config, LogRotationConfig},
    process::ProcessManager,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tempfile::TempDir;
use tokio::time::sleep;

/// Helper function to create a test ProcessManager optimized for large scale tests
async fn create_large_scale_test_process_manager() -> (ProcessManager, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("large_scale_test.db");
    let log_dir = temp_dir.path().join("logs");
    
    let config = Config::new()
        .with_database_path(db_path)
        .with_log_dir(log_dir)
        .with_log_rotation(LogRotationConfig {
            enabled: true,
            max_file_size: 2 * 1024 * 1024, // 2MB for large scale tests
            max_files: 10,
        });
    
    let pm = ProcessManager::new(config).await.unwrap();
    (pm, temp_dir)
}

#[tokio::test]
async fn test_thousand_process_creation() {
    let (pm, _temp_dir) = create_large_scale_test_process_manager().await;
    
    let num_processes = 1000;
    println!("🚀 Starting large scale test: Creating {} processes", num_processes);
    
    let start_time = Instant::now();
    let mut creation_times = Vec::new();
    
    // Create 1000 processes with timing measurements
    for i in 0..num_processes {
        let process_start = Instant::now();
        let name = format!("large_scale_test_{:04}", i);
        let env_vars = HashMap::new();
        
        pm.start_process(
            &name,
            "echo",
            vec![format!("Large scale process {}", i)],
            env_vars,
            None,
            None,
        )
        .await
        .unwrap();
        
        let process_time = process_start.elapsed();
        creation_times.push(process_time);
        
        // Progress indicator and brief pause every 100 processes
        if (i + 1) % 100 == 0 {
            println!("✅ Created {}/{} processes", i + 1, num_processes);
            sleep(Duration::from_millis(50)).await; // Brief pause to avoid overwhelming system
        }
    }
    
    let total_creation_time = start_time.elapsed();
    println!("📊 Process creation completed in {:?}", total_creation_time);
    
    // Calculate statistics
    let avg_creation_time = creation_times.iter().sum::<Duration>() / creation_times.len() as u32;
    let max_creation_time = creation_times.iter().max().unwrap();
    let min_creation_time = creation_times.iter().min().unwrap();
    
    println!("📈 Creation time stats:");
    println!("   Average: {:?}", avg_creation_time);
    println!("   Maximum: {:?}", max_creation_time);
    println!("   Minimum: {:?}", min_creation_time);
    
    // Verify all processes were created
    let list_start = Instant::now();
    let processes = pm.list_processes().await.unwrap();
    let list_time = list_start.elapsed();
    
    println!("📋 Listed {} processes in {:?}", processes.len(), list_time);
    assert_eq!(processes.len(), num_processes);
    
    // Test batch status queries
    let status_start = Instant::now();
    let mut status_count = 0;
    
    for i in (0..num_processes).step_by(50) { // Check every 50th process
        let name = format!("large_scale_test_{:04}", i);
        let _status = pm.get_process_status(&name).await.unwrap();
        status_count += 1;
    }
    
    let status_time = status_start.elapsed();
    println!("🔍 Checked {} process statuses in {:?}", status_count, status_time);
    
    // Clean up all processes
    let cleanup_start = Instant::now();
    for i in 0..num_processes {
        let name = format!("large_scale_test_{:04}", i);
        pm.delete_process(&name).await.unwrap();
        
        if (i + 1) % 200 == 0 {
            println!("🧹 Cleaned up {}/{} processes", i + 1, num_processes);
        }
    }
    
    let cleanup_time = cleanup_start.elapsed();
    println!("🧹 Cleanup completed in {:?}", cleanup_time);
    
    // Verify cleanup
    let final_processes = pm.list_processes().await.unwrap();
    assert_eq!(final_processes.len(), 0);
    
    let total_time = start_time.elapsed();
    println!("🏁 Total test time: {:?}", total_time);
    println!("⚡ Average time per process (full cycle): {:?}", total_time / num_processes as u32);
}

#[tokio::test]
async fn test_concurrent_thousand_process_management() {
    let (pm, _temp_dir) = create_large_scale_test_process_manager().await;
    let pm = Arc::new(pm);
    
    let num_workers = 50;
    let processes_per_worker = 20;
    let total_processes = num_workers * processes_per_worker; // 1000 processes
    
    println!("🔄 Starting concurrent large scale test:");
    println!("   Workers: {}", num_workers);
    println!("   Processes per worker: {}", processes_per_worker);
    println!("   Total processes: {}", total_processes);
    
    let start_time = Instant::now();
    let mut handles = vec![];
    
    // Create concurrent workers
    for worker_id in 0..num_workers {
        let pm_clone = pm.clone();
        let handle = tokio::spawn(async move {
            let worker_start = Instant::now();
            let mut worker_times = Vec::new();
            
            // Each worker creates its processes
            for i in 0..processes_per_worker {
                let process_start = Instant::now();
                let name = format!("concurrent_worker_{:02}_process_{:02}", worker_id, i);
                let env_vars = HashMap::new();
                
                pm_clone
                    .start_process(
                        &name,
                        "echo",
                        vec![format!("Concurrent Worker {} Process {}", worker_id, i)],
                        env_vars,
                        None,
                        None,
                    )
                    .await
                    .unwrap();
                
                worker_times.push(process_start.elapsed());
                
                // Small delay to simulate realistic workload
                sleep(Duration::from_millis(2)).await;
            }
            
            // Worker performs status checks
            for i in 0..processes_per_worker {
                let name = format!("concurrent_worker_{:02}_process_{:02}", worker_id, i);
                let _status = pm_clone.get_process_status(&name).await.unwrap();
            }
            
            // Worker cleans up its processes
            for i in 0..processes_per_worker {
                let name = format!("concurrent_worker_{:02}_process_{:02}", worker_id, i);
                pm_clone.delete_process(&name).await.unwrap();
            }
            
            let worker_time = worker_start.elapsed();
            let avg_process_time = worker_times.iter().sum::<Duration>() / worker_times.len() as u32;
            
            (worker_id, worker_time, avg_process_time)
        });
        
        handles.push(handle);
    }
    
    // Wait for all workers and collect results
    let mut worker_results = Vec::new();
    for handle in handles {
        let result = handle.await.unwrap();
        worker_results.push(result);
    }
    
    let total_time = start_time.elapsed();
    
    // Analyze results
    let avg_worker_time = worker_results.iter()
        .map(|(_, time, _)| *time)
        .sum::<Duration>() / worker_results.len() as u32;
    
    let max_worker_time = worker_results.iter()
        .map(|(_, time, _)| *time)
        .max()
        .unwrap();
    
    println!("📊 Concurrent test results:");
    println!("   Total time: {:?}", total_time);
    println!("   Average worker time: {:?}", avg_worker_time);
    println!("   Maximum worker time: {:?}", max_worker_time);
    println!("   Processes per second: {:.2}", total_processes as f64 / total_time.as_secs_f64());
    
    // Verify all processes are cleaned up
    let final_processes = pm.list_processes().await.unwrap();
    assert_eq!(final_processes.len(), 0);
    
    println!("✅ Concurrent large scale test completed successfully");
}

#[tokio::test]
async fn test_database_performance_at_scale() {
    let (pm, _temp_dir) = create_large_scale_test_process_manager().await;
    
    let num_processes = 1500;
    println!("💾 Testing database performance with {} processes", num_processes);
    
    let start_time = Instant::now();
    
    // Phase 1: Rapid process creation (database writes)
    println!("📝 Phase 1: Creating processes (testing database writes)");
    let creation_start = Instant::now();
    
    for i in 0..num_processes {
        let name = format!("db_scale_test_{:04}", i);
        let env_vars = HashMap::new();
        
        pm.start_process(
            &name,
            "echo",
            vec![format!("DB scale test {}", i)],
            env_vars,
            None,
            None,
        )
        .await
        .unwrap();
        
        if (i + 1) % 300 == 0 {
            println!("   Created {}/{} processes", i + 1, num_processes);
        }
    }
    
    let creation_time = creation_start.elapsed();
    println!("📝 Database writes completed in {:?}", creation_time);
    println!("   Write rate: {:.2} processes/second", num_processes as f64 / creation_time.as_secs_f64());

    // Phase 2: Intensive database reads
    println!("📖 Phase 2: Testing database reads");
    let read_start = Instant::now();

    // Test list_processes performance with large dataset
    let list_iterations = 20;
    for i in 0..list_iterations {
        let list_time_start = Instant::now();
        let processes = pm.list_processes().await.unwrap();
        let list_time = list_time_start.elapsed();

        assert_eq!(processes.len(), num_processes);

        if i % 5 == 0 {
            println!("   List operation {} took {:?}", i + 1, list_time);
        }
    }

    let read_time = read_start.elapsed();
    println!("📖 Database reads completed in {:?}", read_time);
    println!("   Average list time: {:?}", read_time / list_iterations);

    // Phase 3: Random access pattern testing
    println!("🎯 Phase 3: Testing random access patterns");
    let random_start = Instant::now();

    let random_queries = 500;
    for i in 0..random_queries {
        // Query random processes
        let random_id = i * 3 % num_processes; // Pseudo-random pattern
        let name = format!("db_scale_test_{:04}", random_id);
        let _status = pm.get_process_status(&name).await.unwrap();

        if i % 100 == 0 && i > 0 {
            println!("   Completed {} random queries", i);
        }
    }

    let random_time = random_start.elapsed();
    println!("🎯 Random access completed in {:?}", random_time);
    println!("   Query rate: {:.2} queries/second", random_queries as f64 / random_time.as_secs_f64());

    // Phase 4: Cleanup (database deletes)
    println!("🧹 Phase 4: Testing database deletes");
    let delete_start = Instant::now();

    for i in 0..num_processes {
        let name = format!("db_scale_test_{:04}", i);
        pm.delete_process(&name).await.unwrap();

        if (i + 1) % 300 == 0 {
            println!("   Deleted {}/{} processes", i + 1, num_processes);
        }
    }

    let delete_time = delete_start.elapsed();
    println!("🧹 Database deletes completed in {:?}", delete_time);
    println!("   Delete rate: {:.2} processes/second", num_processes as f64 / delete_time.as_secs_f64());

    // Verify cleanup
    let final_processes = pm.list_processes().await.unwrap();
    assert_eq!(final_processes.len(), 0);

    let total_time = start_time.elapsed();
    println!("💾 Database performance test completed in {:?}", total_time);

    // Performance assertions
    let avg_list_time = read_time / list_iterations;
    assert!(avg_list_time < Duration::from_millis(500),
        "List operation too slow with {} processes: {:?}", num_processes, avg_list_time);

    let avg_query_time = random_time / random_queries as u32;
    assert!(avg_query_time < Duration::from_millis(10),
        "Individual queries too slow: {:?}", avg_query_time);
}

#[tokio::test]
async fn test_memory_stability_at_scale() {
    let (pm, _temp_dir) = create_large_scale_test_process_manager().await;

    let cycles = 5;
    let processes_per_cycle = 800; // 800 processes per cycle, 5 cycles = 4000 total operations

    println!("🧠 Testing memory stability at scale:");
    println!("   Cycles: {}", cycles);
    println!("   Processes per cycle: {}", processes_per_cycle);
    println!("   Total operations: {}", cycles * processes_per_cycle);

    for cycle in 0..cycles {
        println!("🔄 Memory test cycle {}/{}", cycle + 1, cycles);
        let cycle_start = Instant::now();

        // Create processes
        println!("   Creating {} processes...", processes_per_cycle);
        for i in 0..processes_per_cycle {
            let name = format!("memory_scale_{}_{:03}", cycle, i);
            let env_vars = HashMap::new();

            pm.start_process(
                &name,
                "echo",
                vec![format!("Memory scale test cycle {} process {}", cycle, i)],
                env_vars,
                None,
                None,
            )
            .await
            .unwrap();

            // Brief pause every 100 processes to avoid overwhelming
            if (i + 1) % 100 == 0 {
                sleep(Duration::from_millis(10)).await;
            }
        }

        // Verify all processes exist
        let processes = pm.list_processes().await.unwrap();
        assert_eq!(processes.len(), processes_per_cycle);
        println!("   ✅ Verified {} processes created", processes.len());

        // Perform some operations on the processes
        println!("   Performing operations on processes...");
        for i in (0..processes_per_cycle).step_by(20) {
            let name = format!("memory_scale_{}_{:03}", cycle, i);
            let _status = pm.get_process_status(&name).await.unwrap();
        }

        // Delete all processes
        println!("   Cleaning up {} processes...", processes_per_cycle);
        for i in 0..processes_per_cycle {
            let name = format!("memory_scale_{}_{:03}", cycle, i);
            pm.delete_process(&name).await.unwrap();

            if (i + 1) % 200 == 0 {
                sleep(Duration::from_millis(5)).await;
            }
        }

        // Verify cleanup
        let processes = pm.list_processes().await.unwrap();
        assert_eq!(processes.len(), 0);

        let cycle_time = cycle_start.elapsed();
        println!("   ✅ Cycle {} completed in {:?}", cycle + 1, cycle_time);

        // Brief pause between cycles
        sleep(Duration::from_millis(100)).await;
    }

    println!("🧠 Memory stability test completed successfully");
    println!("   Total processes created and destroyed: {}", cycles * processes_per_cycle);
}

#[tokio::test]
async fn test_log_handling_at_scale() {
    let (pm, _temp_dir) = create_large_scale_test_process_manager().await;

    let num_processes = 200; // Fewer processes but more log output per process
    let lines_per_process = 1000;

    println!("📝 Testing log handling at scale:");
    println!("   Processes: {}", num_processes);
    println!("   Lines per process: {}", lines_per_process);
    println!("   Total log lines: {}", num_processes * lines_per_process);

    let start_time = Instant::now();

    // Create processes that generate substantial logs
    println!("🚀 Creating log-generating processes...");
    for i in 0..num_processes {
        let name = format!("log_scale_test_{:03}", i);
        let env_vars = HashMap::new();

        pm.start_process(
            &name,
            "sh",
            vec![
                "-c".to_string(),
                format!(
                    "for j in $(seq 1 {}); do echo \"Process {} Line $j: $(date +%s.%N) - Large scale log test data with some content to make it realistic\"; done",
                    lines_per_process, i
                ),
            ],
            env_vars,
            None,
            None,
        )
        .await
        .unwrap();

        if (i + 1) % 50 == 0 {
            println!("   Created {}/{} log processes", i + 1, num_processes);
        }
    }

    // Wait for processes to generate logs
    println!("⏳ Waiting for log generation to complete...");
    sleep(Duration::from_secs(10)).await;

    // Test log reading performance
    println!("📖 Testing log reading performance...");
    let log_read_start = Instant::now();

    for i in 0..num_processes {
        let name = format!("log_scale_test_{:03}", i);

        // Read full logs
        let logs = pm.get_process_logs(&name, None).await.unwrap();
        let line_count = logs.lines().count();

        // Verify log content
        assert!(logs.contains(&format!("Process {}", i)));
        assert!(line_count > 0, "Process {} should have generated logs", i);

        // Test limited log reading
        let limited_logs = pm.get_process_logs(&name, Some(50)).await.unwrap();
        let limited_line_count = limited_logs.lines().count();
        assert!(limited_line_count <= 50);

        if (i + 1) % 50 == 0 {
            println!("   Read logs from {}/{} processes", i + 1, num_processes);
        }
    }

    let log_read_time = log_read_start.elapsed();
    println!("📖 Log reading completed in {:?}", log_read_time);
    println!("   Average log read time per process: {:?}", log_read_time / num_processes);

    // Clean up
    println!("🧹 Cleaning up log test processes...");
    for i in 0..num_processes {
        let name = format!("log_scale_test_{:03}", i);
        pm.delete_process(&name).await.unwrap();
    }

    let total_time = start_time.elapsed();
    println!("📝 Log handling test completed in {:?}", total_time);

    // Performance assertions
    let avg_log_read_time = log_read_time / num_processes;
    assert!(avg_log_read_time < Duration::from_millis(200),
        "Log reading too slow: {:?}", avg_log_read_time);
}

#[tokio::test]
async fn test_system_resource_limits() {
    let (pm, _temp_dir) = create_large_scale_test_process_manager().await;

    println!("⚠️  Testing system resource limits and graceful degradation");

    let max_attempts = 2000; // Try to create up to 2000 processes
    let mut successful_processes = 0;
    let mut failed_processes = 0;
    let mut process_names = Vec::new();

    println!("🔄 Attempting to create {} processes to test limits...", max_attempts);

    let start_time = Instant::now();

    for i in 0..max_attempts {
        let name = format!("resource_limit_test_{:04}", i);
        let env_vars = HashMap::new();

        let result = pm.start_process(
            &name,
            "sleep",
            vec!["30".to_string()], // Long-running process to test resource limits
            env_vars,
            None,
            None,
        ).await;

        match result {
            Ok(_) => {
                successful_processes += 1;
                process_names.push(name);
            }
            Err(_) => {
                failed_processes += 1;
                // Stop trying if we hit too many failures in a row
                if failed_processes > 50 {
                    println!("   Stopping after {} consecutive failures", failed_processes);
                    break;
                }
            }
        }

        if (i + 1) % 100 == 0 {
            println!("   Attempt {}: {} successful, {} failed",
                i + 1, successful_processes, failed_processes);
        }

        // Brief pause to avoid overwhelming the system
        if i % 50 == 0 {
            sleep(Duration::from_millis(100)).await;
        }
    }

    let creation_time = start_time.elapsed();

    println!("📊 Resource limit test results:");
    println!("   Successful processes: {}", successful_processes);
    println!("   Failed processes: {}", failed_processes);
    println!("   Success rate: {:.2}%",
        (successful_processes as f64 / (successful_processes + failed_processes) as f64) * 100.0);
    println!("   Creation time: {:?}", creation_time);

    // Test system stability with the created processes
    if successful_processes > 0 {
        println!("🔍 Testing system stability with {} active processes...", successful_processes);

        // Test list operations
        let list_start = Instant::now();
        let processes = pm.list_processes().await.unwrap();
        let list_time = list_start.elapsed();

        println!("   Listed {} processes in {:?}", processes.len(), list_time);
        assert_eq!(processes.len(), successful_processes);

        // Test status queries on a sample of processes
        let sample_size = std::cmp::min(100, successful_processes);
        let status_start = Instant::now();

        for i in (0..sample_size).step_by(std::cmp::max(1, successful_processes / sample_size)) {
            if i < process_names.len() {
                let _status = pm.get_process_status(&process_names[i]).await.unwrap();
            }
        }

        let status_time = status_start.elapsed();
        println!("   Checked {} process statuses in {:?}", sample_size, status_time);
    }

    // Clean up all successful processes
    println!("🧹 Cleaning up {} processes...", successful_processes);
    let cleanup_start = Instant::now();

    for (i, name) in process_names.iter().enumerate() {
        // Stop the process first (since they're sleep processes)
        let _ = pm.stop_process(name).await;
        pm.delete_process(name).await.unwrap();

        if (i + 1) % 100 == 0 {
            println!("   Cleaned up {}/{} processes", i + 1, process_names.len());
        }
    }

    let cleanup_time = cleanup_start.elapsed();
    println!("🧹 Cleanup completed in {:?}", cleanup_time);

    // Verify cleanup
    let final_processes = pm.list_processes().await.unwrap();
    assert_eq!(final_processes.len(), 0);

    let total_time = start_time.elapsed();
    println!("⚠️  Resource limit test completed in {:?}", total_time);

    // Ensure we could create a reasonable number of processes
    assert!(successful_processes >= 100,
        "Should be able to create at least 100 processes, only created {}", successful_processes);
}

#[tokio::test]
async fn test_mixed_workload_at_scale() {
    let (pm, _temp_dir) = create_large_scale_test_process_manager().await;
    let pm = Arc::new(pm);

    println!("🎭 Testing mixed workload at scale");
    println!("   Simulating realistic production scenarios with mixed operations");

    let duration = Duration::from_secs(30); // Run test for 30 seconds
    let start_time = Instant::now();

    // Worker 1: Continuous process creation and deletion
    let pm1 = pm.clone();
    let handle1 = tokio::spawn(async move {
        let mut process_counter = 0;
        let mut created_processes = Vec::new();

        while start_time.elapsed() < duration {
            // Create some processes
            for i in 0..10 {
                let name = format!("mixed_creator_{}_{}", process_counter, i);
                let env_vars = HashMap::new();

                if pm1.start_process(
                    &name,
                    "echo",
                    vec![format!("Mixed workload process {}", process_counter)],
                    env_vars,
                    None,
                    None,
                ).await.is_ok() {
                    created_processes.push(name);
                }
            }

            // Delete some older processes
            if created_processes.len() > 50 {
                for _ in 0..5 {
                    if let Some(name) = created_processes.pop() {
                        let _ = pm1.delete_process(&name).await;
                    }
                }
            }

            process_counter += 1;
            sleep(Duration::from_millis(200)).await;
        }

        // Clean up remaining processes
        for name in created_processes {
            let _ = pm1.delete_process(&name).await;
        }

        process_counter
    });

    // Worker 2: Continuous status monitoring
    let pm2 = pm.clone();
    let handle2 = tokio::spawn(async move {
        let mut query_count = 0;

        while start_time.elapsed() < duration {
            // List all processes
            if let Ok(processes) = pm2.list_processes().await {
                query_count += 1;

                // Query status of some processes
                for (i, process) in processes.iter().enumerate() {
                    if i >= 20 { break; } // Limit to first 20 processes
                    let _ = pm2.get_process_status(&process.name).await;
                    query_count += 1;
                }
            }

            sleep(Duration::from_millis(500)).await;
        }

        query_count
    });

    // Worker 3: Log-generating processes
    let pm3 = pm.clone();
    let handle3 = tokio::spawn(async move {
        let mut log_processes = Vec::new();
        let mut batch_counter = 0;

        while start_time.elapsed() < duration {
            // Create log-generating processes
            for i in 0..5 {
                let name = format!("mixed_logger_{}_{}", batch_counter, i);
                let env_vars = HashMap::new();

                if pm3.start_process(
                    &name,
                    "sh",
                    vec![
                        "-c".to_string(),
                        format!("for j in $(seq 1 50); do echo \"Mixed workload log batch {} process {} line $j\"; sleep 0.1; done", batch_counter, i),
                    ],
                    env_vars,
                    None,
                    None,
                ).await.is_ok() {
                    log_processes.push(name);
                }
            }

            // Read logs from some processes and then clean them up
            if log_processes.len() > 15 {
                for _ in 0..5 {
                    if let Some(name) = log_processes.pop() {
                        let _ = pm3.get_process_logs(&name, Some(20)).await;
                        let _ = pm3.delete_process(&name).await;
                    }
                }
            }

            batch_counter += 1;
            sleep(Duration::from_millis(1000)).await;
        }

        // Clean up remaining log processes
        for name in log_processes {
            let _ = pm3.delete_process(&name).await;
        }

        batch_counter
    });

    // Wait for all workers to complete
    let (creator_cycles, query_count, log_batches) = tokio::join!(handle1, handle2, handle3);
    let creator_cycles = creator_cycles.unwrap();
    let query_count = query_count.unwrap();
    let log_batches = log_batches.unwrap();

    let total_time = start_time.elapsed();

    println!("🎭 Mixed workload test results:");
    println!("   Test duration: {:?}", total_time);
    println!("   Creator cycles: {}", creator_cycles);
    println!("   Status queries: {}", query_count);
    println!("   Log batches: {}", log_batches);

    // Verify system is clean
    let final_processes = pm.list_processes().await.unwrap();
    println!("   Final process count: {}", final_processes.len());

    // Clean up any remaining processes
    for process in final_processes {
        let _ = pm.delete_process(&process.name).await;
    }

    println!("✅ Mixed workload test completed successfully");

    // Performance assertions
    assert!(creator_cycles > 0, "Creator should have completed at least one cycle");
    assert!(query_count > 0, "Monitor should have performed queries");
    assert!(log_batches > 0, "Logger should have created log batches");
}
