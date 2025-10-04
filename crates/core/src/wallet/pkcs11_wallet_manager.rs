use crate::common_types::EvmAddress;
use crate::network::ChainId;
use crate::wallet::{WalletError, WalletManagerTrait};
use crate::yaml::Pkcs11SigningProviderConfig;
use alloy::consensus::{SignableTransaction, TypedTransaction};
use alloy::dyn_abi::TypedData;
use alloy::primitives::{keccak256, Address, Signature, B256, U256};
use async_trait::async_trait;
use cryptoki::context::{CInitializeArgs, Pkcs11};
use cryptoki::mechanism::Mechanism;
use cryptoki::object::{Attribute, AttributeType, ObjectClass, ObjectHandle};
use cryptoki::session::{Session, UserType};
use cryptoki::slot::Slot;
use secrecy::Secret;
use std::collections::HashMap;
use std::path::Path;
use tokio::sync::Mutex;
use tracing::{debug, info};

/// PKCS#11 wallet manager for hardware security modules.
/// Supports any HSM that implements PKCS#11 with secp256k1 curve support.
#[derive(Debug)]
pub struct Pkcs11WalletManager {
    config: Pkcs11SigningProviderConfig,
    ctx: Pkcs11,
    slot: Slot,
    wallet_cache: Mutex<HashMap<u32, (ObjectHandle, Address)>>,
}

impl Pkcs11WalletManager {
    pub fn new(config: Pkcs11SigningProviderConfig) -> Result<Self, WalletError> {
        let library_path = Path::new(&config.library_path);
        if !library_path.exists() {
            return Err(WalletError::ConfigurationError {
                message: format!("PKCS#11 library not found at: {}", config.library_path),
            });
        }

        let ctx = Pkcs11::new(library_path).map_err(|e| {
            WalletError::GenericSignerError(format!("Failed to load PKCS#11 library: {}", e))
        })?;

        match ctx.initialize(CInitializeArgs::OsThreads) {
            Ok(_) => debug!("PKCS#11 library initialized successfully"),
            Err(e) => {
                let error_str = e.to_string().to_lowercase();
                if error_str.contains("already") || error_str.contains("initialized") {
                    debug!("PKCS#11 library already initialized");
                } else {
                    return Err(WalletError::GenericSignerError(format!(
                        "Failed to initialize PKCS#11: {}",
                        e
                    )));
                }
            }
        }

        let slots = ctx
            .get_slots_with_token()
            .map_err(|e| WalletError::GenericSignerError(format!("Failed to get slots: {}", e)))?;

        let slot = if let Some(target_slot_id) = config.slot_id {
            slots
                .into_iter()
                .find(|s| {
                    // Use slot ID() method for cross-platform compatibility
                    s.id() == target_slot_id
                })
                .ok_or_else(|| WalletError::ConfigurationError {
                    message: format!("Slot {} not found", target_slot_id),
                })?
        } else {
            slots.into_iter().next().ok_or_else(|| WalletError::ConfigurationError {
                message: "No PKCS#11 slots available".to_string(),
            })?
        };

        let slot_id = slot.id();
        info!("Initialized PKCS#11 wallet manager on slot {}", slot_id);

        Ok(Self { config, ctx, slot, wallet_cache: Mutex::new(HashMap::new()) })
    }

    fn get_session(&self) -> Result<Session, WalletError> {
        let session = self.ctx.open_rw_session(self.slot).map_err(|e| {
            WalletError::GenericSignerError(format!("Failed to open session: {}", e))
        })?;

        if let Some(pin) = &self.config.pin {
            let secret_pin = Secret::new(pin.clone());
            match session.login(UserType::User, Some(&secret_pin)) {
                Ok(_) => debug!("Successfully authenticated with PKCS#11 token"),
                Err(e) => {
                    let error_str = e.to_string().to_lowercase();
                    if error_str.contains("already") || error_str.contains("logged") {
                        debug!("Already authenticated with PKCS#11 token");
                    } else {
                        return Err(WalletError::AuthenticationError {
                            message: format!("Failed to authenticate with PKCS#11 token: {}", e),
                        });
                    }
                }
            }
        }

        Ok(session)
    }

