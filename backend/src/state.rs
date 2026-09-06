use std::sync::Arc;

use axum::extract::FromRequestParts;
use axum::http::request::Parts;

use crate::config::Config;
use crate::db::{Database, User};
use crate::tasks::TaskManager;

#[derive(Clone)]
pub struct AppState {
    pub db: Database,
    pub tasks: Arc<TaskManager>,
    pub config: Arc<Config>,
}

impl AppState {
    pub fn new(db: Database, tasks: TaskManager, config: Config) -> Self {
        Self {
            db,
            tasks: Arc::new(tasks),
            config: Arc::new(config),
        }
    }

    pub async fn verify_key(&self, key: &str) -> bool {
        self.db.verify_api_key(key).unwrap_or(false)
    }

    pub async fn verify_session(&self, token: &str) -> Option<User> {
        let auth = self
            .db
            .get_auth_settings(self.config.as_ref())
            .unwrap_or(crate::db::AuthSettings {
                session_ttl_hours: self.config.session_ttl_hours,
                remember_me_ttl_hours: self.config.remember_me_ttl_hours,
                sliding: self.config.session_sliding,
            });
        // Sliding uses the shorter base TTL so remember-me sessions still expire eventually.
        self.db
            .verify_session_sliding(token, auth.sliding, auth.session_ttl_hours)
            .ok()
            .flatten()
            .map(|(u, _)| u)
    }
}

#[derive(Clone, Debug)]
pub struct AuthUser {
    pub user: Option<User>,
    pub token: Option<String>,
    pub via_api_key: bool,
}

impl AuthUser {
    pub fn is_admin(&self) -> bool {
        self.user.as_ref().is_some_and(|u| u.role == "admin")
    }
}

impl<S: Send + Sync> FromRequestParts<S> for AuthUser {
    type Rejection = ();

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(parts
            .extensions
            .get::<AuthUser>()
            .cloned()
            .unwrap_or(AuthUser {
                user: None,
                token: None,
                via_api_key: false,
            }))
    }
}
