use std::path::PathBuf;
use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub database_path: PathBuf,
    pub log_dir: PathBuf,
}

impl Config {
    pub fn new() -> Self {
        let home_dir = env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let pmr_dir = PathBuf::from(home_dir).join(".pmr");
        
        Self {
            database_path: pmr_dir.join("processes.db"),
            log_dir: pmr_dir.join("logs"),
        }
    }

    pub fn ensure_directories(&self) -> crate::Result<()> {
        if let Some(parent) = self.database_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::create_dir_all(&self.log_dir)?;
        Ok(())
    }
}
