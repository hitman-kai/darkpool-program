use std::collections::BTreeSet;

use anchor_lang::prelude::*;
use anchor_lang::AccountsExit;
use anchor_lang::Bumps;
use anchor_lang::system_program;
use anchor_lang::solana_program::hash::hashv;
#[cfg(feature = "poseidon")]
use ark_bn254::Fr;
#[cfg(feature = "poseidon")]
use ark_ff::BigInteger;
#[cfg(feature = "poseidon")]
use light_poseidon::{Poseidon, PoseidonHasher, PoseidonParameters};

#[cfg(feature = "poseidon")]
mod poseidon_constants_fr;

const MAX_EXPIRATION_WINDOW: i64 = 30 * 24 * 60 * 60; // 30 days
const MIN_EXPIRATION_WINDOW: i64 = 60; // 1 minute
const MIN_RATE_LIMIT_SECONDS: i64 = 10;
const MIN_AUTHORITY_DELAY_SECONDS: i64 = 15 * 60; // 15 minutes
const MAX_AUTHORITY_DELAY_SECONDS: i64 = 7 * 24 * 60 * 60; // 7 days
const DEFAULT_AUTHORITY_DELAY_SECONDS: i64 = 24 * 60 * 60; // 24 hours
const MAX_FEE_BPS: u16 = 1000; // 10%
const SHIELDED_TREE_MAX_DEPTH: u8 = 20;
const SHIELDED_TREE_MAX_DEPTH_USIZE: usize = 20;
const SHIELDED_ZEROES: [[u8; 32]; 20] = [
    [32, 152, 245, 251, 158, 35, 158, 171, 60, 234, 195, 242, 123, 129, 228, 129, 220, 49, 36, 213, 95, 254, 213, 35, 168, 57, 238, 132, 70, 182, 72, 100],
    [13, 42, 45, 22, 205, 89, 26, 187, 101, 40, 94, 17, 180, 106, 87, 213, 202, 232, 251, 75, 0, 101, 203, 59, 250, 170, 20, 26, 206, 64, 19, 104],
    [7, 230, 128, 188, 164, 144, 187, 159, 30, 157, 9, 226, 43, 39, 19, 170, 154, 248, 17, 191, 202, 193, 138, 63, 48, 122, 4, 77, 41, 195, 73, 86],
    [26, 96, 199, 12, 249, 100, 63, 169, 166, 232, 102, 231, 38, 216, 134, 135, 103, 167, 75, 119, 158, 170, 117, 171, 53, 100, 112, 132, 68, 137, 93, 10],
    [35, 128, 16, 171, 32, 169, 232, 4, 174, 118, 77, 61, 79, 40, 79, 47, 232, 125, 196, 86, 9, 223, 129, 106, 190, 21, 102, 92, 138, 89, 109, 233],
    [44, 72, 21, 148, 130, 187, 172, 227, 120, 26, 173, 2, 24, 18, 27, 237, 220, 21, 100, 198, 176, 251, 252, 223, 68, 146, 25, 246, 109, 234, 204, 22],
    [20, 144, 239, 87, 158, 76, 47, 31, 235, 191, 118, 86, 98, 194, 98, 101, 10, 202, 52, 139, 195, 56, 66, 130, 156, 127, 239, 116, 41, 115, 4, 189],
    [2, 174, 165, 120, 72, 121, 181, 87, 218, 135, 152, 109, 57, 0, 63, 93, 211, 113, 234, 126, 198, 48, 133, 186, 54, 134, 94, 179, 44, 231, 11, 56],
    [38, 18, 236, 132, 199, 68, 152, 120, 180, 11, 100, 4, 241, 50, 151, 132, 43, 135, 13, 84, 177, 130, 87, 84, 144, 158, 43, 149, 189, 17, 233, 175],
    [46, 58, 251, 29, 26, 113, 8, 171, 63, 66, 175, 162, 246, 61, 231, 230, 50, 223, 180, 29, 62, 7, 117, 89, 60, 168, 141, 34, 218, 205, 155, 223],
    [47, 54, 75, 84, 109, 9, 110, 26, 213, 154, 151, 53, 145, 253, 60, 45, 29, 1, 232, 241, 88, 213, 111, 132, 231, 187, 240, 130, 79, 123, 234, 179],
    [41, 119, 29, 68, 205, 189, 160, 125, 44, 75, 37, 73, 50, 181, 75, 211, 177, 42, 255, 129, 196, 248, 161, 128, 128, 129, 22, 149, 142, 214, 241, 1],
    [15, 119, 124, 198, 191, 232, 68, 85, 38, 197, 122, 249, 255, 61, 22, 97, 82, 86, 92, 107, 244, 172, 132, 67, 235, 107, 184, 101, 202, 223, 144, 161],
    [8, 119, 83, 29, 39, 42, 224, 20, 107, 126, 103, 193, 129, 172, 179, 148, 81, 215, 227, 188, 113, 129, 71, 144, 21, 160, 87, 72, 117, 194, 239, 100],
    [46, 126, 88, 247, 224, 107, 203, 217, 98, 29, 24, 121, 215, 136, 205, 193, 27, 234, 0, 183, 154, 188, 177, 123, 58, 165, 93, 168, 14, 98, 38, 98],
    [30, 252, 182, 206, 135, 120, 223, 214, 244, 240, 57, 171, 6, 140, 6, 28, 19, 31, 20, 59, 233, 55, 36, 12, 43, 156, 143, 95, 148, 220, 239, 70],
    [40, 141, 85, 76, 44, 168, 81, 79, 81, 244, 221, 60, 70, 60, 133, 160, 173, 195, 241, 213, 156, 166, 238, 248, 183, 244, 164, 124, 224, 119, 173, 20],
    [16, 93, 55, 210, 6, 102, 160, 34, 253, 254, 183, 194, 234, 214, 181, 67, 62, 229, 205, 54, 73, 252, 78, 188, 166, 230, 101, 201, 65, 188, 153, 112],
    [10, 84, 243, 220, 193, 107, 4, 107, 122, 89, 109, 162, 19, 197, 87, 82, 26, 179, 104, 252, 46, 229, 18, 50, 202, 47, 180, 86, 159, 137, 10, 20],
    [31, 119, 64, 186, 175, 91, 166, 84, 77, 246, 87, 254, 66, 194, 157, 80, 60, 132, 108, 213, 146, 155, 176, 133, 163, 81, 163, 241, 180, 153, 121, 235],
];


