use rrelayerr_core::{
    authentication::types::JwtRole,
    common_types::{EvmAddress, PagingQuery, PagingResult},
    user::types::User,
};
use serde::Serialize;

use crate::api::{http::HttpClient, types::ApiResult};

pub struct UserApi {
    client: HttpClient,
}

impl UserApi {
    pub fn new(client: HttpClient) -> Self {
        Self { client }
    }

    /// Get all users with pagination
    pub async fn get(&self, paging_context: &PagingQuery) -> ApiResult<PagingResult<User>> {
        self.client.get_with_query("users", Some(paging_context)).await
    }

    /// Add a new user
    pub async fn add(&self, user: &EvmAddress, role: &JwtRole) -> ApiResult<()> {
        #[derive(Serialize)]
        struct AddUserRequest {
            user: String,
            role: JwtRole,
        }

        self.client
            .post_status(
                "users/add",
                &AddUserRequest { user: user.to_string(), role: role.clone() },
            )
            .await
    }

    /// Edit an existing user's role
    pub async fn edit(&self, user: &EvmAddress, new_role: &JwtRole) -> ApiResult<()> {
        #[derive(Serialize)]
        struct EditUserRequest {
            user: String,
            #[serde(rename = "newRole")]
            new_role: JwtRole,
        }

        self.client
            .put_status(
                "users/edit",
                &EditUserRequest { user: user.to_string(), new_role: new_role.clone() },
            )
            .await
    }

    /// Delete a user
    pub async fn delete(&self, user: &EvmAddress) -> ApiResult<()> {
        self.client.delete_status(&format!("users/{}", user)).await
    }

    /// Get a single user by address
    pub async fn get_by_address(&self, address: &EvmAddress) -> ApiResult<Option<User>> {
        self.client.get(&format!("users/{}", address.to_string())).await
    }
}
