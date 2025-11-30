use anchor_lang::prelude::*;

const MAX_EXPIRATION_WINDOW: i64 = 30 * 24 * 60 * 60; // 30 days
const MIN_EXPIRATION_WINDOW: i64 = 60; // 1 minute
const MIN_RATE_LIMIT_SECONDS: i64 = 10;
const MIN_AUTHORITY_DELAY_SECONDS: i64 = 15 * 60; // 15 minutes
const MAX_AUTHORITY_DELAY_SECONDS: i64 = 7 * 24 * 60 * 60; // 7 days
const DEFAULT_AUTHORITY_DELAY_SECONDS: i64 = 24 * 60 * 60; // 24 hours

declare_id!("95XwPFvP6znDJN2XS4JRp29NjUNGEKDAmCRfKaZEzNfw");

#[program]
pub mod darkdrop {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        let config = &mut ctx.accounts.config;
        config.authority = ctx.accounts.authority.key();
        config.is_initialized = true;
        config.pending_authority = Pubkey::default();
        config.pending_authority_set_at = 0;
        config.authority_delay_seconds = DEFAULT_AUTHORITY_DELAY_SECONDS;
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

        require!(
            ctx.accounts.config.is_initialized,
            DarkDropError::ConfigNotInitialized
        );
        require!(
            ctx.accounts.config.authority == ctx.accounts.payer.key(),
            DarkDropError::UnauthorizedCreator
        );
        require!(amount > 0, DarkDropError::InvalidAmount);
        require!(asset_type <= 1, DarkDropError::InvalidAssetType);
        require!(
            recipient != ctx.accounts.payer.key(),
            DarkDropError::InvalidRecipient
        );
        require!(
            expires_at > now
                && expires_at - now >= MIN_EXPIRATION_WINDOW
                && expires_at - now <= MAX_EXPIRATION_WINDOW,
            DarkDropError::InvalidExpiration
        );

        let rate_limit_account = &mut ctx.accounts.rate_limit_account;
        if rate_limit_account.last_drop_at != 0 {
            require!(
                now - rate_limit_account.last_drop_at >= MIN_RATE_LIMIT_SECONDS,
                DarkDropError::RateLimitExceeded
            );
        }
        rate_limit_account.last_drop_at = now;
        rate_limit_account.bump = ctx.bumps.rate_limit_account;

        let drop = &mut ctx.accounts.drop;
        drop.nullifier = nullifier;
        drop.recipient = recipient;
        drop.amount = amount;
        drop.asset_type = asset_type;
        drop.created_at = now;
        drop.expires_at = expires_at;
        drop.status = DropStatus::Active;
        drop.claimed_at = 0;
        drop.claimer = Pubkey::default();
        drop.bump = ctx.bumps.drop;

        let nullifier_account = &mut ctx.accounts.nullifier_account;
        if nullifier_account.nullifier == [0u8; 32] {
            // Only initialize if it's a new account
            nullifier_account.nullifier = nullifier;
            nullifier_account.is_used = false;
            nullifier_account.claimer = Pubkey::default();
            nullifier_account.used_at = 0;
            nullifier_account.bump = ctx.bumps.nullifier_account;
        }

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