declare_id!("EPpgM9ogD8wTVESMmin8kwemTmkVPQhPq9w1Mpz8Gxb7");

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

    pub fn initialize_shielded(
        ctx: Context<InitializeShielded>,
        tree_depth: u8,
    ) -> Result<()> {
        let program_id = ctx.program_id;
        let config_info = ctx.accounts.shielded_config.to_account_info();
        let tree_info = ctx.accounts.shielded_tree.to_account_info();
        let vault_info = ctx.accounts.shielded_vault.to_account_info();
        let authority_info = ctx.accounts.authority.to_account_info();
        let system_program_info = ctx.accounts.system_program.to_account_info();

        let (config_pda, config_bump) =
            Pubkey::find_program_address(&[b"shielded_config"], program_id);
        let (tree_pda, tree_bump) = Pubkey::find_program_address(&[b"shielded_tree"], program_id);
        let (vault_pda, vault_bump) =
            Pubkey::find_program_address(&[b"shielded_vault"], program_id);

        require_keys_eq!(config_pda, *config_info.key, ErrorCode::ConstraintSeeds);
        require_keys_eq!(tree_pda, *tree_info.key, ErrorCode::ConstraintSeeds);
        require_keys_eq!(vault_pda, *vault_info.key, ErrorCode::ConstraintSeeds);
        require_keys_eq!(
            system_program::ID,
            *system_program_info.key,
            ErrorCode::ConstraintAddress
        );
        require!(authority_info.is_writable, ErrorCode::ConstraintMut);
        require!(config_info.is_writable, ErrorCode::ConstraintMut);
        require!(tree_info.is_writable, ErrorCode::ConstraintMut);
        require!(vault_info.is_writable, ErrorCode::ConstraintMut);

        require!(
            tree_depth > 0 && tree_depth <= SHIELDED_TREE_MAX_DEPTH,
            DarkPoolError::InvalidTreeDepth
        );

        let config_uninitialized =
            config_info.owner == &system_program::ID && config_info.lamports() == 0;
        let tree_uninitialized = tree_info.owner == &system_program::ID && tree_info.lamports() == 0;
        let vault_uninitialized =
            vault_info.owner == &system_program::ID && vault_info.lamports() == 0;

        if config_uninitialized || tree_uninitialized || vault_uninitialized {
            require!(
                config_uninitialized && tree_uninitialized && vault_uninitialized,
                DarkPoolError::AccountAlreadyInitialized
            );
        } else {
            require!(
                config_info.owner == program_id,
                DarkPoolError::AccountAlreadyInitialized
            );
            require!(
                tree_info.owner == program_id,
                DarkPoolError::AccountAlreadyInitialized
            );
            require!(
                vault_info.owner == &system_program::ID,
                DarkPoolError::AccountAlreadyInitialized
            );

            let mut config_data: &[u8] = &config_info.try_borrow_data()?;
            let existing_config =
                ShieldedConfig::try_deserialize(&mut config_data).map_err(|_| {
                    error!(DarkPoolError::AccountAlreadyInitialized)
                })?;
            require_keys_eq!(
                existing_config.authority,
                ctx.accounts.authority.key(),
                ErrorCode::ConstraintSigner
            );
        }

        let config_space = (8 + ShieldedConfig::LEN) as u64;
        let config_lamports = Rent::get()?.minimum_balance(config_space as usize);
        let config_seeds: &[&[u8]] = &[b"shielded_config", &[config_bump]];
        let tree_space = (8 + ShieldedMerkleTree::LEN) as u64;
        let tree_lamports = Rent::get()?.minimum_balance(tree_space as usize);
        let tree_seeds: &[&[u8]] = &[b"shielded_tree", &[tree_bump]];
        let vault_seeds: &[&[u8]] = &[b"shielded_vault", &[vault_bump]];

        if config_uninitialized {
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
                    system_program_info.clone(),
                    system_program::CreateAccount {
                        from: authority_info.clone(),
                        to: tree_info.clone(),
                    },
                    &[tree_seeds],
                ),
                tree_lamports,
                tree_space,
                program_id,
            )?;

            system_program::create_account(
                CpiContext::new_with_signer(
                    system_program_info,
                    system_program::CreateAccount {
                        from: authority_info,
                        to: vault_info.clone(),
                    },
                    &[vault_seeds],
                ),
                Rent::get()?.minimum_balance(0),
                0,
                &system_program::ID,
            )?;
        }

        let zeroes = SHIELDED_ZEROES;
        let root = zeroes[(tree_depth as usize).saturating_sub(1)];
        let tree_state = ShieldedMerkleTree {
            depth: tree_depth,
            filled_subtrees: zeroes,
            zeroes,
            root,
            next_leaf_index: 0,
        };
        let mut tree_data = tree_info.try_borrow_mut_data()?;
        let mut tree_cursor: &mut [u8] = &mut tree_data;
        tree_state.try_serialize(&mut tree_cursor)?;

        let config_state = ShieldedConfig {
            authority: ctx.accounts.authority.key(),
            is_initialized: true,
            tree_depth,
            vault_bump,
            current_root: root,
            next_leaf_index: 0,
        };
        let mut config_data = config_info.try_borrow_mut_data()?;
        let mut config_cursor: &mut [u8] = &mut config_data;
        config_state.try_serialize(&mut config_cursor)?;

        emit!(ShieldedInitialized {
            authority: ctx.accounts.authority.key(),
            tree_depth,
        });

        Ok(())
    }

    pub fn deposit_shielded(
        ctx: Context<DepositShielded>,
        commitment: [u8; 32],
        amount: u64,
    ) -> Result<()> {
        let program_id = ctx.program_id;
        let config_info = ctx.accounts.shielded_config.to_account_info();
        let tree_info = ctx.accounts.shielded_tree.to_account_info();
        let vault_info = ctx.accounts.shielded_vault.to_account_info();
        let system_program_info = ctx.accounts.system_program.to_account_info();

        let (config_pda, _) = Pubkey::find_program_address(&[b"shielded_config"], program_id);
        let (tree_pda, _) = Pubkey::find_program_address(&[b"shielded_tree"], program_id);
        let (vault_pda, _) = Pubkey::find_program_address(&[b"shielded_vault"], program_id);

        require_keys_eq!(config_pda, *config_info.key, ErrorCode::ConstraintSeeds);
        require_keys_eq!(tree_pda, *tree_info.key, ErrorCode::ConstraintSeeds);
        require_keys_eq!(vault_pda, *vault_info.key, ErrorCode::ConstraintSeeds);
        require_keys_eq!(
            system_program::ID,
            *system_program_info.key,
            ErrorCode::ConstraintAddress
        );
        require!(config_info.is_writable, ErrorCode::ConstraintMut);
        require!(tree_info.is_writable, ErrorCode::ConstraintMut);
        require!(vault_info.is_writable, ErrorCode::ConstraintMut);
        require!(
            ctx.accounts.depositor.to_account_info().is_writable,
            ErrorCode::ConstraintMut
        );

        require!(
            ctx.accounts.shielded_config.is_initialized,
            DarkPoolError::ShieldedConfigNotInitialized
        );
        require!(amount > 0, DarkPoolError::InvalidAmount);
        require!(commitment != [0u8; 32], DarkPoolError::InvalidCommitment);
        require!(
            ctx.accounts.shielded_config.tree_depth > 0
                && ctx.accounts.shielded_config.tree_depth <= SHIELDED_TREE_MAX_DEPTH,
            DarkPoolError::InvalidTreeDepth
        );
        require!(
            ctx.accounts.shielded_tree.depth == ctx.accounts.shielded_config.tree_depth,
            DarkPoolError::InvalidTreeDepth
        );
        let max_leaves = max_leaves_for_depth(ctx.accounts.shielded_config.tree_depth)?;
        require!(
            ctx.accounts.shielded_config.next_leaf_index < max_leaves,
            DarkPoolError::ShieldedTreeFull
        );
        require!(
            ctx.accounts.shielded_tree.next_leaf_index
                == ctx.accounts.shielded_config.next_leaf_index,
            DarkPoolError::ShieldedStateMismatch
        );

        let ix = anchor_lang::solana_program::system_instruction::transfer(
            &ctx.accounts.depositor.key(),
            &ctx.accounts.shielded_vault.key(),
            amount,
        );
        anchor_lang::solana_program::program::invoke(
            &ix,
            &[
                ctx.accounts.depositor.to_account_info(),
                vault_info.clone(),
                system_program_info.clone(),
            ],
        )?;

        let leaf_index;
        let new_root;
        {
            let tree_state = &mut ctx.accounts.shielded_tree;
            let insert = merkle_insert(tree_state, commitment)?;
            leaf_index = insert.0;
            new_root = insert.1;
        }

        ctx.accounts.shielded_config.current_root = new_root;
        ctx.accounts.shielded_config.next_leaf_index =
            ctx.accounts.shielded_tree.next_leaf_index;

        emit!(ShieldedDeposit {
            commitment,
            amount,
            leaf_index,
            new_root,
        });

        Ok(())
    }

    pub fn spend_shielded(
        ctx: Context<SpendShielded>,
        nullifier: [u8; 32],
        amount: u64,
        root: [u8; 32],
    ) -> Result<()> {
        process_spend_shielded(ctx, nullifier, amount, root)
    }

    pub fn spend_shielded_with_proof(
        ctx: Context<SpendShielded>,
        nullifier: [u8; 32],
        amount: u64,
        root: [u8; 32],
        proof: Vec<u8>,
    ) -> Result<()> {
        verify_shielded_proof(&proof, &nullifier, amount, &root)?;
        process_spend_shielded(ctx, nullifier, amount, root)
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

        let mut drop_state = {
            let drop_data = drop_info.try_borrow_data()?;
            DropAccount::try_deserialize(&mut &drop_data[..])?
        };
        require_eq!(drop_state.bump, drop_bump, ErrorCode::ConstraintSeeds);

        let mut nullifier_state = {
            let nullifier_data = nullifier_info.try_borrow_data()?;
            NullifierAccount::try_deserialize(&mut &nullifier_data[..])?
        };
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

        let drop_state = {
            let drop_data = drop_info.try_borrow_data()?;
            DropAccount::try_deserialize(&mut &drop_data[..])?
        };
        require_eq!(drop_state.bump, drop_bump, ErrorCode::ConstraintSeeds);

        let nullifier_state = {
            let nullifier_data = nullifier_info.try_borrow_data()?;
            NullifierAccount::try_deserialize(&mut &nullifier_data[..])?
        };
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
}

