use anchor_lang::prelude::*;
use anchor_spl::token::{Token, TokenAccount};

declare_id!("3v2dzYGyixvk3aWigSWi5nMMhHAfyQtNe4Rx21gjVyS5"); 

#[program]
pub mod token_burner {
    use super::*;

    /// Creates and initializes the on-chain `BurnerState` account.
    /// * authority  – wallet that governs future upgrades or admin ops
    /// * state PDA – stores config + timestamp
    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        let state = &mut ctx.accounts.state;           // mutable alias to PDA
        let clock = Clock::get()?;                     // current cluster time

        // Populate persistent fields
        state.authority      = ctx.accounts.authority.key(); //admin address
        state.is_initialized = true;                   // sanity flag
        state.created_at     = clock.unix_timestamp;   // cluster time

        msg!("Token Burner initialized with authority: {}", state.authority);
        Ok(())
    }

    /// Creates a vault PDA so the caller can later receive rent refunds.
    pub fn create_vault(ctx: Context<CreateVault>) -> Result<()> {
        let vault = &mut ctx.accounts.vault;

        vault.owner              = ctx.accounts.user.key(); // vault owner
        vault.bump               = ctx.bumps.vault;         // PDA bump
        vault.lamports_collected = 0;                       // optional tally

        msg!("Vault created for user: {}", vault.owner);
        Ok(())
    }

    /// Withdraws lamports above the rent‑exempt minimum from the vault to the caller.
    pub fn withdraw_vault(ctx: Context<WithdrawVault>) -> Result<()> {
        let vault_ai = ctx.accounts.vault.to_account_info();
        let user_ai  = ctx.accounts.user.to_account_info();

        let rent_floor = Rent::get()?.minimum_balance(vault_ai.data_len());
        let withdrawable = vault_ai.lamports().saturating_sub(rent_floor);

        if withdrawable > 0 {
            // Manual lamport transfer, PDA → user wallet
            **vault_ai.try_borrow_mut_lamports()? -= withdrawable;
            **user_ai.try_borrow_mut_lamports()?  += withdrawable;
            msg!("Withdrew {} lamports to user", withdrawable);
        } else {
            msg!("No lamports to withdraw");
        }
        Ok(())
    }

    /// Validates a single token account for future burning/closing.
    /// * Checks ownership matches the signer
    /// * Verifies it's a real SPL token account  
    /// * Logs basic account info
    pub fn validate_token_account(ctx: Context<ValidateTokenAccount>) -> Result<()> {
        let token_account = &ctx.accounts.token_account;
        let user = &ctx.accounts.user;
        
        // Security: Verify the token account owner matches the signer
        require!(
            token_account.owner == user.key(),
            BurnerError::UnauthorizedAccount
        );
        
        // Log account details for debugging
        msg!(
            "Valid token account - Mint: {}, Balance: {}, Owner: {}",
            token_account.mint,
            token_account.amount,
            token_account.owner
        );
        
        // Check if account is empty (will be useful in later stages)
        if token_account.amount == 0 {
            msg!("Token account is empty and ready to close");
        } else {
            msg!("Token account has {} tokens", token_account.amount);
        }
        
        Ok(())
    }
}

// Account context for `initialize`
#[derive(Accounts)]
pub struct Initialize<'info> {
    /// payer & future authority must sign to create the state PDA.
    #[account(mut)]
    pub authority: Signer<'info>,
    /// Program-Derived Account that stores state.
    ///   • `init`       – create it if it doesn't exist
    ///   • `payer`      – who funds rent
    ///   • `space`      – bytes to allocate (8-byte Anchor discriminator + our struct)
    ///   • `seeds`      – derive address from static seed `b"state"`
    ///   • `bump`       – auto-adds the bump so the derive matches on-chain 
    #[account(
        init,
        payer = authority,
        space = 8 + BurnerState::INIT_SPACE,   // 8‑byte discriminator + struct size
        seeds = [b"state"],
        bump
    )]
    pub state: Account<'info, BurnerState>,   
     
    /// System program (required by `init` to create accounts)
    pub system_program: Program<'info, System>,
}

// Account context for `create_vault`
#[derive(Accounts)]
pub struct CreateVault<'info> {
    #[account(mut)]
    pub user: Signer<'info>, // wallet creating the vault

    #[account(
        init,
        payer = user,
        space = 8 + VaultAccount::INIT_SPACE,
        seeds = [b"vault", user.key().as_ref()],
        bump
    )]
    pub vault: Account<'info, VaultAccount>, // vault PDA derived from ("vault", user)

    pub system_program: Program<'info, System>,
}

// Account context for `withdraw_vault`
#[derive(Accounts)]
pub struct WithdrawVault<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        seeds = [b"vault", user.key().as_ref()],
        bump = vault.bump,
        constraint = vault.owner == user.key() @ BurnerError::InvalidOwner
    )]
    pub vault: Account<'info, VaultAccount>, // caller's vault PDA, must match owner
}

// Account context for `validate_token_account`
#[derive(Accounts)]
pub struct ValidateTokenAccount<'info> {
    /// User who owns the token account
    pub user: Signer<'info>,
    
    /// SPL Token account to validate
    /// Anchor's Account<TokenAccount> automatically:
    /// • Verifies it's owned by the Token Program
    /// • Deserializes the account data
    /// • Makes fields like mint, owner, amount available
    pub token_account: Account<'info, TokenAccount>,
}

// Persistent data layout – one instance lives at the `state` PDA
#[account]
#[derive(InitSpace)]
pub struct BurnerState {
    pub authority: Pubkey,   // who can administer the contract
    pub is_initialized: bool,
    pub created_at: i64,     // Unix timestamp
}

// Per‑user vault PDA – mainly holds lamports, plus metadata
#[account]
#[derive(InitSpace)]
pub struct VaultAccount {
    pub owner: Pubkey,           // user controlling withdrawals
    pub bump: u8,                // PDA bump
    pub lamports_collected: u64, // optional stats
}

#[error_code]
pub enum BurnerError {
    #[msg("Invalid owner")] // thrown when caller != vault.owner
    InvalidOwner,
    
    #[msg("Token account not owned by user")] // thrown when token account owner != signer
    UnauthorizedAccount,
}