# DARKPOOL PROGRAM

```
██████╗  █████╗ ██████╗ ██╗  ██╗██████╗  ██████╗  ██████╗ ██╗     
██╔══██╗██╔══██╗██╔══██╗██║ ██╔╝██╔══██╗██╔═══██╗██╔═══██╗██║     
██║  ██║███████║██████╔╝█████╔╝ ██████╔╝██║   ██║██║   ██║██║     
██║  ██║██╔══██║██╔══██╗██╔═██╗ ██╔═══╝ ██║   ██║██║   ██║██║     
██████╔╝██║  ██║██║  ██║██║  ██╗██║     ╚██████╔╝╚██████╔╝███████╗
╚═════╝ ╚═╝  ╚═╝╚═╝  ╚═╝╚═╝  ╚═╝╚═╝      ╚═════╝  ╚═════╝ ╚══════╝
```

**On-chain coordination for DarkPool drops**

Nullifier-verified SOL drops with rate limits, vaults, and authority controls.

---

## Overview

DarkPool Program is an Anchor-based Solana program that manages SOL vaults, drop metadata, and nullifier state for DarkDrop's pool-like flows. It enforces double-spend prevention, expiration windows, fee routing, and authority-guarded drop creation.

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                            DARKPOOL PROGRAM                                  │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌─────────────┐    create_drop    ┌────────────────────┐                   │
│  │  AUTHORITY  │──────────────────►│   DROP PDA         │                   │
│  │  (SERVICE)  │                   │  (metadata only)   │                   │
│  └──────┬──────┘                   └──────────┬─────────┘                   │
│         │                                     │                             │
│         ▼                                     ▼                             │
│  ┌─────────────┐                      ┌────────────────┐                    │
│  │ NULLIFIER   │◄────────────────────►│ NULLIFIER PDA  │                    │
│  │  DERIVE     │     claim_drop       │ (spent flags)  │                    │
│  └─────────────┘                      └────────────────┘                    │
│                                                                             │
│  Notes: Funds are held in vault PDAs. Privacy rails remain off-chain and    │
│  integrate with this program for deposit/claim orchestration.              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## State Machines

### Drop Lifecycle

```
           ┌────────────┐
           │   NONE     │
           └─────┬──────┘
                 │ create_drop
                 ▼
           ┌────────────┐
           │   ACTIVE   │
           └─────┬──────┘
          claim  │   expire
                 ▼
    ┌────────────┴────────────┐
    │                         │
┌───────────┐           ┌───────────┐
│  CLAIMED  │           │  EXPIRED  │
└───────────┘           └───────────┘
```

### Nullifier State

```
    ┌──────────────┐         ┌──────────────┐
    │   UNUSED     │────────►│     USED     │
    └──────────────┘  claim  └──────────────┘
```

### Authority Transfer

```
┌────────────┐  propose   ┌──────────────┐  accept  ┌────────────┐
│  AUTHORITY │──────────►│  PENDING      │────────►│  AUTHORITY │
└────────────┘           └──────────────┘         └────────────┘
         │ cancel
         ▼
   ┌────────────┐
   │  AUTHORITY │
   └────────────┘
```

---

## Program ID

**Devnet**: `95XwPFvP6znDJN2XS4JRp29NjUNGEKDAmCRfKaZEzNfw`

---

## Instructions

### initialize

Initializes program configuration. Must be called once before any drops can be created.

**Accounts**
- `config` (PDA) - seeds: `["config"]`
- `authority` - signer, payer
- `system_program`

---

### create_drop

Creates a new drop record and nullifier entry (authority-only flow).

**Params**
- `nullifier: [u8; 32]`
- `recipient: Pubkey`
- `amount: u64`
- `asset_type: u8` (0=SOL, 1=USDC)
- `expires_at: i64`

