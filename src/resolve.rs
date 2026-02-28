use crate::{
    csv_handler::TransactionRecord,
    engine::{Account, DisputeState, StoredTransaction},
};
use std::collections::HashMap;

pub fn execute(
    stored_transactions: &mut HashMap<u32, StoredTransaction>,
    account: &mut Account,
    transaction: TransactionRecord,
) {
    let stored_transaction = match stored_transactions.get_mut(&transaction.tx) {
        Some(transaction) => transaction,
        None => {
            eprintln!(
                "[client: {}, tx: {}] Resolve rejected: transaction not found",
                transaction.client, transaction.tx
            );
            return;
        }
    };

    if stored_transaction.client != transaction.client {
        eprintln!(
            "[client: {}, tx: {}] Resolve rejected: transaction belongs to client {}",
            transaction.client, transaction.tx, stored_transaction.client
        );
        return;
    }

    if stored_transaction.dispute_state != DisputeState::Disputed {
        eprintln!(
            "[client: {}, tx: {}] Resolve rejected: transaction is not disputed (state: {:?})",
            transaction.client, transaction.tx, stored_transaction.dispute_state
        );
        return;
    }

    // From here onwards the resolve is considered valid

    let Some(new_held) = account.held.checked_sub(stored_transaction.amount) else {
        eprintln!(
            "[client: {}, tx: {}] Resolve rejected: arithmetic overflow",
            transaction.client, transaction.tx
        );
        return;
    };
    let Some(new_available) = account.available.checked_add(stored_transaction.amount) else {
        eprintln!(
            "[client: {}, tx: {}] Resolve rejected: arithmetic overflow",
            transaction.client, transaction.tx
        );
        return;
    };
    account.held = new_held;
    account.available = new_available;
    stored_transaction.dispute_state = DisputeState::Resolved;
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
                dispute_state: DisputeState::Disputed,
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
    fn resolve_transitions_to_resolved_state() {
        let mut ledger = HashMap::new();
        let mut account = setup_disputed_account(&mut ledger, 1, 1, dec!(10.0));

        execute(&mut ledger, &mut account, make_resolve(1, 1));

        assert_eq!(
            ledger.get(&1).unwrap().dispute_state,
            DisputeState::Resolved
        );
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
        assert_eq!(
            ledger.get(&1).unwrap().dispute_state,
            DisputeState::Disputed
        );
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
                dispute_state: DisputeState::None,
                is_deposit: true,
            },
        );

        execute(&mut ledger, &mut account, make_resolve(1, 1));

        assert_eq!(account.available, dec!(10.0));
        assert_eq!(account.held, Decimal::ZERO);
    }
}
