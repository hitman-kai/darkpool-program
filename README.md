# DarkDrop Program

Solana program for privacy-focused token drops using nullifier-based verification.

## Overview

DarkDrop is a custom Solana program built with Anchor that enables anonymous token drops with on-chain nullifier verification. The program prevents double-spending and enhances privacy through cryptographic nullifiers.

## Features

- Nullifier-based privacy system
- On-chain verification
- PDA account management
- Authority-controlled drop creation
- Rate limiting and spam protection
- Expiration logic
- Authority management with timelock

## Program ID

**Devnet**: `95XwPFvP6znDJN2XS4JRp29NjUNGEKDAmCRfKaZEzNfw`

## Instructions

### initialize

Initializes program configuration. Must be called once before any drops can be created.

**Accounts:**
- `config` (PDA) - Config account, seeds: `["config"]`
- `authority` - Authority signer, writable
- `system_program` - System program

**Returns:** Sets initial authority and default timelock delay (24 hours).

### create_drop

Creates a new drop with nullifier verification. Only the configured authority can create drops.

**Parameters:**
- `nullifier: [u8; 32]` - Unique 32-byte nullifier for the drop
- `recipient: Pubkey` - Recipient's public key (32 bytes)
- `amount: u64` - Amount to drop in lamports (must be > 0)
- `asset_type: u8` - Asset type (0=SOL, 1=USDC)
- `expires_at: i64` - Unix timestamp when drop expires (must be 1 minute to 30 days from now)

**Accounts:**
- `drop` (PDA) - Drop account, writable, seeds: `["drop", nullifier]`
- `nullifier_account` (PDA) - Nullifier tracking, writable, seeds: `["nullifier", nullifier]`
- `config` (PDA) - Program config, read-only, seeds: `["config"]`
- `rate_limit_account` (PDA) - Rate limiting, writable, seeds: `["rate_limit", payer]`
- `payer` - Transaction payer, signer, writable (must be authority)
- `system_program` - System program

**Validations:**
- Authority must match config authority
- Amount must be greater than 0
- Asset type must be 0 or 1
- Recipient cannot be the payer
- Expiration must be 1 minute to 30 days in the future
- Rate limit: 10-second cooldown between drops per payer

**Events:** Emits `DropCreated` event.

### claim_drop

Claims a drop using nullifier. Prevents double-spending by marking nullifier as used.

**Parameters:**
- `nullifier: [u8; 32]` - Nullifier to claim

**Accounts:**
- `drop` (PDA) - Drop account, writable, seeds: `["drop", nullifier]`
- `nullifier_account` (PDA) - Nullifier tracking, writable, seeds: `["nullifier", nullifier]`
- `claimer` - Claimer's wallet, signer, writable

**Validations:**
- Drop must be in `Active` status
- Drop must not be expired
- Nullifier must not already be used
- Nullifier must match drop's nullifier

**Events:** Emits `DropClaimed` event.

### propose_authority

Proposes a new authority with timelock delay. Current authority can propose a new authority.

**Parameters:**
- `new_authority: Pubkey` - Proposed new authority
- `delay_seconds: i64` - Delay period (15 minutes to 7 days)

**Accounts:**
- `config` (PDA) - Config account, writable
- `authority` - Current authority signer

**Validations:**
- Must be current authority
- No pending proposal can exist
- Delay must be between 15 minutes and 7 days

**Events:** Emits `AuthorityProposed` event.

### cancel_authority_proposal

Cancels a pending authority proposal.

**Accounts:**
- `config` (PDA) - Config account, writable
- `authority` - Current authority signer

**Events:** Emits `AuthorityProposalCancelled` event.

### accept_authority

Accepts authority transfer after delay period has elapsed.

**Accounts:**
- `config` (PDA) - Config account, writable
- `pending_authority` - Pending authority signer

**Validations:**
- Must be the pending authority
- Delay period must have elapsed

**Events:** Emits `AuthorityAccepted` event.

### update_authority_delay

Updates the authority transfer delay period.

**Parameters:**
- `new_delay_seconds: i64` - New delay period (15 minutes to 7 days)

**Accounts:**
- `config` (PDA) - Config account, writable
- `authority` - Current authority signer

**Events:** Emits `AuthorityDelayUpdated` event.

### expire_drop

Manually expires an active drop that has passed its expiration time. Reclaims rent from PDA accounts.

**Parameters:**
- `nullifier: [u8; 32]` - Nullifier of drop to expire

**Accounts:**
- `drop` (PDA) - Drop account, writable, closeable
- `nullifier_account` (PDA) - Nullifier account, writable, closeable
- `config` (PDA) - Config account, read-only
- `authority` - Authority signer
- `rent_collector` - Account to receive closed account rent

**Validations:**
- Must be authority
- Drop must be expired
- Drop must be active (not already claimed)
- Nullifier must not be used

**Events:** Emits `DropExpired` event.

## Data Structures

### Config Account

Global program configuration stored in a PDA.

**Layout (Borsh serialized):**
```
authority: Pubkey (32 bytes)
is_initialized: bool (1 byte)
pending_authority: Pubkey (32 bytes)
pending_authority_set_at: i64 (8 bytes)
authority_delay_seconds: i64 (8 bytes)
Total: 81 bytes + 8 (discriminator) = 89 bytes
```

**PDA Derivation:**
```rust
seeds = [b"config"]
```

