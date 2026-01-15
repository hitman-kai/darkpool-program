use std::collections::BTreeSet;

use anchor_lang::prelude::*;
use anchor_lang::AccountsExit;
use anchor_lang::Bumps;
use anchor_lang::system_program;

const MAX_EXPIRATION_WINDOW: i64 = 30 * 24 * 60 * 60; // 30 days
const MIN_EXPIRATION_WINDOW: i64 = 60; // 1 minute
const MIN_RATE_LIMIT_SECONDS: i64 = 10;
const MIN_AUTHORITY_DELAY_SECONDS: i64 = 15 * 60; // 15 minutes
const MAX_AUTHORITY_DELAY_SECONDS: i64 = 7 * 24 * 60 * 60; // 7 days
const DEFAULT_AUTHORITY_DELAY_SECONDS: i64 = 24 * 60 * 60; // 24 hours
const MAX_FEE_BPS: u16 = 1000; // 10%

declare_id!("95XwPFvP6znDJN2XS4JRp29NjUNGEKDAmCRfKaZEzNfw");

#[program]
pub mod darkpool {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>, treasury: Pubkey, fee_bps: u16) -> Result<()> {
        require!(treasury != Pubkey::default(), DarkPoolError::InvalidTreasury);
        require!(fee_bps <= MAX_FEE_BPS, DarkPoolError::InvalidFeeBps);

        let program_id = ctx.program_id;
        let config_info = ctx.accounts.config.to_account_info();
        let authority_info = ctx.accounts.authority.to_account_info();
        let sol_vault_info = ctx.accounts.sol_vault.to_account_info();
        let system_program_info = ctx.accounts.system_program.to_account_info();

        let (config_pda, config_bump) = Pubkey::find_program_address(&[b"config"], program_id);
        let (sol_vault_pda, sol_vault_bump) =
            Pubkey::find_program_address(&[b"sol_vault"], program_id);

        require_keys_eq!(config_pda, *config_info.key, ErrorCode::ConstraintSeeds);
        require_keys_eq!(sol_vault_pda, *sol_vault_info.key, ErrorCode::ConstraintSeeds);
        require_keys_eq!(
            system_program::ID,
            *system_program_info.key,
            ErrorCode::ConstraintAddress
        );
        require!(authority_info.is_writable, ErrorCode::ConstraintMut);
        require!(config_info.is_writable, ErrorCode::ConstraintMut);
        require!(sol_vault_info.is_writable, ErrorCode::ConstraintMut);

        require!(
            config_info.owner == &system_program::ID && config_info.lamports() == 0,
            DarkPoolError::AccountAlreadyInitialized
        );
        require!(
            sol_vault_info.owner == &system_program::ID && sol_vault_info.lamports() == 0,
            DarkPoolError::AccountAlreadyInitialized
        );

        let config_space = (8 + Config::LEN) as u64;
        let config_lamports = Rent::get()?.minimum_balance(config_space as usize);
        let config_seeds: &[&[u8]] = &[b"config", &[config_bump]];
        let sol_vault_seeds: &[&[u8]] = &[b"sol_vault", &[sol_vault_bump]];

        system_program::create_account(
            CpiContext::new_with_signer(
                system_program_info.clone(),
                system_program::CreateAccount {
                    from: authority_info.clone(),
                    to: config_info.clone(),
                },
                &[config_seeds],
            ),
            config_lamports,
            config_space,
            program_id,
        )?;

        system_program::create_account(
            CpiContext::new_with_signer(
                system_program_info,
                system_program::CreateAccount {
                    from: authority_info.clone(),
                    to: sol_vault_info.clone(),
                },
                &[sol_vault_seeds],
            ),
            Rent::get()?.minimum_balance(0),
            0,
            &system_program::ID,
        )?;

        let config_state = Config {
            authority: ctx.accounts.authority.key(),
            is_initialized: true,
            pending_authority: Pubkey::default(),
            pending_authority_set_at: 0,
            authority_delay_seconds: DEFAULT_AUTHORITY_DELAY_SECONDS,
            treasury,
            fee_bps,
            sol_vault_bump,
        };
        let mut data = config_info.try_borrow_mut_data()?;
        let mut cursor: &mut [u8] = &mut data;
        config_state.try_serialize(&mut cursor)?;

