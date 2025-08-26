use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use tokio::{sync::Mutex, time::sleep};

use crate::{network::types::Network, relayer::types::Relayer, transaction::types::Transaction};

#[derive(Clone)]
pub enum CacheValue {
    Networks(Vec<Network>),
    Relayer(Option<Relayer>),
    IsRelayerApiKey(bool),
    Transaction(Option<Transaction>),
    AuthenticationChallenge(String),
}

impl CacheValue {
    fn name(&self) -> &'static str {
        match self {
            CacheValue::Networks(_) => "Networks",
            CacheValue::Relayer(_) => "Relayer",
            CacheValue::IsRelayerApiKey(_) => "IsRelayerApiKey",
            CacheValue::Transaction(_) => "Transaction",
            CacheValue::AuthenticationChallenge(_) => "AuthenticationChallenge",
        }
    }

    /// Extracts networks from a Networks cache value.
    ///
    /// # Returns
    /// * `Vec<Network>` - The cached network list
    ///
    /// # Panics
    /// * Panics if this cache value is not of type Networks
    pub fn to_networks(&self) -> Vec<Network> {
        match self {
            CacheValue::Networks(networks) => networks.clone(),
            _ => panic!("CacheValue name '{}' not supported on to_networks", self.name()),
        }
    }

    /// Extracts a relayer from a Relayer cache value.
    ///
    /// # Returns
    /// * `Option<Relayer>` - The cached relayer, if present
    ///
    /// # Panics
    /// * Panics if this cache value is not of type Relayer
    pub fn to_relayer(&self) -> Option<Relayer> {
        match self {
            CacheValue::Relayer(relayer) => relayer.clone(),
            _ => panic!("CacheValue name '{}' not supported on to_relayer", self.name()),
        }
    }

    /// Extracts a boolean from an IsRelayerApiKey cache value.
    ///
    /// # Returns
    /// * `bool` - Whether the API key is a relayer API key
    ///
    /// # Panics
    /// * Panics if this cache value is not of type IsRelayerApiKey
    pub fn to_is_relayer_api_key(&self) -> bool {
        match self {
            CacheValue::IsRelayerApiKey(result) => *result,
            _ => panic!("CacheValue name '{}' not supported on to_is_relayer_api_key", self.name()),
        }
    }

    /// Extracts a transaction from a Transaction cache value.
    ///
    /// # Returns
    /// * `Option<Transaction>` - The cached transaction, if present
    ///
    /// # Panics
    /// * Panics if this cache value is not of type Transaction
    pub fn to_transaction(&self) -> Option<Transaction> {
        match self {
            CacheValue::Transaction(transaction) => transaction.clone(),
            _ => panic!("CacheValue name '{}' not supported on to_transaction", self.name()),
        }
    }

    /// Extracts an authentication challenge from an AuthenticationChallenge cache value.
    ///
    /// # Returns
    /// * `String` - The cached authentication challenge
    ///
    /// # Panics
    /// * Panics if this cache value is not of type AuthenticationChallenge
    pub fn to_authentication_challenge(&self) -> String {
        match self {
            CacheValue::AuthenticationChallenge(challenge) => challenge.clone(),
            _ => panic!(
                "CacheValue name '{}' not supported on to_authentication_challenge",
                self.name()
            ),
        }
    }
}

struct CacheEntry {
    value: CacheValue,
    expiration_time: Instant,
}

pub struct Cache {
    cache: Arc<Mutex<HashMap<String, CacheEntry>>>,
}

impl Cache {
    /// Creates a new cache instance with automatic expiration cleanup.
    ///
    /// The cache automatically removes expired entries every 30 seconds.
    ///
    /// # Returns
    /// * `Self` - A new cache instance
    pub async fn new() -> Self {
        let cache = Cache { cache: Arc::new(Mutex::new(HashMap::new())) };

        // Discard expired entries every 30 seconds
        cache.start_expiration_thread(Duration::from_secs(30)).await;

        cache
    }

    /// Inserts a value into the cache with the default expiration time (10 minutes).
    ///
    /// # Arguments
    /// * `key` - The cache key
    /// * `value` - The value to cache
    pub async fn insert(&self, key: String, value: CacheValue) {
        self.insert_with_expiry(key, value, Duration::from_secs(60 * 10)).await
    }

    /// Inserts a value into the cache with a custom expiration duration.
    ///
    /// # Arguments
    /// * `key` - The cache key
    /// * `value` - The value to cache
    /// * `expiration_duration` - How long the value should be cached
    pub async fn insert_with_expiry(
        &self,
        key: String,
        value: CacheValue,
        expiration_duration: Duration,
    ) {
        let expiration_time = Instant::now() + expiration_duration;
        let entry = CacheEntry { value, expiration_time };

        self.cache.lock().await.insert(key, entry);
    }

    /// Retrieves a value from the cache.
    ///
    /// Returns None if the key doesn't exist or the value has expired.
    ///
    /// # Arguments
    /// * `key` - The cache key to look up
    ///
    /// # Returns
    /// * `Option<CacheValue>` - The cached value if found and not expired
    pub async fn get(&self, key: &str) -> Option<CacheValue> {
        let cache = self.cache.lock().await;
        if let Some(entry) = cache.get(key) {
            if entry.expiration_time > Instant::now() {
                return Some(entry.value.clone());
            }
        }
        None
    }

    /// Removes a value from the cache and returns it.
    ///
    /// # Arguments
    /// * `key` - The cache key to remove
    ///
    /// # Returns
    /// * `Option<CacheValue>` - The removed value if it existed
    pub async fn delete(&self, key: &str) -> Option<CacheValue> {
        self.cache.lock().await.remove(key).map(|entry| entry.value)
    }

    async fn start_expiration_thread(&self, cleanup_interval: Duration) {
        let cache_ref = Arc::clone(&self.cache);
        tokio::spawn(async move {
            loop {
                sleep(cleanup_interval).await; // Use tokio's async sleep function
                let mut cache = cache_ref.lock().await; // Acquire the lock asynchronously
                cache.retain(|_, entry| entry.expiration_time > Instant::now());
            }
        });
    }
}
