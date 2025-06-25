use crate::lock::{AtomicWriter, FileLock};
use crate::process::ProcessState;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

pub struct Storage {
    data_dir: PathBuf,
    processes_file: PathBuf,
}

impl Storage {
    pub fn new() -> Result<Self> {
        let data_dir = dirs::data_dir()
            .context("Failed to get data directory")?
            .join("pmr");

        // Create data directory if it doesn't exist
        fs::create_dir_all(&data_dir).context("Failed to create data directory")?;

        let processes_file = data_dir.join("processes.json");

        Ok(Self {
            data_dir,
            processes_file,
        })
    }

    pub fn data_dir(&self) -> &PathBuf {
        &self.data_dir
    }

    pub fn load_processes(&self) -> Result<HashMap<String, ProcessState>> {
        if !self.processes_file.exists() {
            return Ok(HashMap::new());
        }

        let content =
            fs::read_to_string(&self.processes_file).context("Failed to read processes file")?;

        let processes: HashMap<String, ProcessState> =
            serde_json::from_str(&content).context("Failed to parse processes file")?;

        Ok(processes)
    }

    pub async fn save_processes(&self, processes: &HashMap<String, ProcessState>) -> Result<()> {
        // 获取文件锁以防止并发写入
        let _lock = FileLock::acquire(&self.processes_file, 3)
            .await
            .context("Failed to acquire lock for processes file")?;

        let content =
            serde_json::to_string_pretty(processes).context("Failed to serialize processes")?;

        // 使用原子写入确保数据完整性
        let writer = AtomicWriter::new(&self.processes_file);
        writer
            .write_content(&content)
            .context("Failed to write content to temporary file")?;
        writer.commit().context("Failed to commit atomic write")?;

        Ok(())
    }

    pub async fn save_process(&self, process: &ProcessState) -> Result<()> {
        // 获取文件锁以防止并发修改
        let _lock = FileLock::acquire(&self.processes_file, 3)
            .await
            .context("Failed to acquire lock for processes file")?;

        let mut processes = self.load_processes()?;
        processes.insert(process.config.name.clone(), process.clone());

        // 在持有锁的情况下直接保存，避免重复加锁问题
        let content =
            serde_json::to_string_pretty(&processes).context("Failed to serialize processes")?;

        // 使用原子写入确保数据完整性
        let writer = AtomicWriter::new(&self.processes_file);
        writer
            .write_content(&content)
            .context("Failed to write content to temporary file")?;
        writer.commit().context("Failed to commit atomic write")?;

        Ok(())
    }

    pub async fn delete_process(&self, name: &str) -> Result<bool> {
        // 获取文件锁以防止并发修改
        let _lock = FileLock::acquire(&self.processes_file, 3)
            .await
            .context("Failed to acquire lock for processes file")?;

        let mut processes = self.load_processes()?;
        let removed = processes.remove(name).is_some();

        if removed {
            // 在持有锁的情况下直接保存，避免重复加锁问题
            let content = serde_json::to_string_pretty(&processes)
                .context("Failed to serialize processes")?;

            // 使用原子写入确保数据完整性
            let writer = AtomicWriter::new(&self.processes_file);
            writer
                .write_content(&content)
                .context("Failed to write content to temporary file")?;
            writer.commit().context("Failed to commit atomic write")?;

            // Clean up log files
            let stdout_path = self.data_dir.join(format!("{}.stdout.log", name));
            let stderr_path = self.data_dir.join(format!("{}.stderr.log", name));

            let _ = fs::remove_file(stdout_path);
            let _ = fs::remove_file(stderr_path);
        }

        Ok(removed)
    }

    pub fn get_process(&self, name: &str) -> Result<Option<ProcessState>> {
        let processes = self.load_processes()?;
        Ok(processes.get(name).cloned())
    }

    pub fn process_exists(&self, name: &str) -> Result<bool> {
        let processes = self.load_processes()?;
        Ok(processes.contains_key(name))
    }

    pub fn create_log_files(&self, name: &str) -> Result<(PathBuf, PathBuf)> {
        let stdout_path = self.data_dir.join(format!("{}.stdout.log", name));
        let stderr_path = self.data_dir.join(format!("{}.stderr.log", name));

        // Create empty log files if they don't exist
        if !stdout_path.exists() {
            fs::write(&stdout_path, "")?;
        }
        if !stderr_path.exists() {
            fs::write(&stderr_path, "")?;
        }

        Ok((stdout_path, stderr_path))
    }

    /// 安全地更新单个进程状态（带锁保护）
    pub async fn update_process_status<F>(&self, name: &str, updater: F) -> Result<bool>
    where
        F: FnOnce(&mut ProcessState) -> Result<()>,
    {
        // 获取文件锁以防止并发修改
        let _lock = FileLock::acquire(&self.processes_file, 3)
            .await
            .context("Failed to acquire lock for processes file")?;

        let mut processes = self.load_processes()?;

        if let Some(process) = processes.get_mut(name) {
            updater(process)?;

            // 在持有锁的情况下直接保存，避免重复加锁问题
            let content = serde_json::to_string_pretty(&processes)
                .context("Failed to serialize processes")?;

            // 使用原子写入确保数据完整性
            let writer = AtomicWriter::new(&self.processes_file);
            writer
                .write_content(&content)
                .context("Failed to write content to temporary file")?;
            writer.commit().context("Failed to commit atomic write")?;

            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// 安全地批量更新进程状态（带锁保护）
    #[allow(dead_code)]
    pub async fn batch_update_processes<F>(&self, updater: F) -> Result<()>
    where
        F: FnOnce(&mut HashMap<String, ProcessState>) -> Result<()>,
    {
        // 获取文件锁以防止并发修改
        let _lock = FileLock::acquire(&self.processes_file, 3)
            .await
            .context("Failed to acquire lock for processes file")?;

        let mut processes = self.load_processes()?;
        updater(&mut processes)?;

        // 在持有锁的情况下直接保存，避免重复加锁问题
        let content =
            serde_json::to_string_pretty(&processes).context("Failed to serialize processes")?;

        // 使用原子写入确保数据完整性
        let writer = AtomicWriter::new(&self.processes_file);
        writer
            .write_content(&content)
            .context("Failed to write content to temporary file")?;
        writer.commit().context("Failed to commit atomic write")?;

        Ok(())
    }
}
