use crate::transaction::types::{Transaction, TransactionHash, TransactionId};

#[derive(Clone, Debug)]
pub enum CompetitionType {
    /// Cancel transaction - just needs to win once to cancel original
    Cancel,
    /// Replace transaction - becomes the new active transaction for ongoing operations
    Replace,
}

/// Represents a transaction in the inmempool queue that may have a competing transaction
/// (either a cancel or replace transaction) with the same nonce.
#[derive(Clone, Debug)]
pub struct CompetitiveTransaction {
    /// The original transaction
    pub original: Transaction,
    /// Optional competing transaction (cancel or replace) with same nonce
    pub competitive: Option<(Transaction, CompetitionType)>,
}

#[derive(Clone, Debug)]
pub enum CompetitionResult {
    /// Original transaction mined - competitor should be marked as DROPPED
    OriginalWon { original_hash: TransactionHash, competitor_id: Option<TransactionId> },
    /// Competitor transaction mined - original should be marked as CANCELLED
    CompetitorWon { competitor_hash: TransactionHash, original_id: TransactionId },
    /// No competition - only original transaction exists
    NoCompetition { original_hash: TransactionHash },
}

impl CompetitiveTransaction {
    /// Create a new competitive transaction with just the original
    pub fn new(original: Transaction) -> Self {
        Self { original, competitive: None }
    }

    /// Add a competitive transaction (cancel or replace)
    pub fn add_competitor(&mut self, competitor: Transaction, competition_type: CompetitionType) {
        self.competitive = Some((competitor, competition_type));
    }

    /// Check if this has a competing transaction
    pub fn has_competitor(&self) -> bool {
        self.competitive.is_some()
    }

    /// Get the active transaction that should receive gas bumps
    /// This is the competitor if it exists (because that's what we want to win),
    /// otherwise the original transaction
    pub fn get_active_transaction(&self) -> &Transaction {
        match &self.competitive {
            Some((competitor, _)) => competitor,
            None => &self.original,
        }
    }

    /// Get mutable reference to the active transaction that should receive gas bumps
    pub fn get_active_transaction_mut(&mut self) -> &mut Transaction {
        match &mut self.competitive {
            Some((competitor, _)) => competitor,
            None => &mut self.original,
        }
    }

    /// Get the competition type if there is a competitor
    pub fn get_competition_type(&self) -> Option<&CompetitionType> {
        self.competitive.as_ref().map(|(_, comp_type)| comp_type)
    }

    /// Get the nonce (both transactions should have the same nonce)
    pub fn nonce(&self) -> u64 {
        self.original.nonce.into()
    }

    /// Check which transaction (if any) matches the given hash
    pub fn check_mined_hash(&self, mined_hash: &TransactionHash) -> CompetitionResult {
        if self.original.known_transaction_hash.as_ref() == Some(mined_hash) {
            // Original transaction won
            CompetitionResult::OriginalWon {
                original_hash: *mined_hash,
                competitor_id: self.competitive.as_ref().map(|(tx, _)| tx.id),
            }
        } else if let Some((ref competitor, _)) = &self.competitive {
            if competitor.known_transaction_hash.as_ref() == Some(mined_hash) {
                // Competitor won
                CompetitionResult::CompetitorWon {
                    competitor_hash: *mined_hash,
                    original_id: self.original.id,
                }
            } else {
                // Hash doesn't match either - this shouldn't happen
                CompetitionResult::NoCompetition { original_hash: *mined_hash }
            }
        } else {
            // No competitor, original mined normally
            CompetitionResult::NoCompetition { original_hash: *mined_hash }
        }
    }

    /// Get transaction by ID
    pub fn get_transaction_by_id(&self, id: &TransactionId) -> Option<&Transaction> {
        if self.original.id == *id {
            Some(&self.original)
        } else if let Some((ref competitor, _)) = &self.competitive {
            if competitor.id == *id {
                Some(competitor)
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Get mutable reference to transaction by ID
    pub fn get_transaction_by_id_mut(&mut self, id: &TransactionId) -> Option<&mut Transaction> {
        if self.original.id == *id {
            Some(&mut self.original)
        } else if let Some((ref mut competitor, _)) = &mut self.competitive {
            if competitor.id == *id {
                Some(competitor)
            } else {
                None
            }
        } else {
            None
        }
    }
}
