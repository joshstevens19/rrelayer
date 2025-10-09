# rrelayer rust sdk

The official SDK for interacting with rrelayer services.

rrelayer is an opensource powerful, high-performance blockchain transaction relay service built in Rust, designed for seamless integration with any EVM-compatible network.
This tool transforms complex blockchain interactions into simple REST API calls, eliminating the need for applications to manage wallets, transaction queuing, or nonce management.
For rrelayer supports advanced wallet infrastructure supporting multiple signing providers including AWS KMS hardware security modules,
Turnkey self-custody solutions, Fireblocks enterprise MPC custody, Privy managed wallets, AWS Secrets Manager, GCP Secret Manager, PKCS#11 hardware security modules, and raw mnemonic development setups.
It's highly scalable and production-ready, enabling you to build robust Web3 applications with reliability and focus exclusively on your business logic.
rrelayer has some super cool out-of-the-box features, like automatic top-ups (with safe support), permissions config including allowlists, API keys with restricted access,
webhooks, rate limiting, and the ability to configure the gas bump blocks.

## Features

- Config Driven: Define everything in a simple rrelayer.yaml file, making it easy to turn on and off features
- Multi-Chain Support: Supports all EVM networks
- Transaction Relaying: Submit transactions to supported blockchain networks efficiently.
- Transaction Signing: Securely sign transactions using many of the signing providers.
- Transaction Fee Estimation: Estimate transaction fees for better cost management.
- Transaction Nonce Management: Handle nonce management to ensure transaction order.
- Transaction Status Monitoring: Track the status of submitted transactions.
- Transaction/Signing History: See all the transactions signed by the relayer or messages signed with a deep audit log
- SDK Integration: Integrate easily with our JS/TS API or rust API or
- Exposed API: If the SDK is not supported, you can just hit the API directly
- Extensible Architecture: Easily add support for new blockchain networks with a config update.
- Configurable Network Policies: Define and enforce network-specific policies for transaction processing.
- Automated top-ups: Build in background tasks to automatically top up relayers when gas or token funds are becoming low, with safe proxy support.
- Permissions: Add contract/addresses allowlists to relayers and turn on and off if they can sign messages, typed data, send transactions, and send native ETH.
- API Keys: Built-in API keys for relayers to allow you to give access to a system without giving access to every part of the rrelayer
- Webhooks: Notifications built in get notified of the transaction's status every step of the way or if balances are low.
- Rate Limiting: Built-in rate limiting allowing you to rate limit user transactions allowance by just updating the rrelayer.yaml
- Flexibility: No hard constraints on features like you can config how often it bumps gas depending on your need. Maybe a liquidation bot may want to bump every block for example.
- CLI: rrelayer is CLI first, so you can do everything with the command line tool.
- Full transactions support: rrelayer can send blob transactions and any kind of EVM transaction.

## What can I use rrelayer for?

- Session key relayers: Assign session keys to your backend to do stuff on behalf of users
- DApp backends: Handle user transactions without wallet management complexity
- NFT platforms: Automated minting, transfers, and marketplace operations with reliable execution
- DeFi protocols: Yield farming automation, liquidation bots, and cross-chain operations
- Enterprise Web3: Simplified blockchain integration for traditional businesses with audit compliance
- Development workflows: Consistent APIs for local development and comprehensive E2E testing
- Gasless transactions: Meta-transaction infrastructure for improved user experience
- Multi-chain applications: Unified transaction interface across different EVM networks
- High-frequency operations: Advanced queuing system for batch processing and optimization
- Production infrastructure: Enterprise-grade transaction reliability with comprehensive monitoring
- Loads more stuff like liquidation bot or trading bots etc

## Usage

### Create Client And Authentication

rrelayer has two ways to create clients based on the authentication direction, which is basic auth and API keys;
we will explore both below.

#### Basic Auth

Using the basic authentication which uses the username and password in your [api config](/config/api-config)

```rs [Basic Auth - config.rs]
use std::str::FromStr;
use rrelayer::{CreateClientAuth, CreateClientConfig, RelayerId, TransactionSpeed, create_client, AdminRelayerClient};
use dotenvy::dotenv;
use std::env;

// Client also exposes some admin methods in which API keys cannot do
let client = create_client(CreateClientConfig {
    server_url: "http://localhost:8000".to_string(),
    auth: CreateClientAuth {
        username: env::var("RRELAYER_AUTH_USERNAME")
                          .expect("RRELAYER_AUTH_USERNAME must be set"),
        password: env::var("RRELAYER_AUTH_PASSWORD")
                          .expect("RRELAYER_AUTH_PASSWORD must be set"),
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