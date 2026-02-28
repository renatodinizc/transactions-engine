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
                "[client: {}, tx: {}] Withdrawal rejected: invalid amount ({:?})",
                transaction.client, transaction.tx, transaction.amount
            );
            return;
        }
    };

    if account.locked {
        eprintln!(
            "[client: {}, tx: {}] Withdrawal rejected: account is locked",
            transaction.client, transaction.tx
        );
        return;
    }

    if stored_transactions.contains_key(&transaction.tx) {
        eprintln!(
            "[client: {}, tx: {}] Withdrawal rejected: duplicate transaction ID",
            transaction.client, transaction.tx
        );
        return;
    }

    if account.available < amount {
        eprintln!(
            "[client: {}, tx: {}] Withdrawal rejected: insufficient funds (available: {}, requested: {})",
            transaction.client, transaction.tx, account.available, amount
        );
        return;
    }

    let Some(new_available) = account.available.checked_sub(amount) else {
        eprintln!(
            "[client: {}, tx: {}] Withdrawal rejected: arithmetic overflow",
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
            is_deposit: false,
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::csv_handler::TransactionOperation;
    use rust_decimal_macros::dec;

    fn make_withdrawal(client: u16, tx: u32, amount: Option<Decimal>) -> TransactionRecord {
        TransactionRecord {
            operation: TransactionOperation::Withdrawal,
            client,
            tx,
            amount,
        }
    }

    #[test]
    fn withdrawal_decreases_available() {
        let mut ledger = HashMap::new();
        let mut account = Account::default();
        account.available = dec!(10.0);
        execute(
            &mut ledger,
            &mut account,
            make_withdrawal(1, 1, Some(dec!(3.0))),
        );
        assert_eq!(account.available, dec!(7.0));
    }

    #[test]
    fn withdrawal_with_insufficient_funds_is_rejected() {
        let mut ledger = HashMap::new();
        let mut account = Account::default();
        account.available = dec!(2.0);
        execute(
            &mut ledger,
            &mut account,
            make_withdrawal(1, 1, Some(dec!(3.0))),
        );
        assert_eq!(account.available, dec!(2.0));
    }

    #[test]
    fn withdrawal_of_exact_balance_succeeds() {
        let mut ledger = HashMap::new();
        let mut account = Account::default();
        account.available = dec!(5.0);
        execute(
            &mut ledger,
            &mut account,
            make_withdrawal(1, 1, Some(dec!(5.0))),
        );
        assert_eq!(account.available, Decimal::ZERO);
    }

    #[test]
    fn withdrawal_with_none_amount_is_ignored() {
        let mut ledger = HashMap::new();
        let mut account = Account::default();
        account.available = dec!(10.0);
        execute(&mut ledger, &mut account, make_withdrawal(1, 1, None));
        assert_eq!(account.available, dec!(10.0));
    }

    #[test]
    fn withdrawal_with_negative_amount_is_ignored() {
        let mut ledger = HashMap::new();
        let mut account = Account::default();
        account.available = dec!(10.0);
        execute(
            &mut ledger,
            &mut account,
            make_withdrawal(1, 1, Some(dec!(-5.0))),
        );
        assert_eq!(account.available, dec!(10.0));
    }

    #[test]
    fn withdrawal_on_locked_account_is_ignored() {
        let mut ledger = HashMap::new();
        let mut account = Account::default();
        account.available = dec!(10.0);
        account.locked = true;
        execute(
            &mut ledger,
            &mut account,
            make_withdrawal(1, 1, Some(dec!(5.0))),
        );
        assert_eq!(account.available, dec!(10.0));
    }

    #[test]
    fn withdrawal_stores_transaction_in_ledger() {
        let mut ledger = HashMap::new();
        let mut account = Account::default();
        account.available = dec!(10.0);
        execute(
            &mut ledger,
            &mut account,
            make_withdrawal(1, 42, Some(dec!(3.0))),
        );
        let stored = ledger.get(&42).unwrap();
        assert_eq!(stored.client, 1);
        assert_eq!(stored.amount, dec!(3.0));
        assert!(!stored.is_deposit);
    }

    #[test]
    fn failed_withdrawal_does_not_store_in_ledger() {
        let mut ledger = HashMap::new();
        let mut account = Account::default();
        account.available = dec!(2.0);
        execute(
            &mut ledger,
            &mut account,
            make_withdrawal(1, 1, Some(dec!(5.0))),
        );
        assert!(ledger.get(&1).is_none());
    }
}
