"""Generates a large CSV file for benchmarking the payments engine."""

import random
import sys

NUM_TRANSACTIONS = 1_500_000
NUM_CLIENTS = 10_000
OUTPUT_FILE = "benchmark_transactions.csv"

random.seed(42)

# Track state to generate realistic transactions
client_deposits = {}  # client -> list of (tx_id, amount)

tx_id = 1

with open(OUTPUT_FILE, "w") as f:
    f.write("type, client, tx, amount\n")

    for _ in range(NUM_TRANSACTIONS):
        client = random.randint(1, NUM_CLIENTS)
        deposits = client_deposits.setdefault(client, [])

        # Weight towards deposits/withdrawals (90%), disputes/resolves/chargebacks (10%)
        roll = random.random()

        if roll < 0.50:
            # Deposit
            amount = round(random.uniform(0.01, 10000.0), 4)
            f.write(f"deposit, {client}, {tx_id}, {amount}\n")
            deposits.append((tx_id, amount))
            tx_id += 1

        elif roll < 0.90:
            # Withdrawal
            amount = round(random.uniform(0.01, 5000.0), 4)
            f.write(f"withdrawal, {client}, {tx_id}, {amount}\n")
            tx_id += 1

        elif roll < 0.95 and deposits:
            # Dispute a random deposit
            dep_tx, _ = random.choice(deposits)
            f.write(f"dispute, {client}, {dep_tx},\n")

        elif roll < 0.98 and deposits:
            # Resolve a random deposit
            dep_tx, _ = random.choice(deposits)
            f.write(f"resolve, {client}, {dep_tx},\n")

        else:
            # Deposit fallback (keeps tx_id moving)
            amount = round(random.uniform(0.01, 1000.0), 4)
            f.write(f"deposit, {client}, {tx_id}, {amount}\n")
            deposits.append((tx_id, amount))
            tx_id += 1

print(f"Generated {OUTPUT_FILE} with ~{NUM_TRANSACTIONS} transactions across {NUM_CLIENTS} clients")