pub struct Initialize<'info> {
    pub config: UncheckedAccount<'info>,
    pub authority: Signer<'info>,
    pub sol_vault: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

pub struct InitializeShielded<'info> {
    pub shielded_config: UncheckedAccount<'info>,
    pub authority: Signer<'info>,
    pub shielded_vault: UncheckedAccount<'info>,
    pub shielded_tree: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

impl<'info> Bumps for InitializeShielded<'info> {
    type Bumps = ();
}

impl<'info> Accounts<'info, ()> for InitializeShielded<'info> {
    fn try_accounts(
        program_id: &Pubkey,
        accounts: &mut &'info [AccountInfo<'info>],
        ix_data: &[u8],
        bumps: &mut (),
        reallocs: &mut BTreeSet<Pubkey>,
    ) -> Result<Self> {
        let shielded_config =
            UncheckedAccount::try_accounts(program_id, accounts, ix_data, bumps, reallocs)?;
        let authority = Signer::try_accounts(program_id, accounts, ix_data, bumps, reallocs)?;
        let shielded_vault =
            UncheckedAccount::try_accounts(program_id, accounts, ix_data, bumps, reallocs)?;
        let shielded_tree =
            UncheckedAccount::try_accounts(program_id, accounts, ix_data, bumps, reallocs)?;
        let system_program =
            Program::try_accounts(program_id, accounts, ix_data, bumps, reallocs)?;
        Ok(Self {
            shielded_config,
            authority,
            shielded_vault,
            shielded_tree,
            system_program,
        })
    }
}

