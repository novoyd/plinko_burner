use anchor_lang::prelude::*;
use anchor_spl::token::{Token, TokenAccount, CloseAccount, close_account, Burn, burn};

declare_id!("Cz4m7mpWX6nSUZxfKp2vjnHgYdF5rx9fmEwe9fWrabXd"); 

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

    /// Closes an empty SPL token account and sends the rent to the user's vault.
    /// Designed with ALT support in mind for batch operations in future stages.
    /// * Verifies the token account is empty (0 balance)
    /// * Closes the account using SPL Token program
    /// * Rent lamports are sent to the user's vault PDA
    pub fn close_token_account(ctx: Context<CloseTokenAccount>) -> Result<()> {
        let token_account = &ctx.accounts.token_account;
        let user = &ctx.accounts.user;
        
        // Security: Verify the token account owner matches the signer
        require!(
            token_account.owner == user.key(),
            BurnerError::UnauthorizedAccount
        );
        
        // Verify the token account is empty
        require!(
            token_account.amount == 0,
            BurnerError::AccountNotEmpty
        );
        
        msg!(
            "Closing token account - Mint: {}, Owner: {}",
            token_account.mint,
            token_account.owner
        );
        
        // Create CPI context for closing the token account
        let cpi_accounts = CloseAccount {
            account: ctx.accounts.token_account.to_account_info(),
            destination: ctx.accounts.vault.to_account_info(),
            authority: ctx.accounts.user.to_account_info(),
        };
        
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
        
        // Close the token account - rent goes to vault
        close_account(cpi_ctx)?;
        
        // Update vault lamports collected (optional tracking)
        let vault = &mut ctx.accounts.vault;
        let rent = Rent::get()?;
        let rent_lamports = rent.minimum_balance(TokenAccount::LEN);
        vault.lamports_collected = vault.lamports_collected.saturating_add(rent_lamports);
        
        msg!("Token account closed successfully, {} lamports sent to vault", rent_lamports);
        Ok(())
    }

    /// Burns all tokens in an account and then closes it.
    /// This is the main functionality for Stage 5 - burning standard SPL tokens.
    /// * Burns all tokens in the account to reduce total supply
    /// * Closes the empty account and sends rent to user's vault
    /// * Designed with ALT support in mind for batch operations
    pub fn burn_and_close_token_account(ctx: Context<BurnAndCloseTokenAccount>) -> Result<()> {
        let token_account = &ctx.accounts.token_account;
        let user = &ctx.accounts.user;
        
        // Security: Verify the token account owner matches the signer
        require!(
            token_account.owner == user.key(),
            BurnerError::UnauthorizedAccount
        );
        
        let token_amount = token_account.amount;
        
        msg!(
            "Burning and closing token account - Mint: {}, Amount: {}, Owner: {}",
            token_account.mint,
            token_amount,
            token_account.owner
        );
        
        // Only burn if there are tokens to burn
        if token_amount > 0 {
            // Create CPI context for burning tokens
            let burn_accounts = Burn {
                mint: ctx.accounts.mint.to_account_info(),
                from: ctx.accounts.token_account.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            };
            
            let burn_ctx = CpiContext::new(ctx.accounts.token_program.to_account_info(), burn_accounts);
            
            // Burn all tokens in the account
            burn(burn_ctx, token_amount)?;
            
            msg!("Burned {} tokens from mint {}", token_amount, token_account.mint);
        } else {
            msg!("No tokens to burn, proceeding to close account");
        }
        
        // Create CPI context for closing the token account
        let close_accounts = CloseAccount {
            account: ctx.accounts.token_account.to_account_info(),
            destination: ctx.accounts.vault.to_account_info(),
            authority: ctx.accounts.user.to_account_info(),
        };
        
        let close_ctx = CpiContext::new(ctx.accounts.token_program.to_account_info(), close_accounts);
        
        // Close the token account - rent goes to vault
        close_account(close_ctx)?;
        
        // Update vault lamports collected (optional tracking)
        let vault = &mut ctx.accounts.vault;
        let rent = Rent::get()?;
        let rent_lamports = rent.minimum_balance(TokenAccount::LEN);
        vault.lamports_collected = vault.lamports_collected.saturating_add(rent_lamports);
        
        msg!(
            "Burned {} tokens and closed account successfully, {} lamports sent to vault",
            token_amount,
            rent_lamports
        );
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

// Account context for `close_token_account`
// Designed to work efficiently with ALTs for batch operations
#[derive(Accounts)]
pub struct CloseTokenAccount<'info> {
    /// User who owns the token account
    #[account(mut)]
    pub user: Signer<'info>,
    
    /// SPL Token account to close (must be empty)
    /// Using AccountInfo instead of Account<TokenAccount> would be more ALT-friendly
    /// but Account<TokenAccount> provides better type safety for now
    #[account(mut)]
    pub token_account: Account<'info, TokenAccount>,
    
    /// User's vault PDA to receive the rent lamports
    #[account(
        mut,
        seeds = [b"vault", user.key().as_ref()],
        bump = vault.bump,
        constraint = vault.owner == user.key() @ BurnerError::InvalidOwner
    )]
    pub vault: Account<'info, VaultAccount>,
    
    /// SPL Token program
    pub token_program: Program<'info, Token>,
}

// Account context for `burn_and_close_token_account`
// Designed to work efficiently with ALTs for batch operations
#[derive(Accounts)]
pub struct BurnAndCloseTokenAccount<'info> {
    /// User who owns the token account
    #[account(mut)]
    pub user: Signer<'info>,
    
    /// SPL Token account to burn and close
    #[account(mut)]
    pub token_account: Account<'info, TokenAccount>,
    
    /// The mint of the token (required for burning)
    #[account(mut)]
    pub mint: Account<'info, anchor_spl::token::Mint>,
    
    /// User's vault PDA to receive the rent lamports
    #[account(
        mut,
        seeds = [b"vault", user.key().as_ref()],
        bump = vault.bump,
        constraint = vault.owner == user.key() @ BurnerError::InvalidOwner
    )]
    pub vault: Account<'info, VaultAccount>,
    
    /// SPL Token program
    pub token_program: Program<'info, Token>,
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
    
    #[msg("Token account is not empty")] // thrown when trying to close non-empty account
    AccountNotEmpty,
}