### DropAccount

Stores drop metadata and status.

**Layout:**
```
nullifier: [u8; 32] (32 bytes)
recipient: Pubkey (32 bytes)
amount: u64 (8 bytes)
asset_type: u8 (1 byte)
status: DropStatus (1 byte) - Active=0, Claimed=1, Expired=2
expires_at: i64 (8 bytes)
created_at: i64 (8 bytes)
claimed_at: i64 (8 bytes)
claimer: Pubkey (32 bytes)
bump: u8 (1 byte)
Total: 131 bytes + 8 (discriminator) = 139 bytes
```

**PDA Derivation:**
```rust
seeds = [b"drop", nullifier]
```

### NullifierAccount

Tracks nullifier usage to prevent double-spending.

**Layout:**
```
nullifier: [u8; 32] (32 bytes)
is_used: bool (1 byte)
claimer: Pubkey (32 bytes)
used_at: i64 (8 bytes)
bump: u8 (1 byte)
Total: 74 bytes + 8 (discriminator) = 82 bytes
```

**PDA Derivation:**
```rust
seeds = [b"nullifier", nullifier]
```

### RateLimitAccount

Per-payer rate limiting to prevent spam.

**Layout:**
```
last_drop_at: i64 (8 bytes)
bump: u8 (1 byte)
Total: 9 bytes + 8 (discriminator) = 17 bytes
```

**PDA Derivation:**
```rust
seeds = [b"rate_limit", payer_pubkey]
```

## Events

Events are emitted for off-chain indexing and monitoring.

### DropCreated

Emitted when a drop is created.

```rust
pub struct DropCreated {
    pub nullifier: [u8; 32],
    pub recipient: Pubkey,
    pub amount: u64,
    pub asset_type: u8,
    pub expires_at: i64,
    pub payer: Pubkey,
}
```

### DropClaimed

Emitted when a drop is claimed.

```rust
pub struct DropClaimed {
    pub nullifier: [u8; 32],
    pub claimer: Pubkey,
    pub claimed_at: i64,
}
```

### DropExpired

Emitted when a drop expires.

```rust
pub struct DropExpired {
    pub nullifier: [u8; 32],
    pub recipient: Pubkey,
    pub expires_at: i64,
}
```

### AuthorityProposed

Emitted when a new authority is proposed.

```rust
pub struct AuthorityProposed {
    pub current_authority: Pubkey,
    pub pending_authority: Pubkey,
    pub delay_seconds: i64,
}
```

### AuthorityProposalCancelled

Emitted when an authority proposal is cancelled.

```rust
pub struct AuthorityProposalCancelled {
    pub authority: Pubkey,
    pub cancelled_authority: Pubkey,
}
```

### AuthorityAccepted

Emitted when authority transfer is accepted.

```rust
pub struct AuthorityAccepted {
    pub previous_authority: Pubkey,
    pub new_authority: Pubkey,
}
```

### AuthorityDelayUpdated

Emitted when authority delay is updated.

```rust
pub struct AuthorityDelayUpdated {
    pub authority: Pubkey,
    pub new_delay_seconds: i64,
}
```

## Client Integration

### TypeScript/JavaScript Example

```typescript
import * as anchor from "@coral-xyz/anchor";
import { PublicKey, Keypair } from "@solana/web3.js";
import { BN } from "@coral-xyz/anchor";

// Derive PDAs
const [configPDA] = PublicKey.findProgramAddressSync(
  [Buffer.from("config")],
  program.programId
);

const [dropPDA] = PublicKey.findProgramAddressSync(
  [Buffer.from("drop"), nullifierBuffer],
  program.programId
);

// Create drop
await program.methods
  .createDrop(
    Array.from(nullifier),
    recipient,
    new BN(amount),
    assetType,
    expiresAt
  )
  .accounts({
    drop: dropPDA,
    nullifierAccount: nullifierPDA,
    config: configPDA,
    rateLimitAccount: rateLimitPDA,
    payer: authority.publicKey,
    systemProgram: SystemProgram.programId,
  })
  .rpc();
```

See `examples/` directory for complete integration examples.

## Testing

Run tests using Anchor's test framework:

```bash
anchor test
```

Tests cover:
- Program initialization
- Drop creation and validation
- Double-claim prevention
- Rate limiting enforcement
- Input validation
- Expiration logic

See `tests/darkdrop.ts` for test implementation.

## Examples

Complete usage examples are available in the `examples/` directory:

- `create-drop.ts` - Create a drop
- `claim-drop.ts` - Claim a drop
- `full-example.ts` - End-to-end example

Run examples:

```bash
ts-node examples/full-example.ts
```

## Building

### Using Docker (Recommended)

```powershell
.\build-with-docker.ps1
```

### Local Build

```powershell
.\build-program.ps1
```

## Deployment

```powershell
.\deploy-devnet.ps1
```

## Requirements

- Rust 1.76.0+
- Anchor 0.29.0
- Solana CLI 1.18.26+
- Docker (for Docker builds)

## Security

This program includes:
- Authority access control
- Input validation
- Rate limiting (10-second cooldown per payer)
- Expiration enforcement
- Double-spend prevention via nullifiers

**Note**: This program has not undergone a security audit. Do not deploy to mainnet without completing a professional security audit.

## License

MIT License - see [LICENSE](LICENSE) file for details

