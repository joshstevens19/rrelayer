use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    middleware::from_fn,
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::Deserialize;

use super::types::User;
use crate::{
    app_state::AppState,
    authentication::{guards::admin_jwt_guard, types::JwtRole},
    rrelayer_error,
    shared::common_types::{EvmAddress, PagingContext, PagingQuery, PagingResult},
};

// TODO! add paged caching
async fn get_users(
    State(state): State<Arc<AppState>>,
    Query(paging): Query<PagingQuery>,
) -> Result<Json<PagingResult<User>>, StatusCode> {
    state.db.get_users(&PagingContext::new(paging.limit, paging.offset)).await.map(Json).map_err(
        |e| {
            rrelayer_error!("{}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        },
    )
}

#[derive(Debug, Deserialize)]
struct EditUserRequest {
    pub user: EvmAddress,
    #[serde(rename = "newRole")]
    pub new_role: JwtRole,
}

async fn edit_user(
    State(state): State<Arc<AppState>>,
    Json(edit_user_request): Json<EditUserRequest>,
) -> StatusCode {
    match state.db.edit_user(&edit_user_request.user, &edit_user_request.new_role).await {
        Ok(_) => StatusCode::NO_CONTENT,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

#[derive(Debug, Deserialize)]
struct AddUserRequest {
    pub user: EvmAddress,
    pub role: JwtRole,
}

async fn add_user(
    State(state): State<Arc<AppState>>,
    Json(add_user_request): Json<AddUserRequest>,
) -> StatusCode {
    match state.db.add_user(&add_user_request.user, &add_user_request.role).await {
        Ok(_) => StatusCode::NO_CONTENT,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

async fn delete_user(
    State(state): State<Arc<AppState>>,
    Path(user): Path<EvmAddress>,
) -> StatusCode {
    match state.db.delete_user(&user).await {
        Ok(_) => StatusCode::NO_CONTENT,
        Err(e) => {
            rrelayer_error!("{}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

pub fn create_user_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(get_users))
        .route("/edit", put(edit_user))
        .route("/add", post(add_user))
        .route("/:user", delete(delete_user))
        .route_layer(from_fn(admin_jwt_guard))
}
