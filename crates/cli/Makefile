help:
	cargo run -- --help
init:
	cargo run -- init --path $(CURDIR)/../../playground
start:
	cargo run -- start --path $(CURDIR)/../../playground/example

auth_new_account:
	cargo run -- auth --path $(CURDIR)/../../playground/example new-account account2
auth_login:
	cargo run -- auth --path $(CURDIR)/../../playground/example login account2
auth_logout:
	cargo run -- auth --path $(CURDIR)/../../playground/example logout account2
auth_info:
	cargo run -- auth --path $(CURDIR)/../../playground/example info account1
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
	cargo run -- network --path $(CURDIR)/../../playground/example enable sepolia_ethereum
network_disable:
	cargo run -- network --path $(CURDIR)/../../playground/example disable sepolia_ethereum
network_gas:
	cargo run -- network --path $(CURDIR)/../../playground/example gas sepolia_ethereum

user_list:
	cargo run -- user --path $(CURDIR)/../../playground/example list
user_delete:
	cargo run -- user --path $(CURDIR)/../../playground/example delete 0xb058b2eDFe8b2C5FdcED21EcE660C3e040ebd18D
user_add:
	cargo run -- user --path $(CURDIR)/../../playground/example add 0xb058b2eDFe8b2C5FdcED21EcE660C3e040ebd18D admin
user_edit:
	cargo run -- user --path $(CURDIR)/../../playground/example edit 0xb058b2eDFe8b2C5FdcED21EcE660C3e040ebd18D manager

list:
	cargo run -- list --path $(CURDIR)/../../playground/example

create:
	cargo run -- create --path $(CURDIR)/../../playground/example relayer1 sepolia_ethereum

config_get:
	cargo run -- config --path $(CURDIR)/../../playground/example get 3c24e942-3109-4516-bd3b-6e2a904ef473