//! Per-endpoint health tracking and health-aware selection for a network's RPC urls.
//!
//! Every url gets an [`EndpointClient`] carrying an [`EndpointHealth`] fed from two
//! sides: the RPC logging layer records every call's outcome/latency, and a
//! background prober tracks each endpoint's tip so a fast-but-stale endpoint is
//! detected too. Selection ([`EndpointSelector`]) skips endpoints in cooldown or
//! lagging and weights the rest by health - falling back to the full enabled set
//! when everything looks unhealthy, so a caller is never left without a client.

use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use rand::{thread_rng, Rng};
use tracing::{error, info, warn};

use super::evm_provider::RelayerProvider;
use crate::network::ChainId;

/// Consecutive call failures before an endpoint is held out of rotation.
const FAILURES_BEFORE_COOLDOWN: u32 = 3;
/// How long a failing endpoint stays out of rotation before it may be retried.
const FAILURE_COOLDOWN: Duration = Duration::from_secs(30);
/// Blocks behind the best-known tip before an endpoint counts as lagging.
const MAX_ENDPOINT_LAG_BLOCKS: u64 = 20;
/// Cadence of the background tip probe (which doubles as the half-open recovery
/// probe: a successful probe on a cooled-down endpoint puts it back in rotation).
const ENDPOINT_PROBE_INTERVAL: Duration = Duration::from_secs(10);
/// Upper bound on a single probe call so one dead endpoint cannot stall a cycle.
const ENDPOINT_PROBE_TIMEOUT: Duration = Duration::from_secs(10);
/// Smoothing factor for the error-rate/latency EWMAs.
const EWMA_ALPHA: f64 = 0.2;
/// Rolling window of successful-call latencies kept for percentile snapshots.
const LATENCY_WINDOW: usize = 128;
/// Floor selection weight so an endpoint with a bad-but-recovering record is not
/// starved of the occasional live request.
const MIN_SELECTION_WEIGHT: f64 = 0.05;

pub(crate) fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64
}

#[derive(Debug, Default)]
struct HealthStats {
    error_ewma: f64,
    latency_ewma_ms: f64,
    has_latency_sample: bool,
    latency_window_ms: VecDeque<u64>,
}

/// Live health state for one RPC endpoint, shared between the logging layer (per-call
/// outcomes), the background prober (tip/lag + recovery) and selection.
#[derive(Debug)]
pub struct EndpointHealth {
    stats: Mutex<HealthStats>,
    consecutive_failures: AtomicU32,
    /// Unix millis until which the failure cooldown holds; 0 = no cooldown.
    cooldown_until_ms: AtomicU64,
    /// Lag-based unhealthiness - owned exclusively by the prober so a call success
    /// on a stale endpoint cannot clear it.
    lagging: AtomicBool,
    /// True once the endpoint answered `eth_chainId` with the configured chain.
    /// An endpoint unreachable at boot stays unverified (and unselectable) until
    /// the prober verifies it.
    chain_verified: AtomicBool,
    /// Permanently out of rotation: the endpoint recovered serving the WRONG chain.
    disabled: AtomicBool,
    /// Last tip observed by the prober; 0 = never observed.
    last_block: AtomicU64,
    /// Blocks behind the best tip at the last probe; u64::MAX = unknown.
    lag_blocks: AtomicU64,
}

impl EndpointHealth {
    pub(crate) fn new() -> Self {
        Self {
            stats: Mutex::new(HealthStats::default()),
            consecutive_failures: AtomicU32::new(0),
            cooldown_until_ms: AtomicU64::new(0),
            lagging: AtomicBool::new(false),
            chain_verified: AtomicBool::new(false),
            disabled: AtomicBool::new(false),
            last_block: AtomicU64::new(0),
            lag_blocks: AtomicU64::new(u64::MAX),
        }
    }

    pub(crate) fn record_success(&self, latency: Duration) {
        self.consecutive_failures.store(0, Ordering::Relaxed);
        // A real success ends the failure cooldown (half-open recovery); the lag
        // flag is deliberately untouched - only the prober may clear staleness
        self.cooldown_until_ms.store(0, Ordering::Relaxed);

        if let Ok(mut stats) = self.stats.lock() {
            stats.error_ewma *= 1.0 - EWMA_ALPHA;

            let latency_ms = latency.as_millis() as u64;
            stats.latency_ewma_ms = if stats.has_latency_sample {
                (1.0 - EWMA_ALPHA) * stats.latency_ewma_ms + EWMA_ALPHA * latency_ms as f64
            } else {
                latency_ms as f64
            };
            stats.has_latency_sample = true;

            stats.latency_window_ms.push_back(latency_ms);
            if stats.latency_window_ms.len() > LATENCY_WINDOW {
                stats.latency_window_ms.pop_front();
            }
        }
    }

