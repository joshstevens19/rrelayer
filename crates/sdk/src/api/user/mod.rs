use alloy::primitives::Address;
use rrelayerr_core::{authentication::types::JwtRole, user::types::User};
use serde::Serialize;

use crate::api::{
    http::HttpClient,
    types::{ApiResult, PagingContext, PagingResult},
};

pub struct UserApi {
    client: HttpClient,
}

impl UserApi {
    pub fn new(client: HttpClient) -> Self {
        Self { client }
    }

    /// Get all users with pagination
    pub async fn get(&self, paging_context: &PagingContext) -> ApiResult<PagingResult<User>> {
        self.client.get_with_query("users", Some(paging_context)).await
    }

    /// Add a new user
    pub async fn add(&self, user: &Address, role: JwtRole) -> ApiResult<()> {
        #[derive(Serialize)]
        struct AddUserRequest {
            user: String,
            role: JwtRole,
        }

        self.client.post("users/add", &AddUserRequest { user: user.to_string(), role }).await
    }

    /// Edit an existing user's role
    pub async fn edit(&self, user: &Address, new_role: JwtRole) -> ApiResult<()> {
        #[derive(Serialize)]
        struct EditUserRequest {
            user: String,
            #[serde(rename = "newRole")]
            new_role: JwtRole,
        }

        self.client.put("users/edit", &EditUserRequest { user: user.to_string(), new_role }).await
    }

    /// Delete a user
    pub async fn delete(&self, user: &Address) -> ApiResult<()> {
        self.client.delete(&format!("users/{}", user)).await
    }

    /// Get a single user by address
    pub async fn get_by_address(&self, address: &Address) -> ApiResult<Option<User>> {
        self.client.get(&format!("users/{}", address.to_string())).await
    }
}
