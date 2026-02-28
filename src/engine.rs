use crate::{
    TransactionOperation, TransactionRecord, chargeback, deposit, dispute, resolve, withdraw,
};
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
    pub is_deposit: bool,
}

pub fn process_transactions(transactions: Vec<TransactionRecord>) -> HashMap<u16, Account> {
    // Storing accounts in a hashmap permits O(1) lookups instead
    // of using Vec<ClientAccount> which would do an O(n) lookup.
    let mut client_accounts: HashMap<u16, Account> = HashMap::new();
    let mut stored_transactions: HashMap<u32, StoredTransaction> = HashMap::new();

    for transaction in transactions {
        let client_account = client_accounts.entry(transaction.client).or_default();

        match transaction.operation {
            TransactionOperation::Deposit => {
                deposit::execute(&mut stored_transactions, client_account, transaction)
            }
            TransactionOperation::Withdrawal => {
                withdraw::execute(&mut stored_transactions, client_account, transaction)
            }
            TransactionOperation::Dispute => {
                dispute::execute(&mut stored_transactions, client_account, transaction)
            }
            TransactionOperation::Resolve => {
                resolve::execute(&mut stored_transactions, client_account, transaction)
            }
            TransactionOperation::Chargeback => {
                chargeback::execute(&mut stored_transactions, client_account, transaction)
            }
        }
    }

    client_accounts
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

    // ── Spec reference ──────────────────────────────────────────────

    #[test]
    fn spec_example() {
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

    // ── Full lifecycle flows ────────────────────────────────────────

    #[test]
    fn deposit_dispute_resolve_returns_funds_to_available() {
        let transactions = vec![
            make_tx(TransactionOperation::Deposit, 1, 1, Some(dec!(10.0))),
            make_tx(TransactionOperation::Dispute, 1, 1, None),
            make_tx(TransactionOperation::Resolve, 1, 1, None),
        ];

        let accounts = process_transactions(transactions);
        let a1 = accounts.get(&1).unwrap();
        assert_eq!(a1.available, dec!(10.0));
        assert_eq!(a1.held, Decimal::ZERO);
        assert!(!a1.locked);
    }

    #[test]
    fn deposit_dispute_chargeback_removes_funds_and_locks() {
        let transactions = vec![
            make_tx(TransactionOperation::Deposit, 1, 1, Some(dec!(10.0))),
            make_tx(TransactionOperation::Dispute, 1, 1, None),
            make_tx(TransactionOperation::Chargeback, 1, 1, None),
        ];

        let accounts = process_transactions(transactions);
        let a1 = accounts.get(&1).unwrap();
        assert_eq!(a1.available, Decimal::ZERO);
        assert_eq!(a1.held, Decimal::ZERO);
        assert_eq!(a1.total(), Decimal::ZERO);
        assert!(a1.locked);
    }

    #[test]
    fn dispute_resolve_redispute_chargeback_full_lifecycle() {
        let transactions = vec![
            make_tx(TransactionOperation::Deposit, 1, 1, Some(dec!(10.0))),
            make_tx(TransactionOperation::Dispute, 1, 1, None),
            make_tx(TransactionOperation::Resolve, 1, 1, None),
            // Re-dispute the same transaction after resolve
            make_tx(TransactionOperation::Dispute, 1, 1, None),
            make_tx(TransactionOperation::Chargeback, 1, 1, None),
        ];

        let accounts = process_transactions(transactions);
        let a1 = accounts.get(&1).unwrap();
        assert_eq!(a1.available, Decimal::ZERO);
        assert_eq!(a1.held, Decimal::ZERO);
        assert!(a1.locked);
    }

    // ── Multi-client isolation ──────────────────────────────────────

    #[test]
    fn chargeback_on_one_client_does_not_affect_another() {
        let transactions = vec![
            make_tx(TransactionOperation::Deposit, 1, 1, Some(dec!(10.0))),
            make_tx(TransactionOperation::Deposit, 2, 2, Some(dec!(20.0))),
            make_tx(TransactionOperation::Dispute, 1, 1, None),
            make_tx(TransactionOperation::Chargeback, 1, 1, None),
            // Client 2 can still operate normally
            make_tx(TransactionOperation::Withdrawal, 2, 3, Some(dec!(5.0))),
        ];

        let accounts = process_transactions(transactions);

        let a1 = accounts.get(&1).unwrap();
        assert_eq!(a1.available, Decimal::ZERO);
        assert!(a1.locked);

        let a2 = accounts.get(&2).unwrap();
        assert_eq!(a2.available, dec!(15.0));
        assert!(!a2.locked);
    }

    #[test]
    fn interleaved_multi_client_transactions() {
        let transactions = vec![
            make_tx(TransactionOperation::Deposit, 1, 1, Some(dec!(100.0))),
            make_tx(TransactionOperation::Deposit, 2, 2, Some(dec!(50.0))),
            make_tx(TransactionOperation::Withdrawal, 1, 3, Some(dec!(30.0))),
            make_tx(TransactionOperation::Deposit, 3, 4, Some(dec!(75.0))),
            make_tx(TransactionOperation::Dispute, 2, 2, None),
            make_tx(TransactionOperation::Withdrawal, 3, 5, Some(dec!(25.0))),
            make_tx(TransactionOperation::Resolve, 2, 2, None),
            make_tx(TransactionOperation::Withdrawal, 1, 6, Some(dec!(20.0))),
        ];

        let accounts = process_transactions(transactions);

        let a1 = accounts.get(&1).unwrap();
        assert_eq!(a1.available, dec!(50.0));
        assert_eq!(a1.held, Decimal::ZERO);
        assert!(!a1.locked);

        let a2 = accounts.get(&2).unwrap();
        assert_eq!(a2.available, dec!(50.0));
        assert_eq!(a2.held, Decimal::ZERO);
        assert!(!a2.locked);

        let a3 = accounts.get(&3).unwrap();
        assert_eq!(a3.available, dec!(50.0));
        assert_eq!(a3.held, Decimal::ZERO);
        assert!(!a3.locked);
    }

    // ── Edge cases ──────────────────────────────────────────────────

    #[test]
    fn dispute_after_partial_withdrawal_causes_negative_available() {
        // Deposit 10, withdraw 7, then dispute the deposit of 10
        // available goes to 3 - 10 = -7 (liability/overdraft)
        let transactions = vec![
            make_tx(TransactionOperation::Deposit, 1, 1, Some(dec!(10.0))),
            make_tx(TransactionOperation::Withdrawal, 1, 2, Some(dec!(7.0))),
            make_tx(TransactionOperation::Dispute, 1, 1, None),
        ];

        let accounts = process_transactions(transactions);
        let a1 = accounts.get(&1).unwrap();
        assert_eq!(a1.available, dec!(-7.0));
        assert_eq!(a1.held, dec!(10.0));
        assert_eq!(a1.total(), dec!(3.0));
    }

    #[test]
    fn duplicate_tx_id_rejects_second_deposit() {
        let transactions = vec![
            make_tx(TransactionOperation::Deposit, 1, 1, Some(dec!(10.0))),
            make_tx(TransactionOperation::Deposit, 1, 1, Some(dec!(5.0))),
            make_tx(TransactionOperation::Dispute, 1, 1, None),
        ];

        let accounts = process_transactions(transactions);
        let a1 = accounts.get(&1).unwrap();
        // Second deposit rejected — only 10 credited, dispute holds all of it
        assert_eq!(a1.available, Decimal::ZERO);
        assert_eq!(a1.held, dec!(10.0));
    }

    #[test]
    fn locked_account_rejects_deposit_and_withdrawal() {
        let transactions = vec![
            make_tx(TransactionOperation::Deposit, 1, 1, Some(dec!(20.0))),
            make_tx(TransactionOperation::Deposit, 1, 2, Some(dec!(10.0))),
            make_tx(TransactionOperation::Dispute, 1, 1, None),
            make_tx(TransactionOperation::Chargeback, 1, 1, None),
            // Account is now locked — these should be ignored
            make_tx(TransactionOperation::Deposit, 1, 3, Some(dec!(100.0))),
            make_tx(TransactionOperation::Withdrawal, 1, 4, Some(dec!(5.0))),
        ];

        let accounts = process_transactions(transactions);
        let a1 = accounts.get(&1).unwrap();
        assert_eq!(a1.available, dec!(10.0));
        assert_eq!(a1.held, Decimal::ZERO);
        assert!(a1.locked);
    }

    #[test]
    fn cross_client_dispute_is_rejected() {
        let transactions = vec![
            make_tx(TransactionOperation::Deposit, 1, 1, Some(dec!(10.0))),
            make_tx(TransactionOperation::Deposit, 2, 2, Some(dec!(20.0))),
            // Client 2 tries to dispute client 1's transaction
            make_tx(TransactionOperation::Dispute, 2, 1, None),
        ];

        let accounts = process_transactions(transactions);

        let a1 = accounts.get(&1).unwrap();
        assert_eq!(a1.available, dec!(10.0));
        assert_eq!(a1.held, Decimal::ZERO);

        let a2 = accounts.get(&2).unwrap();
        assert_eq!(a2.available, dec!(20.0));
        assert_eq!(a2.held, Decimal::ZERO);
    }

    #[test]
    fn resolve_without_prior_dispute_is_ignored() {
        let transactions = vec![
            make_tx(TransactionOperation::Deposit, 1, 1, Some(dec!(10.0))),
            make_tx(TransactionOperation::Resolve, 1, 1, None),
        ];

        let accounts = process_transactions(transactions);
        let a1 = accounts.get(&1).unwrap();
        assert_eq!(a1.available, dec!(10.0));
        assert_eq!(a1.held, Decimal::ZERO);
    }

    #[test]
    fn chargeback_without_prior_dispute_is_ignored() {
        let transactions = vec![
            make_tx(TransactionOperation::Deposit, 1, 1, Some(dec!(10.0))),
            make_tx(TransactionOperation::Chargeback, 1, 1, None),
        ];

        let accounts = process_transactions(transactions);
        let a1 = accounts.get(&1).unwrap();
        assert_eq!(a1.available, dec!(10.0));
        assert_eq!(a1.held, Decimal::ZERO);
        assert!(!a1.locked);
    }

    #[test]
    fn dispute_before_deposit_exists_is_ignored() {
        let transactions = vec![
            // Dispute arrives before the deposit it references
            make_tx(TransactionOperation::Dispute, 1, 1, None),
            make_tx(TransactionOperation::Deposit, 1, 1, Some(dec!(10.0))),
        ];

        let accounts = process_transactions(transactions);
        let a1 = accounts.get(&1).unwrap();
        // Dispute was a no-op, deposit still credited normally
        assert_eq!(a1.available, dec!(10.0));
        assert_eq!(a1.held, Decimal::ZERO);
    }

    #[test]
    fn two_chargebacks_on_same_client_from_different_deposits() {
        let transactions = vec![
            make_tx(TransactionOperation::Deposit, 1, 1, Some(dec!(10.0))),
            make_tx(TransactionOperation::Deposit, 1, 2, Some(dec!(5.0))),
            make_tx(TransactionOperation::Dispute, 1, 1, None),
            make_tx(TransactionOperation::Chargeback, 1, 1, None),
            // Account already locked, but dispute resolution still processes
            make_tx(TransactionOperation::Dispute, 1, 2, None),
            make_tx(TransactionOperation::Chargeback, 1, 2, None),
        ];

        let accounts = process_transactions(transactions);
        let a1 = accounts.get(&1).unwrap();
        assert_eq!(a1.available, Decimal::ZERO);
        assert_eq!(a1.held, Decimal::ZERO);
        assert_eq!(a1.total(), Decimal::ZERO);
        assert!(a1.locked);
    }

    #[test]
    fn resolve_on_already_chargebacked_tx_is_ignored() {
        let transactions = vec![
            make_tx(TransactionOperation::Deposit, 1, 1, Some(dec!(10.0))),
            make_tx(TransactionOperation::Dispute, 1, 1, None),
            make_tx(TransactionOperation::Chargeback, 1, 1, None),
            // Chargeback cleared disputed flag, so resolve sees undisputed tx
            make_tx(TransactionOperation::Resolve, 1, 1, None),
        ];

        let accounts = process_transactions(transactions);
        let a1 = accounts.get(&1).unwrap();
        assert_eq!(a1.available, Decimal::ZERO);
        assert_eq!(a1.held, Decimal::ZERO);
        assert!(a1.locked);
    }
}