impl<'info> ToAccountMetas for InitializeShielded<'info> {
    fn to_account_metas(&self, is_signer: Option<bool>) -> Vec<AccountMeta> {
        let mut metas = Vec::new();
        let override_signer = is_signer;
        metas.extend(self.shielded_config.to_account_metas(override_signer));
        metas.extend(self.authority.to_account_metas(override_signer));
        metas.extend(self.shielded_vault.to_account_metas(override_signer));
        metas.extend(self.shielded_tree.to_account_metas(override_signer));
        metas.extend(self.system_program.to_account_metas(override_signer));
        metas
    }
}

impl<'info> ToAccountInfos<'info> for InitializeShielded<'info> {
    fn to_account_infos(&self) -> Vec<AccountInfo<'info>> {
        let mut infos = Vec::new();
        infos.extend(self.shielded_config.to_account_infos());
        infos.extend(self.authority.to_account_infos());
        infos.extend(self.shielded_vault.to_account_infos());
        infos.extend(self.shielded_tree.to_account_infos());
        infos.extend(self.system_program.to_account_infos());
        infos
    }
}

impl<'info> AccountsExit<'info> for InitializeShielded<'info> {}

pub(crate) mod __client_accounts_initialize_shielded {
    use super::*;
    use anchor_lang::prelude::borsh;

