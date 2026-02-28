use assert_cmd::cargo::cargo_bin_cmd;
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::str::{FromStr, from_utf8};

fn run_engine(args: &[&str]) -> std::process::Output {
    cargo_bin_cmd!("transactions-engine")
        .args(args)
        .output()
        .expect("Failed to execute command")
}

// ── CLI argument validation ────────────────────────────────────────

#[test]
fn no_args_exits_with_error() {
    let output = run_engine(&[]);

    assert!(!output.status.success());

    let stderr = from_utf8(&output.stderr).unwrap();
    assert!(stderr.contains("[system]"));
}

#[test]
fn multiple_args_exits_with_error() {
    let output = run_engine(&["file1.csv", "file2.csv"]);

    assert!(!output.status.success());

    let stderr = from_utf8(&output.stderr).unwrap();
    assert!(stderr.contains("[system]"));
}

#[test]
fn nonexistent_file_exits_with_error() {
    let output = run_engine(&["nonexistent.csv"]);

    assert!(!output.status.success());

    let stderr = from_utf8(&output.stderr).unwrap();
    assert!(stderr.contains("[system]"));
}

#[test]
fn empty_input_file_succeeds() {
    let path = std::env::temp_dir().join("tx_engine_test_empty.csv");
    std::fs::write(&path, "type, client, tx, amount\n").unwrap();

    let output = run_engine(&[path.to_str().unwrap()]);

    assert!(output.status.success(), "Should handle empty input gracefully");
    let stdout = from_utf8(&output.stdout).unwrap();
    assert!(stdout.trim().is_empty(), "No clients means no output rows");

    let _ = std::fs::remove_file(&path);
}

// ── Full pipeline integration ──────────────────────────────────────

struct ExpectedAccount {
    available: Decimal,
    held: Decimal,
    total: Decimal,
    locked: bool,
}

fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).unwrap()
}

fn expected_accounts() -> HashMap<u16, ExpectedAccount> {
    let entries: Vec<(u16, &str, &str, &str, bool)> = vec![
        // Basic operations
        (1, "1.5", "0", "1.5", false),
        (2, "2", "0", "2", false),
        (15, "85", "0", "85", false),
        (16, "5", "0", "5", false),
        (31, "0", "0", "0", false),
        // Dispute flows
        (3, "10", "0", "10", false),
        (4, "0", "0", "0", true),
        (12, "0", "0", "0", true),
        (17, "350", "0", "350", false),
        (26, "10", "0", "10", false),
        (33, "5", "0", "5", false),
        // Locked account behavior
        (5, "10", "0", "10", true),
        (25, "-20", "0", "-20", true),
        (18, "0", "10", "10", true),
        // Edge cases
        (6, "-7", "10", "3", false),
        (7, "20", "0", "20", false),
        (8, "10", "0", "10", false),
        (9, "10", "0", "10", false),
        (10, "10", "0", "10", false),
        (11, "10", "0", "10", false),
        (24, "5", "0", "5", false),
        (27, "0", "0", "0", true),
        (32, "10", "0", "10", false),
        // Bad & malformed data
        (13, "40", "0", "40", false),
        (28, "12", "0", "12", false),
        (29, "5", "0", "5", true),
        // Precision
        (14, "11.6244", "0", "11.6244", false),
        (22, "0.0001", "0", "0.0001", false),
        (23, "0.0002", "0", "0.0002", false),
        (30, "3.3333", "0", "3.3333", false),
        // Multi-client interleaved
        (19, "70", "0", "70", false),
        (20, "20", "0", "20", false),
        (21, "0", "0", "0", true),
        (36, "5", "0", "5", false),
        (37, "-5", "0", "-5", true),
        (38, "5", "0", "5", false),
        (39, "5", "0", "5", false),
        (40, "5", "0", "5", false),
        // Other
        (34, "-10", "0", "-10", true),
        (35, "60", "0", "60", true),
        (41, "100", "0", "100", false),
        // Long-lived accounts
        (42, "-75", "0", "-75", true),
        (43, "500", "0", "500", true),
        (44, "350", "0", "350", true),
        (45, "175", "0", "175", true),
    ];

    entries
        .into_iter()
        .map(|(client, available, held, total, locked)| {
            (
                client,
                ExpectedAccount {
                    available: dec(available),
                    held: dec(held),
                    total: dec(total),
                    locked,
                },
            )
        })
        .collect()
}

#[test]
fn full_pipeline_against_sample_data() {
    let output = run_engine(&["sample_transactions.csv"]);

    assert!(output.status.success(), "Binary exited with error");

    let stdout = from_utf8(&output.stdout).unwrap();
    let mut reader = csv::ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_reader(stdout.as_bytes());

    // Verify CSV header
    let headers = reader.headers().unwrap();
    let expected_headers: &[&str] = &["client", "available", "held", "total", "locked"];
    assert_eq!(headers, expected_headers);

    // Parse all output rows keyed by client ID
    let mut actual: HashMap<u16, (Decimal, Decimal, Decimal, bool)> = HashMap::new();
    for result in reader.records() {
        let record = result.unwrap();
        let client: u16 = record[0].parse().unwrap();
        let available = dec(&record[1]);
        let held = dec(&record[2]);
        let total = dec(&record[3]);
        let locked: bool = record[4].parse().unwrap();
        actual.insert(client, (available, held, total, locked));
    }

    let expected = expected_accounts();

    assert_eq!(
        actual.len(),
        expected.len(),
        "Expected {} clients, got {}",
        expected.len(),
        actual.len()
    );

    for (client, exp) in &expected {
        let (available, held, total, locked) = actual
            .get(client)
            .unwrap_or_else(|| panic!("Client {client} missing from output"));

        assert_eq!(
            *available, exp.available,
            "Client {client} available: expected {}, got {}",
            exp.available, available
        );
        assert_eq!(
            *held, exp.held,
            "Client {client} held: expected {}, got {}",
            exp.held, held
        );
        assert_eq!(
            *total, exp.total,
            "Client {client} total: expected {}, got {}",
            exp.total, total
        );
        assert_eq!(
            *locked, exp.locked,
            "Client {client} locked: expected {}, got {}",
            exp.locked, locked
        );
    }
}
