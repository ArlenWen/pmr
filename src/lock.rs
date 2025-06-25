use anyhow::{Context, Result};
use fs2::FileExt;
use std::fs::{File, OpenOptions};
use std::path::Path;
use std::time::Duration;
use tokio::time::sleep;

/// 文件锁管理器，用于防止并发访问冲突
pub struct FileLock {
    file: File,
    path: String,
}

impl FileLock {
    /// 尝试获取文件锁，带重试机制
    pub async fn acquire<P: AsRef<Path>>(path: P, max_retries: u32) -> Result<Self> {
        let path_str = path.as_ref().to_string_lossy().to_string();
        let lock_path = format!("{}.lock", path_str);

        for attempt in 0..=max_retries {
            match Self::try_acquire(&lock_path).await {
                Ok(lock) => {
                    if attempt > 0 {
                        eprintln!("Successfully acquired lock after {} attempts", attempt);
                    }
                    return Ok(lock);
                }
                Err(_e) if attempt < max_retries => {
                    // 等待一段时间后重试
                    let wait_time = Duration::from_millis(100 + attempt as u64 * 50);
                    eprintln!(
                        "Failed to acquire lock (attempt {}), retrying in {:?}...",
                        attempt + 1,
                        wait_time
                    );
                    sleep(wait_time).await;
                }
                Err(e) => {
                    return Err(e.context(format!(
                        "Failed to acquire lock after {} attempts",
                        max_retries + 1
                    )));
                }
            }
        }

        unreachable!()
    }

    /// 尝试获取文件锁（单次尝试）
    async fn try_acquire(lock_path: &str) -> Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(lock_path)
            .with_context(|| format!("Failed to create lock file: {}", lock_path))?;

        // 尝试获取独占锁（非阻塞）
        file.try_lock_exclusive()
            .with_context(|| format!("Failed to acquire exclusive lock on: {}", lock_path))?;

        Ok(Self {
            file,
            path: lock_path.to_string(),
        })
    }

    /// 释放锁
    #[allow(dead_code)]
    pub fn release(self) -> Result<()> {
        // 解锁文件
        FileExt::unlock(&self.file)
            .with_context(|| format!("Failed to unlock file: {}", self.path))?;

        // 删除锁文件
        if let Err(e) = std::fs::remove_file(&self.path) {
            eprintln!("Warning: Failed to remove lock file {}: {}", self.path, e);
        }

        Ok(())
    }
}

impl Drop for FileLock {
    fn drop(&mut self) {
        // 确保锁被释放
        if let Err(e) = FileExt::unlock(&self.file) {
            eprintln!("Warning: Failed to unlock file in Drop: {}", e);
        }

        // 尝试删除锁文件
        if let Err(e) = std::fs::remove_file(&self.path) {
            eprintln!(
                "Warning: Failed to remove lock file {} in Drop: {}",
                self.path, e
            );
        }
    }
}

/// 原子文件写入工具
pub struct AtomicWriter {
    temp_path: String,
    final_path: String,
}

impl AtomicWriter {
    /// 创建原子写入器
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        let final_path = path.as_ref().to_string_lossy().to_string();
        let temp_path = format!("{}.tmp.{}", final_path, std::process::id());

        Self {
            temp_path,
            final_path,
        }
    }

    /// 写入内容到临时文件
    pub fn write_content(&self, content: &str) -> Result<()> {
        std::fs::write(&self.temp_path, content)
            .with_context(|| format!("Failed to write to temporary file: {}", self.temp_path))
    }

    /// 原子性地提交写入（重命名临时文件为最终文件）
    pub fn commit(self) -> Result<()> {
        std::fs::rename(&self.temp_path, &self.final_path)
            .with_context(|| format!("Failed to rename {} to {}", self.temp_path, self.final_path))
    }

    /// 取消写入（删除临时文件）
    #[allow(dead_code)]
    pub fn abort(self) -> Result<()> {
        if std::path::Path::new(&self.temp_path).exists() {
            std::fs::remove_file(&self.temp_path)
                .with_context(|| format!("Failed to remove temporary file: {}", self.temp_path))?;
        }
        Ok(())
    }
}

impl Drop for AtomicWriter {
    fn drop(&mut self) {
        // 清理临时文件
        if std::path::Path::new(&self.temp_path).exists() {
            if let Err(e) = std::fs::remove_file(&self.temp_path) {
                eprintln!(
                    "Warning: Failed to cleanup temporary file {} in Drop: {}",
                    self.temp_path, e
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::task;

    #[tokio::test]
    async fn test_file_lock_prevents_concurrent_access() {
        let test_file = "/tmp/test_lock_file";

        // 启动两个并发任务尝试获取同一个锁
        let task1 = task::spawn(async {
            let _lock = FileLock::acquire(test_file, 0).await.unwrap();
            tokio::time::sleep(Duration::from_millis(100)).await;
            "task1"
        });

        let task2 = task::spawn(async {
            tokio::time::sleep(Duration::from_millis(10)).await;
            let result = FileLock::acquire(test_file, 0).await;
            assert!(result.is_err()); // 应该失败，因为锁已被task1持有
            "task2"
        });

        let (result1, result2) = tokio::join!(task1, task2);
        assert_eq!(result1.unwrap(), "task1");
        assert_eq!(result2.unwrap(), "task2");
    }

    #[tokio::test]
    async fn test_atomic_writer() {
        let test_file = "/tmp/test_atomic_write";
        let content = "test content";

        let writer = AtomicWriter::new(test_file);
        writer.write_content(content).unwrap();
        writer.commit().unwrap();

        let read_content = std::fs::read_to_string(test_file).unwrap();
        assert_eq!(read_content, content);

        // 清理
        let _ = std::fs::remove_file(test_file);
    }
}
