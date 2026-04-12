//! JWT Token Manager
//!
//! Creates test JWT tokens for authentication testing.

use chrono::{Duration, Utc};
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// JWT Claims structure
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    /// Subject (user ID)
    pub sub: String,

    /// JWT ID
    pub jti: String,

    /// Issued at timestamp
    pub iat: i64,

    /// Expiration timestamp
    pub exp: i64,

    /// User roles (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub roles: Option<Vec<String>>,
}

/// JWT Token Manager for creating test tokens
pub struct JwtTokenManager {
    /// Secret key for signing tokens
    secret_key: String,

    /// Default token expiration in seconds
    default_expiration_seconds: i64,

    /// Algorithm for signing
    algorithm: Algorithm,
}

impl JwtTokenManager {
    /// Create a new JWT token manager
    pub fn new(secret_key: impl Into<String>) -> Self {
        Self {
            secret_key: secret_key.into(),
            default_expiration_seconds: 3600, // 1 hour
            algorithm: Algorithm::HS256,
        }
    }

    /// Set default expiration time
    pub fn with_expiration(mut self, seconds: i64) -> Self {
        self.default_expiration_seconds = seconds;
        self
    }

    /// Create a JWT token for a user
    pub fn create_token(
        &self,
        user_id: &str,
        roles: Option<Vec<String>>,
    ) -> Result<(String, Claims), String> {
        let now = Utc::now();
        let exp = now + Duration::seconds(self.default_expiration_seconds);

        let claims = Claims {
            sub: user_id.to_string(),
            jti: Uuid::new_v4().to_string(),
            iat: now.timestamp(),
            exp: exp.timestamp(),
            roles,
        };

        let header = Header::new(self.algorithm);
        let token = encode(
            &header,
            &claims,
            &EncodingKey::from_secret(self.secret_key.as_bytes()),
        )
        .map_err(|e| format!("Failed to encode JWT: {}", e))?;

        Ok((token, claims))
    }

    /// Create an expired token (for testing expiration handling)
    pub fn create_expired_token(&self, user_id: &str) -> Result<(String, Claims), String> {
        let now = Utc::now();
        let exp = now - Duration::hours(1); // Expired 1 hour ago

        let claims = Claims {
            sub: user_id.to_string(),
            jti: Uuid::new_v4().to_string(),
            iat: (now - Duration::hours(2)).timestamp(),
            exp: exp.timestamp(),
            roles: None,
        };

        let header = Header::new(self.algorithm);
        let token = encode(
            &header,
            &claims,
            &EncodingKey::from_secret(self.secret_key.as_bytes()),
        )
        .map_err(|e| format!("Failed to encode JWT: {}", e))?;

        Ok((token, claims))
    }

    /// Create a token with admin role
    pub fn create_admin_token(&self, user_id: &str) -> Result<(String, Claims), String> {
        self.create_token(user_id, Some(vec!["admin".to_string()]))
    }

    /// Create a token with specific roles
    pub fn create_token_with_roles(
        &self,
        user_id: &str,
        roles: Vec<String>,
    ) -> Result<(String, Claims), String> {
        self.create_token(user_id, Some(roles))
    }
}

impl Default for JwtTokenManager {
    fn default() -> Self {
        Self::new("test-secret-key-for-integration-tests")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_token() {
        let manager = JwtTokenManager::default();
        let result = manager.create_token("user-123", None);
        assert!(result.is_ok());

        let (token, claims) = result.unwrap();
        assert!(!token.is_empty());
        assert_eq!(claims.sub, "user-123");
        assert!(claims.exp > claims.iat);
    }

    #[test]
    fn test_create_expired_token() {
        let manager = JwtTokenManager::default();
        let result = manager.create_expired_token("user-123");
        assert!(result.is_ok());

        let (_, claims) = result.unwrap();
        assert!(claims.exp < Utc::now().timestamp());
    }

    #[test]
    fn test_create_admin_token() {
        let manager = JwtTokenManager::default();
        let result = manager.create_admin_token("admin-123");
        assert!(result.is_ok());

        let (_, claims) = result.unwrap();
        assert!(claims.roles.is_some());
        assert!(claims.roles.unwrap().contains(&"admin".to_string()));
    }
}
