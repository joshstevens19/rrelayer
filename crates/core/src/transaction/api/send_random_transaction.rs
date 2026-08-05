use crate::app_state::AppState;
use crate::middleware::policy::PolicyContext;
use crate::network::ChainId;
use crate::relayer::Relayer;
use crate::shared::{bad_request, not_found, HttpError};
use crate::transaction::api::send_transaction::send_transaction;
use crate::transaction::api::{RelayTransactionRequest, SendTransactionResult};
use axum::{
    extract::{Path, State},
    http::HeaderMap,
    Json,
};
use rand::seq::SliceRandom;
use std::sync::Arc;

fn filter_eligible<T, F>(candidates: Vec<T>, mut validate: F) -> Result<Vec<T>, HttpError>
where
    F: FnMut(&T) -> Result<(), HttpError>,
{
    let mut first_error = None;
    let eligible = candidates
        .into_iter()
        .filter_map(|candidate| match validate(&candidate) {
            Ok(()) => Some(candidate),
            Err(error) => {
                if first_error.is_none() {
                    first_error = Some(error);
                }
                None
            }
        })
        .collect::<Vec<_>>();

    if eligible.is_empty() {
        Err(first_error.unwrap_or_else(|| bad_request("No policy candidates".to_string())))
    } else {
        Ok(eligible)
    }
}

fn sort_policy_candidates<T, K, F>(candidates: &mut [T], mut key: F)
where
    K: Ord,
    F: FnMut(&T) -> K,
{
    candidates.sort_by_key(|candidate| key(candidate));
}

/// Handles random relayer selection for transaction requests
/// across multiple relayers on the same chain.
///
/// This endpoint selects a random available (non-paused, non-internal) relayer
/// and forwards the transaction request to it.
pub async fn send_transaction_random(
    State(state): State<Arc<AppState>>,
    Path(chain_id): Path<ChainId>,
    policy_context: PolicyContext,
    headers: HeaderMap,
    Json(transaction): Json<RelayTransactionRequest>,
) -> Result<Json<SendTransactionResult>, HttpError> {
    state.validate_allowed_passed_basic_auth(&headers)?;
    let candidates = select_available_relayers(&state, &chain_id).await?;
    let authorized_relayers = filter_eligible(candidates, |relayer| {
        state.validate_auth_basic_or_api_key(&headers, &relayer.address, &relayer.chain_id)
    })?;
    let eligible_relayers = filter_eligible(authorized_relayers, |relayer| {
        state.validate_request_policy(
            &policy_context,
            &headers,
            &relayer.address,
            &relayer.chain_id,
        )
    })?;
    let relayer = eligible_relayers
        .choose(&mut rand::thread_rng())
        .cloned()
        .ok_or_else(|| bad_request(format!("No available relayers for chain {}", chain_id)))?;
    let result = send_transaction(relayer, transaction, &state, &headers, &policy_context).await?;
    Ok(Json(result))
}

/// Selects a random available relayer for the specified chain.
///
/// Filters out paused, internal-only, and relayers only allowed for random selection.
/// Note: The random relayer feature must be explicitly enabled via `allowed_random_relayers`
/// config for the network, otherwise all relayers will be filtered out.
async fn select_available_relayers(
    state: &Arc<AppState>,
    chain_id: &ChainId,
) -> Result<Vec<Relayer>, HttpError> {
    let relayers = state.db.get_all_relayers_for_chain(chain_id).await?;

    if relayers.is_empty() {
        return Err(not_found(format!("No relayers found for chain {}", chain_id)));
    }

    // TODO: it should be smart enough to also only pick the one with enough native funds to send the tx
    let mut available_relayers: Vec<_> = relayers
        .into_iter()
        .filter(|r| {
            !r.paused
                && !state.relayer_internal_only.restricted(&r.address, &r.chain_id)
                && state.relayers_allowed_for_random.is_allowed(&r.address, &r.chain_id)
        })
        .collect();
    sort_policy_candidates(&mut available_relayers, |relayer| {
        (relayer.wallet_index, relayer.id.to_string())
    });

    if available_relayers.is_empty() {
        return Err(bad_request(format!(
            "No available relayers for chain {} (all relayers are paused, internal-only, or not allowed for random selection)",
            chain_id
        )));
    }

    Ok(available_relayers)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transaction::types::{TransactionHash, TransactionId};
    use axum::{body::to_bytes, http::StatusCode, response::IntoResponse};
    use serde_json::json;
    use std::str::FromStr;

    #[test]
    fn none_eligible_returns_first_error_in_candidate_order() {
        let error = filter_eligible(vec![2, 1], |candidate| {
            if *candidate == 2 {
                Err(crate::shared::unauthorized(Some("candidate-2".to_string())))
            } else {
                Err(crate::shared::forbidden("candidate-1".to_string()))
            }
        })
        .unwrap_err();

        assert_eq!(error.0, StatusCode::UNAUTHORIZED);
        assert_eq!(error.1, "candidate-2");
    }

    #[test]
    fn partial_eligibility_discards_failures_and_keeps_only_passing_candidates() {
        let eligible = filter_eligible(vec![1, 2, 3], |candidate| {
            if *candidate == 2 {
                Ok(())
            } else {
                Err(crate::shared::forbidden(format!("candidate-{candidate}")))
            }
        })
        .unwrap();

        assert_eq!(eligible, vec![2]);
    }

    #[test]
    fn candidates_are_ordered_by_wallet_index_then_relayer_id() {
        let mut candidates = vec![(7, "b"), (2, "z"), (7, "a")];
        sort_policy_candidates(&mut candidates, |candidate| (candidate.0, candidate.1));
        assert_eq!(candidates, vec![(2, "z"), (7, "a"), (7, "b")]);
    }

    #[test]
    fn random_candidate_authentication_precedes_request_policy() {
        let mut evaluations = Vec::new();
        let authorized = filter_eligible(vec![1, 2], |candidate| {
            evaluations.push(format!("auth-{candidate}"));
            if *candidate == 2 {
                Ok(())
            } else {
                Err(crate::shared::unauthorized(None))
            }
        })
        .unwrap();
        let eligible = filter_eligible(authorized, |candidate| {
            evaluations.push(format!("policy-{candidate}"));
            Ok(())
        })
        .unwrap();

        assert_eq!(eligible, vec![2]);
        assert_eq!(evaluations, vec!["auth-1", "auth-2", "policy-2"]);
    }

    #[tokio::test]
    async fn random_submission_returns_http_200_with_explicit_null_hash_when_pending() {
        let id = TransactionId::from_str("11111111-1111-4111-8111-111111111111").unwrap();
        let response = Json(SendTransactionResult { id, hash: None }).into_response();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&body).unwrap(),
            json!({
                "id": "11111111-1111-4111-8111-111111111111",
                "hash": null
            })
        );
    }

    #[tokio::test]
    async fn random_submission_returns_http_200_with_known_hash() {
        let id = TransactionId::from_str("11111111-1111-4111-8111-111111111111").unwrap();
        let hash = TransactionHash::from_str(
            "0x2222222222222222222222222222222222222222222222222222222222222222",
        )
        .unwrap();
        let response = Json(SendTransactionResult { id, hash: Some(hash) }).into_response();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&body).unwrap(),
            json!({
                "id": "11111111-1111-4111-8111-111111111111",
                "hash": "0x2222222222222222222222222222222222222222222222222222222222222222"
            })
        );
    }
}
