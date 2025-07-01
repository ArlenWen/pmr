use std::path::{Path, PathBuf};
use std::fs;
use crate::{Result, Error};
use crate::config::LogRotationConfig;

pub struct LogRotator {
    config: LogRotationConfig,
}

impl LogRotator {
    pub fn new(config: LogRotationConfig) -> Self {
        Self { config }
    }

    /// Check if log rotation is needed and perform it if necessary
    pub async fn rotate_if_needed(&self, log_path: &Path) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }

        // Check if the log file exists and its size
        if !log_path.exists() {
            return Ok(());
        }

        let metadata = fs::metadata(log_path)?;
        if metadata.len() <= self.config.max_file_size {
            return Ok(());
        }

        // Perform rotation
        self.rotate_log(log_path).await?;
        Ok(())
    }

    /// Force log rotation regardless of file size
    pub async fn force_rotate(&self, log_path: &Path) -> Result<()> {
        if !log_path.exists() {
            return Ok(());
        }

        // Perform rotation
        self.rotate_log(log_path).await?;
        Ok(())
    }

    /// Rotate the log file
    async fn rotate_log(&self, log_path: &Path) -> Result<()> {
        let log_dir = log_path.parent()
            .ok_or_else(|| Error::Other("Invalid log path".to_string()))?;
        
        let log_name = log_path.file_stem()
            .ok_or_else(|| Error::Other("Invalid log file name".to_string()))?
            .to_string_lossy();

        // Move existing rotated files
        for i in (1..self.config.max_files).rev() {
            let old_file = log_dir.join(format!("{}.{}.log", log_name, i));
            let new_file = log_dir.join(format!("{}.{}.log", log_name, i + 1));
            
            if old_file.exists() {
                if i + 1 > self.config.max_files {
                    // Remove the oldest file
                    let _ = fs::remove_file(&old_file);
                } else {
                    // Move to next number
                    let _ = fs::rename(&old_file, &new_file);
                }
            }
        }

        // Move current log to .1
        let rotated_file = log_dir.join(format!("{}.1.log", log_name));
        fs::rename(log_path, &rotated_file)?;

        // Create new empty log file
        fs::File::create(log_path)?;

        Ok(())
    }

    /// Get the size of a log file
    pub fn get_log_size(&self, log_path: &Path) -> Result<u64> {
        if !log_path.exists() {
            return Ok(0);
        }
        let metadata = fs::metadata(log_path)?;
        Ok(metadata.len())
    }

    /// Check if rotation is needed without performing it
    pub fn needs_rotation(&self, log_path: &Path) -> Result<bool> {
        if !self.config.enabled {
            return Ok(false);
        }

        let size = self.get_log_size(log_path)?;
        Ok(size > self.config.max_file_size)
    }

    /// Get list of rotated log files for a given log path
    pub fn get_rotated_files(&self, log_path: &Path) -> Result<Vec<PathBuf>> {
        let log_dir = log_path.parent()
            .ok_or_else(|| Error::Other("Invalid log path".to_string()))?;
        
        let log_name = log_path.file_stem()
            .ok_or_else(|| Error::Other("Invalid log file name".to_string()))?
            .to_string_lossy();

        let mut rotated_files = Vec::new();
        
        for i in 1..=self.config.max_files {
            let rotated_file = log_dir.join(format!("{}.{}.log", log_name, i));
            if rotated_file.exists() {
                rotated_files.push(rotated_file);
            }
        }

        Ok(rotated_files)
    }

    /// Clean up old rotated files beyond the configured limit
    pub fn cleanup_old_files(&self, log_path: &Path) -> Result<()> {
        let log_dir = log_path.parent()
            .ok_or_else(|| Error::Other("Invalid log path".to_string()))?;
        
        let log_name = log_path.file_stem()
            .ok_or_else(|| Error::Other("Invalid log file name".to_string()))?
            .to_string_lossy();

        // Remove files beyond the max_files limit
        for i in (self.config.max_files + 1)..=20 { // Check up to 20 files
            let old_file = log_dir.join(format!("{}.{}.log", log_name, i));
            if old_file.exists() {
                let _ = fs::remove_file(&old_file);
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::io::Write;

    #[tokio::test]
    async fn test_log_rotation() {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("test.log");

        let config = LogRotationConfig {
            max_file_size: 100, // 100 bytes
            max_files: 3,
            enabled: true,
        };

        let rotator = LogRotator::new(config);

        // Create a log file larger than max_file_size
        let mut file = fs::File::create(&log_path).unwrap();
        file.write_all(&vec![b'x'; 150]).unwrap();
        drop(file);

        // Perform rotation
        rotator.rotate_if_needed(&log_path).await.unwrap();

        // Check that the original file is now empty/small
        assert!(log_path.exists());
        let metadata = fs::metadata(&log_path).unwrap();
        assert_eq!(metadata.len(), 0);

        // Check that rotated file exists
        let rotated_file = temp_dir.path().join("test.1.log");
        assert!(rotated_file.exists());
        let rotated_metadata = fs::metadata(&rotated_file).unwrap();
        assert_eq!(rotated_metadata.len(), 150);
    }

    #[test]
    fn test_needs_rotation() {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("test.log");

        let config = LogRotationConfig {
            max_file_size: 100,
            max_files: 3,
            enabled: true,
        };

        let rotator = LogRotator::new(config);

        // No file exists
        assert!(!rotator.needs_rotation(&log_path).unwrap());

        // Create small file
        let mut file = fs::File::create(&log_path).unwrap();
        file.write_all(b"small").unwrap();
        drop(file);

        assert!(!rotator.needs_rotation(&log_path).unwrap());

        // Create large file
        let mut file = fs::File::create(&log_path).unwrap();
        file.write_all(&vec![b'x'; 150]).unwrap();
        drop(file);

        assert!(rotator.needs_rotation(&log_path).unwrap());
    }

    #[tokio::test]
    async fn test_multiple_rotations() {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("test.log");

        let config = LogRotationConfig {
            max_file_size: 50, // 50 bytes
            max_files: 3,
            enabled: true,
        };

        let rotator = LogRotator::new(config);

        // Create and rotate first file
        let mut file = fs::File::create(&log_path).unwrap();
        file.write_all(b"first rotation content that is longer than 50 bytes").unwrap();
        drop(file);

        rotator.force_rotate(&log_path).await.unwrap();

        // Create and rotate second file
        let mut file = fs::File::create(&log_path).unwrap();
        file.write_all(b"second rotation content that is also longer than 50 bytes").unwrap();
        drop(file);

        rotator.force_rotate(&log_path).await.unwrap();

        // Check that both rotated files exist
        let rotated_file_1 = temp_dir.path().join("test.1.log");
        let rotated_file_2 = temp_dir.path().join("test.2.log");

        assert!(rotated_file_1.exists());
        assert!(rotated_file_2.exists());

        // Check content
        let content_1 = fs::read_to_string(&rotated_file_1).unwrap();
        let content_2 = fs::read_to_string(&rotated_file_2).unwrap();

        assert!(content_1.contains("second rotation"));
        assert!(content_2.contains("first rotation"));
    }

    #[test]
    fn test_disabled_rotation() {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("test.log");

        let config = LogRotationConfig {
            max_file_size: 10, // Very small
            max_files: 3,
            enabled: false, // Disabled
        };

        let rotator = LogRotator::new(config);

        // Create large file
        let mut file = fs::File::create(&log_path).unwrap();
        file.write_all(&vec![b'x'; 150]).unwrap();
        drop(file);

        // Should not need rotation when disabled
        assert!(!rotator.needs_rotation(&log_path).unwrap());
    }

    #[test]
    fn test_get_rotated_files() {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("test.log");

        let config = LogRotationConfig {
            max_file_size: 100,
            max_files: 5,
            enabled: true,
        };

        let rotator = LogRotator::new(config);

        // Create some rotated files manually
        fs::File::create(&log_path).unwrap();
        fs::File::create(temp_dir.path().join("test.1.log")).unwrap();
        fs::File::create(temp_dir.path().join("test.2.log")).unwrap();
        fs::File::create(temp_dir.path().join("test.3.log")).unwrap();

        let rotated_files = rotator.get_rotated_files(&log_path).unwrap();
        assert_eq!(rotated_files.len(), 3);

        // Check that files are in correct order
        assert!(rotated_files[0].to_string_lossy().contains("test.1.log"));
        assert!(rotated_files[1].to_string_lossy().contains("test.2.log"));
        assert!(rotated_files[2].to_string_lossy().contains("test.3.log"));
    }
}
