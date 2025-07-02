use pmr::{
    database::{Database, ProcessRecord, ProcessStatus},
};
use chrono::Utc;
use std::collections::HashMap;
use tempfile::TempDir;
use uuid::Uuid;

/// Helper function to create a test database
async fn create_test_database() -> (Database, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let database_url = format!("sqlite:{}?mode=rwc", db_path.display());
    
    let db = Database::new(&database_url).await.unwrap();
    (db, temp_dir)
}

/// Helper function to create a test ProcessRecord
fn create_test_process_record(name: &str) -> ProcessRecord {
    let mut env_vars = HashMap::new();
    env_vars.insert("TEST_VAR".to_string(), "test_value".to_string());
    
    ProcessRecord {
        id: Uuid::new_v4().to_string(),
        name: name.to_string(),
        command: "echo".to_string(),
        args: vec!["hello".to_string()],
        env_vars,
        working_dir: "/tmp".to_string(),
        pid: Some(12345),
        status: ProcessStatus::Running,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        log_path: "/tmp/test.log".to_string(),
    }
}

#[tokio::test]
async fn test_database_creation() {
    let (_db, _temp_dir) = create_test_database().await;
    // If we get here without panicking, the database was created successfully
}

#[tokio::test]
async fn test_insert_and_get_process() {
    let (db, _temp_dir) = create_test_database().await;
    
    let process = create_test_process_record("test_process");
    
    // Insert process
    db.insert_process(&process).await.unwrap();
    
    // Get process by name
    let retrieved = db.get_process_by_name("test_process").await.unwrap();
    assert!(retrieved.is_some());
    
    let retrieved = retrieved.unwrap();
    assert_eq!(retrieved.name, process.name);
    assert_eq!(retrieved.command, process.command);
    assert_eq!(retrieved.args, process.args);
    assert_eq!(retrieved.env_vars, process.env_vars);
    assert_eq!(retrieved.working_dir, process.working_dir);
    assert_eq!(retrieved.pid, process.pid);
    assert_eq!(retrieved.status, process.status);
    assert_eq!(retrieved.log_path, process.log_path);
}

