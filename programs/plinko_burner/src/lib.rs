use anchor_lang::prelude::*;          // Pulls the full Anchor SDK into scope

declare_id!("3v2dzYGyixvk3aWigSWi5nMMhHAfyQtNe4Rx21gjVyS5");

// Program module – all on-chain handlers live inside `token_burner`
#[program]
pub mod token_burner {
    use super::*;

    /// Creates and initializes the on-chain `BurnerState` account.
    /// * *authority*  – the wallet that can govern future upgrades or admin ops
    /// * *state PDA* – stores config + timestamp
    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        // Mutable reference to the newly created state account
        let state = &mut ctx.accounts.state;

        // Clock gives us access to the current slot time
        let clock = Clock::get()?;

        // Populate persistent fields
        state.authority      = ctx.accounts.authority.key();   // admin address
        state.is_initialized = true;                           // simple sanity flag
        state.created_at     = clock.unix_timestamp;           // cluster time

        
        msg!("Token Burner initialized with authority: {}", state.authority);

        Ok(())
    }
}

// Account context for `initialize`
#[derive(Accounts)]
pub struct Initialize<'info> {
    /// Payer & future authority; must sign to create the state PDA.
    #[account(mut)]
    pub authority: Signer<'info>,

    /// Program-Derived Account that stores state.
    ///   • `init`       – create it if it doesn’t exist
    ///   • `payer`      – who funds rent
    ///   • `space`      – bytes to allocate (8-byte Anchor discriminator + our struct)
    ///   • `seeds`      – derive address from static seed `b"state"`
    ///   • `bump`       – auto-adds the bump so the derive matches on-chain
    #[account(
        init,
        payer = authority,
        space = 8 + BurnerState::INIT_SPACE,   // 8 bytes discriminator + struct size
        seeds = [b"state"],
        bump
    )]
    pub state: Account<'info, BurnerState>,

    /// System program (required by `init` to create accounts)
    pub system_program: Program<'info, System>,
}


// Persistent data layout – one instance lives at the `state` PDA
#[account]
#[derive(InitSpace)]   // auto-calculates `INIT_SPACE` constant for `space =`
pub struct BurnerState {
    pub authority: Pubkey,   // who can administer the contract
    pub is_initialized: bool,
    pub created_at: i64,     // Unix timestamp
    // Adding future fields below; need to bump INIT_SPACE when I do
}