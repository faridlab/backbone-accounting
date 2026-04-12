// Authentication Middleware
// JWT-based authentication and authorization middleware

use actix_web::{dev::ServiceRequest, dev::ServiceResponse, Error, HttpMessage, Result};
use actix_web::{error::ErrorUnauthorized, middleware, web, HttpRequest};
use actix_web_httpauth::extractors::Authentication;
use actix_web_httpauth::middleware::HttpAuthentication;
use chrono::{Duration, Utc};
use futures_util::future::{ok, Ready};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

// JWT Claims
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,            // Subject (user ID)
    pub email: String,          // User email
    pub roles: Vec<String>,     // User roles
    pub permissions: Vec<String>, // User permissions
    pub exp: usize,             // Expiration time
    pub iat: usize,             // Issued at
    pub iss: String,            // Issuer
    pub aud: String,            // Audience
}

impl Claims {
    pub fn new(
        user_id: String,
        email: String,
        roles: Vec<String>,
        permissions: Vec<String>,
        secret: &str,
        expiration_hours: i64,
    ) -> Result<Self,jsonwebtoken::errors::Error> {
        let now = Utc::now();
        let exp = now + Duration::hours(expiration_hours);

        Ok(Self {
            sub: user_id,
            email,
            roles,
            permissions,
            exp: exp.timestamp() as usize,
            iat: now.timestamp() as usize,
            iss: "backbone-api".to_string(),
            aud: "backbone-clients".to_string(),
        })
    }

    pub fn create_token(&self, secret: &str) -> Result<String, jsonwebtoken::errors::Error> {
        let header = Header::default();
        let encoding_key = EncodingKey::from_secret(secret.as_ref());

        encode(&header, &self, &encoding_key)
    }

    pub fn decode_token(token: &str, secret: &str) -> Result<Self, jsonwebtoken::errors::Error> {
        let decoding_key = DecodingKey::from_secret(secret.as_ref());
        let validation = Validation::new(Algorithm::HS256);

        decode::<Claims>(token, &decoding_key, &validation).map(|data| data.claims)
    }

    pub fn is_expired(&self) -> bool {
        let now = Utc::now().timestamp() as usize;
        self.exp <= now
    }

    pub fn has_role(&self, role: &str) -> bool {
        self.roles.contains(&role.to_string())
    }

    pub fn has_permission(&self, permission: &str) -> bool {
        self.permissions.contains(&permission.to_string())
    }

    pub fn has_any_role(&self, roles: &[&str]) -> bool {
        roles.iter().any(|role| self.has_role(role))
    }

    pub fn has_any_permission(&self, permissions: &[&str]) -> bool {
        permissions.iter().any(|permission| self.has_permission(permission))
    }

    pub fn has_all_roles(&self, roles: &[&str]) -> bool {
        roles.iter().all(|role| self.has_role(role))
    }

    pub fn has_all_permissions(&self, permissions: &[&str]) -> bool {
        permissions.iter().all(|permission| self.has_permission(permission))
    }
}

// Authentication Request Extensions
#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub user_id: String,
    pub email: String,
    pub roles: Vec<String>,
    pub permissions: Vec<String>,
}

impl AuthenticatedUser {
    pub fn from_claims(claims: &Claims) -> Self {
        Self {
            user_id: claims.sub.clone(),
            email: claims.email.clone(),
            roles: claims.roles.clone(),
            permissions: claims.permissions.clone(),
        }
    }

    pub fn has_role(&self, role: &str) -> bool {
        self.roles.contains(&role.to_string())
    }

    pub fn has_permission(&self, permission: &str) -> bool {
        self.permissions.contains(&permission.to_string())
    }
}

// JWT Authentication Middleware
pub struct JwtAuth {
    secret: String,
}

impl JwtAuth {
    pub fn new(secret: String) -> Self {
        Self { secret }
    }

    pub fn validate_token(&self, token: &str) -> Result<Claims, String> {
        Claims::decode_token(token, &self.secret)
            .map_err(|e| format!("Invalid token: {}", e))
    }

    pub fn extract_token_from_header(req: &HttpRequest) -> Option<String> {
        if let Some(auth_header) = req.headers().get("Authorization") {
            if let Ok(auth_str) = auth_header.to_str() {
                if auth_str.starts_with("Bearer ") {
                    return Some(auth_str[7..].to_string());
                }
            }
        }
        None
    }
}