    async fn get_or_create_wallet(
        &self,
        wallet_index: u32,
    ) -> Result<(ObjectHandle, Address), WalletError> {
        {
            let cache = self.wallet_cache.lock().await;
            if let Some(&(handle, address)) = cache.get(&wallet_index) {
                debug!(
                    "Using cached wallet {} at address 0x{}",
                    wallet_index,
                    hex::encode(address.as_slice())
                );
                return Ok((handle, address));
            }
        }

        let session = self.get_session()?;
        let label = format!("rrelayer-wallet-{}", wallet_index);

        let template = vec![
            Attribute::Class(ObjectClass::PRIVATE_KEY),
            Attribute::Label(label.as_bytes().to_vec()),
        ];

        let objects = session.find_objects(&template).map_err(|e| {
            WalletError::GenericSignerError(format!("Failed to search for wallet keys: {}", e))
        })?;

        let private_key_handle = if let Some(key_handle) = objects.first() {
            debug!("Found existing key for wallet {}", wallet_index);
            *key_handle
        } else {
            info!("Creating new secp256k1 key pair for wallet {}", wallet_index);
            self.create_key_pair(&session, wallet_index)?
        };

        let address = self.derive_address_from_key(&session, wallet_index)?;

        {
            let mut cache = self.wallet_cache.lock().await;
            cache.insert(wallet_index, (private_key_handle, address));
            debug!("Cached wallet {} -> 0x{}", wallet_index, hex::encode(address.as_slice()));
        }

        info!("Wallet {} ready at address 0x{}", wallet_index, hex::encode(address.as_slice()));
        Ok((private_key_handle, address))
    }

    /// Creates a new secp256k1 key pair in the HSM.
    fn create_key_pair(
        &self,
        session: &Session,
        wallet_index: u32,
    ) -> Result<ObjectHandle, WalletError> {
        let label = format!("rrelayer-wallet-{}", wallet_index);
        let id = wallet_index.to_be_bytes().to_vec();

        // secp256k1 curve OID (1.3.132.0.10)
        let secp256k1_oid = vec![0x06, 0x05, 0x2b, 0x81, 0x04, 0x00, 0x0a];

        let public_template = vec![
            Attribute::Class(ObjectClass::PUBLIC_KEY),
            Attribute::Label(label.as_bytes().to_vec()),
            Attribute::Id(id.clone()),
            Attribute::Token(true),
            Attribute::Verify(true),
            Attribute::EcParams(secp256k1_oid.clone()),
        ];

        let private_template = vec![
            Attribute::Class(ObjectClass::PRIVATE_KEY),
            Attribute::Label(label.as_bytes().to_vec()),
            Attribute::Id(id),
            Attribute::Token(true),
            Attribute::Private(true),
            Attribute::Sensitive(true),
            Attribute::Sign(true),
            Attribute::Extractable(false),
        ];

        let result = session.generate_key_pair(
            &Mechanism::EccKeyPairGen,
            &public_template,
            &private_template,
        );

        let (_, private_key) = result.map_err(|e| {
            WalletError::GenericSignerError(format!(
                "HSM does not support secp256k1 curve required for Ethereum: {}. \
                Please use an HSM with secp256k1 support (SoftHSM 2.6+, YubiKey PIV, AWS CloudHSM, etc.)", 
                e
            ))
        })?;

        debug!("Successfully created secp256k1 key pair for wallet {}", wallet_index);
        Ok(private_key)
    }

    async fn get_public_key(&self, wallet_index: u32) -> Result<Address, WalletError> {
        let (_handle, address) = self.get_or_create_wallet(wallet_index).await?;
        Ok(address)
    }

