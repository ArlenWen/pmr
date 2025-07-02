#[cfg(feature = "http-api")]
use crate::Error;
#[cfg(feature = "http-api")]
use chrono::{DateTime, Utc};
#[cfg(feature = "http-api")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "http-api")]
use std::collections::HashMap;
#[cfg(feature = "http-api")]
use uuid::Uuid;

#[cfg(feature = "http-api")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiToken {
    pub id: String,
    pub token: String,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub is_active: bool,
}

#[cfg(feature = "http-api")]
pub struct AuthManager {
    tokens: HashMap<String, ApiToken>,
    tokens_file: std::path::PathBuf,
}

#[cfg(feature = "http-api")]
impl AuthManager {
    pub fn new() -> crate::Result<Self> {
        let home_dir = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let tokens_file = std::path::PathBuf::from(home_dir).join(".pmr").join("api_tokens.json");

        // Ensure the directory exists
        if let Some(parent) = tokens_file.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut manager = Self {
            tokens: HashMap::new(),
            tokens_file,
        };

        // Load existing tokens
        manager.load_tokens()?;
        Ok(manager)
    }

    /// Load tokens from file
    fn load_tokens(&mut self) -> crate::Result<()> {
        if self.tokens_file.exists() {
            let content = std::fs::read_to_string(&self.tokens_file)?;
            if !content.trim().is_empty() {
                let tokens: Vec<ApiToken> = serde_json::from_str(&content)?;
                for token in tokens {
                    self.tokens.insert(token.token.clone(), token);
                }
            }
        }
        Ok(())
    }

    /// Save tokens to file
    fn save_tokens(&self) -> crate::Result<()> {
        let tokens: Vec<&ApiToken> = self.tokens.values().collect();
        let content = serde_json::to_string_pretty(&tokens)?;
        std::fs::write(&self.tokens_file, content)?;
        Ok(())
    }

    /// Generate a new API token
    pub fn generate_token(&mut self, name: String, expires_in_days: Option<u32>) -> crate::Result<ApiToken> {
        let id = Uuid::new_v4().to_string();
        let token = self.generate_secure_token();
        let created_at = Utc::now();
        let expires_at = expires_in_days.map(|days| {
            created_at + chrono::Duration::days(days as i64)
        });

        let api_token = ApiToken {
            id: id.clone(),
            token: token.clone(),
            name,
            created_at,
            expires_at,
            is_active: true,
        };

        self.tokens.insert(token.clone(), api_token.clone());
        self.save_tokens()?;
        Ok(api_token)
    }

    /// Validate a token
    pub fn validate_token(&self, token: &str) -> bool {
        if let Some(api_token) = self.tokens.get(token) {
            if !api_token.is_active {
                return false;
            }

            if let Some(expires_at) = api_token.expires_at {
                if Utc::now() > expires_at {
                    return false;
                }
            }

            true
        } else {
            false
        }
    }

    /// List all tokens
    pub fn list_tokens(&self) -> Vec<&ApiToken> {
        self.tokens.values().collect()
    }

    /// Revoke a token
    pub fn revoke_token(&mut self, token: &str) -> crate::Result<()> {
        if let Some(api_token) = self.tokens.get_mut(token) {
            api_token.is_active = false;
            self.save_tokens()?;
            Ok(())
        } else {
            Err(Error::Other("Token not found".to_string()))
        }
    }

    /// Generate a secure random token
    fn generate_secure_token(&self) -> String {
        use base64::Engine;
        use rand::RngCore;
        let mut rng = rand::thread_rng();
        let mut random_bytes = [0u8; 32];
        rng.fill_bytes(&mut random_bytes);
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(random_bytes)
    }
}

#[cfg(feature = "http-api")]
impl Default for AuthManager {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self {
            tokens: HashMap::new(),
            tokens_file: std::path::PathBuf::from("/tmp/api_tokens.json"),
        })
    }
}
