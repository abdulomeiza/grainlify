# Dry-Run Simulation API

The bounty escrow contract provides read-only dry-run entrypoints for previewing lock, release, and refund operations without mutating state or performing token transfers. Use these for off-chain previews, UI flows, and integration testing.

## Purpose

- **Preview outcomes** – Determine whether an operation would succeed before submitting a real transaction
- **UI/UX** – Show users expected amounts and outcomes without executing
- **Testing** – Validate integration logic without affecting on-chain state
- **Gas estimation** – Estimate costs by simulating the call path

## Security Guarantees

- **No state mutation** – Storage is never written
- **No token transfers** – No `client.transfer` calls
- **No events** – No events emitted
- **No authorization required** – Safe for any caller (authorization: "any")

## Entrypoints

### dry_run_lock

Simulate locking funds into escrow.

**Parameters:**

| Name       | Type    | Description                    |
|------------|---------|--------------------------------|
| depositor  | Address | Address that would lock funds  |
| bounty_id  | u64     | Bounty identifier              |
| amount     | i128    | Amount to lock                 |
| deadline   | u64     | Deadline timestamp             |

**Returns:** `SimulationResult`

**Errors reflected in result:** `NotInitialized`, `FundsPaused`, `ContractDeprecated`, `ParticipantBlocked`, `ParticipantNotAllowed`, `AmountBelowMinimum`, `AmountAboveMaximum`, `BountyExists`, `InvalidAmount`, `InsufficientFunds`

---

### dry_run_release

Simulate releasing funds to a contributor.

**Parameters:**

| Name        | Type    | Description          |
|-------------|---------|----------------------|
| bounty_id   | u64     | Bounty identifier    |
| contributor | Address | Recipient address    |

**Returns:** `SimulationResult`

**Errors reflected in result:** `NotInitialized`, `FundsPaused`, `BountyNotFound`, `FundsNotLocked`, `InvalidAmount`

---

### dry_run_refund

Simulate refunding funds to the depositor (or approved recipient).

**Parameters:**

| Name      | Type | Description       |
|-----------|------|-------------------|
| bounty_id | u64  | Bounty identifier |

**Returns:** `SimulationResult`

**Errors reflected in result:** `FundsPaused`, `BountyNotFound`, `FundsNotLocked`, `ClaimPending`, `DeadlineNotPassed`, `InvalidAmount`

---

## SimulationResult

```rust
pub struct SimulationResult {
    pub success: bool,           // true if operation would succeed
    pub error_code: u32,         // 0 on success; Error enum discriminant on failure
    pub amount: i128,            // Amount involved (net after fees where applicable)
    pub resulting_status: EscrowStatus,  // Status after simulated operation
    pub remaining_amount: i128,  // Remaining in escrow after operation
}
```

- **success** – `true` when the simulated operation would succeed
- **error_code** – `0` on success; otherwise the `Error` enum value as `u32`
- **amount** – Lock: net amount stored in escrow. Release: escrow amount. Refund: refund amount
- **resulting_status** – `Locked`, `Released`, `Refunded`, or `PartiallyRefunded`
- **remaining_amount** – Lock: same as amount. Release: 0. Refund: remaining after partial refund

## Usage Examples

### Off-chain preview (Rust/SDK)

```rust
let result = escrow_client.dry_run_lock(&depositor, &bounty_id, &amount, &deadline);
if result.success {
    println!("Would lock {} (net) in escrow", result.amount);
} else {
    println!("Would fail with error code {}", result.error_code);
}
```

### RPC / Stellar SDK

Call `dry_run_lock`, `dry_run_release`, or `dry_run_refund` via `simulateTransaction` for a read-only simulation without submitting a transaction.

## Related

- [bounty-escrow-manifest.json](../../../bounty-escrow-manifest.json) – Entrypoint definitions
- [upgrade_safety.rs](src/upgrade_safety.rs) – `simulate_upgrade` for pre-upgrade checks
- [test_dry_run_simulation.rs](src/test_dry_run_simulation.rs) – Test suite
