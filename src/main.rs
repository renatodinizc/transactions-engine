use std::{env, process};
use transactions_engine::csv_handler;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        eprintln!("ERROR: Missing <transactions.csv> arg");
        process::exit(1);
    }

    let user_input = &args[1];

    let parsed_csv = csv_handler::read_and_parse(user_input).map_err(|err| {
        eprintln!("Error reading CSV: {err}");
        process::exit(1)
    });

    println!("parsed_csv: {:?}", parsed_csv);
}
