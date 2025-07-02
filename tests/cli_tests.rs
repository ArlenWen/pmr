use std::process::Command;
use std::path::PathBuf;
use tempfile::TempDir;

/// Helper function to get the path to the pmr binary
fn get_pmr_binary() -> PathBuf {
    let mut path = std::env::current_dir().unwrap();
    path.push("target");
    path.push("debug");
    path.push("pmr");
    path
}

/// Helper function to create a temporary config directory and return pmr command with custom config
fn create_test_command() -> (Command, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let mut cmd = Command::new(get_pmr_binary());
    
    // Set HOME to temp directory so pmr uses it for config
    cmd.env("HOME", temp_dir.path());
    
    (cmd, temp_dir)
}

#[test]
fn test_pmr_help() {
    let (mut cmd, _temp_dir) = create_test_command();
    cmd.arg("--help");
    
    let output = cmd.output().expect("Failed to execute pmr");
    assert!(output.status.success());
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("process management tool"));
    assert!(stdout.contains("start"));
    assert!(stdout.contains("stop"));
    assert!(stdout.contains("list"));
    assert!(stdout.contains("delete"));
}

#[test]
fn test_pmr_version() {
    let (mut cmd, _temp_dir) = create_test_command();
    cmd.arg("--version");
    
    let output = cmd.output().expect("Failed to execute pmr");
    assert!(output.status.success());
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("pmr"));
}

#[test]
fn test_pmr_list_empty() {
    let (mut cmd, _temp_dir) = create_test_command();
    cmd.arg("list");
    
    let output = cmd.output().expect("Failed to execute pmr");
    assert!(output.status.success());
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("No processes found"));
}

#[test]
fn test_pmr_start_simple_process() {
    let (mut cmd, temp_dir) = create_test_command();
    cmd.args(&["start", "test_echo", "echo", "Hello, World!"]);
    
    let output = cmd.output().expect("Failed to execute pmr");
    assert!(output.status.success());
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Process 'test_echo' started") || stdout.contains("exited quickly"));
    
    // Clean up - delete the process
    let (mut cleanup_cmd, _) = create_test_command();
    cleanup_cmd.env("HOME", temp_dir.path());
    cleanup_cmd.args(&["delete", "test_echo"]);
    let _ = cleanup_cmd.output();
}

#[test]
fn test_pmr_start_with_args() {
    let (mut cmd, temp_dir) = create_test_command();
    cmd.args(&["start", "test_args", "echo", "arg1", "arg2", "arg3"]);
    
    let output = cmd.output().expect("Failed to execute pmr");
    assert!(output.status.success());
    
    // Clean up
    let (mut cleanup_cmd, _) = create_test_command();
    cleanup_cmd.env("HOME", temp_dir.path());
    cleanup_cmd.args(&["delete", "test_args"]);
    let _ = cleanup_cmd.output();
}

#[test]
fn test_pmr_start_with_env_vars() {
    let (mut cmd, temp_dir) = create_test_command();
    cmd.args(&[
        "start", 
        "test_env", 
        "-e", "TEST_VAR=test_value",
        "-e", "ANOTHER_VAR=another_value",
        "sh", "-c", "echo $TEST_VAR $ANOTHER_VAR"
    ]);
    
    let output = cmd.output().expect("Failed to execute pmr");
    assert!(output.status.success());
    
    // Clean up
    let (mut cleanup_cmd, _) = create_test_command();
    cleanup_cmd.env("HOME", temp_dir.path());
    cleanup_cmd.args(&["delete", "test_env"]);
    let _ = cleanup_cmd.output();
}

#[test]
fn test_pmr_start_with_working_directory() {
    let (mut cmd, temp_dir) = create_test_command();
    let work_dir = temp_dir.path().to_string_lossy();
    
    cmd.args(&[
        "start", 
        "test_workdir", 
        "--workdir", &work_dir,
        "pwd"
    ]);
    
    let output = cmd.output().expect("Failed to execute pmr");
    assert!(output.status.success());
    
    // Clean up
    let (mut cleanup_cmd, _) = create_test_command();
    cleanup_cmd.env("HOME", temp_dir.path());
    cleanup_cmd.args(&["delete", "test_workdir"]);
    let _ = cleanup_cmd.output();
}

#[test]
fn test_pmr_start_duplicate_name() {
    let (mut cmd1, temp_dir) = create_test_command();
    cmd1.args(&["start", "duplicate", "sleep", "1"]);
    
    let output1 = cmd1.output().expect("Failed to execute pmr");
    assert!(output1.status.success());
    
    // Try to start another process with the same name
    let (mut cmd2, _) = create_test_command();
    cmd2.env("HOME", temp_dir.path());
    cmd2.args(&["start", "duplicate", "sleep", "1"]);
    
    let output2 = cmd2.output().expect("Failed to execute pmr");
    assert!(!output2.status.success());
    
    let stderr = String::from_utf8_lossy(&output2.stderr);
    assert!(stderr.contains("already exists") || stderr.contains("ProcessAlreadyExists"));
    
    // Clean up
    let (mut cleanup_cmd, _) = create_test_command();
    cleanup_cmd.env("HOME", temp_dir.path());
    cleanup_cmd.args(&["delete", "duplicate"]);
    let _ = cleanup_cmd.output();
}

#[test]
fn test_pmr_status_nonexistent() {
    let (mut cmd, _temp_dir) = create_test_command();
    cmd.args(&["status", "nonexistent"]);
    
    let output = cmd.output().expect("Failed to execute pmr");
    assert!(!output.status.success());
    
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not found") || stderr.contains("ProcessNotFound"));
}

