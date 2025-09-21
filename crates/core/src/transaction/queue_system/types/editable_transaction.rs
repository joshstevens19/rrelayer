use crate::transaction::types::Transaction;

#[derive(Debug)]
pub enum EditableTransactionType {
    Pending,
    Inmempool,
}

#[derive(Debug)]
pub struct EditableTransaction {
    pub transaction: Transaction,
    pub type_name: EditableTransactionType,
}

impl EditableTransaction {
    pub fn to_pending(transaction: Transaction) -> EditableTransaction {
        EditableTransaction { transaction, type_name: EditableTransactionType::Pending }
    }

    pub fn to_inmempool(transaction: Transaction) -> EditableTransaction {
        println!("EDITABLE TX READY - {:?}", transaction);
        EditableTransaction { transaction, type_name: EditableTransactionType::Inmempool }
    }
}