    pub(crate) fn record_failure(&self, now: u64) {
        let failures = self.consecutive_failures.fetch_add(1, Ordering::Relaxed) + 1;
        if failures >= FAILURES_BEFORE_COOLDOWN {
            self.cooldown_until_ms
                .store(now + FAILURE_COOLDOWN.as_millis() as u64, Ordering::Relaxed);
        }

        if let Ok(mut stats) = self.stats.lock() {
            stats.error_ewma = (1.0 - EWMA_ALPHA) * stats.error_ewma + EWMA_ALPHA;
        }
    }

    /// Prober-only: record this endpoint's tip against the best tip seen in the same
    /// cycle. Marks (and clears) the lag flag; returns whether the endpoint lags.
    pub(crate) fn record_probe(&self, own_tip: u64, best_tip: u64) -> bool {
        self.last_block.store(own_tip, Ordering::Relaxed);
        let lag = best_tip.saturating_sub(own_tip);
        self.lag_blocks.store(lag, Ordering::Relaxed);

        let lagging = lag > MAX_ENDPOINT_LAG_BLOCKS;
        self.lagging.store(lagging, Ordering::Relaxed);
        lagging
    }

    pub(crate) fn mark_chain_verified(&self) {
        self.chain_verified.store(true, Ordering::Relaxed);
    }

    /// Permanently removes the endpoint from rotation - it answered with the wrong
    /// chain id after boot. Never cleared: a url that flip-flops between chains can
    /// not be trusted with traffic.
    pub(crate) fn disable_wrong_chain(&self) {
        self.disabled.store(true, Ordering::Relaxed);
    }

    /// Whether this endpoint may take traffic at all (right chain, not disabled).
    pub(crate) fn is_enabled(&self) -> bool {
        self.chain_verified.load(Ordering::Relaxed) && !self.disabled.load(Ordering::Relaxed)
    }

    /// Whether this endpoint should be preferred for new requests right now.
    pub(crate) fn is_selectable(&self, now: u64) -> bool {
        self.is_enabled()
            && !self.lagging.load(Ordering::Relaxed)
            && self.cooldown_until_ms.load(Ordering::Relaxed) <= now
    }

    /// Health-derived selection weight: mostly the success rate, discounted by
    /// latency so a slow endpoint is deprioritized without being excluded.
    pub(crate) fn selection_weight(&self) -> f64 {
        let Ok(stats) = self.stats.lock() else {
            return MIN_SELECTION_WEIGHT;
        };

        let success_rate = (1.0 - stats.error_ewma).max(MIN_SELECTION_WEIGHT);
        success_rate / (1.0 + stats.latency_ewma_ms / 1_000.0)
    }
}

/// One RPC url with its client stack and live health state.
#[derive(Clone)]
pub(crate) struct EndpointClient {
    pub(crate) client: Arc<RelayerProvider>,
    pub(crate) url: String,
    pub(crate) health: Arc<EndpointHealth>,
}

/// Health-aware selection over a network's RPC endpoints. Cheap to clone - all
/// holders (the provider, the gas estimator, the prober) share the same health
/// state, so a failure seen anywhere steers every consumer.
#[derive(Clone)]
pub struct EndpointSelector {
    endpoints: Arc<Vec<EndpointClient>>,
}

impl EndpointSelector {
    pub(crate) fn from_endpoints(endpoints: Vec<EndpointClient>) -> Self {
        Self { endpoints: Arc::new(endpoints) }
    }

    pub(crate) fn endpoints(&self) -> &[EndpointClient] {
        &self.endpoints
    }

    /// Picks a client for the next request: healthy endpoints weighted by their
    /// record, cooled-down/lagging ones skipped, with the guarantee that SOME
    /// client is always returned while at least one endpoint exists.
    pub fn client(&self) -> Arc<RelayerProvider> {
        self.pick().1
    }

    pub(crate) fn pick(&self) -> (usize, Arc<RelayerProvider>) {
        let index = self.pick_index(thread_rng().gen::<f64>());
        (index, self.endpoints[index].client.clone())
    }

    fn pick_index(&self, roll: f64) -> usize {
        let now = now_ms();
        let candidates: Vec<EndpointCandidate> = self
            .endpoints
            .iter()
            .map(|endpoint| EndpointCandidate {
                selectable: endpoint.health.is_selectable(now),
                enabled: endpoint.health.is_enabled(),
                weight: endpoint.health.selection_weight(),
            })
            .collect();

        pick_endpoint(&candidates, roll)
    }
}

