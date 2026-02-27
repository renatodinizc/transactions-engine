use crate::engine::Account;
use rust_decimal::Decimal;

pub fn execute(account: &mut Account, amount: Option<Decimal>) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn withdrawal_decreases_available() {
        let mut account = Account::default();
        account.available = dec!(10.0);
        execute(&mut account, Some(dec!(3.0)));
        assert_eq!(account.available, dec!(7.0));
    }

    #[test]
    fn withdrawal_with_insufficient_funds_is_rejected() {
        let mut account = Account::default();
        account.available = dec!(2.0);
        execute(&mut account, Some(dec!(3.0)));
        assert_eq!(account.available, dec!(2.0));
    }

    #[test]
    fn withdrawal_of_exact_balance_succeeds() {
        let mut account = Account::default();
        account.available = dec!(5.0);
        execute(&mut account, Some(dec!(5.0)));
        assert_eq!(account.available, Decimal::ZERO);
    }

    #[test]
    fn withdrawal_with_none_amount_is_ignored() {
        let mut account = Account::default();
        account.available = dec!(10.0);
        execute(&mut account, None);
        assert_eq!(account.available, dec!(10.0));
    }

    #[test]
    fn withdrawal_with_negative_amount_is_ignored() {
        let mut account = Account::default();
        account.available = dec!(10.0);
        execute(&mut account, Some(dec!(-5.0)));
        assert_eq!(account.available, dec!(10.0));
    }

    #[test]
    fn withdrawal_on_locked_account_is_ignored() {
        let mut account = Account::default();
        account.available = dec!(10.0);
        account.locked = true;
        execute(&mut account, Some(dec!(5.0)));
        assert_eq!(account.available, dec!(10.0));
    }
}
