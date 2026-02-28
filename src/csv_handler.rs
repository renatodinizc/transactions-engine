use csv::ReaderBuilder;
use eserde::Deserialize;
use rust_decimal::Decimal;
use serde::Serialize;
use std::{collections::HashMap, io};

use crate::engine::Account;

#[derive(Deserialize, Debug)]
pub struct TransactionRecord {
    #[serde(rename = "type")]
    pub operation: TransactionOperation,
    pub client: u16,
    pub tx: u32,

    #[eserde(compat)]
    pub amount: Option<Decimal>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransactionOperation {
    Deposit,
    Withdrawal,
    Dispute,
    Resolve,
    Chargeback,
}

pub fn read_and_parse(
    file_path: &str,
) -> Result<impl Iterator<Item = TransactionRecord>, csv::Error> {
    let reader = ReaderBuilder::new()
        .trim(csv::Trim::All)
        .flexible(true)
        .from_path(file_path)?;

    let results =
        reader
            .into_deserialize::<TransactionRecord>()
            .filter_map(|result| match result {
                Ok(record) => Some(record),
                Err(error) => {
                    eprintln!("[parse] Skipping malformed record: {error}");
                    None
                }
            });

    Ok(results)
}

#[derive(Debug, Default, Serialize)]
pub struct PrintedAccount {
    pub client: u16,
    pub available: Decimal,
    pub held: Decimal,
    pub total: Decimal,
    pub locked: bool,
}

impl PrintedAccount {
    pub fn new(client: u16, account: Account) -> Self {
        let rounded_available = account.available.round_dp(4);
        let rounded_held = account.held.round_dp(4);

        Self {
            client,
            available: rounded_available,
            held: rounded_held,
            total: rounded_available + rounded_held,
            locked: account.locked,
        }
    }
}

pub fn write_accounts(accounts: HashMap<u16, Account>) -> Result<(), csv::Error> {
    let mut writer = csv::Writer::from_writer(io::stdout());

    for (client, account) in accounts {
        let printed_account = PrintedAccount::new(client, account);
        writer.serialize(&printed_account)?;
    }

    writer.flush()?; // Make sure all buffered content is written to stdout before exiting

    Ok(())
}
