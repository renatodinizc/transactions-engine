use crate::{
    csv_handler::TransactionRecord,
    engine::{Account, StoredTransaction},
};
use std::collections::HashMap;

pub fn execute(
    stored_transactions: &mut HashMap<u32, StoredTransaction>,
    account: &mut Account,
    current_transaction: TransactionRecord,
) {
    let disputed_transaction = match stored_transactions.get_mut(&current_transaction.tx) {
        Some(transaction) => transaction,
        None => {
            eprintln!(
                "Could not find stored transaction for related resolve: {}",
                &current_transaction.tx
            );
            return;
        }
    };

    if disputed_transaction.client != current_transaction.client {
        eprintln!(
            "The resolve's transaction is not from the same client. Resolve transaction's client {}, current client: {}",
            disputed_transaction.client, current_transaction.client
        );
        return;
    }

    if !disputed_transaction.disputed {
        eprintln!(
            "Ignore resolve because related transaction {} is not undergoing a dispute.",
            current_transaction.tx
        );
        return;
    }

    // From here onwards the resolve is considered valid

    account.held -= disputed_transaction.amount;
    account.available += disputed_transaction.amount;
    disputed_transaction.disputed = false;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::csv_handler::TransactionOperation;
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;

    fn make_resolve(client: u16, tx: u32) -> TransactionRecord {
        TransactionRecord {
            operation: TransactionOperation::Resolve,
            client,
            tx,
            amount: None,
        }
    }

    fn setup_disputed_account(
        ledger: &mut HashMap<u32, StoredTransaction>,
        client: u16,
        tx: u32,
        amount: Decimal,
    ) -> Account {
        let mut account = Account::default();
        account.held = amount;
        ledger.insert(
            tx,
            StoredTransaction {
                client,
                amount,
                disputed: true,
                is_deposit: true,
            },
        );
        account
    }

    #[test]
    fn resolve_moves_funds_from_held_to_available() {
        let mut ledger = HashMap::new();
        let mut account = setup_disputed_account(&mut ledger, 1, 1, dec!(10.0));

        execute(&mut ledger, &mut account, make_resolve(1, 1));

        assert_eq!(account.available, dec!(10.0));
        assert_eq!(account.held, Decimal::ZERO);
        assert_eq!(account.total(), dec!(10.0));
    }

    #[test]
    fn resolve_clears_disputed_flag() {
        let mut ledger = HashMap::new();
        let mut account = setup_disputed_account(&mut ledger, 1, 1, dec!(10.0));

        execute(&mut ledger, &mut account, make_resolve(1, 1));

        assert!(!ledger.get(&1).unwrap().disputed);
    }

    #[test]
    fn resolve_nonexistent_tx_is_ignored() {
        let mut ledger = HashMap::new();
        let mut account = Account::default();
        account.held = dec!(10.0);

        execute(&mut ledger, &mut account, make_resolve(1, 99));

        assert_eq!(account.held, dec!(10.0));
        assert_eq!(account.available, Decimal::ZERO);
    }

    #[test]
    fn resolve_wrong_client_is_ignored() {
        let mut ledger = HashMap::new();
        let mut account = setup_disputed_account(&mut ledger, 1, 1, dec!(10.0));

        execute(&mut ledger, &mut account, make_resolve(2, 1));

        assert_eq!(account.held, dec!(10.0));
        assert_eq!(account.available, Decimal::ZERO);
        assert!(ledger.get(&1).unwrap().disputed);
    }

    #[test]
    fn resolve_undisputed_tx_is_ignored() {
        let mut ledger = HashMap::new();
        let mut account = Account::default();
        account.available = dec!(10.0);
        ledger.insert(
            1,
            StoredTransaction {
                client: 1,
                amount: dec!(10.0),
                disputed: false,
                is_deposit: true,
            },
        );

        execute(&mut ledger, &mut account, make_resolve(1, 1));

        assert_eq!(account.available, dec!(10.0));
        assert_eq!(account.held, Decimal::ZERO);
    }
}
