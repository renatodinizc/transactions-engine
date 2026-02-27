# Payments Engine

A toy payments engine that processes a series of transactions from a CSV, updates client accounts, handles disputes and chargebacks, and outputs the state of client accounts as a CSV.

## Usage

```bash
cargo run -- transactions.csv
```

## AI Usage Declaration

I used Claude as a pair-programming partner throughout this project for architectural discussion, code review, and guidance on edge cases. Key decisions like module structure, error handling strategy and dependency choices were mine, informed by discussion with the AI.


## Architecture

This project follows a modular, function-oriented architecture inspired by Luca Palmieri's *Zero to Production in Rust*. Rather than wrapping all state in a monolithic struct with methods, the codebase is organized as focused modules with free functions and explicit parameter passing. Structs carry methods only where they own their own invariants (e.g., `Account` balance operations).

This design choice prioritizes:
- **Explicitness**: every function's dependencies are visible in its signature
- **Testability**: functions can be tested in isolation without constructing complex state objects
- **Reviewability**: a reviewer can understand each function without tracing through `self`

Module boundaries define the architecture, following Rust's model where encapsulation comes from modules, not struct boundaries.

## Design Decisions

### Dependency choices

- **`eserde`** over `serde` for deserialization: `eserde` provides significantly more descriptive error messages during deserialization failures, making debugging malformed input much easier. This is the same approach I use in production systems. Regular `serde` is still used for serialization and where `eserde` lacks trait coverage (e.g., `rust_decimal::Decimal` uses `#[eserde(compat)]` to fall back to vanilla serde).
- **`rust_decimal`** for monetary values: IEEE 754 floating point (`f64`) introduces rounding errors that compound over many transactions. `Decimal` provides exact fixed-point arithmetic, which is essential for financial calculations.
- **`std::env::args()`** over `clap` for CLI parsing: the spec requires a single positional argument. `clap` would be the right choice for a more complex CLI, but here it would be over-engineering.

### Malformed CSV records are skipped, not fatal

Malformed records are logged to stderr and skipped. This means downstream transactions may reference skipped state (e.g., a withdrawal after a skipped deposit), potentially causing silent inconsistency for that client. In production, this would be caught by a reconciliation process.

I chose skipping over aborting because:
- A single malformed row should not halt processing for all clients in the file
- The spec's language consistently treats invalid input as "an error on our partner's side" to be ignored
- At scale (thousands of concurrent TCP streams), aborting everything on one bad row would be catastrophic
- Real payment systems follow this pattern: skip, log, reconcile later

### Disputes only apply to deposits

The spec's dispute mechanics (decrease available, increase held, total unchanged) only produce correct accounting when applied to deposits. Applying the same mechanics to a withdrawal would further decrease available on an account that already lost funds. For this reason, only deposit transactions are stored in the ledger and eligible for disputes.

In a production environment, withdrawal disputes would also be needed (e.g., unauthorized withdrawals), but would require different mechanics -- reversing the withdrawal by increasing available rather than holding funds. This is a meaningful extension that the current architecture could support by adding a transaction type field to the ledger.

## Future Considerations

### Per-client parallel processing

Transactions for different clients are independent and could be parallelized by grouping transactions per client and processing each group concurrently (e.g., using Tokio tasks). I kept sequential processing for simplicity and because the spec prioritizes maintainability over efficiency, but the architecture supports this optimization since client accounts are fully isolated from each other.
