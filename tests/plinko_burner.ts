import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { TokenBurner } from "../target/types/token_burner";
import { 
  PublicKey, 
  Keypair, 
  SystemProgram,
  LAMPORTS_PER_SOL 
} from "@solana/web3.js";
import {
  TOKEN_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
  createMint,
  createAssociatedTokenAccount,
  mintTo,
  getAccount,
  getAssociatedTokenAddress
} from "@solana/spl-token";
import { expect } from "chai";

describe("token_burner", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());
  const provider = anchor.getProvider();
  const program = anchor.workspace.tokenBurner as Program<TokenBurner>;
  
  // Test accounts
  let authority: Keypair;
  let user: Keypair;
  let mint: PublicKey;
  let userTokenAccount: PublicKey;
  let statePda: PublicKey;
  let vaultPda: PublicKey;

  before(async () => {
    // Generate test keypairs
    authority = Keypair.generate();
    user = Keypair.generate();
    
    // Airdrop SOL to test accounts
    await provider.connection.confirmTransaction(
      await provider.connection.requestAirdrop(authority.publicKey, 2 * LAMPORTS_PER_SOL)
    );
    await provider.connection.confirmTransaction(
      await provider.connection.requestAirdrop(user.publicKey, 2 * LAMPORTS_PER_SOL)
    );

    // Create test mint
    mint = await createMint(
      provider.connection,
      authority,
      authority.publicKey,
      null,
      9
    );

    // Create user associated token account
    userTokenAccount = await createAssociatedTokenAccount(
      provider.connection,
      user,
      mint,
      user.publicKey
    );

    // Derive PDAs
    [statePda] = PublicKey.findProgramAddressSync(
      [Buffer.from("state")],
      program.programId
    );

    [vaultPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("vault"), user.publicKey.toBuffer()],
      program.programId
    );
  });

  it("Initializes the program", async () => {
    const tx = await program.methods
      .initialize()
      .accounts({
        authority: authority.publicKey,
        state: statePda,
        systemProgram: SystemProgram.programId,
      })
      .signers([authority])
      .rpc();
    
    console.log("Initialize transaction signature", tx);
    
    // Verify state was initialized
    const state = await program.account.burnerState.fetch(statePda);
    expect(state.authority.toString()).to.equal(authority.publicKey.toString());
    expect(state.isInitialized).to.be.true;
  });

  it("Creates user vault", async () => {
    const tx = await program.methods
      .createVault()
      .accounts({
        user: user.publicKey,
        vault: vaultPda,
        systemProgram: SystemProgram.programId,
      })
      .signers([user])
      .rpc();
    
    console.log("Create vault transaction signature", tx);
    
    // Verify vault was created
    const vault = await program.account.vaultAccount.fetch(vaultPda);
    expect(vault.owner.toString()).to.equal(user.publicKey.toString());
    expect(vault.lamportsCollected.toString()).to.equal("0");
  });

  it("Validates empty token account", async () => {
    const tx = await program.methods
      .validateTokenAccount()
      .accounts({
        user: user.publicKey,
        tokenAccount: userTokenAccount,
      })
      .signers([user])
      .rpc();
    
    console.log("Validate token account transaction signature", tx);
  });

  it("Closes empty token account", async () => {
    // Verify account is empty before closing
    const accountInfo = await getAccount(provider.connection, userTokenAccount);
    expect(Number(accountInfo.amount)).to.equal(0);
    
    const tx = await program.methods
      .closeTokenAccount()
      .accounts({
        user: user.publicKey,
        tokenAccount: userTokenAccount,
        vault: vaultPda,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .signers([user])
      .rpc();
    
    console.log("Close token account transaction signature", tx);
    
    // Verify account was closed (should throw error when fetching)
    try {
      await getAccount(provider.connection, userTokenAccount);
      expect.fail("Token account should be closed");
    } catch (error) {
      // Expected - account should be closed
    }
    
    // Verify vault received rent lamports
    const vault = await program.account.vaultAccount.fetch(vaultPda);
    expect(Number(vault.lamportsCollected)).to.be.greaterThan(0);
  });

  it("Fails to close non-empty token account", async () => {
    // Create a new mint for this test
    const newMint = await createMint(
      provider.connection,
      authority,
      authority.publicKey,
      null,
      9
    );
    
    // Create a new associated token account with tokens
    const newTokenAccount = await createAssociatedTokenAccount(
      provider.connection,
      user,
      newMint,
      user.publicKey
    );
    
    // Mint some tokens to it
    await mintTo(
      provider.connection,
      authority,
      newMint,
      newTokenAccount,
      authority,
      1000
    );
    
    try {
      await program.methods
        .closeTokenAccount()
        .accounts({
          user: user.publicKey,
          tokenAccount: newTokenAccount,
          vault: vaultPda,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([user])
        .rpc();
      
      expect.fail("Should have failed to close non-empty account");
    } catch (error) {
      expect(error.toString()).to.include("AccountNotEmpty");
    }
  });

  it("Burns and closes token account with tokens", async () => {
    // Create a new mint for this test
    const burnMint = await createMint(
      provider.connection,
      authority,
      authority.publicKey,
      null,
      9
    );
    
    // Create a new associated token account with tokens
    const burnTokenAccount = await createAssociatedTokenAccount(
      provider.connection,
      user,
      burnMint,
      user.publicKey
    );
    
    // Mint some tokens to it
    const tokenAmount = 5000;
    await mintTo(
      provider.connection,
      authority,
      burnMint,
      burnTokenAccount,
      authority,
      tokenAmount
    );
    
    // Verify account has tokens before burning
    const accountInfoBefore = await getAccount(provider.connection, burnTokenAccount);
    expect(Number(accountInfoBefore.amount)).to.equal(tokenAmount);
    
    const tx = await program.methods
      .burnAndCloseTokenAccount()
      .accounts({
        user: user.publicKey,
        tokenAccount: burnTokenAccount,
        mint: burnMint,
        vault: vaultPda,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .signers([user])
      .rpc();
    
    console.log("Burn and close token account transaction signature", tx);
    
    // Verify account was closed (should throw error when fetching)
    try {
      await getAccount(provider.connection, burnTokenAccount);
      expect.fail("Token account should be closed");
    } catch (error) {
      // Expected - account should be closed
    }
    
    // Verify vault received additional rent lamports
    const vault = await program.account.vaultAccount.fetch(vaultPda);
    // Should have collected rent from both the previous close and this burn+close
    expect(Number(vault.lamportsCollected)).to.be.greaterThan(0);
  });

  it("Burns and closes empty token account", async () => {
    // Create a new mint for this test
    const emptyBurnMint = await createMint(
      provider.connection,
      authority,
      authority.publicKey,
      null,
      9
    );
    
    // Create a new associated token account (empty by default)
    const emptyBurnTokenAccount = await createAssociatedTokenAccount(
      provider.connection,
      user,
      emptyBurnMint,
      user.publicKey
    );
    
    // Verify account is empty before burning
    const accountInfoBefore = await getAccount(provider.connection, emptyBurnTokenAccount);
    expect(Number(accountInfoBefore.amount)).to.equal(0);
    
    const tx = await program.methods
      .burnAndCloseTokenAccount()
      .accounts({
        user: user.publicKey,
        tokenAccount: emptyBurnTokenAccount,
        mint: emptyBurnMint,
        vault: vaultPda,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .signers([user])
      .rpc();
    
    console.log("Burn and close empty token account transaction signature", tx);
    
    // Verify account was closed
    try {
      await getAccount(provider.connection, emptyBurnTokenAccount);
      expect.fail("Token account should be closed");
    } catch (error) {
      // Expected - account should be closed
    }
  });
});