        Ok(())
    }

    pub fn create_drop(
        ctx: Context<CreateDrop>,
        nullifier: [u8; 32],
        recipient: Pubkey,
        amount: u64,
        asset_type: u8,
        expires_at: i64,
    ) -> Result<()> {
        let clock = Clock::get()?;
        let now = clock.unix_timestamp;
        let program_id = ctx.program_id;

        let (drop_pda, drop_bump) =
            Pubkey::find_program_address(&[b"drop", nullifier.as_ref()], program_id);
        let (nullifier_pda, nullifier_bump) =
            Pubkey::find_program_address(&[b"nullifier", nullifier.as_ref()], program_id);
        let payer_key = ctx.accounts.payer.key();
        let (rate_limit_pda, rate_limit_bump) =
            Pubkey::find_program_address(&[b"rate_limit", payer_key.as_ref()], program_id);
        let (config_pda, _) = Pubkey::find_program_address(&[b"config"], program_id);

        let drop_info = ctx.accounts.drop.to_account_info();
        let nullifier_info = ctx.accounts.nullifier_account.to_account_info();
        let rate_limit_info = ctx.accounts.rate_limit_account.to_account_info();
        let config_info = ctx.accounts.config.to_account_info();
        let system_program_info = ctx.accounts.system_program.to_account_info();

        require_keys_eq!(drop_pda, *drop_info.key, ErrorCode::ConstraintSeeds);
        require_keys_eq!(
            nullifier_pda,
            *nullifier_info.key,
            ErrorCode::ConstraintSeeds
        );
        require_keys_eq!(
            rate_limit_pda,
            *rate_limit_info.key,
            ErrorCode::ConstraintSeeds
        );
        require_keys_eq!(config_pda, *config_info.key, ErrorCode::ConstraintSeeds);
        require_keys_eq!(
            system_program::ID,
            *system_program_info.key,
            ErrorCode::ConstraintAddress
        );
        require!(drop_info.is_writable, ErrorCode::ConstraintMut);
        require!(nullifier_info.is_writable, ErrorCode::ConstraintMut);
        require!(rate_limit_info.is_writable, ErrorCode::ConstraintMut);
        require!(
            ctx.accounts.payer.to_account_info().is_writable,
            ErrorCode::ConstraintMut
        );

        require!(
            ctx.accounts.config.is_initialized,
            DarkPoolError::ConfigNotInitialized
        );
        require!(
            ctx.accounts.config.authority == ctx.accounts.payer.key(),
            DarkPoolError::UnauthorizedCreator
        );
        require!(amount > 0, DarkPoolError::InvalidAmount);
        require!(asset_type == 0, DarkPoolError::InvalidAssetType);
        require!(
            recipient != ctx.accounts.payer.key(),
            DarkPoolError::InvalidRecipient
        );
        require!(
            expires_at > now
                && expires_at - now >= MIN_EXPIRATION_WINDOW
                && expires_at - now <= MAX_EXPIRATION_WINDOW,
            DarkPoolError::InvalidExpiration
        );

        let mut rate_limit_state: RateLimitAccount;
        if rate_limit_info.owner == program_id {
            let mut data_slice: &[u8] = &rate_limit_info.try_borrow_data()?;
            rate_limit_state = RateLimitAccount::try_deserialize(&mut data_slice)?;
            require_eq!(
                rate_limit_state.bump,
                rate_limit_bump,
                ErrorCode::ConstraintSeeds
            );
        } else {
            require!(
                rate_limit_info.owner == &system_program::ID && rate_limit_info.lamports() == 0,
                DarkPoolError::AccountAlreadyInitialized
            );
            let space = (8 + RateLimitAccount::LEN) as u64;
            let lamports = Rent::get()?.minimum_balance(space as usize);
            let seeds: &[&[u8]] = &[b"rate_limit", payer_key.as_ref(), &[rate_limit_bump]];
            system_program::create_account(
                CpiContext::new_with_signer(
                    system_program_info.clone(),
                    system_program::CreateAccount {
                        from: ctx.accounts.payer.to_account_info(),
                        to: rate_limit_info.clone(),
                    },
                    &[seeds],
                ),
                lamports,
                space,
                program_id,
            )?;
            rate_limit_state = RateLimitAccount {
                last_drop_at: 0,
                bump: rate_limit_bump,
            };
        }

        if rate_limit_state.last_drop_at != 0 {
            require!(
                now - rate_limit_state.last_drop_at >= MIN_RATE_LIMIT_SECONDS,
                DarkPoolError::RateLimitExceeded
            );
        }
        rate_limit_state.last_drop_at = now;
        rate_limit_state.bump = rate_limit_bump;
        let mut rate_limit_data = rate_limit_info.try_borrow_mut_data()?;
        let mut rate_limit_cursor: &mut [u8] = &mut rate_limit_data;
        rate_limit_state.try_serialize(&mut rate_limit_cursor)?;

        require!(
            drop_info.owner == &system_program::ID && drop_info.lamports() == 0,
            DarkPoolError::AccountAlreadyInitialized
        );
        let drop_space = (8 + DropAccount::LEN) as u64;
        let drop_lamports = Rent::get()?.minimum_balance(drop_space as usize);
        let drop_seeds: &[&[u8]] = &[b"drop", nullifier.as_ref(), &[drop_bump]];
        system_program::create_account(
            CpiContext::new_with_signer(
                system_program_info.clone(),
                system_program::CreateAccount {
                    from: ctx.accounts.payer.to_account_info(),
                    to: drop_info.clone(),
                },
                &[drop_seeds],
            ),
            drop_lamports,
            drop_space,
            program_id,
        )?;

        let drop_state = DropAccount {
            nullifier,
            recipient,
            amount,
            asset_type,
            created_at: now,
            expires_at,
            status: DropStatus::Active,
            claimed_at: 0,
            claimer: Pubkey::default(),
            bump: drop_bump,
        };
        let mut drop_data = drop_info.try_borrow_mut_data()?;
        let mut drop_cursor: &mut [u8] = &mut drop_data;
        drop_state.try_serialize(&mut drop_cursor)?;

        let mut nullifier_state: NullifierAccount;
        if nullifier_info.owner == program_id {
            let mut data_slice: &[u8] = &nullifier_info.try_borrow_data()?;
            nullifier_state = NullifierAccount::try_deserialize(&mut data_slice)?;
            require_eq!(
                nullifier_state.bump,
                nullifier_bump,
                ErrorCode::ConstraintSeeds
            );
        } else {
            require!(
                nullifier_info.owner == &system_program::ID && nullifier_info.lamports() == 0,
                DarkPoolError::AccountAlreadyInitialized
            );
            let space = (8 + NullifierAccount::LEN) as u64;
            let lamports = Rent::get()?.minimum_balance(space as usize);
            let seeds: &[&[u8]] = &[b"nullifier", nullifier.as_ref(), &[nullifier_bump]];
            system_program::create_account(
                CpiContext::new_with_signer(
                    system_program_info,
                    system_program::CreateAccount {
                        from: ctx.accounts.payer.to_account_info(),
                        to: nullifier_info.clone(),
                    },
                    &[seeds],
                ),
                lamports,
                space,
                program_id,
            )?;
            nullifier_state = NullifierAccount {
                nullifier: [0u8; 32],
                is_used: false,
                claimer: Pubkey::default(),
                used_at: 0,
                bump: nullifier_bump,
            };
        }

        if nullifier_state.nullifier == [0u8; 32] {
            nullifier_state.nullifier = nullifier;
            nullifier_state.is_used = false;
            nullifier_state.claimer = Pubkey::default();
            nullifier_state.used_at = 0;
            nullifier_state.bump = nullifier_bump;
        } else {
            require!(
                nullifier_state.nullifier == nullifier,
                DarkPoolError::InvalidNullifier
            );
        }
        let mut nullifier_data = nullifier_info.try_borrow_mut_data()?;
        let mut nullifier_cursor: &mut [u8] = &mut nullifier_data;
        nullifier_state.try_serialize(&mut nullifier_cursor)?;

        emit!(DropCreated {
            nullifier,
            recipient,
            amount,
            asset_type,
            expires_at,
            payer: ctx.accounts.payer.key(),
        });

        msg!(
            "Drop created: nullifier={:?}, recipient={}",
            nullifier,
            recipient
        );
        Ok(())
    }

    pub fn deposit_pool(
        ctx: Context<DepositPool>,
        nullifier: [u8; 32],
        amount: u64,
        asset_type: u8,
        expires_at: i64,
    ) -> Result<()> {
        let clock = Clock::get()?;
        let now = clock.unix_timestamp;
        let program_id = ctx.program_id;

        let (drop_pda, drop_bump) =
            Pubkey::find_program_address(&[b"drop", nullifier.as_ref()], program_id);
        let (nullifier_pda, nullifier_bump) =
            Pubkey::find_program_address(&[b"nullifier", nullifier.as_ref()], program_id);
        let (rate_limit_pda, rate_limit_bump) = Pubkey::find_program_address(
            &[b"rate_limit", ctx.accounts.payer.key().as_ref()],
            program_id,
        );
        let (config_pda, _) = Pubkey::find_program_address(&[b"config"], program_id);
        let (sol_vault_pda, _) = Pubkey::find_program_address(&[b"sol_vault"], program_id);

        let drop_info = ctx.accounts.drop.to_account_info();
        let nullifier_info = ctx.accounts.nullifier_account.to_account_info();
        let rate_limit_info = ctx.accounts.rate_limit_account.to_account_info();
        let config_info = ctx.accounts.config.to_account_info();
        let sol_vault_info = ctx.accounts.sol_vault.to_account_info();
        let system_program_info = ctx.accounts.system_program.to_account_info();

        require_keys_eq!(drop_pda, *drop_info.key, ErrorCode::ConstraintSeeds);
        require_keys_eq!(
            nullifier_pda,
            *nullifier_info.key,
            ErrorCode::ConstraintSeeds
        );
        require_keys_eq!(
            rate_limit_pda,
            *rate_limit_info.key,
            ErrorCode::ConstraintSeeds
        );
        require_keys_eq!(config_pda, *config_info.key, ErrorCode::ConstraintSeeds);
        require_keys_eq!(
            sol_vault_pda,
            *sol_vault_info.key,
            ErrorCode::ConstraintSeeds
        );
        require_keys_eq!(
            system_program::ID,
            *system_program_info.key,
            ErrorCode::ConstraintAddress
        );
        require!(drop_info.is_writable, ErrorCode::ConstraintMut);
        require!(nullifier_info.is_writable, ErrorCode::ConstraintMut);
        require!(rate_limit_info.is_writable, ErrorCode::ConstraintMut);
        require!(sol_vault_info.is_writable, ErrorCode::ConstraintMut);
        require!(
            ctx.accounts.payer.to_account_info().is_writable,
            ErrorCode::ConstraintMut
        );

        require!(
            ctx.accounts.config.is_initialized,
            DarkPoolError::ConfigNotInitialized
        );
        require!(amount > 0, DarkPoolError::InvalidAmount);
        require!(asset_type <= 1, DarkPoolError::InvalidAssetType);
        if expires_at != 0 {
            require!(
                expires_at > now
                    && expires_at - now >= MIN_EXPIRATION_WINDOW
                    && expires_at - now <= MAX_EXPIRATION_WINDOW,
                DarkPoolError::InvalidExpiration
            );
        }

        let mut rate_limit_state: RateLimitAccount;
        if rate_limit_info.owner == program_id {
            let mut data_slice: &[u8] = &rate_limit_info.try_borrow_data()?;
            rate_limit_state = RateLimitAccount::try_deserialize(&mut data_slice)?;
            require_eq!(
                rate_limit_state.bump,
                rate_limit_bump,
                ErrorCode::ConstraintSeeds
            );
        } else {
            require!(
                rate_limit_info.owner == &system_program::ID && rate_limit_info.lamports() == 0,
                DarkPoolError::AccountAlreadyInitialized
            );
            let space = (8 + RateLimitAccount::LEN) as u64;
            let lamports = Rent::get()?.minimum_balance(space as usize);
            let payer_key = ctx.accounts.payer.key();
            let seeds: &[&[u8]] = &[b"rate_limit", payer_key.as_ref(), &[rate_limit_bump]];
            system_program::create_account(
                CpiContext::new_with_signer(
                    system_program_info.clone(),
                    system_program::CreateAccount {
                        from: ctx.accounts.payer.to_account_info(),
                        to: rate_limit_info.clone(),
                    },
                    &[seeds],
                ),
                lamports,
                space,
                program_id,
            )?;
            rate_limit_state = RateLimitAccount {
                last_drop_at: 0,
                bump: rate_limit_bump,
            };
        }

        if rate_limit_state.last_drop_at != 0 {
            require!(
                now - rate_limit_state.last_drop_at >= MIN_RATE_LIMIT_SECONDS,
                DarkPoolError::RateLimitExceeded
            );
        }
        rate_limit_state.last_drop_at = now;
        rate_limit_state.bump = rate_limit_bump;
        let mut rate_limit_data = rate_limit_info.try_borrow_mut_data()?;
        let mut rate_limit_cursor: &mut [u8] = &mut rate_limit_data;
        rate_limit_state.try_serialize(&mut rate_limit_cursor)?;

        let ix = anchor_lang::solana_program::system_instruction::transfer(
            &ctx.accounts.payer.key(),
            &ctx.accounts.sol_vault.key(),
            amount,
        );
        anchor_lang::solana_program::program::invoke(
            &ix,
            &[
                ctx.accounts.payer.to_account_info(),
                ctx.accounts.sol_vault.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
        )?;

        require!(
            drop_info.owner == &system_program::ID && drop_info.lamports() == 0,
            DarkPoolError::AccountAlreadyInitialized
        );
        let drop_space = (8 + DropAccount::LEN) as u64;
        let drop_lamports = Rent::get()?.minimum_balance(drop_space as usize);
        let drop_seeds: &[&[u8]] = &[b"drop", nullifier.as_ref(), &[drop_bump]];
        system_program::create_account(
            CpiContext::new_with_signer(
                system_program_info.clone(),
                system_program::CreateAccount {
                    from: ctx.accounts.payer.to_account_info(),
                    to: drop_info.clone(),
                },
                &[drop_seeds],
            ),
            drop_lamports,
            drop_space,
            program_id,
        )?;

        let drop_state = DropAccount {
            nullifier,
            recipient: ctx.accounts.payer.key(),
            amount,
            asset_type,
            created_at: now,
            expires_at,
            status: DropStatus::Active,
            claimed_at: 0,
            claimer: Pubkey::default(),
            bump: drop_bump,
        };
        let mut drop_data = drop_info.try_borrow_mut_data()?;
        let mut drop_cursor: &mut [u8] = &mut drop_data;
        drop_state.try_serialize(&mut drop_cursor)?;

        let mut nullifier_state: NullifierAccount;
        if nullifier_info.owner == program_id {
            let mut data_slice: &[u8] = &nullifier_info.try_borrow_data()?;
            nullifier_state = NullifierAccount::try_deserialize(&mut data_slice)?;
            require_eq!(
                nullifier_state.bump,
                nullifier_bump,
                ErrorCode::ConstraintSeeds
            );
        } else {
            require!(
                nullifier_info.owner == &system_program::ID && nullifier_info.lamports() == 0,
                DarkPoolError::AccountAlreadyInitialized
            );
            let space = (8 + NullifierAccount::LEN) as u64;
            let lamports = Rent::get()?.minimum_balance(space as usize);
            let seeds: &[&[u8]] = &[b"nullifier", nullifier.as_ref(), &[nullifier_bump]];
            system_program::create_account(
                CpiContext::new_with_signer(
                    system_program_info,
                    system_program::CreateAccount {
                        from: ctx.accounts.payer.to_account_info(),
                        to: nullifier_info.clone(),
                    },
                    &[seeds],
                ),
                lamports,
                space,
                program_id,
            )?;
            nullifier_state = NullifierAccount {
                nullifier: [0u8; 32],
                is_used: false,
                claimer: Pubkey::default(),
                used_at: 0,
                bump: nullifier_bump,
            };
        }

        if nullifier_state.nullifier == [0u8; 32] {
            nullifier_state.nullifier = nullifier;
            nullifier_state.is_used = false;
            nullifier_state.claimer = Pubkey::default();
            nullifier_state.used_at = 0;
            nullifier_state.bump = nullifier_bump;
        } else {
            require!(
                nullifier_state.nullifier == nullifier,
                DarkPoolError::InvalidNullifier
            );
        }
        let mut nullifier_data = nullifier_info.try_borrow_mut_data()?;
        let mut nullifier_cursor: &mut [u8] = &mut nullifier_data;
        nullifier_state.try_serialize(&mut nullifier_cursor)?;

        emit!(DropCreated {
            nullifier,
            recipient: ctx.accounts.payer.key(),
            amount,
            asset_type,
            expires_at,
            payer: ctx.accounts.payer.key(),
        });

        Ok(())
    }

    pub fn claim_drop(ctx: Context<ClaimDrop>, nullifier: [u8; 32]) -> Result<()> {
        let clock = Clock::get()?;
        let now = clock.unix_timestamp;
        let program_id = ctx.program_id;

        let (drop_pda, drop_bump) =
            Pubkey::find_program_address(&[b"drop", nullifier.as_ref()], program_id);
        let (nullifier_pda, nullifier_bump) =
            Pubkey::find_program_address(&[b"nullifier", nullifier.as_ref()], program_id);
        let (config_pda, _) = Pubkey::find_program_address(&[b"config"], program_id);
        let (sol_vault_pda, _) = Pubkey::find_program_address(&[b"sol_vault"], program_id);

        let drop_info = ctx.accounts.drop.to_account_info();
        let nullifier_info = ctx.accounts.nullifier_account.to_account_info();
        let config_info = ctx.accounts.config.to_account_info();
        let sol_vault_info = ctx.accounts.sol_vault.to_account_info();
        let treasury_info = ctx.accounts.treasury.to_account_info();
        let system_program_info = ctx.accounts.system_program.to_account_info();

        require_keys_eq!(drop_pda, *drop_info.key, ErrorCode::ConstraintSeeds);
        require_keys_eq!(
            nullifier_pda,
            *nullifier_info.key,
            ErrorCode::ConstraintSeeds
        );
        require_keys_eq!(config_pda, *config_info.key, ErrorCode::ConstraintSeeds);
        require_keys_eq!(
            sol_vault_pda,
            *sol_vault_info.key,
            ErrorCode::ConstraintSeeds
        );
        require_keys_eq!(
            system_program::ID,
            *system_program_info.key,
            ErrorCode::ConstraintAddress
        );
        require!(drop_info.is_writable, ErrorCode::ConstraintMut);
        require!(nullifier_info.is_writable, ErrorCode::ConstraintMut);
        require!(sol_vault_info.is_writable, ErrorCode::ConstraintMut);
        require!(treasury_info.is_writable, ErrorCode::ConstraintMut);
        require!(
            ctx.accounts.claimer.to_account_info().is_writable,
            ErrorCode::ConstraintMut
        );

        require!(drop_info.owner == program_id, ErrorCode::ConstraintOwner);
        require!(
            nullifier_info.owner == program_id,
            ErrorCode::ConstraintOwner
        );

        let mut drop_data_slice: &[u8] = &drop_info.try_borrow_data()?;
        let mut drop_state = DropAccount::try_deserialize(&mut drop_data_slice)?;
        require_eq!(drop_state.bump, drop_bump, ErrorCode::ConstraintSeeds);

        let mut nullifier_data_slice: &[u8] = &nullifier_info.try_borrow_data()?;
        let mut nullifier_state =
            NullifierAccount::try_deserialize(&mut nullifier_data_slice)?;
        require_eq!(
            nullifier_state.bump,
            nullifier_bump,
            ErrorCode::ConstraintSeeds
        );
        require!(
            nullifier_state.nullifier == nullifier,
            DarkPoolError::InvalidNullifier
        );

        require!(
            drop_state.status == DropStatus::Active,
            DarkPoolError::DropNotActive
        );

        require!(
            !nullifier_state.is_used,
            DarkPoolError::NullifierAlreadyUsed
        );

        require!(
            drop_state.nullifier == nullifier,
            DarkPoolError::InvalidNullifier
        );

        if drop_state.expires_at != 0 && now > drop_state.expires_at {
            drop_state.status = DropStatus::Expired;
            let mut drop_data = drop_info.try_borrow_mut_data()?;
            let mut drop_cursor: &mut [u8] = &mut drop_data;
            drop_state.try_serialize(&mut drop_cursor)?;
            return err!(DarkPoolError::DropExpired);
        }

        let config_state = &ctx.accounts.config;
        require!(
            config_state.is_initialized,
            DarkPoolError::ConfigNotInitialized
        );
        require!(
            treasury_info.key() == config_state.treasury,
            DarkPoolError::InvalidTreasury
        );
        let fee = (drop_state.amount as u128)
            .checked_mul(config_state.fee_bps as u128)
            .unwrap_or(0)
            .checked_div(10_000)
            .unwrap_or(0) as u64;
        let payout = drop_state.amount.saturating_sub(fee);

        require!(
            sol_vault_info.lamports() >= drop_state.amount,
            DarkPoolError::InsufficientVaultBalance
        );
        let vault_seeds = &[b"sol_vault".as_ref(), &[config_state.sol_vault_bump]];
        let ix = anchor_lang::solana_program::system_instruction::transfer(
            &ctx.accounts.sol_vault.key(),
            &ctx.accounts.claimer.key(),
            payout,
        );
        anchor_lang::solana_program::program::invoke_signed(
            &ix,
            &[
                sol_vault_info.clone(),
                ctx.accounts.claimer.to_account_info(),
                system_program_info.clone(),
            ],
            &[vault_seeds],
        )?;
        if fee > 0 {
            let fee_ix = anchor_lang::solana_program::system_instruction::transfer(
                &ctx.accounts.sol_vault.key(),
                &ctx.accounts.treasury.key(),
                fee,
            );
            anchor_lang::solana_program::program::invoke_signed(
                &fee_ix,
                &[
                    sol_vault_info,
                    treasury_info,
                    system_program_info,
                ],
                &[vault_seeds],
            )?;
        }

        drop_state.status = DropStatus::Claimed;
        drop_state.claimed_at = now;
        drop_state.claimer = ctx.accounts.claimer.key();

        nullifier_state.is_used = true;
        nullifier_state.used_at = now;
        nullifier_state.claimer = ctx.accounts.claimer.key();

        let mut drop_data = drop_info.try_borrow_mut_data()?;
        let mut drop_cursor: &mut [u8] = &mut drop_data;
        drop_state.try_serialize(&mut drop_cursor)?;

        let mut nullifier_data = nullifier_info.try_borrow_mut_data()?;
        let mut nullifier_cursor: &mut [u8] = &mut nullifier_data;
        nullifier_state.try_serialize(&mut nullifier_cursor)?;

        emit!(DropClaimed {
            nullifier,
            claimer: ctx.accounts.claimer.key(),
            claimed_at: now,
        });

        msg!(
            "Drop claimed: nullifier={:?}, claimer={}",
            nullifier,
            ctx.accounts.claimer.key()
        );
        Ok(())
    }

    pub fn propose_authority(ctx: Context<ProposeAuthority>, new_authority: Pubkey) -> Result<()> {
        let program_id = ctx.program_id;
        let (config_pda, _) = Pubkey::find_program_address(&[b"config"], program_id);
        let config_info = ctx.accounts.config.to_account_info();
        require_keys_eq!(config_pda, *config_info.key, ErrorCode::ConstraintSeeds);
        require!(config_info.is_writable, ErrorCode::ConstraintMut);

        let config = &mut ctx.accounts.config;
        require!(
            config.is_initialized,
            DarkPoolError::ConfigNotInitialized
        );
        require!(
            ctx.accounts.authority.key() == config.authority,
            DarkPoolError::UnauthorizedCreator
        );
        require!(
            new_authority != Pubkey::default(),
            DarkPoolError::InvalidAuthority
        );
        require!(
            config.pending_authority == Pubkey::default(),
            DarkPoolError::PendingAuthorityExists
        );

        config.pending_authority = new_authority;
        config.pending_authority_set_at = Clock::get()?.unix_timestamp;

        emit!(AuthorityProposed {
            current_authority: ctx.accounts.authority.key(),
            pending_authority: new_authority,
            delay_seconds: config.authority_delay_seconds,
        });

        Ok(())
    }

    pub fn cancel_authority_proposal(ctx: Context<CancelAuthorityProposal>) -> Result<()> {
        let program_id = ctx.program_id;
        let (config_pda, _) = Pubkey::find_program_address(&[b"config"], program_id);
        let config_info = ctx.accounts.config.to_account_info();
        require_keys_eq!(config_pda, *config_info.key, ErrorCode::ConstraintSeeds);
        require!(config_info.is_writable, ErrorCode::ConstraintMut);

        let config = &mut ctx.accounts.config;
        require!(
            config.is_initialized,
            DarkPoolError::ConfigNotInitialized
        );
        require!(
            ctx.accounts.authority.key() == config.authority,
            DarkPoolError::UnauthorizedCreator
        );
        require!(
            config.pending_authority != Pubkey::default(),
            DarkPoolError::NoPendingAuthority
        );

        let cancelled_authority = config.pending_authority;
        config.pending_authority = Pubkey::default();
        config.pending_authority_set_at = 0;

        emit!(AuthorityProposalCancelled {
            authority: ctx.accounts.authority.key(),
            cancelled_authority,
        });

        Ok(())
    }

    pub fn accept_authority(ctx: Context<AcceptAuthority>) -> Result<()> {
        let program_id = ctx.program_id;
        let (config_pda, _) = Pubkey::find_program_address(&[b"config"], program_id);
        let config_info = ctx.accounts.config.to_account_info();
        require_keys_eq!(config_pda, *config_info.key, ErrorCode::ConstraintSeeds);
        require!(config_info.is_writable, ErrorCode::ConstraintMut);

        let config = &mut ctx.accounts.config;
        require!(
            config.is_initialized,
            DarkPoolError::ConfigNotInitialized
        );
        require!(
            config.pending_authority == ctx.accounts.pending_authority.key(),
            DarkPoolError::NoPendingAuthority
        );
        let now = Clock::get()?.unix_timestamp;
        require!(
            config.pending_authority_set_at > 0,
            DarkPoolError::NoPendingAuthority
        );
        require!(
            now - config.pending_authority_set_at >= config.authority_delay_seconds,
            DarkPoolError::AuthorityDelayNotElapsed
        );

        let previous_authority = config.authority;
        config.authority = ctx.accounts.pending_authority.key();
        config.pending_authority = Pubkey::default();
        config.pending_authority_set_at = 0;

        emit!(AuthorityAccepted {
            previous_authority,
            new_authority: config.authority,
        });

        Ok(())
    }

    pub fn update_authority_delay(
        ctx: Context<UpdateAuthorityDelay>,
        new_delay_seconds: i64,
    ) -> Result<()> {
        let program_id = ctx.program_id;
        let (config_pda, _) = Pubkey::find_program_address(&[b"config"], program_id);
        let config_info = ctx.accounts.config.to_account_info();
        require_keys_eq!(config_pda, *config_info.key, ErrorCode::ConstraintSeeds);
        require!(config_info.is_writable, ErrorCode::ConstraintMut);

        require!(
            ctx.accounts.authority.key() == ctx.accounts.config.authority,
            DarkPoolError::UnauthorizedCreator
        );
        require!(
            ctx.accounts.config.is_initialized,
            DarkPoolError::ConfigNotInitialized
        );
        require!(
            new_delay_seconds >= MIN_AUTHORITY_DELAY_SECONDS
                && new_delay_seconds <= MAX_AUTHORITY_DELAY_SECONDS,
            DarkPoolError::InvalidAuthorityDelay
        );

        let config = &mut ctx.accounts.config;
        config.authority_delay_seconds = new_delay_seconds;

        emit!(AuthorityDelayUpdated {
            authority: ctx.accounts.authority.key(),
            new_delay_seconds,
        });

        Ok(())
    }

    pub fn expire_drop(ctx: Context<ExpireDrop>, nullifier: [u8; 32]) -> Result<()> {
        let clock = Clock::get()?;
        let now = clock.unix_timestamp;
        let program_id = ctx.program_id;

        let (drop_pda, drop_bump) =
            Pubkey::find_program_address(&[b"drop", nullifier.as_ref()], program_id);
        let (nullifier_pda, nullifier_bump) =
            Pubkey::find_program_address(&[b"nullifier", nullifier.as_ref()], program_id);
        let (config_pda, _) = Pubkey::find_program_address(&[b"config"], program_id);

        let drop_info = ctx.accounts.drop.to_account_info();
        let nullifier_info = ctx.accounts.nullifier_account.to_account_info();
        let config_info = ctx.accounts.config.to_account_info();
        let rent_collector_info = ctx.accounts.rent_collector.to_account_info();

        require_keys_eq!(drop_pda, *drop_info.key, ErrorCode::ConstraintSeeds);
        require_keys_eq!(
            nullifier_pda,
            *nullifier_info.key,
            ErrorCode::ConstraintSeeds
        );
        require_keys_eq!(config_pda, *config_info.key, ErrorCode::ConstraintSeeds);
        require_keys_neq!(*drop_info.key, *nullifier_info.key, ErrorCode::ConstraintAddress);
        require_keys_neq!(
            *rent_collector_info.key,
            *drop_info.key,
            ErrorCode::ConstraintAddress
        );
        require_keys_neq!(
            *rent_collector_info.key,
            *nullifier_info.key,
            ErrorCode::ConstraintAddress
        );
        require!(drop_info.is_writable, ErrorCode::ConstraintMut);
        require!(nullifier_info.is_writable, ErrorCode::ConstraintMut);
        require!(rent_collector_info.is_writable, ErrorCode::ConstraintMut);

        require!(drop_info.owner == program_id, ErrorCode::ConstraintOwner);
        require!(
            nullifier_info.owner == program_id,
            ErrorCode::ConstraintOwner
        );

        let mut drop_data_slice: &[u8] = &drop_info.try_borrow_data()?;
        let drop_state = DropAccount::try_deserialize(&mut drop_data_slice)?;
        require_eq!(drop_state.bump, drop_bump, ErrorCode::ConstraintSeeds);

        let mut nullifier_data_slice: &[u8] = &nullifier_info.try_borrow_data()?;
        let nullifier_state = NullifierAccount::try_deserialize(&mut nullifier_data_slice)?;
        require_eq!(
            nullifier_state.bump,
            nullifier_bump,
            ErrorCode::ConstraintSeeds
        );
        require!(
            nullifier_state.nullifier == nullifier,
            DarkPoolError::InvalidNullifier
        );

        require!(
            ctx.accounts.config.authority == ctx.accounts.authority.key(),
            DarkPoolError::UnauthorizedCreator
        );
        require!(
            ctx.accounts.config.is_initialized,
            DarkPoolError::ConfigNotInitialized
        );
        require!(
            drop_state.nullifier == nullifier,
            DarkPoolError::InvalidNullifier
        );
        require!(
            drop_state.status == DropStatus::Active,
            DarkPoolError::DropNotActive
        );
        require!(
            drop_state.expires_at != 0 && now > drop_state.expires_at,
            DarkPoolError::DropNotExpired
        );
        require!(
            !nullifier_state.is_used,
            DarkPoolError::NullifierAlreadyUsed
        );

        emit!(DropExpired {
            nullifier,
            recipient: drop_state.recipient,
            expires_at: drop_state.expires_at,
        });

        // Close the drop and nullifier accounts to the rent collector.
        let mut rent_lamports = rent_collector_info.try_borrow_mut_lamports()?;
        let mut drop_lamports = drop_info.try_borrow_mut_lamports()?;
        let new_rent_lamports = rent_lamports
            .checked_add(**drop_lamports)
            .ok_or(DarkPoolError::NumericalOverflow)?;
        **rent_lamports = new_rent_lamports;
        **drop_lamports = 0;
        drop_info.try_borrow_mut_data()?.fill(0);

        let mut nullifier_lamports = nullifier_info.try_borrow_mut_lamports()?;
        let new_rent_lamports = rent_lamports
            .checked_add(**nullifier_lamports)
            .ok_or(DarkPoolError::NumericalOverflow)?;
        **rent_lamports = new_rent_lamports;
        **nullifier_lamports = 0;
        nullifier_info.try_borrow_mut_data()?.fill(0);

        Ok(())
    }

pub struct Initialize<'info> {
    pub config: UncheckedAccount<'info>,
    pub authority: Signer<'info>,
    pub sol_vault: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

impl<'info> Bumps for Initialize<'info> {
    type Bumps = ();
}

impl<'info> Accounts<'info, ()> for Initialize<'info> {
    fn try_accounts(
        program_id: &Pubkey,
        accounts: &mut &'info [AccountInfo<'info>],
        ix_data: &[u8],
        bumps: &mut (),
        reallocs: &mut BTreeSet<Pubkey>,
    ) -> Result<Self> {
        let config = UncheckedAccount::try_accounts(program_id, accounts, ix_data, bumps, reallocs)?;
        let authority = Signer::try_accounts(program_id, accounts, ix_data, bumps, reallocs)?;
        let sol_vault =
            UncheckedAccount::try_accounts(program_id, accounts, ix_data, bumps, reallocs)?;
        let system_program =
            Program::try_accounts(program_id, accounts, ix_data, bumps, reallocs)?;
        Ok(Self {
            config,
            authority,
            sol_vault,
            system_program,
        })
    }
}

