# rrelayer_cli

This is the cli for rrelayer, it contains all the logic for the cli and is how users interact with rrelayer.

You can get to the full rrelayer [documentation](https://rrelayer.xyz/getting-started/installation).

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

## Working with CLI locally

The best way to work with the CLI is to use the `Makefile` predefined commands.

You can also run your own commands using cargo run, example below would create a new rrelayer project in the path you specified.

```bash
cargo run -- new --path PATH_TO_CREATE_PROJECT
```