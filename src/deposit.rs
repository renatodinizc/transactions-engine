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
            return eprintln!(
                "not a valid amount number to deposit: {:?}",
                transaction.amount
            );
        }
    };

    if account.locked {
        return;
    }

    account.available += amount;

    stored_transactions.insert(
        transaction.tx,
        StoredTransaction {
            amount,
            client: transaction.client,
            disputed: false,
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
        let mut account = Account::new();
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
        let mut account = Account::new();
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
        let mut account = Account::new();
        execute(&mut ledger, &mut account, make_deposit(1, 1, None));
        assert_eq!(account.available, Decimal::ZERO);
    }

    #[test]
    fn deposit_with_zero_amount_is_ignored() {
        let mut ledger = HashMap::new();
        let mut account = Account::new();
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
        let mut account = Account::new();
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
        let mut account = Account::new();
        account.locked = true;
        execute(
            &mut ledger,
            &mut account,
            make_deposit(1, 1, Some(dec!(10.0))),
        );
        assert_eq!(account.available, Decimal::ZERO);
    }

    #[test]
    fn deposit_stores_transaction_in_ledger() {
        let mut ledger = HashMap::new();
        let mut account = Account::new();
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