impl<'info> ToAccountMetas for Initialize<'info> {
    fn to_account_metas(&self, is_signer: Option<bool>) -> Vec<AccountMeta> {
        let mut metas = Vec::new();
        let override_signer = is_signer;
        metas.extend(self.config.to_account_metas(override_signer));
        metas.extend(self.authority.to_account_metas(override_signer));
        metas.extend(self.sol_vault.to_account_metas(override_signer));
        metas.extend(self.system_program.to_account_metas(override_signer));
        metas
    }
}

impl<'info> ToAccountInfos<'info> for Initialize<'info> {
    fn to_account_infos(&self) -> Vec<AccountInfo<'info>> {
        let mut infos = Vec::new();
        infos.extend(self.config.to_account_infos());
        infos.extend(self.authority.to_account_infos());
        infos.extend(self.sol_vault.to_account_infos());
        infos.extend(self.system_program.to_account_infos());
        infos
    }
}

impl<'info> AccountsExit<'info> for Initialize<'info> {}

pub(crate) mod __client_accounts_initialize {
    use super::*;
    use anchor_lang::prelude::borsh;

    #[derive(anchor_lang::AnchorSerialize)]
    pub struct Initialize {
        pub config: Pubkey,
        pub authority: Pubkey,
        pub sol_vault: Pubkey,
        pub system_program: Pubkey,
    }

