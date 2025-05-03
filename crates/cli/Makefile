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