    pub fn claim_drop(ctx: Context<ClaimDrop>, nullifier: [u8; 32]) -> Result<()> {
        let drop = &mut ctx.accounts.drop;
        let nullifier_account = &mut ctx.accounts.nullifier_account;
        let clock = Clock::get()?;
        let now = clock.unix_timestamp;

        require!(
            drop.status == DropStatus::Active,
            DarkDropError::DropNotActive
        );

        require!(
            !nullifier_account.is_used,
            DarkDropError::NullifierAlreadyUsed
        );

        require!(drop.nullifier == nullifier, DarkDropError::InvalidNullifier);

        if drop.expires_at != 0 && now > drop.expires_at {
            drop.status = DropStatus::Expired;
            return err!(DarkDropError::DropExpired);
        }

        drop.status = DropStatus::Claimed;
        drop.claimed_at = now;
        drop.claimer = ctx.accounts.claimer.key();

        nullifier_account.is_used = true;
        nullifier_account.used_at = now;
        nullifier_account.claimer = ctx.accounts.claimer.key();

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
        let config = &mut ctx.accounts.config;
        require!(
            ctx.accounts.authority.key() == config.authority,
            DarkDropError::UnauthorizedCreator
        );
        require!(
            new_authority != Pubkey::default(),
            DarkDropError::InvalidAuthority
        );
        require!(
            config.pending_authority == Pubkey::default(),
            DarkDropError::PendingAuthorityExists
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
        let config = &mut ctx.accounts.config;
        require!(
            ctx.accounts.authority.key() == config.authority,
            DarkDropError::UnauthorizedCreator
        );
        require!(
            config.pending_authority != Pubkey::default(),
            DarkDropError::NoPendingAuthority
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
        let config = &mut ctx.accounts.config;
        require!(
            config.pending_authority == ctx.accounts.pending_authority.key(),
            DarkDropError::NoPendingAuthority
        );
        let now = Clock::get()?.unix_timestamp;
        require!(
            config.pending_authority_set_at > 0,
            DarkDropError::NoPendingAuthority
        );
        require!(
            now - config.pending_authority_set_at >= config.authority_delay_seconds,
            DarkDropError::AuthorityDelayNotElapsed
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
        require!(
            ctx.accounts.authority.key() == ctx.accounts.config.authority,
            DarkDropError::UnauthorizedCreator
        );
        require!(
            new_delay_seconds >= MIN_AUTHORITY_DELAY_SECONDS
                && new_delay_seconds <= MAX_AUTHORITY_DELAY_SECONDS,
            DarkDropError::InvalidAuthorityDelay
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
        let drop = &ctx.accounts.drop;
        let nullifier_account = &ctx.accounts.nullifier_account;
        let clock = Clock::get()?;
        let now = clock.unix_timestamp;

        require!(
            ctx.accounts.config.authority == ctx.accounts.authority.key(),
            DarkDropError::UnauthorizedCreator
        );
        require!(drop.nullifier == nullifier, DarkDropError::InvalidNullifier);
        require!(
            drop.status == DropStatus::Active,
            DarkDropError::DropNotActive
        );
        require!(
            drop.expires_at != 0 && now > drop.expires_at,
            DarkDropError::DropNotExpired
        );
        require!(
            !nullifier_account.is_used,
            DarkDropError::NullifierAlreadyUsed
        );

        emit!(DropExpired {
            nullifier,
            recipient: drop.recipient,
            expires_at: drop.expires_at,
        });

        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + Config::LEN,
        seeds = [b"config"],
        bump
    )]
    pub config: Account<'info, Config>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(nullifier: [u8; 32])]
pub struct CreateDrop<'info> {
    #[account(
        init,
        payer = payer,
        space = 8 + DropAccount::LEN,
        seeds = [b"drop", nullifier.as_ref()],
        bump
    )]
    pub drop: Account<'info, DropAccount>,

    #[account(
        init_if_needed,
        payer = payer,
        space = 8 + NullifierAccount::LEN,
        seeds = [b"nullifier", nullifier.as_ref()],
        bump
    )]
    pub nullifier_account: Account<'info, NullifierAccount>,

    #[account(
        seeds = [b"config"],
        bump
    )]
    pub config: Account<'info, Config>,

    #[account(
        init_if_needed,
        payer = payer,
        space = 8 + RateLimitAccount::LEN,
        seeds = [b"rate_limit", payer.key().as_ref()],
        bump
    )]
    pub rate_limit_account: Account<'info, RateLimitAccount>,

    #[account(mut)]
    pub payer: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(nullifier: [u8; 32])]
pub struct ClaimDrop<'info> {
    #[account(
        mut,
        seeds = [b"drop", nullifier.as_ref()],
        bump = drop.bump
    )]
    pub drop: Account<'info, DropAccount>,

    #[account(
        mut,
        seeds = [b"nullifier", nullifier.as_ref()],
        bump = nullifier_account.bump
    )]
    pub nullifier_account: Account<'info, NullifierAccount>,

    #[account(mut)]
    pub claimer: Signer<'info>,
}

#[derive(Accounts)]
pub struct ProposeAuthority<'info> {
    #[account(
        mut,
        seeds = [b"config"],
        bump
    )]
    pub config: Account<'info, Config>,
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct CancelAuthorityProposal<'info> {
    #[account(
        mut,
        seeds = [b"config"],
        bump
    )]
    pub config: Account<'info, Config>,
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct AcceptAuthority<'info> {
    #[account(
        mut,
        seeds = [b"config"],
        bump
    )]
    pub config: Account<'info, Config>,
    pub pending_authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct UpdateAuthorityDelay<'info> {
    #[account(
        mut,
        seeds = [b"config"],
        bump
    )]
    pub config: Account<'info, Config>,
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
#[instruction(nullifier: [u8; 32])]
pub struct ExpireDrop<'info> {
    #[account(
        mut,
        seeds = [b"drop", nullifier.as_ref()],
        bump = drop.bump,
        close = rent_collector
    )]
    pub drop: Account<'info, DropAccount>,

    #[account(
        mut,
        seeds = [b"nullifier", nullifier.as_ref()],
        bump = nullifier_account.bump,
        close = rent_collector
    )]
    pub nullifier_account: Account<'info, NullifierAccount>,

    #[account(
        seeds = [b"config"],
        bump
    )]
    pub config: Account<'info, Config>,

    pub authority: Signer<'info>,

    /// CHECK: Rent collector receives closed account rent. No validation needed.
    #[account(mut)]
    pub rent_collector: UncheckedAccount<'info>,
}

#[account]
pub struct Config {
    pub authority: Pubkey,
    pub is_initialized: bool,
    pub pending_authority: Pubkey,
    pub pending_authority_set_at: i64,
    pub authority_delay_seconds: i64,
}

impl Config {
    pub const LEN: usize = 32 + 1 + 32 + 8 + 8;
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
pub enum DarkDropError {
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
}

#[cfg(test)]
mod tests {
    use super::*;
}