// Actix-web authentication extractor
impl actix_web_httpauth::extractors::AuthenticationError for AuthenticatedUser {
    fn from_request(req: &ServiceRequest, credentials: Self::Credentials) -> Result<Self, Error> {
        let token = match credentials {
            actix_web_httpauth::extractors::Authentication::Bearer(token) => token,
            _ => return Err(ErrorUnauthorized("Bearer token required")),
        };

        let jwt_secret = req
            .app_data::<web::Data<String>>()
            .ok_or_else(|| ErrorUnauthorized("JWT secret not configured"))?;

        let claims = Claims::decode_token(token, jwt_secret)
            .map_err(|_| ErrorUnauthorized("Invalid JWT token"))?;

        if claims.is_expired() {
            return Err(ErrorUnauthorized("Token has expired"));
        }

        Ok(AuthenticatedUser::from_claims(&claims))
    }
}

// Authorization middleware factory
pub fn create_jwt_auth(jwt_secret: String) -> HttpAuthentication<JwtAuth> {
    HttpAuthentication::with_fn(JwtAuth::new(jwt_secret), |req, auth: JwtAuth| {
        let token = JwtAuth::extract_token_from_header(req)
            .ok_or_else(|| ErrorUnauthorized("No token provided"))?;

        auth.validate_token(&token)
            .map_err(|_| ErrorUnauthorized("Invalid token"))?;

        Ok(())
    })
}

// Role-based authorization middleware
pub fn require_role(role: &'static str) -> impl Fn(&AuthenticatedUser) -> bool {
    move |user: &AuthenticatedUser| user.has_role(role)
}

// Permission-based authorization middleware
pub fn require_permission(permission: &'static str) -> impl Fn(&AuthenticatedUser) -> bool {
    move |user: &AuthenticatedUser| user.has_permission(permission)
}

// Multiple roles authorization middleware
pub fn require_any_role(roles: &'static [&'static str]) -> impl Fn(&AuthenticatedUser) -> bool {
    move |user: &AuthenticatedUser| user.has_any_role(roles)
}

// Multiple permissions authorization middleware
pub fn require_any_permission(permissions: &'static [&'static str]) -> impl Fn(&AuthenticatedUser) -> bool {
    move |user: &AuthenticatedUser| user.has_any_permission(permissions)
}

// All roles authorization middleware
pub fn require_all_roles(roles: &'static [&'static str]) -> impl Fn(&AuthenticatedUser) -> bool {
    move |user: &AuthenticatedUser| user.has_all_roles(roles)
}

// All permissions authorization middleware
pub fn require_all_permissions(permissions: &'static [&'static str]) -> impl Fn(&AuthenticatedUser) -> bool {
    move |user: &AuthenticatedUser| user.has_all_permissions(permissions)
}

// Rate limiting middleware
pub struct RateLimiter {
    requests: std::sync::RwLock<HashMap<String, std::sync::RwLock<RequestInfo>>>,
    max_requests: usize,
    window_seconds: u64,
}

#[derive(Debug, Clone)]
struct RequestInfo {
    count: usize,
    window_start: chrono::DateTime<chrono::Utc>,
}

impl RateLimiter {
    pub fn new(max_requests: usize, window_seconds: u64) -> Self {
        Self {
            requests: std::sync::RwLock::new(HashMap::new()),
            max_requests,
            window_seconds,
        }
    }

    pub fn is_allowed(&self, key: &str) -> bool {
        let mut requests = self.requests.write().unwrap();
        let now = Utc::now();

        match requests.get_mut(key) {
            Some(info) => {
                let window_elapsed = now.signed_duration_since(info.window_start).num_seconds();

                if window_elapsed >= self.window_seconds as i64 {
                    // Reset window
                    info.count = 1;
                    info.window_start = now;
                    true
                } else if info.count < self.max_requests {
                    info.count += 1;
                    true
                } else {
                    false
                }
            }
            None => {
                requests.insert(
                    key.to_string(),
                    std::sync::RwLock::new(RequestInfo {
                        count: 1,
                        window_start: now,
                    }),
                );
                true
            }
        }
    }

    pub fn cleanup_expired_entries(&self) {
        let mut requests = self.requests.write().unwrap();
        let now = Utc::now();

        requests.retain(|_, info| {
            let info = info.read().unwrap();
            now.signed_duration_since(info.window_start).num_seconds() < (self.window_seconds * 2) as i64
        });
    }
}

