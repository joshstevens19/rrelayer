help:
	cargo run -- --help
init:
	cargo run -- init --path $(CURDIR)/../../playground
start:
	cargo run -- start --path $(CURDIR)/../../playground/example
auth_new_account:
	cargo run -- auth --path $(CURDIR)/../../playground/example new-account --account account2
auth_login:
	cargo run -- auth --path $(CURDIR)/../../playground/example login --account account2
auth_info:
	cargo run -- auth --path $(CURDIR)/../../playground/example info --account account1
auth_list_accounts:
	cargo run -- auth --path $(CURDIR)/../../playground/example list-accounts
network_list:
	cargo run -- network --path $(CURDIR)/../../playground/example list
network_list_disabled:
	cargo run -- network --path $(CURDIR)/../../playground/example list --filter disabled
network_list_enabled:
	cargo run -- network --path $(CURDIR)/../../playground/example list --filter enabled
network_add:
	cargo run -- network --path $(CURDIR)/../../playground/example add
network_enable:
	cargo run -- network --path $(CURDIR)/../../playground/example enable --name sepolia_ethereum
network_disable:
	cargo run -- network --path $(CURDIR)/../../playground/example disable --name sepolia_ethereum
network_gas:
	cargo run -- network --path $(CURDIR)/../../playground/example gas --name sepolia_ethereum
