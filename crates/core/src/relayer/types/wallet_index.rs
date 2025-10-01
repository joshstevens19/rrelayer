use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WalletIndex {
    /// Signing providers wallet with positive index
    Normal(u32),
    /// Private key wallet with negative database index (to avoid a big refactor)
    PrivateKey(i32),
}

impl WalletIndex {
    /// Get the underlying index value for wallet manager operations
    pub fn index(&self) -> u32 {
        match self {
            WalletIndex::Normal(index) => *index,
            WalletIndex::PrivateKey(db_index) => {
                // Convert negative database index to positive array index using maximum u32 range
                // to completely avoid conflicts with mnemonic-derived wallets
                // u32::MAX = 4,294,967,295, so we use high range for private keys
                // -1 -> 4,294,967,294, -2 -> 4,294,967,293, -3 -> 4,294,967,292, etc.
                u32::MAX - (-db_index - 1) as u32
            }
        }
    }

    /// Check if this is a private key wallet
    pub fn is_private_key(&self) -> bool {
        matches!(self, WalletIndex::PrivateKey(_))
    }

    /// Get the database storage value (i32)
    pub fn db_value(&self) -> i32 {
        match self {
            WalletIndex::Normal(index) => *index as i32,
            WalletIndex::PrivateKey(db_index) => *db_index,
        }
    }
}