    #[derive(anchor_lang::AnchorSerialize)]
    pub struct InitializeShielded {
        pub shielded_config: Pubkey,
        pub authority: Pubkey,
        pub shielded_vault: Pubkey,
        pub shielded_tree: Pubkey,
        pub system_program: Pubkey,
    }

    #[automatically_derived]
    impl anchor_lang::ToAccountMetas for InitializeShielded {
        fn to_account_metas(
            &self,
            _is_signer: Option<bool>,
        ) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
            vec![
                anchor_lang::solana_program::instruction::AccountMeta::new(
                    self.shielded_config,
                    false,
                ),
                anchor_lang::solana_program::instruction::AccountMeta::new(self.authority, true),
                anchor_lang::solana_program::instruction::AccountMeta::new(
                    self.shielded_vault,
                    false,
                ),
                anchor_lang::solana_program::instruction::AccountMeta::new(
                    self.shielded_tree,
                    false,
                ),
                anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                    self.system_program,
                    false,
                ),
            ]
        }
    }
}

pub struct DepositShielded<'info> {
    pub shielded_config: Account<'info, ShieldedConfig>,
    pub shielded_tree: Account<'info, ShieldedMerkleTree>,
    pub shielded_vault: SystemAccount<'info>,
    pub depositor: Signer<'info>,
    pub system_program: Program<'info, System>,
}

impl<'info> Bumps for DepositShielded<'info> {
    type Bumps = ();
}

impl<'info> Accounts<'info, ()> for DepositShielded<'info> {
    fn try_accounts(
        program_id: &Pubkey,
        accounts: &mut &'info [AccountInfo<'info>],
        ix_data: &[u8],
        bumps: &mut (),
        reallocs: &mut BTreeSet<Pubkey>,
    ) -> Result<Self> {
        let shielded_config =
            Account::try_accounts(program_id, accounts, ix_data, bumps, reallocs)?;
        let shielded_tree =
            Account::try_accounts(program_id, accounts, ix_data, bumps, reallocs)?;
        let shielded_vault =
            SystemAccount::try_accounts(program_id, accounts, ix_data, bumps, reallocs)?;
        let depositor = Signer::try_accounts(program_id, accounts, ix_data, bumps, reallocs)?;
        let system_program =
            Program::try_accounts(program_id, accounts, ix_data, bumps, reallocs)?;
        Ok(Self {
            shielded_config,
            shielded_tree,
            shielded_vault,
            depositor,
            system_program,
        })
    }
}

impl<'info> ToAccountMetas for DepositShielded<'info> {
    fn to_account_metas(&self, is_signer: Option<bool>) -> Vec<AccountMeta> {
        let mut metas = Vec::new();
        let override_signer = is_signer;
        metas.extend(self.shielded_config.to_account_metas(override_signer));
        metas.extend(self.shielded_tree.to_account_metas(override_signer));
        metas.extend(self.shielded_vault.to_account_metas(override_signer));
        metas.extend(self.depositor.to_account_metas(override_signer));
        metas.extend(self.system_program.to_account_metas(override_signer));
        metas
    }
}

