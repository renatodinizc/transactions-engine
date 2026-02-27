use std::collections::HashMap;

use crate::{TransactionOperation, TransactionRecord};
use rust_decimal::Decimal;

#[derive(Debug)]
pub struct Account {
    pub available: Decimal,
    pub held: Decimal,
    pub locked: bool,
}

impl Account {
    pub fn new() -> Self {
        Self {
            available: Decimal::ZERO,
            held: Decimal::ZERO,
            locked: false,
        }
    }

    // Having a "total" attribute could be a liability: its an attribute dependend on avaiable + held
    // and should be computed at the moment to avoid inconsistencies.
    pub fn total(&self) -> Decimal {
        self.available + self.held
    }
}

pub fn process_transactions(transactions: Vec<TransactionRecord>) -> HashMap<u16, Account> {
    // Storing accounts in a hashmap permits O(1) lookups instead
    // of using Vec<ClientAccount> which would do an O(n) lookup.
    let mut all_client_accounts: HashMap<u16, Account> = HashMap::new();

    for transaction in transactions {
        match transaction.operation {
            TransactionOperation::Deposit => {
                let client_account = all_client_accounts
                    .entry(transaction.client)
                    .or_insert_with(|| Account::new());

                deposit_op(client_account, transaction.amount);
            }
            TransactionOperation::Withdrawal => {
                let client_account = all_client_accounts
                    .entry(transaction.client)
                    .or_insert_with(|| Account::new());

                withdraw_op(client_account, transaction.amount);
            }
            _ => println!("Operation not implemented yet."),
        }
    }

    all_client_accounts
}

fn withdraw_op(account: &mut Account, amount: Option<Decimal>) {
    let amount = match amount {
        Some(a) if a > Decimal::ZERO => a,
        _ => return eprintln!("not a valid amount number to withdraw: {:?}", amount),
    };

    if account.locked {
        return;
    }

    if account.available >= amount {
        account.available -= amount;
    }
}

fn deposit_op(account: &mut Account, amount: Option<Decimal>) {
    let amount = match amount {
        Some(a) if a > Decimal::ZERO => a,
        _ => return eprintln!("not a valid amount number to deposit: {:?}", amount),
    };

    if account.locked {
        return;
    }

    account.available += amount;
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

    // -- Deposit tests --

    #[test]
    fn deposit_increases_available() {
        let mut account = Account::new();
        deposit_op(&mut account, Some(dec!(10.0)));
        assert_eq!(account.available, dec!(10.0));
        assert_eq!(account.total(), dec!(10.0));
    }

    #[test]
    fn multiple_deposits_accumulate() {
        let mut account = Account::new();
        deposit_op(&mut account, Some(dec!(1.1111)));
        deposit_op(&mut account, Some(dec!(2.2222)));
        assert_eq!(account.available, dec!(3.3333));
    }

    #[test]
    fn deposit_with_none_amount_is_ignored() {
        let mut account = Account::new();
        deposit_op(&mut account, None);
        assert_eq!(account.available, Decimal::ZERO);
    }

    #[test]
    fn deposit_with_zero_amount_is_ignored() {
        let mut account = Account::new();
        deposit_op(&mut account, Some(Decimal::ZERO));
        assert_eq!(account.available, Decimal::ZERO);
    }

    #[test]
    fn deposit_with_negative_amount_is_ignored() {
        let mut account = Account::new();
        deposit_op(&mut account, Some(dec!(-5.0)));
        assert_eq!(account.available, Decimal::ZERO);
    }

    #[test]
    fn deposit_on_locked_account_is_ignored() {
        let mut account = Account::new();
        account.locked = true;
        deposit_op(&mut account, Some(dec!(10.0)));
        assert_eq!(account.available, Decimal::ZERO);
    }

    // -- Withdrawal tests --

    #[test]
    fn withdrawal_decreases_available() {
        let mut account = Account::new();
        account.available = dec!(10.0);
        withdraw_op(&mut account, Some(dec!(3.0)));
        assert_eq!(account.available, dec!(7.0));
    }

    #[test]
    fn withdrawal_with_insufficient_funds_is_rejected() {
        let mut account = Account::new();
        account.available = dec!(2.0);
        withdraw_op(&mut account, Some(dec!(3.0)));
        assert_eq!(account.available, dec!(2.0));
    }

    #[test]
    fn withdrawal_of_exact_balance_succeeds() {
        let mut account = Account::new();
        account.available = dec!(5.0);
        withdraw_op(&mut account, Some(dec!(5.0)));
        assert_eq!(account.available, Decimal::ZERO);
    }

    #[test]
    fn withdrawal_with_none_amount_is_ignored() {
        let mut account = Account::new();
        account.available = dec!(10.0);
        withdraw_op(&mut account, None);
        assert_eq!(account.available, dec!(10.0));
    }

    #[test]
    fn withdrawal_with_negative_amount_is_ignored() {
        let mut account = Account::new();
        account.available = dec!(10.0);
        withdraw_op(&mut account, Some(dec!(-5.0)));
        assert_eq!(account.available, dec!(10.0));
    }

    #[test]
    fn withdrawal_on_locked_account_is_ignored() {
        let mut account = Account::new();
        account.available = dec!(10.0);
        account.locked = true;
        withdraw_op(&mut account, Some(dec!(5.0)));
        assert_eq!(account.available, dec!(10.0));
    }

    // -- process_transactions integration --

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
}