#[tokio::test]
async fn test_get_nonexistent_process() {
    let (db, _temp_dir) = create_test_database().await;
    
    let result = db.get_process_by_name("nonexistent").await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_get_all_processes() {
    let (db, _temp_dir) = create_test_database().await;
    
    // Initially empty
    let processes = db.get_all_processes().await.unwrap();
    assert_eq!(processes.len(), 0);
    
    // Insert multiple processes
    let process1 = create_test_process_record("process1");
    let process2 = create_test_process_record("process2");
    let process3 = create_test_process_record("process3");
    
    db.insert_process(&process1).await.unwrap();
    db.insert_process(&process2).await.unwrap();
    db.insert_process(&process3).await.unwrap();
    
    // Get all processes
    let processes = db.get_all_processes().await.unwrap();
    assert_eq!(processes.len(), 3);
    
    // Check that all processes are present
    let names: Vec<String> = processes.iter().map(|p| p.name.clone()).collect();
    assert!(names.contains(&"process1".to_string()));
    assert!(names.contains(&"process2".to_string()));
    assert!(names.contains(&"process3".to_string()));
}

#[tokio::test]
async fn test_update_process_status() {
    let (db, _temp_dir) = create_test_database().await;
    
    let process = create_test_process_record("test_process");
    db.insert_process(&process).await.unwrap();
    
    // Update status
    db.update_process_status("test_process", ProcessStatus::Stopped, Some(54321))
        .await
        .unwrap();
    
    // Verify update
    let updated = db.get_process_by_name("test_process").await.unwrap().unwrap();
    assert_eq!(updated.status, ProcessStatus::Stopped);
    assert_eq!(updated.pid, Some(54321));
    assert!(updated.updated_at > process.updated_at);
}

#[tokio::test]
async fn test_delete_process() {
    let (db, _temp_dir) = create_test_database().await;
    
    let process = create_test_process_record("test_process");
    db.insert_process(&process).await.unwrap();
    
    // Verify process exists
    let retrieved = db.get_process_by_name("test_process").await.unwrap();
    assert!(retrieved.is_some());
    
    // Delete process
    let deleted = db.delete_process("test_process").await.unwrap();
    assert!(deleted);
    
    // Verify process is gone
    let retrieved = db.get_process_by_name("test_process").await.unwrap();
    assert!(retrieved.is_none());
}

#[tokio::test]
async fn test_delete_nonexistent_process() {
    let (db, _temp_dir) = create_test_database().await;
    
    let deleted = db.delete_process("nonexistent").await.unwrap();
    assert!(!deleted);
}

#[tokio::test]
async fn test_duplicate_process_name() {
    let (db, _temp_dir) = create_test_database().await;
    
    let process1 = create_test_process_record("duplicate_name");
    let process2 = create_test_process_record("duplicate_name");
    
    // Insert first process
    db.insert_process(&process1).await.unwrap();
    
    // Try to insert second process with same name - should fail
    let result = db.insert_process(&process2).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_process_status_serialization() {
    let (db, _temp_dir) = create_test_database().await;
    
    let statuses = vec![
        ProcessStatus::Running,
        ProcessStatus::Stopped,
        ProcessStatus::Failed,
        ProcessStatus::Unknown,
    ];
    
    for (i, status) in statuses.iter().enumerate() {
        let mut process = create_test_process_record(&format!("process_{}", i));
        process.status = status.clone();
        
        db.insert_process(&process).await.unwrap();
        
        let retrieved = db.get_process_by_name(&process.name).await.unwrap().unwrap();
        assert_eq!(retrieved.status, *status);
    }
}

#[tokio::test]
async fn test_process_with_empty_args() {
    let (db, _temp_dir) = create_test_database().await;
    
    let mut process = create_test_process_record("empty_args");
    process.args = vec![];
    
    db.insert_process(&process).await.unwrap();
    
    let retrieved = db.get_process_by_name("empty_args").await.unwrap().unwrap();
    assert_eq!(retrieved.args, vec![] as Vec<String>);
}

#[tokio::test]
async fn test_process_with_empty_env_vars() {
    let (db, _temp_dir) = create_test_database().await;
    
    let mut process = create_test_process_record("empty_env");
    process.env_vars = HashMap::new();
    
    db.insert_process(&process).await.unwrap();
    
    let retrieved = db.get_process_by_name("empty_env").await.unwrap().unwrap();
    assert_eq!(retrieved.env_vars, HashMap::new());
}

#[tokio::test]
async fn test_process_with_no_pid() {
    let (db, _temp_dir) = create_test_database().await;
    
    let mut process = create_test_process_record("no_pid");
    process.pid = None;
    
    db.insert_process(&process).await.unwrap();
    
    let retrieved = db.get_process_by_name("no_pid").await.unwrap().unwrap();
    assert_eq!(retrieved.pid, None);
}

#[tokio::test]
async fn test_process_ordering() {
    let (db, _temp_dir) = create_test_database().await;
    
    // Insert processes with different creation times
    let mut process1 = create_test_process_record("first");
    let mut process2 = create_test_process_record("second");
    let mut process3 = create_test_process_record("third");
    
    // Set different creation times
    let base_time = Utc::now();
    process1.created_at = base_time - chrono::Duration::seconds(10);
    process2.created_at = base_time - chrono::Duration::seconds(5);
    process3.created_at = base_time;
    
    db.insert_process(&process1).await.unwrap();
    db.insert_process(&process2).await.unwrap();
    db.insert_process(&process3).await.unwrap();
    
    // Get all processes (should be ordered by created_at DESC)
    let processes = db.get_all_processes().await.unwrap();
    assert_eq!(processes.len(), 3);
    
    // Check ordering (newest first)
    assert_eq!(processes[0].name, "third");
    assert_eq!(processes[1].name, "second");
    assert_eq!(processes[2].name, "first");
}
