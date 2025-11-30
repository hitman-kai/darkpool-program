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

### create_drop

Creates a new drop with nullifier verification.

**Parameters:**
- `nullifier: [u8; 32]` - Unique nullifier for the drop
- `recipient: Pubkey` - Recipient's public key
- `amount: u64` - Amount to drop
- `asset_type: u8` - Asset type (0=SOL, 1=USDC)
- `expires_at: i64` - Unix timestamp when drop expires

**Accounts:**
- `drop` (PDA) - Drop account, writable
- `nullifier_account` (PDA) - Nullifier tracking, writable
- `config` (PDA) - Program config, read-only
- `rate_limit_account` (PDA) - Rate limiting, writable
- `payer` - Transaction payer, signer, writable
- `system_program` - System program

### claim_drop

Claims a drop using nullifier.

**Parameters:**
- `nullifier: [u8; 32]` - Nullifier to claim

**Accounts:**
- `drop` (PDA) - Drop account, writable
- `nullifier_account` (PDA) - Nullifier tracking, writable
- `claimer` - Claimer's wallet, signer, writable

### initialize

Initializes program configuration.

**Accounts:**
- `config` (PDA) - Config account
- `authority` - Authority signer
- `system_program` - System program

### propose_authority

Proposes a new authority with timelock delay.

### cancel_authority_proposal

Cancels a pending authority proposal.

### accept_authority

Accepts authority transfer after delay period.

### update_authority_delay

Updates the authority transfer delay period.

### expire_drop

Manually expires an active drop that has passed its expiration time.

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

[Add your license here]