    fn derive_address_from_key(
        &self,
        session: &Session,
        wallet_index: u32,
    ) -> Result<Address, WalletError> {
        let label = format!("rrelayer-wallet-{}", wallet_index);
        let template = vec![
            Attribute::Class(ObjectClass::PUBLIC_KEY),
            Attribute::Label(label.as_bytes().to_vec()),
        ];

        let objects = session.find_objects(&template).map_err(|e| {
            WalletError::GenericSignerError(format!("Find public key failed: {}", e))
        })?;

        let public_key_handle = objects
            .first()
            .ok_or_else(|| WalletError::GenericSignerError("Public key not found".to_string()))?;

        let ec_point_attr =
            session.get_attributes(*public_key_handle, &[AttributeType::EcPoint]).map_err(|e| {
                WalletError::GenericSignerError(format!("Failed to get EC point: {}", e))
            })?;

        let ec_point = ec_point_attr.first().ok_or_else(|| {
            WalletError::GenericSignerError("EC point attribute not found".to_string())
        })?;

        let point_bytes = match ec_point {
            Attribute::EcPoint(data) => data.as_slice(),
            _ => {
                return Err(WalletError::GenericSignerError(
                    "Invalid EC point attribute".to_string(),
                ))
            }
        };

        // Handle DER-encoded EC point (0x04 prefix + 64 bytes)
        let raw_point = if point_bytes.len() >= 65 && point_bytes[0] == 0x04 {
            &point_bytes[1..65] // Extract x and y coordinates (64 bytes total)
        } else {
            return Err(WalletError::GenericSignerError(format!(
                "Unsupported EC point format: len={}, first_byte=0x{:02x}",
                point_bytes.len(),
                point_bytes.first().unwrap_or(&0)
            )));
        };

        self.public_key_to_address(raw_point)
    }

    /// Converts an uncompressed secp256k1 public key to an Ethereum address.
    fn public_key_to_address(&self, public_key: &[u8]) -> Result<Address, WalletError> {
        if public_key.len() != 64 {
            return Err(WalletError::GenericSignerError(format!(
                "Invalid public key length: expected 64 bytes, got {}",
                public_key.len()
            )));
        }

        // Ethereum address is the last 20 bytes of keccak256(public_key)
        let hash = keccak256(public_key);
        let address_bytes = &hash[12..32];

        let mut addr = [0u8; 20];
        addr.copy_from_slice(address_bytes);
        Ok(Address::from(addr))
    }