impl<'info> ToAccountInfos<'info> for DepositShielded<'info> {
    fn to_account_infos(&self) -> Vec<AccountInfo<'info>> {
        let mut infos = Vec::new();
        infos.extend(self.shielded_config.to_account_infos());
        infos.extend(self.shielded_tree.to_account_infos());
        infos.extend(self.shielded_vault.to_account_infos());
        infos.extend(self.depositor.to_account_infos());
        infos.extend(self.system_program.to_account_infos());
        infos
    }
}

impl<'info> AccountsExit<'info> for DepositShielded<'info> {
    fn exit(&self, program_id: &Pubkey) -> Result<()> {
        self.shielded_config.exit(program_id)?;
        self.shielded_tree.exit(program_id)?;
        Ok(())
    }
}

pub(crate) mod __client_accounts_deposit_shielded {
    use super::*;
    use anchor_lang::prelude::borsh;

    #[derive(anchor_lang::AnchorSerialize)]
    pub struct DepositShielded {
        pub shielded_config: Pubkey,
        pub shielded_tree: Pubkey,
        pub shielded_vault: Pubkey,
        pub depositor: Pubkey,
        pub system_program: Pubkey,
    }

    #[automatically_derived]
    impl anchor_lang::ToAccountMetas for DepositShielded {
        fn to_account_metas(
            &self,
            _is_signer: Option<bool>,
        ) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
            vec![
                anchor_lang::solana_program::instruction::AccountMeta::new(
                    self.shielded_config,
                    false,
                ),
                anchor_lang::solana_program::instruction::AccountMeta::new(
                    self.shielded_tree,
                    false,
                ),
                anchor_lang::solana_program::instruction::AccountMeta::new(
                    self.shielded_vault,
                    false,
                ),
                anchor_lang::solana_program::instruction::AccountMeta::new(self.depositor, true),
                anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                    self.system_program,
                    false,
                ),
            ]
        }
    }
}

#[derive(Accounts)]
#[instruction(nullifier: [u8; 32])]
pub struct SpendShielded<'info> {
    #[account(mut)]
    pub shielded_config: Box<Account<'info, ShieldedConfig>>,
    #[account(mut)]
    pub shielded_tree: Box<Account<'info, ShieldedMerkleTree>>,
    #[account(mut)]
    pub shielded_vault: SystemAccount<'info>,
    #[account(mut)]
    pub nullifier_account: Signer<'info>,
    #[account(mut)]
    pub recipient: SystemAccount<'info>,
    #[account(mut)]
    pub spender: Signer<'info>,
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

fn max_leaves_for_depth(depth: u8) -> Result<u32> {
    require!(
        depth > 0 && depth <= SHIELDED_TREE_MAX_DEPTH,
        DarkPoolError::InvalidTreeDepth
    );
    1u32
        .checked_shl(depth as u32)
        .ok_or(error!(DarkPoolError::NumericalOverflow))
}

#[cfg(feature = "poseidon")]
fn poseidon_params() -> PoseidonParameters<Fr> {
    let ark = poseidon_constants_fr::POSEIDON_ARK_FR.to_vec();
    let mds = poseidon_constants_fr::POSEIDON_MDS_FR
        .iter()
        .map(|row| row.to_vec())
        .collect::<Vec<_>>();
    PoseidonParameters::new(ark, mds, 8, 57, 3, 5)
}

#[cfg(feature = "poseidon")]
fn fr_from_bytes(value: &[u8; 32]) -> Fr {
    <Fr as ark_ff::PrimeField>::from_be_bytes_mod_order(value)
}

#[cfg(feature = "poseidon")]
fn fr_to_bytes(value: &Fr) -> [u8; 32] {
    let mut out = [0u8; 32];
    let bytes = <Fr as ark_ff::PrimeField>::into_bigint(*value).to_bytes_be();
    let start = 32usize.saturating_sub(bytes.len());
    out[start..].copy_from_slice(&bytes);
    out
}

#[cfg(feature = "poseidon")]
fn hash_pair_bytes(left: &[u8; 32], right: &[u8; 32]) -> Result<[u8; 32]> {
    let mut poseidon = Poseidon::<Fr>::new(poseidon_params());
    let left_fr = fr_from_bytes(left);
    let right_fr = fr_from_bytes(right);
    let hash = poseidon
        .hash(&[left_fr, right_fr])
        .map_err(|_| error!(DarkPoolError::PoseidonHashFailed))?;
    Ok(fr_to_bytes(&hash))
}

