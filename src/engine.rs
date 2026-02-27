use crate::{TransactionOperation, TransactionRecord, deposit, dispute, withdraw};
use rust_decimal::Decimal;
use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct Account {
    pub available: Decimal,
    pub held: Decimal,
    pub locked: bool,
}

impl Account {
    // Having a "total" attribute could be a liability: its an attribute dependent on available + held
    // and should be computed at the moment to avoid inconsistencies.
    pub fn total(&self) -> Decimal {
        self.available + self.held
    }
}

#[derive(Debug, Clone)]
pub struct StoredTransaction {
    pub client: u16,
    pub amount: Decimal,
    pub disputed: bool,
}

pub fn process_transactions(transactions: Vec<TransactionRecord>) -> HashMap<u16, Account> {
    // Storing accounts in a hashmap permits O(1) lookups instead
    // of using Vec<ClientAccount> which would do an O(n) lookup.
    let mut all_client_accounts: HashMap<u16, Account> = HashMap::new();
    let mut stored_transactions: HashMap<u32, StoredTransaction> = HashMap::new();

    for transaction in transactions {
        match transaction.operation {
            TransactionOperation::Deposit => {
                let client_account = all_client_accounts.entry(transaction.client).or_default();

                deposit::execute(&mut stored_transactions, client_account, transaction);
            }
            TransactionOperation::Withdrawal => {
                let client_account = all_client_accounts.entry(transaction.client).or_default();

                withdraw::execute(client_account, transaction.amount);
            }
            TransactionOperation::Dispute => {
                let client_account = all_client_accounts.entry(transaction.client).or_default();

                dispute::execute(&mut stored_transactions, client_account, transaction);
            }
            _ => println!("Operation not implemented yet."),
        }
    }

    all_client_accounts
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn make_tx(
        operation: TransactionOperation,
        client: u16,
        tx: u32,
        amount: Option<Decimal>,
    ) -> TransactionRecord {
        TransactionRecord {
            operation,
            client,
            tx,
            amount,
        }
    }

    #[test]
    fn process_spec_example() {
        let transactions = vec![
            make_tx(TransactionOperation::Deposit, 1, 1, Some(dec!(1.0))),
            make_tx(TransactionOperation::Deposit, 2, 2, Some(dec!(2.0))),
            make_tx(TransactionOperation::Deposit, 1, 3, Some(dec!(2.0))),
            make_tx(TransactionOperation::Withdrawal, 1, 4, Some(dec!(1.5))),
            make_tx(TransactionOperation::Withdrawal, 2, 5, Some(dec!(3.0))),
        ];

        let accounts = process_transactions(transactions);

        let a1 = accounts.get(&1).unwrap();
        assert_eq!(a1.available, dec!(1.5));
        assert_eq!(a1.held, Decimal::ZERO);
        assert_eq!(a1.total(), dec!(1.5));
        assert!(!a1.locked);

        let a2 = accounts.get(&2).unwrap();
        assert_eq!(a2.available, dec!(2.0));
        assert_eq!(a2.held, Decimal::ZERO);
        assert_eq!(a2.total(), dec!(2.0));
        assert!(!a2.locked);
    }

    #[test]
    fn deposit_then_dispute_holds_funds() {
        let transactions = vec![
            make_tx(TransactionOperation::Deposit, 1, 1, Some(dec!(10.0))),
            make_tx(TransactionOperation::Dispute, 1, 1, None),
        ];

        let accounts = process_transactions(transactions);
        let a1 = accounts.get(&1).unwrap();
        assert_eq!(a1.available, Decimal::ZERO);
        assert_eq!(a1.held, dec!(10.0));
        assert_eq!(a1.total(), dec!(10.0));
        assert!(!a1.locked);
    }

    #[test]
    fn dispute_does_not_affect_other_clients() {
        let transactions = vec![
            make_tx(TransactionOperation::Deposit, 1, 1, Some(dec!(10.0))),
            make_tx(TransactionOperation::Deposit, 2, 2, Some(dec!(20.0))),
            make_tx(TransactionOperation::Dispute, 1, 1, None),
        ];

        let accounts = process_transactions(transactions);

        let a1 = accounts.get(&1).unwrap();
        assert_eq!(a1.available, Decimal::ZERO);
        assert_eq!(a1.held, dec!(10.0));

        let a2 = accounts.get(&2).unwrap();
        assert_eq!(a2.available, dec!(20.0));
        assert_eq!(a2.held, Decimal::ZERO);
    }

    #[test]
    fn dispute_nonexistent_tx_leaves_account_unchanged() {
        let transactions = vec![
            make_tx(TransactionOperation::Deposit, 1, 1, Some(dec!(10.0))),
            make_tx(TransactionOperation::Dispute, 1, 99, None),
        ];

        let accounts = process_transactions(transactions);
        let a1 = accounts.get(&1).unwrap();
        assert_eq!(a1.available, dec!(10.0));
        assert_eq!(a1.held, Decimal::ZERO);
    }

    #[test]
    fn deposit_dispute_then_withdrawal_rejected_on_held_funds() {
        let transactions = vec![
            make_tx(TransactionOperation::Deposit, 1, 1, Some(dec!(10.0))),
            make_tx(TransactionOperation::Dispute, 1, 1, None),
            make_tx(TransactionOperation::Withdrawal, 1, 2, Some(dec!(5.0))),
        ];

        let accounts = process_transactions(transactions);
        let a1 = accounts.get(&1).unwrap();
        // Withdrawal should fail: available is 0, funds are held
        assert_eq!(a1.available, Decimal::ZERO);
        assert_eq!(a1.held, dec!(10.0));
    }

    #[test]
    fn partial_dispute_allows_withdrawal_of_remaining() {
        let transactions = vec![
            make_tx(TransactionOperation::Deposit, 1, 1, Some(dec!(10.0))),
            make_tx(TransactionOperation::Deposit, 1, 2, Some(dec!(5.0))),
            // Dispute only the first deposit
            make_tx(TransactionOperation::Dispute, 1, 1, None),
            // Withdraw from remaining available (5.0)
            make_tx(TransactionOperation::Withdrawal, 1, 3, Some(dec!(3.0))),
        ];

        let accounts = process_transactions(transactions);
        let a1 = accounts.get(&1).unwrap();
        assert_eq!(a1.available, dec!(2.0));
        assert_eq!(a1.held, dec!(10.0));
        assert_eq!(a1.total(), dec!(12.0));
    }
}