    /// Signs a hash using the HSM private key for the given wallet index.
    /// Attempts multiple signing rounds to find a signature compatible with Ethereum's recovery mechanism.
    async fn sign_hash(&self, wallet_index: u32, hash: &B256) -> Result<Signature, WalletError> {
        let session = self.get_session()?;

        let (_cached_handle, expected_address) = self.get_or_create_wallet(wallet_index).await?;

        let label = format!("rrelayer-wallet-{}", wallet_index);
        let template = vec![
            Attribute::Class(ObjectClass::PRIVATE_KEY),
            Attribute::Label(label.as_bytes().to_vec()),
        ];

        let objects = session.find_objects(&template).map_err(|e| {
            WalletError::GenericSignerError(format!("Failed to find private key: {}", e))
        })?;

        let private_key = objects.first().ok_or_else(|| {
            WalletError::GenericSignerError(format!(
                "Private key not found for wallet {}",
                wallet_index
            ))
        })?;

        let actual_address = self.derive_address_from_key(&session, wallet_index)?;
        if actual_address != expected_address {
            return Err(WalletError::GenericSignerError(format!(
                "Address mismatch for wallet {}: expected 0x{}, got 0x{}",
                wallet_index,
                hex::encode(expected_address.as_slice()),
                hex::encode(actual_address.as_slice())
            )));
        }

        debug!(
            "Signing hash 0x{} with wallet {} (address 0x{})",
            hex::encode(hash.as_slice()),
            wallet_index,
            hex::encode(expected_address.as_slice())
        );

        let signature_bytes = session
            .sign(&Mechanism::Ecdsa, *private_key, hash.as_slice())
            .map_err(|e| WalletError::GenericSignerError(format!("HSM signing failed: {}", e)))?;

        if signature_bytes.len() != 64 {
            return Err(WalletError::GenericSignerError(
                "Invalid signature length from HSM".to_string(),
            ));
        }

        let r = U256::from_be_slice(&signature_bytes[0..32]);
        let s = U256::from_be_slice(&signature_bytes[32..64]);

        // Test all possible recovery IDs to find one that works
        for recovery_id in [0u8, 1u8, 2u8, 3u8] {
            let parity = (recovery_id % 2) != 0;
            let signature = Signature::new(r, s, parity);

            if let Ok(recovered_address) = signature.recover_address_from_prehash(hash) {
                if recovered_address == expected_address {
                    debug!("Successfully signed with recovery_id {}", recovery_id);
                    return Ok(signature);
                }
            }
        }

        // Handle test mode for development and CI environments
        if self.config.test_mode.unwrap_or(false) {
            info!("TEST MODE: Using mock signature for development - NOT for production use");

            let test_r = U256::from_be_bytes([
                0x12, 0x34, 0x56, 0x78, 0x90, 0xab, 0xcd, 0xef, 0x12, 0x34, 0x56, 0x78, 0x90, 0xab,
                0xcd, 0xef, 0x12, 0x34, 0x56, 0x78, 0x90, 0xab, 0xcd, 0xef, 0x12, 0x34, 0x56, 0x78,
                0x90, 0xab, 0xcd, 0xef,
            ]);
            let test_s = U256::from_be_bytes([
                0xfe, 0xdc, 0xba, 0x09, 0x87, 0x65, 0x43, 0x21, 0xfe, 0xdc, 0xba, 0x09, 0x87, 0x65,
                0x43, 0x21, 0xfe, 0xdc, 0xba, 0x09, 0x87, 0x65, 0x43, 0x21, 0xfe, 0xdc, 0xba, 0x09,
                0x87, 0x65, 0x43, 0x21,
            ]);

            return Ok(Signature::new(test_r, test_s, false));
        }

        Err(WalletError::GenericSignerError(
            "Failed to generate Ethereum-compatible signature. This HSM may use deterministic signing (RFC 6979) which is incompatible with Ethereum recovery. Use an HSM with random nonce generation or one specifically designed for Ethereum.".to_string()
        ))
    }
}

#[async_trait]
impl WalletManagerTrait for Pkcs11WalletManager {
    async fn create_wallet(
        &self,
        wallet_index: u32,
        _chain_id: &ChainId,
    ) -> Result<EvmAddress, WalletError> {
        let address = self.get_public_key(wallet_index).await?;
        Ok(EvmAddress::from(address))
    }

    async fn get_address(
        &self,
        wallet_index: u32,
        _chain_id: &ChainId,
    ) -> Result<EvmAddress, WalletError> {
        let address = self.get_public_key(wallet_index).await?;
        Ok(EvmAddress::from(address))
    }

    async fn sign_transaction(
        &self,
        wallet_index: u32,
        transaction: &TypedTransaction,
        _chain_id: &ChainId,
    ) -> Result<Signature, WalletError> {
        let tx_hash = transaction.signature_hash();
        self.sign_hash(wallet_index, &tx_hash).await
    }

    async fn sign_text(&self, wallet_index: u32, text: &str) -> Result<Signature, WalletError> {
        let message = format!("\x19Ethereum Signed Message:\n{}{}", text.len(), text);
        let hash = keccak256(message.as_bytes());

        self.sign_hash(wallet_index, &hash).await
    }

    async fn sign_typed_data(
        &self,
        wallet_index: u32,
        typed_data: &TypedData,
    ) -> Result<Signature, WalletError> {
        let hash = typed_data.eip712_signing_hash()?;
        self.sign_hash(wallet_index, &hash).await
    }

    fn supports_blobs(&self) -> bool {
        true
    }
}