/// Selection view of one endpoint.
#[derive(Debug, Clone, Copy)]
pub(crate) struct EndpointCandidate {
    /// Healthy right now (enabled, not lagging, not in cooldown).
    pub(crate) selectable: bool,
    /// Allowed to take traffic at all (verified for the right chain, not disabled).
    pub(crate) enabled: bool,
    /// Health-derived selection weight.
    pub(crate) weight: f64,
}

/// Weighted pick with graceful degradation: healthy endpoints first; when none are
/// healthy fall back to every enabled endpoint (a fully unhealthy pool must still
/// hand out a client rather than fail closed); only when nothing is enabled - which
/// boot policy prevents - fall back to the raw set. Panics on an empty candidate
/// list, exactly like the previous random pick did.
pub(crate) fn pick_endpoint(candidates: &[EndpointCandidate], roll: f64) -> usize {
    let indices_where = |predicate: fn(&EndpointCandidate) -> bool| -> Vec<usize> {
        candidates
            .iter()
            .enumerate()
            .filter(|(_, candidate)| predicate(candidate))
            .map(|(index, _)| index)
            .collect()
    };

    let mut pool = indices_where(|candidate| candidate.selectable);
    if pool.is_empty() {
        pool = indices_where(|candidate| candidate.enabled);
    }
    if pool.is_empty() {
        pool = (0..candidates.len()).collect();
    }

    let total: f64 =
        pool.iter().map(|&index| candidates[index].weight.max(MIN_SELECTION_WEIGHT)).sum();
    let mut target = roll.clamp(0.0, 0.999_999) * total;
    for &index in &pool {
        let weight = candidates[index].weight.max(MIN_SELECTION_WEIGHT);
        if target < weight {
            return index;
        }
        target -= weight;
    }

    pool[pool.len() - 1]
}

/// Spawns the background prober for a network's endpoints: every ~10s each endpoint
/// reports its tip; lag beyond [`MAX_ENDPOINT_LAG_BLOCKS`] behind the best tip marks
/// it lagging (and back within it, recovered). The probe traffic flows through the
/// same logging layer as live traffic, so a successful probe on a cooled-down
/// endpoint doubles as the half-open recovery signal. Endpoints that were
/// unreachable at boot get their chain id verified here before they may ever take
/// traffic; one that recovers serving the wrong chain is disabled permanently.
pub(crate) fn spawn_endpoint_prober(
    selector: EndpointSelector,
    network: String,
    expected_chain_id: ChainId,
) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(ENDPOINT_PROBE_INTERVAL);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            ticker.tick().await;
            probe_endpoints_once(&selector, &network, expected_chain_id).await;
        }
    });
}

