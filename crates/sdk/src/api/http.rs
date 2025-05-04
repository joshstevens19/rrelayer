use reqwest::{
    header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE},
    Client,
};
use serde::{de::DeserializeOwned, Serialize};

use crate::api::types::{ApiBaseConfig, ApiResult};

#[derive(Clone)]
pub struct HttpClient {
    client: Client,
    base_config: ApiBaseConfig,
}

impl HttpClient {
    pub fn new(base_config: ApiBaseConfig) -> Self {
        Self { client: Client::new(), base_config }
    }

    fn build_url(&self, endpoint: &str) -> String {
        let server_url = match &self.base_config {
            ApiBaseConfig::WithAuthToken { server_url, .. } => server_url,
            ApiBaseConfig::WithApiKey { server_url, .. } => server_url,
            ApiBaseConfig::Basic { server_url } => server_url,
        };
        format!("{}/{}", server_url.trim_end_matches('/'), endpoint.trim_start_matches('/'))
    }

    fn build_headers(&self, additional_headers: Option<HeaderMap>) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        match &self.base_config {
            ApiBaseConfig::WithAuthToken { auth_token, .. } => {
                headers.insert(
                    AUTHORIZATION,
                    HeaderValue::from_str(&format!("Bearer {}", auth_token)).unwrap(),
                );
            }
            ApiBaseConfig::WithApiKey { api_key, .. } => {
                headers.insert("x-api-key", HeaderValue::from_str(api_key).unwrap());
            }
            ApiBaseConfig::Basic { .. } => {}
        }

        if let Some(additional) = additional_headers {
            for (key, value) in additional {
                if let Some(key) = key {
                    headers.insert(key, value);
                }
            }
        }

        headers
    }

    pub async fn get<T>(&self, endpoint: &str) -> ApiResult<T>
    where
        T: DeserializeOwned,
    {
        let url = self.build_url(endpoint);
        let headers = self.build_headers(None);

        let response = self.client.get(&url).headers(headers).send().await?.error_for_status()?;

        Ok(response.json::<T>().await?)
    }

    pub async fn get_with_query<T, Q>(&self, endpoint: &str, query: Option<Q>) -> ApiResult<T>
    where
        T: DeserializeOwned,
        Q: Serialize,
    {
        let url = self.build_url(endpoint);
        let headers = self.build_headers(None);

        let mut request = self.client.get(&url).headers(headers);
        if let Some(q) = query {
            request = request.query(&q);
        }

        let response = request.send().await?.error_for_status()?;
        Ok(response.json().await?)
    }

    pub async fn post<T, B>(&self, endpoint: &str, body: &B) -> ApiResult<T>
    where
        T: DeserializeOwned,
        B: Serialize,
    {
        let url = self.build_url(endpoint);
        let headers = self.build_headers(None);

        let response =
            self.client.post(&url).headers(headers).json(body).send().await?.error_for_status()?;

        Ok(response.json::<T>().await?)
    }

    pub async fn post_status<B>(&self, endpoint: &str, body: &B) -> ApiResult<()>
    where
        B: Serialize,
    {
        let url = self.build_url(endpoint);
        let headers = self.build_headers(None);

        self.client.post(&url).headers(headers).json(body).send().await?.error_for_status()?;

        Ok(())
    }

    pub async fn put<T, B>(&self, endpoint: &str, body: &B) -> ApiResult<T>
    where
        T: DeserializeOwned,
        B: Serialize,
    {
        let url = self.build_url(endpoint);
        let headers = self.build_headers(None);

        let response =
            self.client.put(&url).headers(headers).json(body).send().await?.error_for_status()?;

        Ok(response.json().await?)
    }

    pub async fn put_status<B>(&self, endpoint: &str, body: &B) -> ApiResult<()>
    where
        B: Serialize,
    {
        let url = self.build_url(endpoint);
        let headers = self.build_headers(None);

        self.client.put(&url).headers(headers).json(body).send().await?.error_for_status()?;

        Ok(())
    }

    pub async fn delete<T>(&self, endpoint: &str) -> ApiResult<T>
    where
        T: DeserializeOwned,
    {
        let url = self.build_url(endpoint);
        let headers = self.build_headers(None);

        let response =
            self.client.delete(&url).headers(headers).send().await?.error_for_status()?;

        Ok(response.json().await?)
    }

    pub async fn delete_status(&self, endpoint: &str) -> ApiResult<()> {
        let url = self.build_url(endpoint);
        let headers = self.build_headers(None);

        self.client.delete(&url).headers(headers).send().await?.error_for_status()?;

        Ok(())
    }

    pub async fn delete_with_body<T, B>(&self, endpoint: &str, body: &B) -> ApiResult<T>
    where
        T: DeserializeOwned,
        B: Serialize,
    {
        let url = self.build_url(endpoint);
        let headers = self.build_headers(None);

        let response = self
            .client
            .delete(&url)
            .headers(headers)
            .json(body)
            .send()
            .await?
            .error_for_status()?;

        Ok(response.json().await?)
    }

    pub async fn get_status(&self, endpoint: &str) -> ApiResult<()> {
        let url = self.build_url(endpoint);
        let headers = self.build_headers(None);

        self.client.get(&url).headers(headers).send().await?.error_for_status()?;

        Ok(())
    }
}
