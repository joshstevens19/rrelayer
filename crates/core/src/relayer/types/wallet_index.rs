use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const MAX_NORMAL_WALLET_INDEX: u32 = i32::MAX as u32;
pub const MIN_PRIVATE_KEY_WALLET_MANAGER_INDEX: u32 = MAX_NORMAL_WALLET_INDEX + 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum WalletIndexError {
    #[error("normal wallet index {index} exceeds maximum database index {max}")]
    NormalIndexOutOfRange { index: u32, max: u32 },

    #[error("normal wallet database index must be non-negative, got {index}")]
    NormalDbIndexMustBeNonNegative { index: i32 },

    #[error("private key wallet database index must be negative, got {index}")]
    PrivateKeyIndexMustBeNegative { index: i32 },

    #[error("private key wallet database index {index} cannot be converted")]
    PrivateKeyIndexOutOfRange { index: i32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum WalletIndex {
    Normal(u32),
    PrivateKey(i32),
}

impl WalletIndex {
    pub fn normal(index: u32) -> Result<Self, WalletIndexError> {
        if index > MAX_NORMAL_WALLET_INDEX {
            return Err(WalletIndexError::NormalIndexOutOfRange {
                index,
                max: MAX_NORMAL_WALLET_INDEX,
            });
        }
        Ok(Self::Normal(index))
    }

    pub fn private_key(db_index: i32) -> Result<Self, WalletIndexError> {
        if db_index >= 0 {
            return Err(WalletIndexError::PrivateKeyIndexMustBeNegative { index: db_index });
        }
        if db_index == i32::MIN {
            return Err(WalletIndexError::PrivateKeyIndexOutOfRange { index: db_index });
        }
        Ok(Self::PrivateKey(db_index))
    }

    pub fn from_db_value(db_value: i32, is_private_key: bool) -> Result<Self, WalletIndexError> {
        if is_private_key {
            Self::private_key(db_value)
        } else {
            let index = u32::try_from(db_value).map_err(|_| {
                WalletIndexError::NormalDbIndexMustBeNonNegative { index: db_value }
            })?;
            Self::normal(index)
        }
    }

    pub fn is_private_key_manager_index(wallet_index: u32) -> bool {
        wallet_index >= MIN_PRIVATE_KEY_WALLET_MANAGER_INDEX
    }

    pub fn index(&self) -> u32 {
        match self {
            WalletIndex::Normal(index) => *index,
            WalletIndex::PrivateKey(db_index) => {
                debug_assert!(*db_index < 0 && *db_index != i32::MIN);
                let offset = (-i64::from(*db_index) - 1) as u32;
                u32::MAX - offset
            }
        }
    }

    pub fn private_key_internal_index(&self) -> Option<u32> {
        match self {
            WalletIndex::Normal(_) => None,
            WalletIndex::PrivateKey(_) => Some(u32::MAX - self.index()),
        }
    }

    pub fn is_private_key(&self) -> bool {
        matches!(self, WalletIndex::PrivateKey(_))
    }

    pub fn db_value(&self) -> i32 {
        match self {
            WalletIndex::Normal(index) => *index as i32,
            WalletIndex::PrivateKey(db_index) => *db_index,
        }
    }
}

impl<'de> Deserialize<'de> for WalletIndex {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        enum UncheckedWalletIndex {
            Normal(u32),
            PrivateKey(i32),
        }

        match UncheckedWalletIndex::deserialize(deserializer)? {
            UncheckedWalletIndex::Normal(index) => {
                Self::normal(index).map_err(serde::de::Error::custom)
            }
            UncheckedWalletIndex::PrivateKey(index) => {
                Self::private_key(index).map_err(serde::de::Error::custom)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_normal_indexes_use_non_negative_database_namespace() {
        let zero = WalletIndex::normal(0).unwrap();
        let max = WalletIndex::normal(MAX_NORMAL_WALLET_INDEX).unwrap();
        assert_eq!((zero.index(), zero.db_value()), (0, 0));
        assert_eq!((max.index(), max.db_value()), (MAX_NORMAL_WALLET_INDEX, i32::MAX));
        assert!(WalletIndex::normal(MAX_NORMAL_WALLET_INDEX + 1).is_err());
    }

    #[test]
    fn private_key_rows_remain_in_separate_negative_namespace() {
        let first = WalletIndex::private_key(-1).unwrap();
        let second = WalletIndex::private_key(-2).unwrap();
        assert_eq!((first.index(), first.private_key_internal_index()), (u32::MAX, Some(0)));
        assert_eq!((second.index(), second.private_key_internal_index()), (u32::MAX - 1, Some(1)));
        assert_eq!((first.db_value(), second.db_value()), (-1, -2));
    }

    #[test]
    fn persisted_namespace_combinations_are_validated() {
        assert!(WalletIndex::from_db_value(7, false).is_ok());
        assert!(WalletIndex::from_db_value(-1, true).is_ok());
        assert!(WalletIndex::from_db_value(-1, false).is_err());
        assert!(WalletIndex::from_db_value(0, true).is_err());
        assert!(WalletIndex::private_key(i32::MIN).is_err());
    }

    #[test]
    fn private_key_manager_boundary_cannot_overlap_normal_namespace() {
        assert!(!WalletIndex::is_private_key_manager_index(MAX_NORMAL_WALLET_INDEX));
        assert!(!WalletIndex::is_private_key_manager_index(MAX_NORMAL_WALLET_INDEX + 1));
        assert!(WalletIndex::is_private_key_manager_index(MIN_PRIVATE_KEY_WALLET_MANAGER_INDEX));
        assert!(WalletIndex::is_private_key_manager_index(u32::MAX));
    }
}
