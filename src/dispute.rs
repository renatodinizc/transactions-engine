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
                "Could not find stored transaction for related dispute: {}",
                &current_transaction.tx
            );
            return;
        }
    };

    if disputed_transaction.client != current_transaction.client {
        eprintln!(
            "The dispute's transaction is not from the same client. Disputed transaction's client {}, current client: {}",
            disputed_transaction.client, current_transaction.client
        );
        return;
    }

    if disputed_transaction.disputed {
        eprintln!(
            "A dispute is already undergoing for the transaction {}.",
            current_transaction.tx
        );
        return;
    }

    if !disputed_transaction.is_deposit {
        eprintln!(
            "The disputed operation is not a deposit and should be ignored. Related transaction tx: {}.",
            current_transaction.tx
        );
        return;
    }

    // From here onwards the dispute is considered valid

    account.available -= disputed_transaction.amount;
    account.held += disputed_transaction.amount;
    disputed_transaction.disputed = true;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::csv_handler::TransactionOperation;
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;

    fn make_dispute(client: u16, tx: u32) -> TransactionRecord {
        TransactionRecord {
            operation: TransactionOperation::Dispute,
            client,
            tx,
            amount: None,
        }
    }

    fn setup_account_with_deposit(
        ledger: &mut HashMap<u32, StoredTransaction>,
        client: u16,
        tx: u32,
        amount: Decimal,
    ) -> Account {
        let mut account = Account::default();
        account.available = amount;
        ledger.insert(
            tx,
            StoredTransaction {
                client,
                amount,
                disputed: false,
                is_deposit: true,
            },
        );
        account
    }

    #[test]
    fn dispute_moves_funds_from_available_to_held() {
        let mut ledger = HashMap::new();
        let mut account = setup_account_with_deposit(&mut ledger, 1, 1, dec!(10.0));

        execute(&mut ledger, &mut account, make_dispute(1, 1));

        assert_eq!(account.available, Decimal::ZERO);
        assert_eq!(account.held, dec!(10.0));
        assert_eq!(account.total(), dec!(10.0));
    }

    #[test]
    fn dispute_marks_transaction_as_disputed() {
        let mut ledger = HashMap::new();
        let mut account = setup_account_with_deposit(&mut ledger, 1, 1, dec!(10.0));

        execute(&mut ledger, &mut account, make_dispute(1, 1));

        assert!(ledger.get(&1).unwrap().disputed);
    }

    #[test]
    fn dispute_nonexistent_tx_is_ignored() {
        let mut ledger = HashMap::new();
        let mut account = Account::default();
        account.available = dec!(10.0);

        execute(&mut ledger, &mut account, make_dispute(1, 99));

        assert_eq!(account.available, dec!(10.0));
        assert_eq!(account.held, Decimal::ZERO);
    }

    #[test]
    fn dispute_wrong_client_is_ignored() {
        let mut ledger = HashMap::new();
        let mut account = setup_account_with_deposit(&mut ledger, 1, 1, dec!(10.0));

        // Client 2 trying to dispute client 1's transaction
        execute(&mut ledger, &mut account, make_dispute(2, 1));

        assert_eq!(account.available, dec!(10.0));
        assert_eq!(account.held, Decimal::ZERO);
        assert!(!ledger.get(&1).unwrap().disputed);
    }

    #[test]
    fn dispute_already_disputed_tx_is_ignored() {
        let mut ledger = HashMap::new();
        let mut account = setup_account_with_deposit(&mut ledger, 1, 1, dec!(10.0));

        execute(&mut ledger, &mut account, make_dispute(1, 1));
        // Second dispute on same tx
        execute(&mut ledger, &mut account, make_dispute(1, 1));

        // Should only have moved funds once
        assert_eq!(account.available, Decimal::ZERO);
        assert_eq!(account.held, dec!(10.0));
    }

    #[test]
    fn dispute_partial_balance() {
        let mut ledger = HashMap::new();
        // Account has 20, but the disputed deposit was only 10
        let mut account = Account::default();
        account.available = dec!(20.0);
        ledger.insert(
            1,
            StoredTransaction {
                client: 1,
                amount: dec!(10.0),
                disputed: false,
                is_deposit: true,
            },
        );

        execute(&mut ledger, &mut account, make_dispute(1, 1));

        assert_eq!(account.available, dec!(10.0));
        assert_eq!(account.held, dec!(10.0));
        assert_eq!(account.total(), dec!(20.0));
    }
}