// Rate limiting middleware factory
pub fn create_rate_limiter(
    max_requests: usize,
    window_seconds: u64,
) -> impl Fn(ServiceRequest, actix_web::dev::Transform) -> Ready<Result<ServiceRequest, Error>> {
    let rate_limiter = std::sync::Arc::new(RateLimiter::new(max_requests, window_seconds));

    move |req: ServiceRequest, _: actix_web::dev::Transform| {
        let client_ip = req
            .connection_info()
            .peer_addr()
            .map(|addr| addr.ip().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        let key = format!("{}:{}", client_ip, req.path());

        if !rate_limiter.is_allowed(&key) {
            return ok(req.error_response(ErrorTooManyRequests("Rate limit exceeded")));
        }

        ok(req)
    }
}

// CORS middleware factory
pub fn create_cors_middleware() -> middleware::Cors {
    middleware::Cors::default()
        .allowed_origin(vec!["http://localhost:3000", "http://localhost:8080"])
        .allowed_methods(vec!["GET", "POST", "PUT", "DELETE", "PATCH", "OPTIONS"])
        .allowed_headers(vec!["Authorization", "Content-Type", "Accept"])
        .expose_headers(vec!["X-Total-Count", "X-Page-Count"])
        .supports_credentials()
        .max_age(3600)
}

// Request logging middleware
pub async fn request_logger(
    req: ServiceRequest,
    next: actix_web::dev::Transform,
) -> Result<ServiceResponse, Error> {
    let start = std::time::Instant::now();
    let method = req.method().clone();
    let path = req.path().to_string();
    let user_agent = req
        .headers()
        .get("User-Agent")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("unknown")
        .to_string();

    let resp = next.call(req).await?;

    let duration = start.elapsed();
    let status = resp.status();

    tracing::info!(
        method = %method,
        path = %path,
        status = %status.as_u16(),
        duration_ms = duration.as_millis(),
        user_agent = %user_agent,
        "HTTP request completed"
    );

    Ok(resp)
}

// Request ID middleware
pub async fn request_id(
    req: ServiceRequest,
    next: actix_web::dev::Transform,
) -> Result<ServiceResponse, Error> {
    let request_id = uuid::Uuid::new_v4().to_string();

    let mut resp = next.call(req).await?;

    resp.response_mut().headers_mut().insert(
        actix_web::http::header::HeaderName::from_static("x-request-id"),
        request_id.parse().unwrap(),
    );

    Ok(resp)
}

// API version middleware
pub async fn api_version_middleware(
    req: ServiceRequest,
    next: actix_web::dev::Transform,
) -> Result<ServiceResponse, Error> {
    let path = req.path();
    let version = path.strip_prefix("/api/v1").unwrap_or(path);

    let mut resp = next.call(req).await?;

    resp.response_mut().headers_mut().insert(
        actix_web::http::header::HeaderName::from_static("x-api-version"),
        "v1".parse().unwrap(),
    );

    resp.response_mut().headers_mut().insert(
        actix_web::http::header::HeaderName::from_static("x-api-path"),
        version.parse().unwrap(),
    );

    Ok(resp)
}

// Security headers middleware
pub async fn security_headers(
    req: ServiceRequest,
    next: actix_web::dev::Transform,
) -> Result<ServiceResponse, Error> {
    let mut resp = next.call(req).await?;

    let headers = resp.response_mut().headers_mut();

    // Security headers
    headers.insert(
        actix_web::http::header::HeaderName::from_static("x-content-type-options"),
        "nosniff".parse().unwrap(),
    );

    headers.insert(
        actix_web::http::header::HeaderName::from_static("x-frame-options"),
        "DENY".parse().unwrap(),
    );

    headers.insert(
        actix_web::http::header::HeaderName::from_static("x-xss-protection"),
        "1; mode=block".parse().unwrap(),
    );

    headers.insert(
        actix_web::http::header::HeaderName::from_static("strict-transport-security"),
        "max-age=31536000; includeSubDomains".parse().unwrap(),
    );

    Ok(resp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_claims_creation() {
        let claims = Claims::new(
            "user123".to_string(),
            "user@example.com".to_string(),
            vec!["user".to_string(), "admin".to_string()],
            vec!["read".to_string(), "write".to_string()],
            "test-secret",
            24,
        ).unwrap();

        assert_eq!(claims.sub, "user123");
        assert_eq!(claims.email, "user@example.com");
        assert_eq!(claims.roles, vec!["user", "admin"]);
        assert_eq!(claims.permissions, vec!["read", "write"]);
        assert!(!claims.is_expired());
    }

    #[test]
    fn test_claims_token_creation() {
        let claims = Claims::new(
            "user123".to_string(),
            "user@example.com".to_string(),
            vec!["user".to_string()],
            vec!["read".to_string()],
            "test-secret",
            1,
        ).unwrap();

        let token = claims.create_token("test-secret").unwrap();
        assert!(!token.is_empty());

        // Verify token can be decoded
        let decoded_claims = Claims::decode_token(&token, "test-secret").unwrap();
        assert_eq!(decoded_claims.sub, "user123");
        assert_eq!(decoded_claims.email, "user@example.com");
    }

    #[test]
    fn test_claims_validation() {
        let claims = Claims::new(
            "user123".to_string(),
            "user@example.com".to_string(),
            vec!["user".to_string()],
            vec!["read".to_string()],
            "test-secret",
            1,
        ).unwrap();

        assert!(claims.has_role("user"));
        assert!(!claims.has_role("admin"));
        assert!(claims.has_permission("read"));
        assert!(!claims.has_permission("write"));

        assert!(claims.has_any_role(&["user", "admin"]));
        assert!(!claims.has_any_role(&["admin", "manager"]));
        assert!(claims.has_any_permission(&["read", "write"]));
        assert!(!claims.has_any_permission(&["write", "delete"]));

        assert!(claims.has_all_roles(&["user"]));
        assert!(!claims.has_all_roles(&["user", "admin"]));
        assert!(claims.has_all_permissions(&["read"]));
        assert!(!claims.has_all_permissions(&["read", "write"]));
    }

    #[test]
    fn test_authenticated_user() {
        let claims = Claims::new(
            "user123".to_string(),
            "user@example.com".to_string(),
            vec!["user".to_string()],
            vec!["read".to_string()],
            "test-secret",
            1,
        ).unwrap();

        let user = AuthenticatedUser::from_claims(&claims);
        assert_eq!(user.user_id, "user123");
        assert_eq!(user.email, "user@example.com");
        assert!(user.has_role("user"));
        assert!(user.has_permission("read"));
    }

    #[test]
    fn test_jwt_auth_token_extraction() {
        let jwt_auth = JwtAuth::new("test-secret".to_string());

        // Test valid token
        let token = "Bearer valid-token";
        assert_eq!(JwtAuth::extract_token_from_header(&create_test_request_with_auth(token)), "valid-token");

        // Test no authorization header
        let request = create_test_request();
        assert!(JwtAuth::extract_token_from_header(&request).is_none());

        // Test invalid authorization header
        let request = create_test_request_with_auth("InvalidToken");
        assert!(JwtAuth::extract_token_from_header(&request).is_none());
    }

    fn create_test_request() -> HttpRequest {
        actix_web::test::TestRequest::get()
            .uri("/")
            .to_request()
    }

    fn create_test_request_with_auth(auth_header: &str) -> HttpRequest {
        actix_web::test::TestRequest::get()
            .uri("/")
            .insert_header(("Authorization", auth_header))
            .to_request()
    }

    #[test]
    fn test_rate_limiter() {
        let rate_limiter = RateLimiter::new(5, 60); // 5 requests per minute

        let client_ip = "127.0.0.1";

        // First 5 requests should be allowed
        for _ in 0..5 {
            assert!(rate_limiter.is_allowed(client_ip));
        }

        // 6th request should be denied
        assert!(!rate_limiter.is_allowed(client_ip));

        // Wait for window to reset (in real implementation, this would require time manipulation)
        // For testing, we'll create a new rate limiter
        let rate_limiter = RateLimiter::new(5, 60);
        assert!(rate_limiter.is_allowed("different_ip"));
    }

    #[test]
    fn test_authorization_functions() {
        // Test role functions
        let check_admin = require_role("admin");
        let check_user = require_role("user");
        let check_any_role = require_any_role(&["admin", "user"]);
        let check_all_roles = require_all_roles(&["user", "reader"]);

        let admin_user = AuthenticatedUser {
            user_id: "admin123".to_string(),
            email: "admin@example.com".to_string(),
            roles: vec!["admin".to_string(), "user".to_string()],
            permissions: vec![],
        };

        let regular_user = AuthenticatedUser {
            user_id: "user123".to_string(),
            email: "user@example.com".to_string(),
            roles: vec!["user".to_string()],
            permissions: vec![],
        };

        assert!(check_admin(&admin_user));
        assert!(!check_admin(&regular_user));
        assert!(check_user(&admin_user));
        assert!(check_user(&regular_user));
        assert!(check_any_role(&admin_user));
        assert!(check_any_role(&regular_user));
        assert!(check_all_roles(&admin_user));
        assert!(!check_all_roles(&regular_user));

        // Test permission functions
        let check_read = require_permission("read");
        let check_write = require_permission("write");
        let check_any_permission = require_any_permission(&["read", "write"]);
        let check_all_permissions = require_all_permissions(&["read", "write"]);

        let user_with_permissions = AuthenticatedUser {
            user_id: "user123".to_string(),
            email: "user@example.com".to_string(),
            roles: vec![],
            permissions: vec!["read".to_string(), "write".to_string()],
        };

        let user_with_read_only = AuthenticatedUser {
            user_id: "user123".to_string(),
            email: "user@example.com".to_string(),
            roles: vec![],
            permissions: vec!["read".to_string()],
        };

        assert!(check_read(&user_with_permissions));
        assert!(check_write(&user_with_permissions));
        assert!(!check_write(&user_with_read_only));
        assert!(check_any_permission(&user_with_permissions));
        assert!(check_any_permission(&user_with_read_only));
        assert!(check_all_permissions(&user_with_permissions));
        assert!(!check_all_permissions(&user_with_read_only));
    }
}