#[cfg(not(feature = "poseidon"))]
fn hash_pair_bytes(left: &[u8; 32], right: &[u8; 32]) -> Result<[u8; 32]> {
    Ok(hashv(&[left, right]).to_bytes())
}

fn merkle_insert(
    tree: &mut ShieldedMerkleTree,
    commitment: [u8; 32],
) -> Result<(u32, [u8; 32])> {
    let depth = tree.depth as usize;
    require!(
        depth > 0 && depth <= SHIELDED_TREE_MAX_DEPTH_USIZE,
        DarkPoolError::InvalidTreeDepth
    );
    let max_leaves = max_leaves_for_depth(tree.depth)?;
    require!(
        tree.next_leaf_index < max_leaves,
        DarkPoolError::ShieldedTreeFull
    );

    let leaf_index = tree.next_leaf_index;
    let mut index = leaf_index;
    let mut current = commitment;
    for level in 0..depth {
        if index % 2 == 0 {
            tree.filled_subtrees[level] = current;
            current = hash_pair_bytes(&current, &tree.zeroes[level])?;
        } else {
            let left = tree.filled_subtrees[level];
            current = hash_pair_bytes(&left, &current)?;
        }
        index /= 2;
    }
    tree.root = current;
    tree.next_leaf_index = tree
        .next_leaf_index
        .checked_add(1)
        .ok_or(DarkPoolError::NumericalOverflow)?;
    Ok((leaf_index, current))
}

fn verify_shielded_proof(
    proof: &[u8],
    nullifier: &[u8; 32],
    amount: u64,
    root: &[u8; 32],
) -> Result<()> {
    // TODO: replace with actual verifier integration (e.g. Groth16/Plonk CPI).
    // For wiring, require non-empty proof and bind to public inputs with a hash.
    require!(!proof.is_empty(), DarkPoolError::InvalidShieldedProof);
    let _ = hashv(&[proof, nullifier, &amount.to_le_bytes(), root]);
    Ok(())
}

