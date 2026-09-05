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
}

#[derive(Serialize)]
pub struct LoginResp {
    pub token: String,
    pub user: User,
}

#[derive(Serialize)]
pub struct MeResp {
    pub user: User,
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

    let token = generate_session_token();
    let expires = Utc::now() + Duration::days(7);
    state
        .db
        .create_session(&user.id, &token, expires)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let _ = state.db.touch_user_login(&user.id);

    Ok(Json(LoginResp { token, user }))
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

pub async fn me(auth: AuthUser) -> Result<impl IntoResponse, StatusCode> {
    let user = auth.user.ok_or(StatusCode::UNAUTHORIZED)?;
    Ok(Json(MeResp { user }))
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
