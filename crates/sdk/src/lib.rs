mod api;
mod types;

use alloy::{primitives::Address, signers};
pub use api::{Authentication, GasApi, NetworkApi, RelayerApi, SignApi, TransactionApi, UserApi};
use rrelayerr_core::authentication::{
    api::{AuthenticateRequest, GenerateSecretResult},
    types::TokenPair,
};

use crate::{
    api::{http::HttpClient, ApiResult, ApiSdkError, HealthApi},
    types::SdkContext,
};

pub struct SDK {
    pub context: SdkContext,
    client: HttpClient,
    pub auth: Authentication,
    pub gas: GasApi,
    pub network: NetworkApi,
    pub relayer: RelayerApi,
    pub sign: SignApi,
    pub transaction: TransactionApi,
    pub user: UserApi,
    pub health: HealthApi,
}

impl SDK {
    pub fn new(server_url: String) -> Self {
        let context = SdkContext::new(server_url);
        let client = HttpClient::new(context.config.clone());

        Self {
            auth: Authentication::new(client.clone()),
            gas: GasApi::new(client.clone()),
            network: NetworkApi::new(client.clone()),
            relayer: RelayerApi::new(client.clone()),
            sign: SignApi::new(client.clone()),
            transaction: TransactionApi::new(client.clone()),
            user: UserApi::new(client.clone()),
            health: HealthApi::new(client.clone()),
            client,
            context,
        }
    }

    /// Generate the auth challenge for the address
    pub async fn get_auth_challenge(&self, address: &Address) -> ApiResult<GenerateSecretResult> {
        self.auth.generate_auth_secret(address).await
    }

    /// Login after signing the auth challenge
    pub async fn login(
        &mut self,
        challenge: &GenerateSecretResult,
        signature: signers::Signature,
    ) -> ApiResult<()> {
        let token_pair = self
            .auth
            .authenticate(AuthenticateRequest {
                id: challenge.id,
                signature,
                signed_by: challenge.address,
            })
            .await?;

        self.update_auth_token(token_pair);
        Ok(())
    }

    /// Refresh the authentication token
    pub async fn refresh_auth(&mut self) -> ApiResult<()> {
        if let Some(current_token) = self.context.token_pair.as_ref() {
            let new_token_pair = self.auth.refresh_auth_token(&current_token.refresh_token).await?;

            self.update_auth_token(new_token_pair);
            Ok(())
        } else {
            Err(ApiSdkError::ConfigError("No refresh token available".into()))
        }
    }

    /// Get current authentication status
    pub fn is_authenticated(&self) -> bool {
        self.context.token_pair.is_some()
    }

    pub fn update_auth_token(&mut self, token_pair: TokenPair) {
        self.context.token_pair = Some(token_pair.clone());

        let new_config = self.context.with_auth_token(token_pair.access_token);

        let new_client = HttpClient::new(new_config);

        self.auth = Authentication::new(new_client.clone());
        self.gas = GasApi::new(new_client.clone());
        self.network = NetworkApi::new(new_client.clone());
        self.relayer = RelayerApi::new(new_client.clone());
        self.sign = SignApi::new(new_client.clone());
        self.transaction = TransactionApi::new(new_client.clone());
        self.user = UserApi::new(new_client.clone());
        self.client = new_client;
    }
}