**Accounts**
- `drop` (PDA) - seeds: `["drop", nullifier]`
- `nullifier_account` (PDA) - seeds: `["nullifier", nullifier]`
- `config` (PDA)
- `rate_limit_account` (PDA) - seeds: `["rate_limit", payer]`
- `payer` - signer
- `system_program`

**Validations**
- Authority matches config
- Amount > 0
- Asset type supported
- Recipient differs from payer
- Expiration 1 minute to 30 days in future
- 10s rate limit per payer

---

### deposit_pool

Deposits SOL into the vault and creates a drop record.

**Params**
- `nullifier: [u8; 32]`
- `amount: u64`
- `asset_type: u8` (0=SOL)
- `expires_at: i64` (0 disables expiry)

---

### claim_drop

Transfers from vaults to the claimer, applies fees, and marks the nullifier used.

**Params**
- `nullifier: [u8; 32]`

**Accounts**
- `drop` (PDA)
- `nullifier_account` (PDA)
- `claimer` - signer

**Validations**
- Drop is Active
- Not expired
- Nullifier unused
- Nullifier matches drop

---

### expire_drop

Expires an active drop that has passed its expiration window.

**Params**
- `nullifier: [u8; 32]`

**Accounts**
- `drop` (PDA, close)
- `nullifier_account` (PDA, close)
- `config` (PDA)
- `authority` - signer
- `rent_collector`

---

### Authority Management

- `propose_authority`
- `cancel_authority_proposal`
- `accept_authority`
- `update_authority_delay`

---

## Accounts

### Config

```
authority: Pubkey
is_initialized: bool
pending_authority: Pubkey
pending_authority_set_at: i64
authority_delay_seconds: i64
usdc_mint: Pubkey
treasury: Pubkey
fee_bps: u16
sol_vault_bump: u8
usdc_vault_bump: u8
vault_authority_bump: u8
```

### DropAccount

```
nullifier: [u8; 32]
recipient: Pubkey
amount: u64
asset_type: u8
status: DropStatus
expires_at: i64
created_at: i64
claimed_at: i64
claimer: Pubkey
bump: u8
```

### NullifierAccount

```
nullifier: [u8; 32]
is_used: bool
claimer: Pubkey
used_at: i64
bump: u8
```

### RateLimitAccount

```
last_drop_at: i64
bump: u8
```

---

## Client Example (TypeScript)

```typescript
import * as anchor from "@coral-xyz/anchor";
import { PublicKey, SystemProgram } from "@solana/web3.js";
import { BN } from "@coral-xyz/anchor";

const program = anchor.workspace.Darkpool;

const [configPDA] = PublicKey.findProgramAddressSync(
  [Buffer.from("config")],
  program.programId
);

await program.methods
  .depositPool(
    Array.from(nullifier),
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
    payerToken: authority.publicKey,
    solVault: solVaultPDA,
    usdcVault: usdcVaultPDA,
    usdcMint,
    systemProgram: SystemProgram.programId,
    tokenProgram: anchor.utils.token.TOKEN_PROGRAM_ID,
  })
  .rpc();
```

---

## Testing

```bash
anchor test
```

Tests cover:
- Initialization
- Drop creation
- Double-claim prevention
- Rate limit enforcement
- Input validation
- Expiration handling

See `tests/darkpool.ts` for implementation.

---

## Build

```powershell
.\build-with-docker.ps1
```

```powershell
.\build-program.ps1
```

---

## Operational Notes

- Use `anchor build --no-idl` for local builds. IDL generation is deferred to a separate tooling phase.
- Account validation is implemented manually (not `#[derive(Accounts)]`) to avoid macro path-resolution issues and keep the on-chain logic explicit and auditable.

---

## Deployment

```powershell
.\deploy-devnet.ps1
```

---

## Security Notes

- This program enforces SOL vault transfers and nullifier state.
- Privacy rails remain off-chain; vault flows do not hide amounts on-chain.
- No security audit has been completed. Do not deploy to mainnet without a professional review.

---

## License

MIT License - see [LICENSE](LICENSE) for details.
