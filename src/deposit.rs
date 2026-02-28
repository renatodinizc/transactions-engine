use crate::{
    csv_handler::TransactionRecord,
    engine::{Account, StoredTransaction},
};
use rust_decimal::Decimal;
use std::collections::HashMap;

pub fn execute(
    stored_transactions: &mut HashMap<u32, StoredTransaction>,
    account: &mut Account,
    transaction: TransactionRecord,
) {
    let amount = match transaction.amount {
        Some(a) if a > Decimal::ZERO => a,
        _ => {
            eprintln!(
                "[client: {}, tx: {}] Deposit rejected: invalid amount ({:?})",
                transaction.client, transaction.tx, transaction.amount
            );
            return;
        }
    };

    if account.locked {
        eprintln!(
            "[client: {}, tx: {}] Deposit rejected: account is locked",
            transaction.client, transaction.tx
        );
        return;
    }

    if stored_transactions.contains_key(&transaction.tx) {
        eprintln!(
            "[client: {}, tx: {}] Deposit rejected: duplicate transaction ID",
            transaction.client, transaction.tx
        );
        return;
    }

    let Some(new_available) = account.available.checked_add(amount) else {
        eprintln!(
            "[client: {}, tx: {}] Deposit rejected: arithmetic overflow",
            transaction.client, transaction.tx
        );
        return;
    };
    account.available = new_available;

    stored_transactions.insert(
        transaction.tx,
        StoredTransaction {
            amount,
            client: transaction.client,
            disputed: false,
            is_deposit: true,
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::csv_handler::TransactionOperation;
    use rust_decimal_macros::dec;

    fn make_deposit(client: u16, tx: u32, amount: Option<Decimal>) -> TransactionRecord {
        TransactionRecord {
            operation: TransactionOperation::Deposit,
            client,
            tx,
            amount,
        }
    }

    #[test]
    fn deposit_increases_available() {
        let mut ledger = HashMap::new();
        let mut account = Account::default();
        execute(
            &mut ledger,
            &mut account,
            make_deposit(1, 1, Some(dec!(10.0))),
        );
        assert_eq!(account.available, dec!(10.0));
        assert_eq!(account.total(), dec!(10.0));
    }

    #[test]
    fn multiple_deposits_accumulate() {
        let mut ledger = HashMap::new();
        let mut account = Account::default();
        execute(
            &mut ledger,
            &mut account,
            make_deposit(1, 1, Some(dec!(1.1111))),
        );
        execute(
            &mut ledger,
            &mut account,
            make_deposit(1, 2, Some(dec!(2.2222))),
        );
        assert_eq!(account.available, dec!(3.3333));
    }

    #[test]
    fn deposit_with_none_amount_is_ignored() {
        let mut ledger = HashMap::new();
        let mut account = Account::default();
        execute(&mut ledger, &mut account, make_deposit(1, 1, None));
        assert_eq!(account.available, Decimal::ZERO);
    }

    #[test]
    fn deposit_with_zero_amount_is_ignored() {
        let mut ledger = HashMap::new();
        let mut account = Account::default();
        execute(
            &mut ledger,
            &mut account,
            make_deposit(1, 1, Some(Decimal::ZERO)),
        );
        assert_eq!(account.available, Decimal::ZERO);
    }

    #[test]
    fn deposit_with_negative_amount_is_ignored() {
        let mut ledger = HashMap::new();
        let mut account = Account::default();
        execute(
            &mut ledger,
            &mut account,
            make_deposit(1, 1, Some(dec!(-5.0))),
        );
        assert_eq!(account.available, Decimal::ZERO);
    }

    #[test]
    fn deposit_on_locked_account_is_ignored() {
        let mut ledger = HashMap::new();
        let mut account = Account::default();
        account.locked = true;
        execute(
            &mut ledger,
            &mut account,
            make_deposit(1, 1, Some(dec!(10.0))),
        );
        assert_eq!(account.available, Decimal::ZERO);
    }

    #[test]
    fn deposit_overflow_is_rejected() {
        let mut ledger = HashMap::new();
        let mut account = Account::default();
        execute(
            &mut ledger,
            &mut account,
            make_deposit(1, 1, Some(Decimal::MAX)),
        );
        assert_eq!(account.available, Decimal::MAX);

        // Second deposit should be rejected due to overflow
        execute(
            &mut ledger,
            &mut account,
            make_deposit(1, 2, Some(dec!(1.0))),
        );
        assert_eq!(account.available, Decimal::MAX);
        assert!(!ledger.contains_key(&2));
    }

    #[test]
    fn deposit_stores_transaction_in_ledger() {
        let mut ledger = HashMap::new();
        let mut account = Account::default();
        execute(
            &mut ledger,
            &mut account,
            make_deposit(1, 42, Some(dec!(10.0))),
        );
        let stored = ledger.get(&42).unwrap();
        assert_eq!(stored.client, 1);
        assert_eq!(stored.amount, dec!(10.0));
        assert!(!stored.disputed);
    }
}