    #[automatically_derived]
    impl anchor_lang::ToAccountMetas for Initialize {
        fn to_account_metas(
            &self,
            _is_signer: Option<bool>,
        ) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
            vec![
                anchor_lang::solana_program::instruction::AccountMeta::new(self.config, false),
                anchor_lang::solana_program::instruction::AccountMeta::new(self.authority, true),
                anchor_lang::solana_program::instruction::AccountMeta::new(self.sol_vault, false),
                anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                    self.system_program,
                    false,
                ),
            ]
        }
    }
}

pub struct CreateDrop<'info> {
    pub drop: UncheckedAccount<'info>,
    pub nullifier_account: UncheckedAccount<'info>,
    pub config: Account<'info, Config>,
    pub rate_limit_account: UncheckedAccount<'info>,
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

impl<'info> Bumps for CreateDrop<'info> {
    type Bumps = ();
}

impl<'info> Accounts<'info, ()> for CreateDrop<'info> {
    fn try_accounts(
        program_id: &Pubkey,
        accounts: &mut &'info [AccountInfo<'info>],
        ix_data: &[u8],
        bumps: &mut (),
        reallocs: &mut BTreeSet<Pubkey>,
    ) -> Result<Self> {
        let drop = UncheckedAccount::try_accounts(program_id, accounts, ix_data, bumps, reallocs)?;
        let nullifier_account =
            UncheckedAccount::try_accounts(program_id, accounts, ix_data, bumps, reallocs)?;
        let config = Account::try_accounts(program_id, accounts, ix_data, bumps, reallocs)?;
        let rate_limit_account =
            UncheckedAccount::try_accounts(program_id, accounts, ix_data, bumps, reallocs)?;
        let payer = Signer::try_accounts(program_id, accounts, ix_data, bumps, reallocs)?;
        let system_program =
            Program::try_accounts(program_id, accounts, ix_data, bumps, reallocs)?;
        Ok(Self {
            drop,
            nullifier_account,
            config,
            rate_limit_account,
            payer,
            system_program,
        })
    }
}

