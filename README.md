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

Each operation (deposit, withdraw, dispute, resolve, chargeback) lives in its own module despite being a single public function. This is deliberate: each operation carries 5-7 unit tests covering its edge cases. Consolidating everything into `engine.rs` would produce a large file where orchestration logic and operation-specific test coverage are interleaved. The current structure keeps the engine focused on wiring and dispatch, while each operation module owns its validation rules and test suite independently.

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

The spec's dispute mechanics (decrease available, increase held, total unchanged) only produce correct accounting when applied to deposits. Applying the same mechanics to a withdrawal would further decrease available on an account that already lost funds. Both deposits and withdrawals are stored in a shared transaction ledger, but each entry carries an `is_deposit` flag. Disputes check this flag and reject any attempt to dispute a non-deposit transaction.

In a production environment, withdrawal disputes would also be needed (e.g., unauthorized withdrawals), but would require different mechanics -- reversing the withdrawal by increasing available rather than holding funds.

### Disputes can produce negative available balances

If a client deposits funds, withdraws some, and the original deposit is later disputed, the available balance goes negative. This is intentional — a negative available balance accurately represents a liability (the client spent money they may not be entitled to). Blocking the dispute when available is insufficient would create a fraud vector: a malicious actor could deposit, immediately withdraw, and become immune to disputes. Real payment processors (Stripe, PayPal, crypto exchanges) all allow negative balances during disputes and recover through clawback mechanisms on future deposits.

### Duplicate transaction IDs are rejected

If a deposit or withdrawal arrives with a transaction ID that already exists in the ledger, the operation is rejected entirely — no balance change, no ledger overwrite. Transaction IDs are expected to be globally unique per the spec. Silently overwriting would lose the original transaction's dispute state and could double-credit or double-debit the account.

### Locked accounts block deposits and withdrawals, not dispute resolution

A chargeback locks (freezes) the client's account. Deposits and withdrawals are rejected on locked accounts, but dispute-related operations (dispute, resolve, chargeback) are still processed. Dispute resolution is corrective and administrative — blocking it would leave funds permanently trapped in held balances with no path to resolution.

## Sample Data

The file `sample_transactions.csv` contains a comprehensive test dataset generated with AI assistance. It covers 41 clients across 247 transactions, exercising every operation type, edge case, and error condition. Transactions from different clients are interleaved to simulate realistic concurrent activity while maintaining chronological order per client.

### Basic operations & spec reference

| Client | Scenario | Flow | Expected |
|--------|----------|------|----------|
| 1 | Deposits and partial withdrawal | deposit → deposit → withdrawal | avail=1.5, unlocked |
| 2 | Withdrawal exceeding balance | deposit → withdrawal (rejected) | avail=2, unlocked |
| 15 | Multiple deposits, withdrawals, one rejected | deposit ×3 → withdrawal ×3 (one rejected) → deposit → withdrawal | avail=85, unlocked |
| 16 | Exact balance withdrawals | deposit → withdrawal → deposit → withdrawal → deposit | avail=5, unlocked |
| 31 | Operations reducing balance to zero | deposit → withdrawal → deposit → withdrawal → deposit → withdrawal | avail=0, unlocked |

### Dispute flows

| Client | Scenario | Flow | Expected |
|--------|----------|------|----------|
| 3 | Dispute resolved | deposit → dispute → resolve | avail=10, unlocked |
| 4 | Dispute chargebacked | deposit → dispute → chargeback | avail=0, locked |
| 12 | Full lifecycle with re-dispute | deposit → dispute → resolve → dispute → chargeback | avail=0, locked |
| 17 | Dispute blocks withdrawal, resolve frees | deposits → withdrawals → dispute → withdrawal (rejected) → resolve → withdrawal | avail=350, unlocked |
| 26 | Triple dispute on same tx | deposit → dispute → dispute (ignored) → dispute (ignored) → resolve | avail=10, unlocked |
| 33 | Withdrawal rejected during dispute | deposit → dispute → withdrawal (rejected) → resolve → withdrawal | avail=5, unlocked |

### Locked account behavior

| Client | Scenario | Flow | Expected |
|--------|----------|------|----------|
| 5 | Locked account rejects deposit and withdrawal | deposits → dispute → chargeback → deposit (rejected) → withdrawal (rejected) | avail=10, locked |
| 25 | Locked account, dispute on withdrawal ignored | deposit → withdrawal → dispute → chargeback → deposit (rejected) → dispute on withdrawal (rejected) | avail=-20, locked |
| 18 | Multiple chargebacks, one dispute left in held | deposits ×3 → disputes → chargebacks → dispute remaining | avail=0, held=10, locked |

### Edge cases

