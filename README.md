# 🦀 rrelayer 🦀

Note rrelayer is brand new and actively underdevelopment, things will change and bugs will exist—if you find any bugs or have any
feature requests, please open an issue on [github](https://github.com/joshstevens19/rrelayer/issues).

rrelayer is an opensource powerful, high-performance blockchain transaction relay service built in Rust, designed for seamless
integration with any EVM-compatible network. This tool transforms complex blockchain interactions into simple REST API calls,
eliminating the need for applications to manage wallets, transaction queuing, or nonce management. For rrelayer
supports advanced wallet infrastructure supporting multiple signing providers including AWS KMS hardware security modules,
Turnkey self-custody solutions, Privy managed wallets, AWS Secrets Manager, GCP Secret Manager, and raw mnemonic development setups.
It's highly scalable and production-ready, enabling you to build robust Web3 applications with reliability and focus exclusively on
your business logic. rrelayer has some super cool out-of-the-box features, like automatic top-ups (with safe support), permissions
config including allowlists, API keys with restricted access, webhooks, rate limiting, and the ability to configure the gas bump blocks.

You can get to the full rrelayer documentation [here](https://rrelayer.xyz/).

> [!NOTE]
> This project was sponsored by the Ethereum Foundation

## Install

```bash
curl -L https://rrelayer.xyz/install.sh | bash
```

If you’re on Windows, you will need to install and use Git BASH or WSL, as your terminal,
since rrelayer installation does not support Powershell or Cmd.

## Use rrelayer

Once installed you can run `rrelayer --help` in your terminal to see all the commands available to you.

```bash
rrelayer --help
```

```bash
Blazing fast EVM relayer tool built in rust

Usage: rrelayer [COMMAND]

Commands:
  new        Create a new rrelayer project
  clone      Clone an existing relayer private key to another network
  auth       Authenticate with rrelayer
  start      Start the relayer service
  network    Manage network configurations and settings
  list       List all configured relayers
  config     Configure operations for a specific relayer
  balance    Check the balance of a relayer's account
  allowlist  Manage allowlist addresses for restricted access
  create     Create a new relayer client instance
  sign       Sign messages and typed data alongside get history of signing
  tx         Send, manage and monitor transactions
  help       Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

We have full documentation https://rrelayer.xyz/getting-started/installation which goes into more detail on how to use
rrelayer and all the commands available to you.

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

## SDKs

- Node - https://rrelayer.xyz/integration/sdk/installation/node
- Rust - https://rrelayer.xyz/integration/sdk/installation/rust

## Docker

## Docker

A pre-built Docker image is available at `ghcr.io/joshstevens19/rrelayer`.

### Usage

To use rrelayer with Docker, you can run the CLI commands by mounting your project directory:

```bash
docker run -it -v $PWD:/app/project ghcr.io/joshstevens19/rrelayer --help
```

### Creating a new project

```bash
docker run -it -v $PWD:/app/project ghcr.io/joshstevens19/rrelayer new
```

### Running with existing project

```bash
export PROJECT_PATH=/path/to/your/project

docker run -it -v $PROJECT_PATH:/app/project ghcr.io/joshstevens19/rrelayer start
```

Docker is recommended for containerized deployments or when deploying rrelayer in cloud environments.

## Helm Chart

Coming soon.

## Building

### Requirements

- Rust (latest stable)

### Locally

To build locally you can just run `cargo build` in the root of the project. This will build everything for you as this is a workspace.

**Note:** The first build may take longer.

Subsequent builds use smart caching and will only rebuild components that have changed.

### Prod

To build for prod you can run `make prod_build` this will build everything for you and optimise it for production.

## Formatting

you can run `cargo fmt` to format the code, rules have been mapped in the `rustfmt.toml` file.

## Contributing

Anyone is welcome to contribute to rrelayer, feel free to look over the issues or open a new one if you have
any new ideas or bugs you have found.

### Playing around with the CLI locally

You can use the `make` commands to run the CLI commands locally, this is useful for testing and developing.
These are located in the `cli` folder > `Makefile`. It uses `CURDIR` to resolve the paths for you, so they should work
out of the box.

## Release

To release a new rrelayer:

1. Checkout `release/x.x.x` branch depending on the next version number
2. Push the branch to GitHub which will queue a build on the CI
3. Once build is successful, a PR will be automatically created with updated changelog and version
4. Review and merge the auto-generated PR - this will auto-deploy the release with binaries built from the release branch