impl<'info> ToAccountMetas for CreateDrop<'info> {
    fn to_account_metas(&self, is_signer: Option<bool>) -> Vec<AccountMeta> {
        let mut metas = Vec::new();
        let override_signer = is_signer;
        metas.extend(self.drop.to_account_metas(override_signer));
        metas.extend(self.nullifier_account.to_account_metas(override_signer));
        metas.extend(self.config.to_account_metas(override_signer));
        metas.extend(self.rate_limit_account.to_account_metas(override_signer));
        metas.extend(self.payer.to_account_metas(override_signer));
        metas.extend(self.system_program.to_account_metas(override_signer));
        metas
    }
}

impl<'info> ToAccountInfos<'info> for CreateDrop<'info> {
    fn to_account_infos(&self) -> Vec<AccountInfo<'info>> {
        let mut infos = Vec::new();
        infos.extend(self.drop.to_account_infos());
        infos.extend(self.nullifier_account.to_account_infos());
        infos.extend(self.config.to_account_infos());
        infos.extend(self.rate_limit_account.to_account_infos());
        infos.extend(self.payer.to_account_infos());
        infos.extend(self.system_program.to_account_infos());
        infos
    }
}

impl<'info> AccountsExit<'info> for CreateDrop<'info> {}

pub(crate) mod __client_accounts_create_drop {
    use super::*;
    use anchor_lang::prelude::borsh;

    #[derive(anchor_lang::AnchorSerialize)]
    pub struct CreateDrop {
        pub drop: Pubkey,
        pub nullifier_account: Pubkey,
        pub config: Pubkey,
        pub rate_limit_account: Pubkey,
        pub payer: Pubkey,
        pub system_program: Pubkey,
    }

    #[automatically_derived]
    impl anchor_lang::ToAccountMetas for CreateDrop {
        fn to_account_metas(
            &self,
            _is_signer: Option<bool>,
        ) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
            vec![
                anchor_lang::solana_program::instruction::AccountMeta::new(self.drop, false),
                anchor_lang::solana_program::instruction::AccountMeta::new(
                    self.nullifier_account,
                    false,
                ),
                anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                    self.config,
                    false,
                ),
                anchor_lang::solana_program::instruction::AccountMeta::new(
                    self.rate_limit_account,
                    false,
                ),
                anchor_lang::solana_program::instruction::AccountMeta::new(self.payer, true),
                anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                    self.system_program,
                    false,
                ),
            ]
        }
    }
}

