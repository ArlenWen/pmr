use std::path::PathBuf;
use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub database_path: PathBuf,
    pub default_log_dir: PathBuf,
    pub log_rotation: LogRotationConfig,
    #[cfg(feature = "http-api")]
    pub api: ApiConfig,
}

#[cfg(feature = "http-api")]
#[derive(Debug, Clone)]
pub struct ApiConfig {
    pub enabled: bool,
    pub port: u16,
    pub auth_tokens_path: PathBuf,
}

#[cfg(feature = "http-api")]
impl Default for ApiConfig {
    fn default() -> Self {
        let home_dir = env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let pmr_dir = PathBuf::from(home_dir).join(".pmr");

        Self {
            enabled: false,
            port: 8080,
            auth_tokens_path: pmr_dir.join("api_tokens.json"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct LogRotationConfig {
    pub max_file_size: u64,  // in bytes
    pub max_files: usize,    // number of rotated files to keep
    pub enabled: bool,
}

impl Default for LogRotationConfig {
    fn default() -> Self {
        Self {
            max_file_size: 10 * 1024 * 1024, // 10MB
            max_files: 5,
            enabled: true,
        }
    }
}

impl Config {
    pub fn new() -> Self {
        let home_dir = env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let pmr_dir = PathBuf::from(home_dir).join(".pmr");

        // Default log directory is in the current working directory
        let default_log_dir = std::env::current_dir()
            .unwrap_or_default()
            .join("logs");

        Self {
            database_path: pmr_dir.join("processes.db"),
            default_log_dir,
            log_rotation: LogRotationConfig::default(),
            #[cfg(feature = "http-api")]
            api: ApiConfig::default(),
        }
    }

    pub fn with_log_dir(mut self, log_dir: PathBuf) -> Self {
        self.default_log_dir = log_dir;
        self
    }

    pub fn with_log_rotation(mut self, config: LogRotationConfig) -> Self {
        self.log_rotation = config;
        self
    }

    pub fn with_database_path(mut self, database_path: PathBuf) -> Self {
        self.database_path = database_path;
        self
    }



    pub fn ensure_directories(&self) -> crate::Result<()> {
        if let Some(parent) = self.database_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::create_dir_all(&self.default_log_dir)?;
        Ok(())
    }

    pub fn ensure_log_directory(&self, log_dir: &PathBuf) -> crate::Result<()> {
        std::fs::create_dir_all(log_dir)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_config_new() {
        let config = Config::new();

        // Check that database path is in ~/.pmr/
        assert!(config.database_path.to_string_lossy().contains(".pmr"));
        assert!(config.database_path.to_string_lossy().ends_with("processes.db"));

        // Check that default log dir is ./logs
        assert!(config.default_log_dir.to_string_lossy().ends_with("logs"));

        // Check default log rotation config
        assert!(config.log_rotation.enabled);
        assert_eq!(config.log_rotation.max_file_size, 10 * 1024 * 1024); // 10MB
        assert_eq!(config.log_rotation.max_files, 5);
    }

    #[test]
    fn test_config_with_custom_log_dir() {
        let temp_dir = TempDir::new().unwrap();
        let custom_log_dir = temp_dir.path().join("custom_logs");

        let config = Config::new().with_log_dir(custom_log_dir.clone());

        assert_eq!(config.default_log_dir, custom_log_dir);
    }

    #[test]
    fn test_config_with_custom_log_rotation() {
        let custom_rotation = LogRotationConfig {
            max_file_size: 1024, // 1KB
            max_files: 10,
            enabled: false,
        };

        let config = Config::new().with_log_rotation(custom_rotation.clone());

        assert_eq!(config.log_rotation.max_file_size, 1024);
        assert_eq!(config.log_rotation.max_files, 10);
        assert!(!config.log_rotation.enabled);
    }

    #[test]
    fn test_ensure_log_directory() {
        let temp_dir = TempDir::new().unwrap();
        let log_dir = temp_dir.path().join("test_logs");

        let config = Config::new();

        // Directory should not exist initially
        assert!(!log_dir.exists());

        // Ensure directory
        config.ensure_log_directory(&log_dir).unwrap();

        // Directory should now exist
        assert!(log_dir.exists());
        assert!(log_dir.is_dir());
    }

    #[test]
    fn test_log_rotation_config_default() {
        let config = LogRotationConfig::default();

        assert!(config.enabled);
        assert_eq!(config.max_file_size, 10 * 1024 * 1024); // 10MB
        assert_eq!(config.max_files, 5);
    }
}
