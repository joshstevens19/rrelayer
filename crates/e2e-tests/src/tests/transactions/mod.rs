mod batch;
mod cancel;
mod concurrent;
mod count;
mod gas_bumping;
mod gas_estimation;
mod get;
mod list;
mod nonce_management;
mod replace;
mod send_blob;
mod send_contract_interaction;
mod send_contract_interaction_safe_proxy;
mod send_eth;
mod send_eth_safe_proxy;
mod status;
mod validation;

use crate::tests::registry::{TestDefinition, TestModule};

pub struct TransactionTests;

impl TestModule for TransactionTests {
    fn get_tests() -> Vec<TestDefinition> {
        vec![
            TestDefinition::new("transaction_send_eth", "Send ETH transaction", |runner| {
                Box::pin(runner.transaction_send_eth())
            }),
            TestDefinition::new(
                "transaction_send_eth_safe_proxy",
                "Send ETH transaction via Safe proxy",
                |runner| Box::pin(runner.transaction_send_eth_safe_proxy()),
            ),
            TestDefinition::new(
                "transaction_send_contract_interaction",
                "Send contract interaction transaction",
                |runner| Box::pin(runner.transaction_send_contract_interaction()),
            ),
            TestDefinition::new(
                "transaction_send_contract_interaction_safe_proxy",
                "Send contract interaction via Safe proxy",
                |runner| Box::pin(runner.transaction_send_contract_interaction_safe_proxy()),
            ),
            TestDefinition::new(
                "transaction_send_blob",
                "Send blob transaction (EIP-4844)",
                |runner| Box::pin(runner.transaction_send_blob()),
            ),
            TestDefinition::new("transaction_get", "Get transaction operation", |runner| {
                Box::pin(runner.transaction_get())
            }),
            TestDefinition::new("transaction_list", "List transactions operation", |runner| {
                Box::pin(runner.transaction_list())
            }),
            TestDefinition::new("transaction_replace", "Transaction replace operation", |runner| {
                Box::pin(runner.transaction_replace())
            }),
            TestDefinition::new("transaction_cancel", "Transaction cancel operation", |runner| {
                Box::pin(runner.transaction_cancel())
            }),
            TestDefinition::new("transaction_batch", "Batch transaction processing", |runner| {
                Box::pin(runner.transaction_batch())
            }),
            TestDefinition::new(
                "transaction_status_operations",
                "Transaction status operations",
                |runner| Box::pin(runner.transaction_status_operations()),
            ),
            TestDefinition::new(
                "transaction_status_pending",
                "Transaction pending state validation",
                |runner| Box::pin(runner.transaction_status_pending()),
            ),
            TestDefinition::new(
                "transaction_status_inmempool",
                "Transaction inmempool state validation",
                |runner| Box::pin(runner.transaction_status_inmempool()),
            ),
            TestDefinition::new(
                "transaction_status_mined",
                "Transaction mined state validation",
                |runner| Box::pin(runner.transaction_status_mined()),
            ),
            TestDefinition::new(
                "transaction_status_confirmed",
                "Transaction confirmed state validation",
                |runner| Box::pin(runner.transaction_status_confirmed()),
            ),
            TestDefinition::new(
                "transaction_status_failed",
                "Transaction failed state validation",
                |runner| Box::pin(runner.transaction_status_failed()),
            ),
            TestDefinition::new(
                "transaction_status_expired",
                "Transaction expired state validation",
                |runner| Box::pin(runner.transaction_status_expired()),
            ),
            TestDefinition::new(
                "transaction_inflight_counts",
                "Transaction inflight count operations",
                |runner| Box::pin(runner.transaction_inflight_counts()),
            ),
            TestDefinition::new(
                "transaction_pending_and_inmempool_count",
                "Transaction pending and inmempool count",
                |runner| Box::pin(runner.transaction_pending_and_inmempool_count()),
            ),
            TestDefinition::new(
                "transaction_gas_estimation",
                "Transaction gas estimation",
                |runner| Box::pin(runner.transaction_gas_estimation()),
            ),
            TestDefinition::new(
                "transaction_gas_price_bumping",
                "Transaction gas price bumping",
                |runner| Box::pin(runner.transaction_gas_price_bumping()),
            ),
            TestDefinition::new(
                "transaction_nonce_management",
                "Transaction nonce management",
                |runner| Box::pin(runner.transaction_nonce_management()),
            ),
            TestDefinition::new(
                "transaction_concurrent",
                "Concurrent transaction handling",
                |runner| Box::pin(runner.transaction_concurrent()),
            ),
            TestDefinition::new(
                "transaction_data_validation",
                "Transaction data validation",
                |runner| Box::pin(runner.transaction_data_validation()),
            ),
            TestDefinition::new(
                "transaction_validation_not_enough_funds",
                "Transaction validation - insufficient funds",
                |runner| Box::pin(runner.transaction_validation_not_enough_funds()),
            ),
            TestDefinition::new(
                "transaction_validation_revert_execution",
                "Transaction validation - contract revert",
                |runner| Box::pin(runner.transaction_validation_revert_execution()),
            ),
            TestDefinition::new(
                "transaction_validation_balance_edge_cases",
                "Transaction validation - balance edge cases",
                |runner| Box::pin(runner.transaction_validation_balance_edge_cases()),
            ),
        ]
    }
}