pub struct DepositPool<'info> {
    pub drop: UncheckedAccount<'info>,
    pub nullifier_account: UncheckedAccount<'info>,
    pub config: Account<'info, Config>,
    pub rate_limit_account: UncheckedAccount<'info>,
    pub payer: Signer<'info>,
    pub sol_vault: SystemAccount<'info>,
    pub system_program: Program<'info, System>,
}

impl<'info> Bumps for DepositPool<'info> {
    type Bumps = ();
}

impl<'info> Accounts<'info, ()> for DepositPool<'info> {
    fn try_accounts(
        program_id: &Pubkey,
        accounts: &mut &'info [AccountInfo<'info>],
        ix_data: &[u8],
        bumps: &mut (),
        reallocs: &mut BTreeSet<Pubkey>,
    ) -> Result<Self> {
        let drop = UncheckedAccount::try_accounts(program_id, accounts, ix_data, bumps, reallocs)?;
        let nullifier_account =
            UncheckedAccount::try_accounts(program_id, accounts, ix_data, bumps, reallocs)?;
        let config = Account::try_accounts(program_id, accounts, ix_data, bumps, reallocs)?;
        let rate_limit_account =
            UncheckedAccount::try_accounts(program_id, accounts, ix_data, bumps, reallocs)?;
        let payer = Signer::try_accounts(program_id, accounts, ix_data, bumps, reallocs)?;
        let sol_vault = SystemAccount::try_accounts(program_id, accounts, ix_data, bumps, reallocs)?;
        let system_program =
            Program::try_accounts(program_id, accounts, ix_data, bumps, reallocs)?;
        Ok(Self {
            drop,
            nullifier_account,
            config,
            rate_limit_account,
            payer,
            sol_vault,
            system_program,
        })
    }
}

impl<'info> ToAccountMetas for DepositPool<'info> {
    fn to_account_metas(&self, is_signer: Option<bool>) -> Vec<AccountMeta> {
        let mut metas = Vec::new();
        let override_signer = is_signer;
        metas.extend(self.drop.to_account_metas(override_signer));
        metas.extend(self.nullifier_account.to_account_metas(override_signer));
        metas.extend(self.config.to_account_metas(override_signer));
        metas.extend(self.rate_limit_account.to_account_metas(override_signer));
        metas.extend(self.payer.to_account_metas(override_signer));
        metas.extend(self.sol_vault.to_account_metas(override_signer));
        metas.extend(self.system_program.to_account_metas(override_signer));
        metas
    }
}

impl<'info> ToAccountInfos<'info> for DepositPool<'info> {
    fn to_account_infos(&self) -> Vec<AccountInfo<'info>> {
        let mut infos = Vec::new();
        infos.extend(self.drop.to_account_infos());
        infos.extend(self.nullifier_account.to_account_infos());
        infos.extend(self.config.to_account_infos());
        infos.extend(self.rate_limit_account.to_account_infos());
        infos.extend(self.payer.to_account_infos());
        infos.extend(self.sol_vault.to_account_infos());
        infos.extend(self.system_program.to_account_infos());
        infos
    }
}

impl<'info> AccountsExit<'info> for DepositPool<'info> {}

pub(crate) mod __client_accounts_deposit_pool {
    use super::*;
    use anchor_lang::prelude::borsh;

    #[derive(anchor_lang::AnchorSerialize)]
    pub struct DepositPool {
        pub drop: Pubkey,
        pub nullifier_account: Pubkey,
        pub config: Pubkey,
        pub rate_limit_account: Pubkey,
        pub payer: Pubkey,
        pub sol_vault: Pubkey,
        pub system_program: Pubkey,
    }

    #[automatically_derived]
    impl anchor_lang::ToAccountMetas for DepositPool {
        fn to_account_metas(
            &self,
            _is_signer: Option<bool>,
        ) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
            vec![
                anchor_lang::solana_program::instruction::AccountMeta::new(self.drop, false),
                anchor_lang::solana_program::instruction::AccountMeta::new(
                    self.nullifier_account,
                    false,
                ),
                anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                    self.config,
                    false,
                ),
                anchor_lang::solana_program::instruction::AccountMeta::new(
                    self.rate_limit_account,
                    false,
                ),
                anchor_lang::solana_program::instruction::AccountMeta::new(self.payer, true),
                anchor_lang::solana_program::instruction::AccountMeta::new(self.sol_vault, false),
                anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                    self.system_program,
                    false,
                ),
            ]
        }
    }
}

pub struct ClaimDrop<'info> {
    pub drop: UncheckedAccount<'info>,
    pub nullifier_account: UncheckedAccount<'info>,
    pub claimer: Signer<'info>,
    pub config: Account<'info, Config>,
    pub sol_vault: SystemAccount<'info>,
    pub treasury: SystemAccount<'info>,
    pub system_program: Program<'info, System>,
}

impl<'info> Bumps for ClaimDrop<'info> {
    type Bumps = ();
}

impl<'info> Accounts<'info, ()> for ClaimDrop<'info> {
    fn try_accounts(
        program_id: &Pubkey,
        accounts: &mut &'info [AccountInfo<'info>],
        ix_data: &[u8],
        bumps: &mut (),
        reallocs: &mut BTreeSet<Pubkey>,
    ) -> Result<Self> {
        let drop = UncheckedAccount::try_accounts(program_id, accounts, ix_data, bumps, reallocs)?;
        let nullifier_account =
            UncheckedAccount::try_accounts(program_id, accounts, ix_data, bumps, reallocs)?;
        let claimer = Signer::try_accounts(program_id, accounts, ix_data, bumps, reallocs)?;
        let config = Account::try_accounts(program_id, accounts, ix_data, bumps, reallocs)?;
        let sol_vault = SystemAccount::try_accounts(program_id, accounts, ix_data, bumps, reallocs)?;
        let treasury = SystemAccount::try_accounts(program_id, accounts, ix_data, bumps, reallocs)?;
        let system_program =
            Program::try_accounts(program_id, accounts, ix_data, bumps, reallocs)?;
        Ok(Self {
            drop,
            nullifier_account,
            claimer,
            config,
            sol_vault,
            treasury,
            system_program,
        })
    }
}

impl<'info> ToAccountMetas for ClaimDrop<'info> {
    fn to_account_metas(&self, is_signer: Option<bool>) -> Vec<AccountMeta> {
        let mut metas = Vec::new();
        let override_signer = is_signer;
        metas.extend(self.drop.to_account_metas(override_signer));
        metas.extend(self.nullifier_account.to_account_metas(override_signer));
        metas.extend(self.claimer.to_account_metas(override_signer));
        metas.extend(self.config.to_account_metas(override_signer));
        metas.extend(self.sol_vault.to_account_metas(override_signer));
        metas.extend(self.treasury.to_account_metas(override_signer));
        metas.extend(self.system_program.to_account_metas(override_signer));
        metas
    }
}

impl<'info> ToAccountInfos<'info> for ClaimDrop<'info> {
    fn to_account_infos(&self) -> Vec<AccountInfo<'info>> {
        let mut infos = Vec::new();
        infos.extend(self.drop.to_account_infos());
        infos.extend(self.nullifier_account.to_account_infos());
        infos.extend(self.claimer.to_account_infos());
        infos.extend(self.config.to_account_infos());
        infos.extend(self.sol_vault.to_account_infos());
        infos.extend(self.treasury.to_account_infos());
        infos.extend(self.system_program.to_account_infos());
        infos
    }
}

impl<'info> AccountsExit<'info> for ClaimDrop<'info> {}

pub(crate) mod __client_accounts_claim_drop {
    use super::*;
    use anchor_lang::prelude::borsh;

    #[derive(anchor_lang::AnchorSerialize)]
    pub struct ClaimDrop {
        pub drop: Pubkey,
        pub nullifier_account: Pubkey,
        pub claimer: Pubkey,
        pub config: Pubkey,
        pub sol_vault: Pubkey,
        pub treasury: Pubkey,
        pub system_program: Pubkey,
    }

    #[automatically_derived]
    impl anchor_lang::ToAccountMetas for ClaimDrop {
        fn to_account_metas(
            &self,
            _is_signer: Option<bool>,
        ) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
            vec![
                anchor_lang::solana_program::instruction::AccountMeta::new(self.drop, false),
                anchor_lang::solana_program::instruction::AccountMeta::new(
                    self.nullifier_account,
                    false,
                ),
                anchor_lang::solana_program::instruction::AccountMeta::new(self.claimer, true),
                anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                    self.config,
                    false,
                ),
                anchor_lang::solana_program::instruction::AccountMeta::new(self.sol_vault, false),
                anchor_lang::solana_program::instruction::AccountMeta::new(self.treasury, false),
                anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                    self.system_program,
                    false,
                ),
            ]
        }
    }
}

