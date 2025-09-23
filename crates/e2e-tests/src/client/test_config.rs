use rrelayer_core::common_types::EvmAddress;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct E2ETestConfig {
    pub anvil_port: u16,
    pub rrelayer_base_url: String,
    pub test_timeout_seconds: u64,
    pub anvil_accounts: Vec<EvmAddress>,
    pub anvil_private_keys: Vec<String>,
    pub chain_id: u64,
    pub test_contract_bytecode: String,
}

impl Default for E2ETestConfig {
    fn default() -> Self {
        Self {
            anvil_port: 8545,
            rrelayer_base_url: "http://localhost:3000".to_string(),
            test_timeout_seconds: 30,
            chain_id: 31337,
            anvil_accounts: vec![
                // THIS KEY WILL HAVE 1 TX A MINUTE AND SIGNATURE LIMITS
                EvmAddress::from_str("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266").unwrap(),
                EvmAddress::from_str("0x70997970C51812dc3A010C7d01b50e0d17dc79C8").unwrap() ,
                EvmAddress::from_str("0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC").unwrap() ,
                EvmAddress::from_str("0x90F79bf6EB2c4f870365E785982E1f101E93b906").unwrap() ,
                EvmAddress::from_str("0x15d34AAf54267DB7D7c367839AAf71A00a2C6A65").unwrap(),
            ],
            anvil_private_keys: vec![
                "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80".to_string(),
                "0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d".to_string(),
                "0x5de4111afa1a4b94908f83103eb1f1706367c2e68ca870fc3fb9a804cdab365a".to_string(),
                "0x7c852118294e51e653712a81e05800f419141751be58f605c371e15141b007a6".to_string(),
                "0x47e179ec197488593b187f80a00eb0da91f1b9d0b13f8733639f19c30a34926a".to_string(),
            ],
            // Simple test contract bytecode (will be generated)
            test_contract_bytecode: "0x608060405234801561001057600080fd5b50610150806100206000396000f3fe608060405234801561001057600080fd5b50600436106100365760003560e01c806327e235e31461003b5780636057361d14610057575b600080fd5b610045600181905550565b6040518082815260200191505060405180910390f35b61005f610061565b005b60016000808282540192505081905550565b6000819050919050565b600080fd5b6000610094610089565b9050919050565b6100a481610074565b81146100af57600080fd5b50565b6000813590506100c18161009b565b92915050565b6000602082840312156100dd576100dc61008f565b5b60006100eb848285016100b2565b91505092915050565b7f4e487b7100000000000000000000000000000000000000000000000000000000600052602260045260246000fd5b6000600282049050600182168061013b57607f821691505b60208210810361014e5761014d6100f4565b5b5091905056fea2646970667358221220".to_string(),
        }
    }
}
