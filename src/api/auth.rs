#[cfg(feature = "http-api")]
use crate::{Error, database::Database};
#[cfg(feature = "http-api")]
use chrono::Utc;
#[cfg(feature = "http-api")]
use std::sync::Arc;
#[cfg(feature = "http-api")]
use uuid::Uuid;

// Re-export ApiToken from database module
#[cfg(feature = "http-api")]
pub use crate::database::ApiToken;

#[cfg(feature = "http-api")]
pub struct AuthManager {
    database: Arc<Database>,
}

#[cfg(feature = "http-api")]
impl AuthManager {
    pub fn new(database: Arc<Database>) -> Self {
        Self { database }
    }

    /// Generate a new API token
    pub async fn generate_token(&self, name: String, expires_in_days: Option<u32>) -> crate::Result<ApiToken> {
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

        self.database.insert_api_token(&api_token).await?;
        Ok(api_token)
    }

    /// Validate a token (blocking version for use in handlers)
    pub fn validate_token_sync(&self, token: &str) -> bool {
        // Use tokio's block_in_place to run async code in sync context
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                match self.database.get_api_token_by_token(token).await {
                    Ok(Some(api_token)) => {
                        if !api_token.is_active {
                            return false;
                        }

                        if let Some(expires_at) = api_token.expires_at {
                            if Utc::now() > expires_at {
                                return false;
                            }
                        }

                        true
                    }
                    _ => false,
                }
            })
        })
    }

    /// Validate a token (async version)
    pub async fn validate_token(&self, token: &str) -> bool {
        match self.database.get_api_token_by_token(token).await {
            Ok(Some(api_token)) => {
                if !api_token.is_active {
                    return false;
                }

                if let Some(expires_at) = api_token.expires_at {
                    if Utc::now() > expires_at {
                        return false;
                    }
                }

                true
            }
            _ => false,
        }
    }

    /// List all tokens
    pub async fn list_tokens(&self) -> crate::Result<Vec<ApiToken>> {
        self.database.get_all_api_tokens().await
    }

    /// Revoke a token
    pub async fn revoke_token(&self, token: &str) -> crate::Result<()> {
        let updated = self.database.update_api_token_status(token, false).await?;
        if updated {
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
