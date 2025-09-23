help:
	cargo run -- sign --help
init:
	cargo run -- init --path $(CURDIR)/../../playground
start:
	RUST_BACKTRACE=1 cargo run -- start --path $(CURDIR)/../../playground/example

auth_status:
	cargo run -- auth --path $(CURDIR)/../../playground/example status

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

list:
	cargo run -- list --path $(CURDIR)/../../playground/example

create:
	cargo run -- create --path $(CURDIR)/../../playground/example --name relayer1 --network sepolia_ethereum

clone:
	cargo run -- clone --path $(CURDIR)/../../playground/example --relayer-id 7c73fbbb-04b1-4a9a-9597-b6b83d891a8f --name relayer2 --network sepolia_ethereum

config_get:
	cargo run -- config --path $(CURDIR)/../../playground/example get --relayer-id "b7b60cca-34a4-4879-8ae0-541b7ba41d9"
config_pause:
	cargo run -- config --path $(CURDIR)/../../playground/example pause --relayer-id 7c73fbbb-04b1-4a9a-9597-b6b83d891a8f
config_unpause:
	cargo run -- config --path $(CURDIR)/../../playground/example unpause --relayer-id 7c73fbbb-04b1-4a9a-9597-b6b83d891a8f
# TODO: How to set back to EIP-1559
config_gas_latest:
	cargo run -- config --path $(CURDIR)/../../playground/example gas --relayer-id 7c73fbbb-04b1-4a9a-9597-b6b83d891a8f latest
config_gas_legacy:
	cargo run -- config --path $(CURDIR)/../../playground/example gas --relayer-id 7c73fbbb-04b1-4a9a-9597-b6b83d891a8f legacy
config_gas_max_price:
	cargo run -- config --path $(CURDIR)/../../playground/example gas --relayer-id 7c73fbbb-04b1-4a9a-9597-b6b83d891a8f max-price --price 1000000
config_gas_max_price_unset:
	cargo run -- config --path $(CURDIR)/../../playground/example gas --relayer-id 7c73fbbb-04b1-4a9a-9597-b6b83d891a8f max-price --price 0

balance:
	cargo run -- balance --path $(CURDIR)/../../playground/example --relayer-id 7c73fbbb-04b1-4a9a-9597-b6b83d891a8f

balance_token:
	cargo run -- balance --path $(CURDIR)/../../playground/example --relayer-id 7c73fbbb-04b1-4a9a-9597-b6b83d891a8f --token 0x73d219B3881E481394DA6B5008A081d623992200

allowlist_create:
	cargo run -- allowlist --path $(CURDIR)/../../playground/example add --relayer-id 7c73fbbb-04b1-4a9a-9597-b6b83d891a8f --address 0x73d219B3881E481394DA6B5008A081d623992200
allowlist__list:
	cargo run -- allowlist --path $(CURDIR)/../../playground/example list --relayer-id 7c73fbbb-04b1-4a9a-9597-b6b83d891a8f
allowlist_delete:
	cargo run -- allowlist --path $(CURDIR)/../../playground/example delete --relayer-id 7c73fbbb-04b1-4a9a-9597-b6b83d891a8f --address 0x73d219B3881E481394DA6B5008A081d623992200

sign_text:
	cargo run -- sign --path $(CURDIR)/../../playground/example text --relayer-id 7ed18611-1739-4a5f-99e3-e70356029984 --message "hello baby"
sign_text_history:
	cargo run -- sign --path $(CURDIR)/../../playground/example text-history --relayer-id 7ed18611-1739-4a5f-99e3-e70356029984
sign_typed_data:
	cargo run -- sign --path $(CURDIR)/../../playground/example typed-data --relayer-id 7ed18611-1739-4a5f-99e3-e70356029984 --data '{"types":{"EIP712Domain":[{"name":"name","type":"string"},{"name":"version","type":"string"},{"name":"chainId","type":"uint256"},{"name":"verifyingContract","type":"address"}],"Person":[{"name":"name","type":"string"},{"name":"wallet","type":"address"}],"Mail":[{"name":"from","type":"Person"},{"name":"to","type":"Person"},{"name":"contents","type":"string"}]},"primaryType":"Mail","domain":{"name":"Ether Mail","version":"1","chainId":1,"verifyingContract":"0xCcCCccccCCCCcCCCCCCcCcCccCcCCCcCcccccccC"},"message":{"from":{"name":"Cow","wallet":"0xCD2a3d9F938E13CD947Ec05AbC7FE734Df8DD826"},"to":{"name":"Bob","wallet":"0xbBbBBBBbbBBBbbbBbbBbbbbBBbBbbbbBbBbbBBbB"},"contents":"Hello, Bob!"}}'
sign_typed_data_history:
	cargo run -- sign --path $(CURDIR)/../../playground/example typed-data-history --relayer-id 7ed18611-1739-4a5f-99e3-e70356029984

tx_queue:
	cargo run -- tx --path $(CURDIR)/../../playground/example queue --relayer-id 3c24e942-3109-4516-bd3b-6e2a904ef473
tx_get:
	cargo run -- tx --path $(CURDIR)/../../playground/example get --tx-id 578eb9d1-f778-4009-9a26-e97b7b9982b6
tx_relayer_list:
	cargo run -- tx --path $(CURDIR)/../../playground/example list --relayer-id 3c24e942-3109-4516-bd3b-6e2a904ef473
tx_withdraw:
	cargo run -- tx --path $(CURDIR)/../../playground/example withdraw --relayer-id 3c24e942-3109-4516-bd3b-6e2a904ef473 --address 0x10AE77B225626C1341bae10C4e6528D5520E1E33 --amount "0.001"
tx_status:
	cargo run -- tx --path $(CURDIR)/../../playground/example status --tx-id 578eb9d1-f778-4009-9a26-e97b7b9982b6
tx_send:
	cargo run -- tx --path $(CURDIR)/../../playground/example send --relayer-id e0aa336b-d936-499d-8873-f7ff0d687702 --transaction '{"to":"0x10AE77B225626C1341bae10C4e6528D5520E1E33","value":"1000000","data":"0x","gas_limit":21000}'
tx_cancel:
	cargo run -- tx --path $(CURDIR)/../../playground/example cancel --tx-id 578eb9d1-f778-4009-9a26-e97b7b9982b6
tx_replace:
	cargo run -- tx --path $(CURDIR)/../../playground/example replace --tx-id 578eb9d1-f778-4009-9a26-e97b7b9982b6 --transaction '{"to":"0x10AE77B225626C1341bae10C4e6528D5520E1E33","value":"1000000","data":"0x","gas_limit":21000}'
tx_send_blob:
	cargo run -- tx --path $(CURDIR)/../../playground/example send --relayer-id 3c24e942-3109-4516-bd3b-6e2a904ef473 --transaction '{"to":"0x10AE77B225626C1341bae10C4e6528D5520E1E33","value":"1000000","data":"0x","blobs":["0x$(shell python3 -c "print('01' * 131072)")"]}'
tx_fund:
	cargo run -- tx --path $(CURDIR)/../../playground/example fund --relayer-id b7b60cca-34a4-4879-8ae0-541b7ba41d9 --amount "10"