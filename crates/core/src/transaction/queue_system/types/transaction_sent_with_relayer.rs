use core::fmt;
use std::fmt::{Display, Formatter};

use crate::gas::GasPriceResult;
use crate::transaction::types::{TransactionHash, TransactionId};

#[derive(Debug, Clone)]
pub struct TransactionSentWithRelayer {
    pub id: TransactionId,
    pub hash: TransactionHash,
    pub sent_with_gas: GasPriceResult,
}

impl Display for TransactionSentWithRelayer {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "TransactionSentWithRelayer {{ id: {}, hash: {}, sent_with_gas: {:?} }}",
            self.id, self.hash, self.sent_with_gas
        )
    }
}