async fn probe_endpoints_once(
    selector: &EndpointSelector,
    network: &str,
    expected_chain_id: ChainId,
) {
    let mut tips: Vec<Option<u64>> = Vec::with_capacity(selector.endpoints().len());

    for endpoint in selector.endpoints() {
        if endpoint.health.disabled.load(Ordering::Relaxed) {
            tips.push(None);
            continue;
        }

        // Late chain verification for endpoints that were unreachable at boot -
        // they must never take traffic before proving they serve the right chain
        if !endpoint.health.chain_verified.load(Ordering::Relaxed) {
            match tokio::time::timeout(ENDPOINT_PROBE_TIMEOUT, endpoint.client.get_chain_id()).await
            {
                Ok(Ok(got)) if got == expected_chain_id.u64() => {
                    info!(
                        "Provider url {} for network {} recovered and verified for chain {} - entering rotation",
                        endpoint.url, network, got
                    );
                    endpoint.health.mark_chain_verified();
                }
                Ok(Ok(got)) => {
                    error!(
                        "Provider url {} for network {} recovered serving chain id {} but the config expects {} - permanently disabling it",
                        endpoint.url,
                        network,
                        got,
                        expected_chain_id.u64()
                    );
                    endpoint.health.disable_wrong_chain();
                    tips.push(None);
                    continue;
                }
                _ => {
                    // Still unreachable; unverified endpoints stay unselectable
                    tips.push(None);
                    continue;
                }
            }
        }

        let tip =
            match tokio::time::timeout(ENDPOINT_PROBE_TIMEOUT, endpoint.client.get_block_number())
                .await
            {
                Ok(Ok(block_number)) => Some(block_number),
                // Call failures already fed the health state through the logging layer
                _ => None,
            };
        tips.push(tip);
    }

    let Some(best_tip) = tips.iter().flatten().copied().max() else {
        return;
    };

    for (endpoint, tip) in selector.endpoints().iter().zip(tips) {
        let Some(tip) = tip else { continue };

        let was_lagging = endpoint.health.lagging.load(Ordering::Relaxed);
        let lagging = endpoint.health.record_probe(tip, best_tip);

        if lagging && !was_lagging {
            warn!(
                "Provider url {} for network {} lags the best tip by {} blocks (own {}, best {}) - removing it from rotation until it catches up",
                endpoint.url,
                network,
                best_tip.saturating_sub(tip),
                tip,
                best_tip
            );
        } else if !lagging && was_lagging {
            info!(
                "Provider url {} for network {} caught back up (own {}, best {}) - returning it to rotation",
                endpoint.url, network, tip, best_tip
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(selectable: bool, enabled: bool, weight: f64) -> EndpointCandidate {
        EndpointCandidate { selectable, enabled, weight }
    }

    #[test]
    fn cooled_down_endpoints_are_skipped_by_selection() {
        let candidates = vec![candidate(false, true, 1.0), candidate(true, true, 1.0)];

        for roll in [0.0, 0.25, 0.5, 0.99] {
            assert_eq!(pick_endpoint(&candidates, roll), 1);
        }
    }

    #[test]
    fn selection_falls_back_to_the_full_enabled_set_when_all_are_unhealthy() {
        // Never "no endpoint": everything cooled down still yields a client
        let candidates = vec![candidate(false, true, 1.0), candidate(false, true, 1.0)];

        assert_eq!(pick_endpoint(&candidates, 0.1), 0);
        assert_eq!(pick_endpoint(&candidates, 0.9), 1);
    }

    #[test]
    fn disabled_endpoints_are_excluded_even_from_the_unhealthy_fallback() {
        // Endpoint 0 is wrong-chain-disabled; even with endpoint 1 cooled down the
        // fallback must not touch a wrong-chain endpoint
        let candidates = vec![candidate(false, false, 1.0), candidate(false, true, 1.0)];

        for roll in [0.0, 0.5, 0.99] {
            assert_eq!(pick_endpoint(&candidates, roll), 1);
        }
    }

    #[test]
    fn selection_weights_bias_toward_healthier_endpoints() {
        let candidates = vec![candidate(true, true, 1.0), candidate(true, true, 3.0)];

        // Cumulative weights: [0,1) -> endpoint 0, [1,4) -> endpoint 1
        assert_eq!(pick_endpoint(&candidates, 0.2), 0); // 0.2 * 4 = 0.8
        assert_eq!(pick_endpoint(&candidates, 0.3), 1); // 0.3 * 4 = 1.2
        assert_eq!(pick_endpoint(&candidates, 0.9), 1);
    }

    #[test]
    fn consecutive_failures_trigger_cooldown_and_success_recovers() {
        let health = EndpointHealth::new();
        health.mark_chain_verified();
        let now = 1_000_000;

        health.record_failure(now);
        health.record_failure(now);
        assert!(health.is_selectable(now), "two failures must not cool down yet");

        health.record_failure(now);
        assert!(!health.is_selectable(now), "the third failure enters cooldown");

        // Half-open by cooldown expiry alone
        let after_cooldown = now + FAILURE_COOLDOWN.as_millis() as u64 + 1;
        assert!(health.is_selectable(after_cooldown));

        // And a success (e.g. the recovery probe) clears it immediately
        health.record_failure(now);
        health.record_failure(now);
        health.record_failure(now);
        assert!(!health.is_selectable(now));
        health.record_success(Duration::from_millis(20));
        assert!(health.is_selectable(now));
        assert_eq!(health.consecutive_failures.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn prober_lag_marks_an_endpoint_out_and_catching_up_recovers_it() {
        let health = EndpointHealth::new();
        health.mark_chain_verified();
        let now = 1_000_000;

        let lagging = health.record_probe(80, 80 + MAX_ENDPOINT_LAG_BLOCKS + 1);
        assert!(lagging);
        assert!(!health.is_selectable(now));
        assert_eq!(health.last_block.load(Ordering::Relaxed), 80);
        assert_eq!(health.lag_blocks.load(Ordering::Relaxed), MAX_ENDPOINT_LAG_BLOCKS + 1);

        // A plain call success must NOT clear staleness - only the prober may
        health.record_success(Duration::from_millis(5));
        assert!(!health.is_selectable(now));

        let lagging = health.record_probe(120, 122);
        assert!(!lagging);
        assert!(health.is_selectable(now));
        assert_eq!(health.lag_blocks.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn unverified_and_disabled_endpoints_are_never_selectable() {
        let unverified = EndpointHealth::new();
        assert!(!unverified.is_selectable(0), "unverified chains cannot take traffic");

        let disabled = EndpointHealth::new();
        disabled.mark_chain_verified();
        assert!(disabled.is_selectable(0));
        disabled.disable_wrong_chain();
        assert!(!disabled.is_selectable(0));
        assert!(!disabled.is_enabled());
    }
}
