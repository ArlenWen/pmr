use anyhow::{Context, Result};
use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::Path;
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;

pub struct Logger;

impl Logger {
    pub fn read_logs(log_path: &Path, lines: usize) -> Result<Vec<String>> {
        if !log_path.exists() {
            return Ok(vec![]);
        }

        let file = File::open(log_path)
            .context("Failed to open log file")?;
        
        let reader = BufReader::new(file);
        let all_lines: Vec<String> = reader.lines()
            .collect::<Result<Vec<_>, _>>()
            .context("Failed to read log lines")?;

        let start_index = if all_lines.len() > lines {
            all_lines.len() - lines
        } else {
            0
        };

        Ok(all_lines[start_index..].to_vec())
    }

    pub async fn append_log(log_path: &Path, content: &str) -> Result<()> {
        use tokio::fs::OpenOptions;
        use tokio::io::AsyncWriteExt;

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)
            .await
            .context("Failed to open log file for writing")?;

        let line = format!("{}\n", content);
        file.write_all(line.as_bytes()).await
            .context("Failed to write to log file")?;

        file.flush().await
            .context("Failed to flush log file")?;

        Ok(())
    }

    pub fn tail_logs(log_path: &Path, lines: usize) -> Result<()> {
        let logs = Self::read_logs(log_path, lines)?;
        for line in logs {
            println!("{}", line);
        }
        Ok(())
    }

    pub async fn follow_logs(log_path: &Path) -> Result<()> {
        use tokio::time::{sleep, Duration};
        use std::fs::metadata;

        if !log_path.exists() {
            println!("Log file does not exist: {}", log_path.display());
            return Ok(());
        }

        let mut last_size = metadata(log_path)?.len();
        let mut file = File::open(log_path)?;
        file.seek(SeekFrom::Start(last_size))?;

        println!("Following logs for {}...", log_path.display());
        println!("Press Ctrl+C to stop");

        loop {
            let current_size = metadata(log_path)?.len();
            
            if current_size > last_size {
                file.seek(SeekFrom::Start(last_size))?;
                let reader = BufReader::new(&file);
                
                for line in reader.lines() {
                    match line {
                        Ok(content) => println!("{}", content),
                        Err(_) => break,
                    }
                }
                
                last_size = current_size;
            }
            
            sleep(Duration::from_millis(100)).await;
        }
    }
}
