use csv::ReaderBuilder;
use eserde::Deserialize;
use rust_decimal::Decimal;

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

pub fn read_and_parse(file_path: &str) -> Result<Vec<TransactionRecord>, csv::Error> {
    let mut reader = ReaderBuilder::new()
        .trim(csv::Trim::All)
        .flexible(true)
        .from_path(file_path)?;

    let results = reader
        .deserialize::<TransactionRecord>()
        .filter_map(|result| match result {
            Ok(record) => Some(record),
            Err(error) => {
                eprintln!("Skipping malformed record: {error}");
                None
            }
        })
        .collect();

    Ok(results)
}
