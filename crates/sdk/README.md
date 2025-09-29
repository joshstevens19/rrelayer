# rrelayer rust sdk

The official SDK for interacting with rrelayer services.

rrelayer is an opensource powerful, high-performance blockchain transaction relay service built in Rust,
designed for seamless integration with any EVM-compatible network. This tool transforms complex
blockchain interactions into simple REST API calls, eliminating the need for applications to
manage wallets, gas optimization, transaction queuing, or nonce management. For enterprise
needs, rrelayer provides advanced wallet infrastructure with support for multiple
secure signing providers including AWS KMS hardware security modules, Turnkey self-custody
solutions, Privy-managed wallets, AWS Secrets Manager, GCP Secret Manager, and raw mnemonic
development setups. It's highly scalable and production-ready, enabling you to build robust
Web3 applications with enterprise-grade reliability and focus exclusively on your business
logic. rrelayer out of the box gives you transaction relay, message signing, automated
gas management, and real-time monitoring through intuitive APIs.

## Usage

### Create Client And Authentication

rrelayer has two ways to create clients based on the authentication direction, which is basic auth and API keys;
we will explore both below.

#### Basic Auth

Using the basic authentication which uses the username and password in your [api config](/config/api-config)

```rs [Basic Auth - config.rs]
use std::str::FromStr;
use rrelayer::{CreateClientAuth, CreateClientConfig, RelayerId, TransactionSpeed, create_client, AdminRelayerClient};

// Client also exposes some admin methods in which API keys cannot do
let client = create_client(CreateClientConfig {
    server_url: "http://localhost:8000".to_string(),
    auth: CreateClientAuth {
        username: "your_username".to_string(),
        password: "your_password".to_string(),
    },
});

let relayer: AdminRelayerClient = client.get_relayer_client(
    &RelayerId::from_str("94afb207-bb47-4392-9229-ba87e4d783cb")?,
    Some(TransactionSpeed::FAST),
).await?;
```

## API Key Auth

Using API keys that have restricted permissions to only use the relayer - docs [here](config/networks/api-keys)

```rs [API Key - config.rs]
use std::str::FromStr;
use rrelayer::{
    CreateRelayerClientConfig, RelayerClient, RelayerId, TransactionSpeed,
    create_relayer_client,
};

let relayer: RelayerClient = create_relayer_client(CreateRelayerClientConfig {
    server_url: "http://localhost:8000".to_string(),
    relayer_id: RelayerId::from_str("94afb207-bb47-4392-9229-ba87e4d783cb")?,
    api_key: "YOUR_API_KEY".to_string(),
    speed: Some(TransactionSpeed::FAST),
});
```


Full documentation can be found [here](https://rrelayer.xyz/integration/sdk/installation/rust)