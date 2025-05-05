use alloy::{primitives::U256, providers::Provider, sol};
use rrelayerr_core::{common_types::EvmAddress, create_retry_client, relayer::types::RelayerId};
use rrelayerr_sdk::SDK;

use crate::{authentication::handle_authenticate, commands::keystore::ProjectLocation};

pub async fn handle_balance(
    relayer_id: &RelayerId,
    token: &Option<EvmAddress>,
    project_path: &ProjectLocation,
    sdk: &mut SDK,
) -> Result<(), Box<dyn std::error::Error>> {
    handle_authenticate(sdk, "account1", project_path).await?;

    let relayer_result = sdk.relayer.get(relayer_id).await?;
    match relayer_result {
        None => {
            println!("Relayer {} not found", relayer_id);
            Ok(())
        }
        Some(relayer_result) => {
            match &token {
                Some(token_address) => {
                    let provider =
                        create_retry_client(relayer_result.provider_urls.get(0).unwrap())?;

                    let relayer_address = relayer_result.relayer.address.into_address();

                    sol! {
                        #[sol(rpc)]
                        interface IERC20 {
                            function balanceOf(address owner) external view returns (uint256);
                            function decimals() external view returns (uint8);
                            function symbol() external view returns (string);
                        }
                    }

                    let erc20 = IERC20::new(token_address.into_address(), &provider);

                    let balance_result = erc20.balanceOf(relayer_address).call().await;
                    let balance = match balance_result {
                        Ok(result) => result._0,
                        Err(e) => {
                            println!("Failed to get token balance: {}", e);
                            return Err(e.into());
                        }
                    };

                    let decimals = match erc20.decimals().call().await {
                        Ok(result) => result._0,
                        Err(_) => 18,
                    };

                    let token_symbol = match erc20.symbol().call().await {
                        Ok(result) => result._0,
                        Err(_) => "Unknown".to_string(),
                    };

                    let divisor = U256::from(10).pow(U256::from(decimals));
                    let token_value = if balance.is_zero() {
                        "0".to_string()
                    } else {
                        // Format with proper decimals
                        let integer_part = balance / divisor;
                        let fractional_part = balance % divisor;

                        if fractional_part.is_zero() {
                            format!("{}", integer_part)
                        } else {
                            // Format fractional part with proper leading zeros
                            let frac_str =
                                format!("{:0>width$}", fractional_part, width = decimals as usize);
                            // Trim trailing zeros
                            let frac_str = frac_str.trim_end_matches('0');

                            if frac_str.is_empty() {
                                format!("{}", integer_part)
                            } else {
                                format!("{}.{}", integer_part, frac_str)
                            }
                        }
                    };

                    println!(
                        "Relayer {} token balance: {} {}",
                        relayer_id, token_value, token_symbol
                    );

                    Ok(())
                }
                None => {
                    // Native token balance logic remains unchanged
                    let provider =
                        create_retry_client(relayer_result.provider_urls.get(0).unwrap())?;

                    let balance =
                        provider.get_balance(relayer_result.relayer.address.into_address()).await?;

                    let eth_value = if balance.is_zero() {
                        "0".to_string()
                    } else {
                        alloy::primitives::utils::format_ether(balance)
                    };

                    println!("Relayer {} native balance: {} ETH", relayer_id, eth_value);

                    Ok(())
                }
            }
        }
    }
}
