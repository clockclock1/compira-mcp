use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};

use crate::auth::{generate_session_token, hash_password, verify_password};
use crate::db::User;
use crate::state::{AppState, AuthUser};

#[derive(Deserialize)]
pub struct LoginReq {
    pub username: String,
    pub password: String,
    #[serde(default)]
    pub remember_me: bool,
}

#[derive(Serialize)]
pub struct LoginResp {
    pub token: String,
    pub user: User,
    pub expires_at: String,
    pub remember_me: bool,
}

#[derive(Serialize)]
pub struct MeResp {
    pub user: User,
    pub expires_at: Option<String>,
}

#[derive(Deserialize)]
pub struct CreateUserReq {
    pub username: String,
    pub password: String,
    #[serde(default = "default_role")]
    pub role: String,
    pub display_name: Option<String>,
}

fn default_role() -> String {
    "user".into()
}

#[derive(Deserialize)]
pub struct UpdateUserReq {
    pub password: Option<String>,
    pub role: Option<String>,
    pub display_name: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateAccountReq {
    pub username: Option<String>,
    pub current_password: String,
    pub new_password: Option<String>,
}

pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginReq>,
) -> Result<impl IntoResponse, StatusCode> {
    let Some((user, hash)) = state
        .db
        .get_user_by_username(&req.username)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    else {
        return Err(StatusCode::UNAUTHORIZED);
    };

    if !verify_password(&req.password, &hash).unwrap_or(false) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let auth_cfg = state
        .db
        .get_auth_settings(state.config.as_ref())
        .unwrap_or(crate::db::AuthSettings {
            session_ttl_hours: state.config.session_ttl_hours,
            remember_me_ttl_hours: state.config.remember_me_ttl_hours,
            sliding: state.config.session_sliding,
        });

    let ttl_hours = if req.remember_me {
        auth_cfg.remember_me_ttl_hours
    } else {
        auth_cfg.session_ttl_hours
    };
    let expires = Utc::now() + Duration::hours(ttl_hours as i64);
    let token = generate_session_token();
    state
        .db
        .create_session(&user.id, &token, expires)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let _ = state.db.touch_user_login(&user.id);

    Ok(Json(LoginResp {
        token,
        user,
        expires_at: expires.to_rfc3339(),
        remember_me: req.remember_me,
    }))
}

pub async fn logout(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<impl IntoResponse, StatusCode> {
    if let Some(token) = auth.token {
        let _ = state.db.delete_session(&token);
    }
    Ok(StatusCode::NO_CONTENT)
}

pub async fn me(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<impl IntoResponse, StatusCode> {
    let user = auth.user.ok_or(StatusCode::UNAUTHORIZED)?;
    let expires_at = auth
        .token
        .as_deref()
        .and_then(|t| state.db.session_expires_at(t).ok().flatten())
        .map(|t| t.to_rfc3339());
    Ok(Json(MeResp { user, expires_at }))
}

/// Change own username / password (requires current password).
pub async fn update_account(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<UpdateAccountReq>,
) -> Result<impl IntoResponse, StatusCode> {
    let user = auth.user.ok_or(StatusCode::UNAUTHORIZED)?;
    let Some((_, hash)) = state
        .db
        .get_user_by_username(&user.username)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    else {
        return Err(StatusCode::UNAUTHORIZED);
    };

    if !verify_password(&req.current_password, &hash).unwrap_or(false) {
        return Err(StatusCode::FORBIDDEN);
    }

    if let Some(username) = req.username.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
        if username.len() < 3 {
            return Err(StatusCode::BAD_REQUEST);
        }
        if username != user.username {
            if state
                .db
                .get_user_by_username(username)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
                .is_some()
            {
                return Err(StatusCode::CONFLICT);
            }
            state
                .db
                .update_user_username(&user.id, username)
                .map_err(|e| {
                    if e.to_string().contains("UNIQUE") {
                        StatusCode::CONFLICT
                    } else {
                        StatusCode::INTERNAL_SERVER_ERROR
                    }
                })?;
        }
    }

    if let Some(new_password) = &req.new_password {
        if new_password.len() < 6 {
            return Err(StatusCode::BAD_REQUEST);
        }
        let new_hash =
            hash_password(new_password).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        state
            .db
            .update_user_password(&user.id, &new_hash)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        // Keep current session; kick other devices.
        let _ = state
            .db
            .delete_user_sessions(&user.id, auth.token.as_deref());
    }

    let updated = state
        .db
        .get_user(&user.id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(MeResp {
        user: updated,
        expires_at: auth
            .token
            .as_deref()
            .and_then(|t| state.db.session_expires_at(t).ok().flatten())
            .map(|t| t.to_rfc3339()),
    }))
}

pub async fn list_users(
    auth: AuthUser,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, StatusCode> {
    require_admin(&auth)?;
    match state.db.list_users() {
        Ok(users) => Ok(Json(users)),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn create_user(
    auth: AuthUser,
    State(state): State<AppState>,
    Json(req): Json<CreateUserReq>,
) -> Result<impl IntoResponse, StatusCode> {
    require_admin(&auth)?;

    if req.username.len() < 3 || req.password.len() < 6 {
        return Err(StatusCode::BAD_REQUEST);
    }
    if req.role != "admin" && req.role != "user" {
        return Err(StatusCode::BAD_REQUEST);
    }

    let hash = hash_password(&req.password).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    match state.db.create_user(
        &req.username,
        &hash,
        &req.role,
        req.display_name.as_deref(),
    ) {
        Ok(user) => Ok((StatusCode::CREATED, Json(user))),
        Err(e) if e.to_string().contains("UNIQUE") => Err(StatusCode::CONFLICT),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn update_user(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateUserReq>,
) -> Result<impl IntoResponse, StatusCode> {
    require_admin(&auth)?;

    if let Some(role) = &req.role {
        if role != "admin" && role != "user" {
            return Err(StatusCode::BAD_REQUEST);
        }
        state
            .db
            .update_user_role(&id, role)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    if let Some(password) = &req.password {
        if password.len() < 6 {
            return Err(StatusCode::BAD_REQUEST);
        }
        let hash = hash_password(password).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        state
            .db
            .update_user_password(&id, &hash)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        // Force re-login everywhere after admin reset.
        let _ = state.db.delete_user_sessions(&id, None);
    }

    let user = state
        .db
        .get_user(&id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(user))
}

pub async fn delete_user(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    require_admin(&auth)?;

    if auth.user.as_ref().map(|u| u.id.as_str()) == Some(id.as_str()) {
        return Err(StatusCode::BAD_REQUEST);
    }

    let admins: Vec<_> = state
        .db
        .list_users()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .into_iter()
        .filter(|u| u.role == "admin")
        .collect();
    if admins.len() <= 1 {
        if let Some(u) = state.db.get_user(&id).ok().flatten() {
            if u.role == "admin" {
                return Err(StatusCode::BAD_REQUEST);
            }
        }
    }

    state
        .db
        .delete_user(&id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(StatusCode::NO_CONTENT)
}

fn require_admin(auth: &AuthUser) -> Result<(), StatusCode> {
    match &auth.user {
        Some(u) if u.role == "admin" => Ok(()),
        Some(_) => Err(StatusCode::FORBIDDEN),
        None => Err(StatusCode::UNAUTHORIZED),
    }
}