fn process_spend_shielded(
    ctx: Context<SpendShielded>,
    nullifier: [u8; 32],
    amount: u64,
    root: [u8; 32],
) -> Result<()> {
    let program_id = ctx.program_id;
    let config = &ctx.accounts.shielded_config;
    let tree = &ctx.accounts.shielded_tree;
    let vault_info = ctx.accounts.shielded_vault.to_account_info();
    let nullifier_info = ctx.accounts.nullifier_account.to_account_info();
    let recipient_info = ctx.accounts.recipient.to_account_info();
    let system_program_info = ctx.accounts.system_program.to_account_info();

    require!(config.is_initialized, DarkPoolError::ShieldedConfigNotInitialized);
    require!(amount > 0, DarkPoolError::InvalidAmount);
    require!(root == config.current_root, DarkPoolError::ShieldedRootMismatch);
    require!(root == tree.root, DarkPoolError::ShieldedStateMismatch);

    if recipient_info.owner == &system_program::ID
        && recipient_info.lamports() == 0
        && recipient_info.data_len() == 0
    {
        let rent_min = Rent::get()?.minimum_balance(0);
        require!(
            amount >= rent_min,
            DarkPoolError::ShieldedRecipientNotRentExempt
        );
    }

    let vault_lamports = **vault_info.lamports.borrow();
    require!(
        vault_lamports >= amount,
        DarkPoolError::InsufficientVaultBalance
    );

    let vault_seeds: &[&[u8]] = &[b"shielded_vault", &[config.vault_bump]];
    let ix = anchor_lang::solana_program::system_instruction::transfer(
        &vault_info.key(),
        &recipient_info.key(),
        amount,
    );
    anchor_lang::solana_program::program::invoke_signed(
        &ix,
        &[vault_info, recipient_info, system_program_info.clone()],
        &[vault_seeds],
    )?;

    let mut nullifier_state: ShieldedNullifier;
    if nullifier_info.owner == program_id {
        let mut data_slice: &[u8] = &nullifier_info.try_borrow_data()?;
        nullifier_state = ShieldedNullifier::try_deserialize(&mut data_slice)?;
    } else {
        let space = (8 + ShieldedNullifier::LEN) as u64;
        let rent_min = Rent::get()?.minimum_balance(space as usize);
        msg!(
            "shielded_nullifier pre-create key={} owner={} lamports={} data_len={} space={} rent_min={} payer={} payer_lamports={}",
            nullifier_info.key(),
            nullifier_info.owner,
            nullifier_info.lamports(),
            nullifier_info.data_len(),
            space,
            rent_min,
            ctx.accounts.spender.key(),
            ctx.accounts.spender.to_account_info().lamports()
        );
        require!(
            nullifier_info.owner == &system_program::ID && nullifier_info.lamports() == 0,
            DarkPoolError::AccountAlreadyInitialized
        );
        let lamports = rent_min + 1_000_000;
        system_program::create_account(
            CpiContext::new(
                system_program_info.clone(),
                system_program::CreateAccount {
                    from: ctx.accounts.spender.to_account_info(),
                    to: nullifier_info.clone(),
                },
            ),
            lamports,
            space,
            program_id,
        )?;
        msg!(
            "shielded_nullifier post-create key={} owner={} lamports={} data_len={} rent_min={} funded={}",
            nullifier_info.key(),
            nullifier_info.owner,
            nullifier_info.lamports(),
            nullifier_info.data_len(),
            rent_min,
            lamports
        );
        nullifier_state = ShieldedNullifier {
            nullifier: [0u8; 32],
            is_used: false,
        };
    }

    require!(
        !nullifier_state.is_used,
        DarkPoolError::NullifierAlreadyUsed
    );
    nullifier_state.nullifier = nullifier;
    nullifier_state.is_used = true;
    let mut nullifier_data = nullifier_info.try_borrow_mut_data()?;
    let pre_len = nullifier_data.len();
    msg!(
        "shielded_nullifier pre-write key={} lamports={} data_len={}",
        nullifier_info.key(),
        nullifier_info.lamports(),
        pre_len
    );
    let mut nullifier_cursor: &mut [u8] = &mut nullifier_data;
    nullifier_state.try_serialize(&mut nullifier_cursor)?;
    let remaining = nullifier_cursor.len();
    msg!(
        "shielded_nullifier post-write key={} lamports={} data_len={} remaining={}",
        nullifier_info.key(),
        nullifier_info.lamports(),
        pre_len,
        remaining
    );

    emit!(ShieldedSpent {
        nullifier,
        recipient: ctx.accounts.recipient.key(),
        amount,
        root,
    });

    Ok(())
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

#[account]
pub struct ShieldedConfig {
    pub authority: Pubkey,
    pub is_initialized: bool,
    pub tree_depth: u8,
    pub vault_bump: u8,
    pub current_root: [u8; 32],
    pub next_leaf_index: u32,
}

impl ShieldedConfig {
    pub const LEN: usize = 32 + 1 + 1 + 1 + 32 + 4;
}

#[account]
pub struct ShieldedMerkleTree {
    pub depth: u8,
    pub filled_subtrees: [[u8; 32]; 20],
    pub zeroes: [[u8; 32]; 20],
    pub root: [u8; 32],
    pub next_leaf_index: u32,
}

impl ShieldedMerkleTree {
    pub const LEN: usize = 1 + (32 * 20 * 2) + 32 + 4;
}

#[account]
pub struct ShieldedNullifier {
    pub nullifier: [u8; 32],
    pub is_used: bool,
}

impl ShieldedNullifier {
    pub const LEN: usize = 32 + 1;
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
pub struct ShieldedInitialized {
    pub authority: Pubkey,
    pub tree_depth: u8,
}

#[event]
pub struct ShieldedDeposit {
    pub commitment: [u8; 32],
    pub amount: u64,
    pub leaf_index: u32,
    pub new_root: [u8; 32],
}

#[event]
pub struct ShieldedSpent {
    pub nullifier: [u8; 32],
    pub recipient: Pubkey,
    pub amount: u64,
    pub root: [u8; 32],
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

    #[msg("Shielded config is not initialized")]
    ShieldedConfigNotInitialized,

    #[msg("Invalid shielded tree depth")]
    InvalidTreeDepth,

    #[msg("Shielded tree is full")]
    ShieldedTreeFull,

    #[msg("Invalid shielded commitment")]
    InvalidCommitment,

    #[msg("Shielded state mismatch")]
    ShieldedStateMismatch,

    #[msg("Shielded root mismatch")]
    ShieldedRootMismatch,

    #[msg("Recipient amount must cover rent exemption when creating a new account")]
    ShieldedRecipientNotRentExempt,

    #[msg("Invalid shielded proof")]
    InvalidShieldedProof,

    #[msg("Poseidon hash failed")]
    PoseidonHashFailed,
}

#[cfg(test)]
mod tests {
    use super::*;
}