pub struct ProposeAuthority<'info> {
    pub config: Account<'info, Config>,
    pub authority: Signer<'info>,
}

pub struct CancelAuthorityProposal<'info> {
    pub config: Account<'info, Config>,
    pub authority: Signer<'info>,
}

pub struct AcceptAuthority<'info> {
    pub config: Account<'info, Config>,
    pub pending_authority: Signer<'info>,
}

pub struct UpdateAuthorityDelay<'info> {
    pub config: Account<'info, Config>,
    pub authority: Signer<'info>,
}

impl<'info> Bumps for ProposeAuthority<'info> {
    type Bumps = ();
}

impl<'info> Accounts<'info, ()> for ProposeAuthority<'info> {
    fn try_accounts(
        program_id: &Pubkey,
        accounts: &mut &'info [AccountInfo<'info>],
        ix_data: &[u8],
        bumps: &mut (),
        reallocs: &mut BTreeSet<Pubkey>,
    ) -> Result<Self> {
        let config = Account::try_accounts(program_id, accounts, ix_data, bumps, reallocs)?;
        let authority = Signer::try_accounts(program_id, accounts, ix_data, bumps, reallocs)?;
        Ok(Self { config, authority })
    }
}

impl<'info> ToAccountMetas for ProposeAuthority<'info> {
    fn to_account_metas(&self, is_signer: Option<bool>) -> Vec<AccountMeta> {
        let mut metas = Vec::new();
        let override_signer = is_signer;
        metas.extend(self.config.to_account_metas(override_signer));
        metas.extend(self.authority.to_account_metas(override_signer));
        metas
    }
}

impl<'info> ToAccountInfos<'info> for ProposeAuthority<'info> {
    fn to_account_infos(&self) -> Vec<AccountInfo<'info>> {
        let mut infos = Vec::new();
        infos.extend(self.config.to_account_infos());
        infos.extend(self.authority.to_account_infos());
        infos
    }
}

impl<'info> AccountsExit<'info> for ProposeAuthority<'info> {}

pub(crate) mod __client_accounts_propose_authority {
    use super::*;
    use anchor_lang::prelude::borsh;

    #[derive(anchor_lang::AnchorSerialize)]
    pub struct ProposeAuthority {
        pub config: Pubkey,
        pub authority: Pubkey,
    }

    #[automatically_derived]
    impl anchor_lang::ToAccountMetas for ProposeAuthority {
        fn to_account_metas(
            &self,
            _is_signer: Option<bool>,
        ) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
            vec![
                anchor_lang::solana_program::instruction::AccountMeta::new(self.config, false),
                anchor_lang::solana_program::instruction::AccountMeta::new(self.authority, true),
            ]
        }
    }
}

impl<'info> Bumps for CancelAuthorityProposal<'info> {
    type Bumps = ();
}

impl<'info> Accounts<'info, ()> for CancelAuthorityProposal<'info> {
    fn try_accounts(
        program_id: &Pubkey,
        accounts: &mut &'info [AccountInfo<'info>],
        ix_data: &[u8],
        bumps: &mut (),
        reallocs: &mut BTreeSet<Pubkey>,
    ) -> Result<Self> {
        let config = Account::try_accounts(program_id, accounts, ix_data, bumps, reallocs)?;
        let authority = Signer::try_accounts(program_id, accounts, ix_data, bumps, reallocs)?;
        Ok(Self { config, authority })
    }
}

impl<'info> ToAccountMetas for CancelAuthorityProposal<'info> {
    fn to_account_metas(&self, is_signer: Option<bool>) -> Vec<AccountMeta> {
        let mut metas = Vec::new();
        let override_signer = is_signer;
        metas.extend(self.config.to_account_metas(override_signer));
        metas.extend(self.authority.to_account_metas(override_signer));
        metas
    }
}

impl<'info> ToAccountInfos<'info> for CancelAuthorityProposal<'info> {
    fn to_account_infos(&self) -> Vec<AccountInfo<'info>> {
        let mut infos = Vec::new();
        infos.extend(self.config.to_account_infos());
        infos.extend(self.authority.to_account_infos());
        infos
    }
}

impl<'info> AccountsExit<'info> for CancelAuthorityProposal<'info> {}

pub(crate) mod __client_accounts_cancel_authority_proposal {
    use super::*;
    use anchor_lang::prelude::borsh;

    #[derive(anchor_lang::AnchorSerialize)]
    pub struct CancelAuthorityProposal {
        pub config: Pubkey,
        pub authority: Pubkey,
    }

    #[automatically_derived]
    impl anchor_lang::ToAccountMetas for CancelAuthorityProposal {
        fn to_account_metas(
            &self,
            _is_signer: Option<bool>,
        ) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
            vec![
                anchor_lang::solana_program::instruction::AccountMeta::new(self.config, false),
                anchor_lang::solana_program::instruction::AccountMeta::new(self.authority, true),
            ]
        }
    }
}

impl<'info> Bumps for AcceptAuthority<'info> {
    type Bumps = ();
}

impl<'info> Accounts<'info, ()> for AcceptAuthority<'info> {
    fn try_accounts(
        program_id: &Pubkey,
        accounts: &mut &'info [AccountInfo<'info>],
        ix_data: &[u8],
        bumps: &mut (),
        reallocs: &mut BTreeSet<Pubkey>,
    ) -> Result<Self> {
        let config = Account::try_accounts(program_id, accounts, ix_data, bumps, reallocs)?;
        let pending_authority =
            Signer::try_accounts(program_id, accounts, ix_data, bumps, reallocs)?;
        Ok(Self {
            config,
            pending_authority,
        })
    }
}

impl<'info> ToAccountMetas for AcceptAuthority<'info> {
    fn to_account_metas(&self, is_signer: Option<bool>) -> Vec<AccountMeta> {
        let mut metas = Vec::new();
        let override_signer = is_signer;
        metas.extend(self.config.to_account_metas(override_signer));
        metas.extend(self.pending_authority.to_account_metas(override_signer));
        metas
    }
}

impl<'info> ToAccountInfos<'info> for AcceptAuthority<'info> {
    fn to_account_infos(&self) -> Vec<AccountInfo<'info>> {
        let mut infos = Vec::new();
        infos.extend(self.config.to_account_infos());
        infos.extend(self.pending_authority.to_account_infos());
        infos
    }
}

impl<'info> AccountsExit<'info> for AcceptAuthority<'info> {}

pub(crate) mod __client_accounts_accept_authority {
    use super::*;
    use anchor_lang::prelude::borsh;

    #[derive(anchor_lang::AnchorSerialize)]
    pub struct AcceptAuthority {
        pub config: Pubkey,
        pub pending_authority: Pubkey,
    }

    #[automatically_derived]
    impl anchor_lang::ToAccountMetas for AcceptAuthority {
        fn to_account_metas(
            &self,
            _is_signer: Option<bool>,
        ) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
            vec![
                anchor_lang::solana_program::instruction::AccountMeta::new(self.config, false),
                anchor_lang::solana_program::instruction::AccountMeta::new(
                    self.pending_authority,
                    true,
                ),
            ]
        }
    }
}

impl<'info> Bumps for UpdateAuthorityDelay<'info> {
    type Bumps = ();
}

impl<'info> Accounts<'info, ()> for UpdateAuthorityDelay<'info> {
    fn try_accounts(
        program_id: &Pubkey,
        accounts: &mut &'info [AccountInfo<'info>],
        ix_data: &[u8],
        bumps: &mut (),
        reallocs: &mut BTreeSet<Pubkey>,
    ) -> Result<Self> {
        let config = Account::try_accounts(program_id, accounts, ix_data, bumps, reallocs)?;
        let authority = Signer::try_accounts(program_id, accounts, ix_data, bumps, reallocs)?;
        Ok(Self { config, authority })
    }
}

impl<'info> ToAccountMetas for UpdateAuthorityDelay<'info> {
    fn to_account_metas(&self, is_signer: Option<bool>) -> Vec<AccountMeta> {
        let mut metas = Vec::new();
        let override_signer = is_signer;
        metas.extend(self.config.to_account_metas(override_signer));
        metas.extend(self.authority.to_account_metas(override_signer));
        metas
    }
}

impl<'info> ToAccountInfos<'info> for UpdateAuthorityDelay<'info> {
    fn to_account_infos(&self) -> Vec<AccountInfo<'info>> {
        let mut infos = Vec::new();
        infos.extend(self.config.to_account_infos());
        infos.extend(self.authority.to_account_infos());
        infos
    }
}

