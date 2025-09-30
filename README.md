# 🦀 rrelayer 🦀

Note rrelayer is brand new and actively under development, things will change and bugs will exist - if you find any bugs or have any
feature requests please open an issue on [github](https://github.com/joshstevens19/rrelayer/issues).

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

## Docker

Coming soon.

## Helm Chart

Coming soon.

## What can I use rrelayer for?

* DApp backends: Handle user transactions without wallet management complexity
* NFT platforms: Automated minting, transfers, and marketplace operations with reliable execution
* DeFi protocols: Yield farming automation, liquidation bots, and cross-chain operations
* Enterprise Web3: Simplified blockchain integration for traditional businesses with audit compliance
* Development workflows: Consistent APIs for local development and comprehensive E2E testing
* Gasless transactions: Meta-transaction infrastructure for improved user experience
* Multi-chain applications: Unified transaction interface across different EVM networks
* High-frequency operations: Advanced queuing system for batch processing and optimization
* Production infrastructure: Enterprise-grade transaction reliability with comprehensive monitoring
- Much more...

## SDKs

- Node - https://rrelayer.xyz/integration/sdk/installation/node
- Rust - https://rrelayer.xyz/integration/sdk/installation/rust

## What networks do you support?

rrelayer supports any EVM chain out of the box. If you have a custom chain, you can easily add support for it by
adding the chain's RPC URL to the YAML configuration file and defining the chain ID. No code changes are required.

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
