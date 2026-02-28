use std::{env, process};
use transactions_engine::{csv_handler, engine};

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        eprintln!("[system] Missing <transactions.csv> argument");
        process::exit(1);
    }

    let user_input = &args[1];

    let transactions = match csv_handler::read_and_parse(user_input) {
        Ok(t) => t,
        Err(err) => {
            eprintln!("[system] Error reading CSV: {err}");
            process::exit(1);
        }
    };

    let client_accounts = engine::process_transactions(transactions);

    if let Err(err) = csv_handler::write_accounts(client_accounts) {
        eprintln!("[system] Error writing CSV: {err}");
        process::exit(1);
    }
}