impl<'info> AccountsExit<'info> for UpdateAuthorityDelay<'info> {}

pub(crate) mod __client_accounts_update_authority_delay {
    use super::*;
    use anchor_lang::prelude::borsh;

    #[derive(anchor_lang::AnchorSerialize)]
    pub struct UpdateAuthorityDelay {
        pub config: Pubkey,
        pub authority: Pubkey,
    }

    #[automatically_derived]
    impl anchor_lang::ToAccountMetas for UpdateAuthorityDelay {
        fn to_account_metas(
            &self,
            _is_signer: Option<bool>,
        ) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
            vec![
                anchor_lang::solana_program::instruction::AccountMeta::new(self.config, false),
                anchor_lang::solana_program::instruction::AccountMeta::new(self.authority, true),
            ]
        }
    }
}

pub struct ExpireDrop<'info> {
    pub drop: UncheckedAccount<'info>,
    pub nullifier_account: UncheckedAccount<'info>,
    pub config: Account<'info, Config>,
    pub authority: Signer<'info>,
    pub rent_collector: SystemAccount<'info>,
}

impl<'info> Bumps for ExpireDrop<'info> {
    type Bumps = ();
}

impl<'info> Accounts<'info, ()> for ExpireDrop<'info> {
    fn try_accounts(
        program_id: &Pubkey,
        accounts: &mut &'info [AccountInfo<'info>],
        ix_data: &[u8],
        bumps: &mut (),
        reallocs: &mut BTreeSet<Pubkey>,
    ) -> Result<Self> {
        let drop = UncheckedAccount::try_accounts(program_id, accounts, ix_data, bumps, reallocs)?;
        let nullifier_account =
            UncheckedAccount::try_accounts(program_id, accounts, ix_data, bumps, reallocs)?;
        let config = Account::try_accounts(program_id, accounts, ix_data, bumps, reallocs)?;
        let authority = Signer::try_accounts(program_id, accounts, ix_data, bumps, reallocs)?;
        let rent_collector =
            SystemAccount::try_accounts(program_id, accounts, ix_data, bumps, reallocs)?;
        Ok(Self {
            drop,
            nullifier_account,
            config,
            authority,
            rent_collector,
        })
    }
}

impl<'info> ToAccountMetas for ExpireDrop<'info> {
    fn to_account_metas(&self, is_signer: Option<bool>) -> Vec<AccountMeta> {
        let mut metas = Vec::new();
        let override_signer = is_signer;
        metas.extend(self.drop.to_account_metas(override_signer));
        metas.extend(self.nullifier_account.to_account_metas(override_signer));
        metas.extend(self.config.to_account_metas(override_signer));
        metas.extend(self.authority.to_account_metas(override_signer));
        metas.extend(self.rent_collector.to_account_metas(override_signer));
        metas
    }
}

impl<'info> ToAccountInfos<'info> for ExpireDrop<'info> {
    fn to_account_infos(&self) -> Vec<AccountInfo<'info>> {
        let mut infos = Vec::new();
        infos.extend(self.drop.to_account_infos());
        infos.extend(self.nullifier_account.to_account_infos());
        infos.extend(self.config.to_account_infos());
        infos.extend(self.authority.to_account_infos());
        infos.extend(self.rent_collector.to_account_infos());
        infos
    }
}

impl<'info> AccountsExit<'info> for ExpireDrop<'info> {}

pub(crate) mod __client_accounts_expire_drop {
    use super::*;
    use anchor_lang::prelude::borsh;

    #[derive(anchor_lang::AnchorSerialize)]
    pub struct ExpireDrop {
        pub drop: Pubkey,
        pub nullifier_account: Pubkey,
        pub config: Pubkey,
        pub authority: Pubkey,
        pub rent_collector: Pubkey,
    }

    #[automatically_derived]
    impl anchor_lang::ToAccountMetas for ExpireDrop {
        fn to_account_metas(
            &self,
            _is_signer: Option<bool>,
        ) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
            vec![
                anchor_lang::solana_program::instruction::AccountMeta::new(self.drop, false),
                anchor_lang::solana_program::instruction::AccountMeta::new(
                    self.nullifier_account,
                    false,
                ),
                anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                    self.config,
                    false,
                ),
                anchor_lang::solana_program::instruction::AccountMeta::new(self.authority, true),
                anchor_lang::solana_program::instruction::AccountMeta::new(
                    self.rent_collector,
                    false,
                ),
            ]
        }
    }
}

#[account]
pub struct Config {
    pub authority: Pubkey,
    pub is_initialized: bool,
    pub pending_authority: Pubkey,
    pub pending_authority_set_at: i64,
    pub authority_delay_seconds: i64,
    pub treasury: Pubkey,
    pub fee_bps: u16,
    pub sol_vault_bump: u8,
}

impl Config {
    pub const LEN: usize = 32 + 1 + 32 + 8 + 8 + 32 + 2 + 1;
}

#[account]
pub struct DropAccount {
    pub nullifier: [u8; 32],
    pub recipient: Pubkey,
    pub amount: u64,
    pub asset_type: u8,
    pub status: DropStatus,
    pub expires_at: i64,
    pub created_at: i64,
    pub claimed_at: i64,
    pub claimer: Pubkey,
    pub bump: u8,
}

impl DropAccount {
    pub const LEN: usize = 32 + 32 + 8 + 1 + 1 + 8 + 8 + 8 + 32 + 1;
}

#[account]
pub struct NullifierAccount {
    pub nullifier: [u8; 32],
    pub is_used: bool,
    pub claimer: Pubkey,
    pub used_at: i64,
    pub bump: u8,
}

impl NullifierAccount {
    pub const LEN: usize = 32 + 1 + 32 + 8 + 1;
}

#[account]
pub struct RateLimitAccount {
    pub last_drop_at: i64,
    pub bump: u8,
}

impl RateLimitAccount {
    pub const LEN: usize = 8 + 1;
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum DropStatus {
    Active,
    Claimed,
    Expired,
}

#[event]
pub struct DropCreated {
    pub nullifier: [u8; 32],
    pub recipient: Pubkey,
    pub amount: u64,
    pub asset_type: u8,
    pub expires_at: i64,
    pub payer: Pubkey,
}

#[event]
pub struct DropClaimed {
    pub nullifier: [u8; 32],
    pub claimer: Pubkey,
    pub claimed_at: i64,
}

#[event]
pub struct DropExpired {
    pub nullifier: [u8; 32],
    pub recipient: Pubkey,
    pub expires_at: i64,
}

#[event]
pub struct AuthorityProposed {
    pub current_authority: Pubkey,
    pub pending_authority: Pubkey,
    pub delay_seconds: i64,
}

#[event]
pub struct AuthorityProposalCancelled {
    pub authority: Pubkey,
    pub cancelled_authority: Pubkey,
}

#[event]
pub struct AuthorityAccepted {
    pub previous_authority: Pubkey,
    pub new_authority: Pubkey,
}

#[event]
pub struct AuthorityDelayUpdated {
    pub authority: Pubkey,
    pub new_delay_seconds: i64,
}

#[error_code]
pub enum DarkPoolError {
    #[msg("This nullifier has already been used")]
    NullifierAlreadyUsed,

    #[msg("Drop is not active")]
    DropNotActive,

    #[msg("Invalid nullifier")]
    InvalidNullifier,

    #[msg("Amount must be greater than zero")]
    InvalidAmount,

    #[msg("Asset type is not supported")]
    InvalidAssetType,

    #[msg("Recipient must differ from payer")]
    InvalidRecipient,

    #[msg("Expiration timestamp is invalid")]
    InvalidExpiration,

    #[msg("Program config is not initialized")]
    ConfigNotInitialized,

    #[msg("Caller is not authorized to create drops")]
    UnauthorizedCreator,

    #[msg("Create drop rate limit exceeded")]
    RateLimitExceeded,

    #[msg("Drop has expired")]
    DropExpired,

    #[msg("Authority cannot be the default public key")]
    InvalidAuthority,

    #[msg("A pending authority already exists")]
    PendingAuthorityExists,

    #[msg("No pending authority to process")]
    NoPendingAuthority,

    #[msg("Authority delay has not elapsed")]
    AuthorityDelayNotElapsed,

    #[msg("Authority delay is out of bounds")]
    InvalidAuthorityDelay,

    #[msg("Drop has not yet expired")]
    DropNotExpired,

    #[msg("Invalid fee bps")]
    InvalidFeeBps,

    #[msg("Invalid treasury address")]
    InvalidTreasury,

    #[msg("Insufficient vault balance")]
    InsufficientVaultBalance,

    #[msg("Account is already initialized")]
    AccountAlreadyInitialized,

    #[msg("Numerical overflow")]
    NumericalOverflow,
}

#[cfg(test)]
mod tests {
    use super::*;
}

}