| Client | Scenario | Flow | Expected |
|--------|----------|------|----------|
| 6 | Negative available after dispute | deposit → withdrawal → dispute | avail=-7, held=10 |
| 7 | Cross-client dispute rejected | deposit → dispute on another client's tx (rejected) | avail=20 |
| 8 | Resolve without prior dispute | deposit → resolve (ignored) | avail=10 |
| 9 | Chargeback without prior dispute | deposit → chargeback (ignored) | avail=10 |
| 10 | Duplicate deposit tx ID rejected | deposit → deposit same tx (rejected) | avail=10 |
| 11 | Dispute before deposit exists | dispute (ignored) → deposit | avail=10 |
| 24 | Duplicate tx IDs across deposit/withdrawal | deposit → deposit same tx (rejected) → withdrawal → deposit same tx as withdrawal (rejected) | avail=5 |
| 27 | Re-dispute after chargeback | deposit → dispute → chargeback → resolve (ignored) → dispute (succeeds) | avail=-10, held=10, locked |
| 32 | Dispute/resolve/chargeback on nonexistent txs | deposit → dispute/resolve/chargeback on missing tx IDs (all ignored) | avail=10 |

### Bad & malformed data

| Client | Scenario | Flow | Expected |
|--------|----------|------|----------|
| 13 | Zero, negative, and missing amounts | bad deposits (ignored ×3) → good deposit → bad withdrawals (ignored ×3) → good withdrawal | avail=40 |
| 28 | No-whitespace CSV formatting | deposit → deposit → withdrawal → dispute → resolve (no spaces) | avail=12 |
| 29 | Extra whitespace in amounts | deposit → deposit → withdrawal → dispute → chargeback (padded amounts) | avail=5, locked |

### Precision

| Client | Scenario | Flow | Expected |
|--------|----------|------|----------|
| 14 | 4-decimal place arithmetic with dispute/resolve | deposits with 4dp → withdrawal → dispute → resolve | avail=11.6244 |
| 22 | Withdrawal of nearly all funds | deposit 1000 → withdrawal 999.9999 | avail=0.0001 |
| 23 | Tiny amounts | deposits at 0.0001 scale → withdrawal | avail=0.0002 |
| 30 | Decimal precision across operations | deposits → withdrawal → dispute → resolve → withdrawal (all 4dp) | avail=3.3333 |

### Long-lived accounts (clients 42-45)

These clients simulate realistic account lifecycles with multiple rounds of normal operations interspersed with dispute cycles. Their transactions are interleaved across clients to simulate concurrent activity.

**Client 42** — Normal ops → dispute resolved → more ops → dispute chargebacked → post-lock activity
- Deposits and withdrawals build up activity, first dispute on a 200 deposit is resolved, operations continue, then a 500 deposit is disputed and chargebacked leaving negative balance. After lock: multiple deposits and withdrawals rejected, a previous deposit is re-disputed and resolved (dispute resolution still works on locked accounts), more deposits/withdrawals rejected.
- Expected: avail=-75, locked

**Client 43** — Normal ops → dispute resolved → normal ops → chargeback → dispute activity on locked → post-lock rejections
- Large initial deposit disputed and resolved, then normal withdrawals, a smaller deposit is chargebacked (locks account). After lock: deposits and withdrawals rejected, a previously resolved deposit is re-disputed and chargebacked (removing another 50 from total), more deposits/withdrawals rejected.
- Expected: avail=500, locked

**Client 44** — Two simultaneous disputes, one resolved one chargebacked, then dispute resolved on locked account
- Three deposits, two disputed simultaneously, one resolved and one chargebacked (locks), further deposit/withdrawal rejected, third deposit disputed and resolved while locked.
- Expected: avail=350, locked

**Client 45** — Three dispute cycles: resolved → chargebacked → resolved on locked
- Active account with deposits/withdrawals between each dispute cycle. First dispute resolved (funds return), second chargebacked (locks account), third dispute resolved on the locked account. Deposits and withdrawals after lock are rejected.
- Expected: avail=175, locked

### Multi-client interleaved operations

| Client | Scenario | Expected |
|--------|----------|----------|
| 19-21 | Three clients with interleaved deposits, withdrawals, and a chargeback on client 21 | 19: avail=70; 20: avail=20; 21: avail=0, locked |
| 36-40 | Five clients with identical setup, disputes on 36 and 37, resolve on 36, chargeback on 37 | 36: avail=5; 37: avail=-5, locked; 38-40: avail=5 |

## Future Considerations

### Per-client parallel processing

Transactions for different clients are independent and could be parallelized by grouping transactions per client and processing each group concurrently (e.g., using Tokio tasks). I kept sequential processing for simplicity and because the spec prioritizes maintainability over efficiency, but the architecture supports this optimization since client accounts are fully isolated from each other.
