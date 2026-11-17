use std::sync::Arc;

use axum::{extract::State, Json};
use chrono::{SecondsFormat, Utc};

use crate::{
    app_state::AppState,
    server::{build_info, ServerInfo},
};

/// Returns detailed server info.
pub async fn get_server_info(State(state): State<Arc<AppState>>) -> Json<ServerInfo<'static>> {
    Json(ServerInfo {
        started_at_timestamp_iso: state.started_at.to_rfc3339_opts(SecondsFormat::Secs, true),
        uptime_seconds: (Utc::now() - state.started_at).num_seconds(),

        commit_hash: build_info::BUILD_COMMIT_HASH,
        commit_timestamp_iso: build_info::BUILD_COMMIT_TIMESTAMP_ISO,
        commit_labels: build_info::BUILD_COMMIT_LABELS.split(" ").collect(),
        build_timestamp_iso: build_info::BUILD_TIMESTAMP_ISO,
    })
}
