use std::{env, process};
use transactions_engine::{csv_handler, engine};

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        eprintln!("ERROR: Missing <transactions.csv> arg");
        process::exit(1);
    }

    let user_input = &args[1];

    let transactions = csv_handler::read_and_parse(user_input)
        .map_err(|err| {
            eprintln!("Error reading CSV: {err}");
            process::exit(1)
        })
        .unwrap();

    let client_accounts = engine::process_transactions(transactions);

    for client in client_accounts.keys() {
        println!("client: {client}");
        println!("account: {:?}", client_accounts.get(client));
    }
}