#[test]
fn test_pmr_stop_nonexistent() {
    let (mut cmd, _temp_dir) = create_test_command();
    cmd.args(&["stop", "nonexistent"]);
    
    let output = cmd.output().expect("Failed to execute pmr");
    assert!(!output.status.success());
    
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not found") || stderr.contains("ProcessNotFound"));
}

#[test]
fn test_pmr_delete_nonexistent() {
    let (mut cmd, _temp_dir) = create_test_command();
    cmd.args(&["delete", "nonexistent"]);
    
    let output = cmd.output().expect("Failed to execute pmr");
    assert!(!output.status.success());
    
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not found") || stderr.contains("ProcessNotFound"));
}

#[test]
fn test_pmr_logs_nonexistent() {
    let (mut cmd, _temp_dir) = create_test_command();
    cmd.args(&["logs", "nonexistent"]);
    
    let output = cmd.output().expect("Failed to execute pmr");
    assert!(!output.status.success());
    
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not found") || stderr.contains("ProcessNotFound"));
}

#[test]
fn test_pmr_full_workflow() {
    let (mut start_cmd, temp_dir) = create_test_command();
    start_cmd.args(&["start", "workflow_test", "echo", "workflow test"]);
    
    // Start process
    let output = start_cmd.output().expect("Failed to start process");
    assert!(output.status.success());
    
    // List processes
    let (mut list_cmd, _) = create_test_command();
    list_cmd.env("HOME", temp_dir.path());
    list_cmd.arg("list");
    
    let output = list_cmd.output().expect("Failed to list processes");
    assert!(output.status.success());
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("workflow_test"));
    
    // Get status
    let (mut status_cmd, _) = create_test_command();
    status_cmd.env("HOME", temp_dir.path());
    status_cmd.args(&["status", "workflow_test"]);
    
    let output = status_cmd.output().expect("Failed to get status");
    assert!(output.status.success());
    
    // Get logs
    let (mut logs_cmd, _) = create_test_command();
    logs_cmd.env("HOME", temp_dir.path());
    logs_cmd.args(&["logs", "workflow_test"]);
    
    let output = logs_cmd.output().expect("Failed to get logs");
    assert!(output.status.success());
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("workflow test"));
    
    // Delete process
    let (mut delete_cmd, _) = create_test_command();
    delete_cmd.env("HOME", temp_dir.path());
    delete_cmd.args(&["delete", "workflow_test"]);
    
    let output = delete_cmd.output().expect("Failed to delete process");
    assert!(output.status.success());
    
    // Verify deletion
    let (mut final_list_cmd, _) = create_test_command();
    final_list_cmd.env("HOME", temp_dir.path());
    final_list_cmd.arg("list");
    
    let output = final_list_cmd.output().expect("Failed to list processes");
    assert!(output.status.success());
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("No processes found"));
}

#[test]
fn test_pmr_invalid_command() {
    let (mut cmd, _temp_dir) = create_test_command();
    cmd.args(&["invalid_command"]);
    
    let output = cmd.output().expect("Failed to execute pmr");
    assert!(!output.status.success());
}

#[test]
fn test_pmr_logs_with_lines_limit() {
    let (mut start_cmd, temp_dir) = create_test_command();
    start_cmd.args(&[
        "start", 
        "lines_test", 
        "sh", "-c", 
        "for i in $(seq 1 10); do echo \"Line $i\"; done"
    ]);
    
    // Start process
    let output = start_cmd.output().expect("Failed to start process");
    assert!(output.status.success());
    
    // Wait a bit for the process to complete
    std::thread::sleep(std::time::Duration::from_millis(500));
    
    // Get logs with line limit
    let (mut logs_cmd, _) = create_test_command();
    logs_cmd.env("HOME", temp_dir.path());
    logs_cmd.args(&["logs", "lines_test", "-n", "3"]);
    
    let output = logs_cmd.output().expect("Failed to get logs");
    assert!(output.status.success());
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.lines().collect();
    
    // Should have at most 3 lines (might be fewer due to timing)
    assert!(lines.len() <= 3);
    
    // Clean up
    let (mut cleanup_cmd, _) = create_test_command();
    cleanup_cmd.env("HOME", temp_dir.path());
    cleanup_cmd.args(&["delete", "lines_test"]);
    let _ = cleanup_cmd.output();
}

#[test]
fn test_pmr_clear_command() {
    let (mut cmd, _temp_dir) = create_test_command();
    cmd.args(&["clear"]);

    let output = cmd.output().expect("Failed to execute pmr clear");
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("No stopped/failed processes to clear") || stdout.contains("Cleared"));
}

#[test]
fn test_pmr_clear_all_command() {
    let (mut cmd, _temp_dir) = create_test_command();
    cmd.args(&["clear", "--all"]);

    let output = cmd.output().expect("Failed to execute pmr clear --all");
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("No all processes to clear") || stdout.contains("Cleared"));
}

#[test]
fn test_pmr_clear_json_format() {
    let (mut cmd, _temp_dir) = create_test_command();
    cmd.args(&["--format", "json", "clear"]);

    let output = cmd.output().expect("Failed to execute pmr clear with JSON format");
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should be valid JSON
    let json_result: Result<serde_json::Value, _> = serde_json::from_str(&stdout);
    assert!(json_result.is_ok(), "Output should be valid JSON: {}", stdout);

    let json = json_result.unwrap();
    assert!(json.get("cleared_count").is_some());
    assert!(json.get("operation_type").is_some());
    assert!(json.get("cleared_processes").is_some());
    assert!(json.get("failed_processes").is_some());
}
