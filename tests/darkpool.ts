import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Darkpool } from "../target/types/darkpool";
import { PublicKey, Keypair, SystemProgram } from "@solana/web3.js";
import { expect } from "chai";
import { BN } from "@coral-xyz/anchor";

// Declare global for browser compatibility
declare global {
  interface Window {
    crypto: Crypto;
  }
}

describe("darkpool", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.Darkpool as Program<Darkpool>;
  const authority = provider.wallet;
  const recipient = Keypair.generate();
  const claimer = Keypair.generate();
  const treasury = Keypair.generate();

  let configPDA: PublicKey;
  let solVaultPDA: PublicKey;
  let nullifier: Uint8Array;
  let dropPDA: PublicKey;
  let nullifierPDA: PublicKey;

  before(async () => {
    // Generate a test nullifier
    nullifier = new Uint8Array(32);
    // Use crypto.getRandomValues for browser or crypto.randomBytes for Node
    if (typeof window !== "undefined" && window.crypto) {
      window.crypto.getRandomValues(nullifier);
    } else {
      const crypto = await import("crypto");
      crypto.randomFillSync(nullifier);
    }

    // Derive PDAs
    [configPDA] = PublicKey.findProgramAddressSync(
      [Buffer.from("config")],
      program.programId
    );
    [solVaultPDA] = PublicKey.findProgramAddressSync(
      [Buffer.from("sol_vault")],
      program.programId
    );

    [dropPDA] = PublicKey.findProgramAddressSync(
      [Buffer.from("drop"), Buffer.from(nullifier)],
      program.programId
    );

    [nullifierPDA] = PublicKey.findProgramAddressSync(
      [Buffer.from("nullifier"), Buffer.from(nullifier)],
      program.programId
    );
  });

  it("Initializes the program", async () => {
    const tx = await program.methods
      .initialize(treasury.publicKey, 0)
      .accounts({
        config: configPDA,
        authority: authority.publicKey,
        solVault: solVaultPDA,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    const config = await program.account.config.fetch(configPDA);
    expect(config.authority.toString()).to.equal(authority.publicKey.toString());
    expect(config.isInitialized).to.be.true;
  });

  it("Deposits to pool (SOL)", async () => {
    const clock = await provider.connection.getSlot();
    const blockTime = await provider.connection.getBlockTime(clock);
    const expiresAt = new BN(blockTime! + 3600); // 1 hour from now

    const [rateLimitPDA] = PublicKey.findProgramAddressSync(
      [Buffer.from("rate_limit"), authority.publicKey.toBuffer()],
      program.programId
    );

    const tx = await program.methods
      .depositPool(
        Array.from(nullifier),
        new BN(1000000), // 0.001 SOL (in lamports)
        0, // SOL
        expiresAt
      )
      .accounts({
        drop: dropPDA,
        nullifierAccount: nullifierPDA,
        config: configPDA,
        rateLimitAccount: rateLimitPDA,
        payer: authority.publicKey,
        solVault: solVaultPDA,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    const drop = await program.account.dropAccount.fetch(dropPDA);
    expect(drop.nullifier).to.deep.equal(Array.from(nullifier));
    expect(drop.recipient.toString()).to.equal(authority.publicKey.toString());
    expect(drop.amount.toNumber()).to.equal(1000000);
    expect(drop.status.active).to.not.be.undefined;
  });

  it("Claims a pool drop and prevents double-claim", async () => {
    await program.methods
      .claimDrop(Array.from(nullifier))
      .accounts({
        drop: dropPDA,
        nullifierAccount: nullifierPDA,
        claimer: claimer.publicKey,
        config: configPDA,
        solVault: solVaultPDA,
        treasury: treasury.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([claimer])
      .rpc();

    // Second claim should fail
    try {
      await program.methods
        .claimDrop(Array.from(nullifier))
        .accounts({
          drop: dropPDA,
          nullifierAccount: nullifierPDA,
          claimer: claimer.publicKey,
          config: configPDA,
          solVault: solVaultPDA,
          treasury: treasury.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([claimer])
        .rpc();
      expect.fail("Should have thrown an error");
    } catch (err) {
      expect(err.toString()).to.include("NullifierAlreadyUsed");
    }
  });

  it("Rejects invalid amount", async () => {
    const invalidNullifier = new Uint8Array(32);
    if (typeof window !== "undefined" && window.crypto) {
      window.crypto.getRandomValues(invalidNullifier);
    } else {
      const crypto = await import("crypto");
      crypto.randomFillSync(invalidNullifier);
    }

    const [invalidDropPDA] = PublicKey.findProgramAddressSync(
      [Buffer.from("drop"), Buffer.from(invalidNullifier)],
      program.programId
    );

    const clock = await provider.connection.getSlot();
    const blockTime = await provider.connection.getBlockTime(clock);
    const expiresAt = new BN(blockTime! + 3600);

    const [rateLimitPDA] = PublicKey.findProgramAddressSync(
      [Buffer.from("rate_limit"), authority.publicKey.toBuffer()],
      program.programId
    );

    try {
      await program.methods
        .depositPool(
          Array.from(invalidNullifier),
          new BN(0), // Invalid: amount must be > 0
          0,
          expiresAt
        )
        .accounts({
          drop: invalidDropPDA,
          nullifierAccount: nullifierPDA,
          config: configPDA,
          rateLimitAccount: rateLimitPDA,
          payer: authority.publicKey,
          solVault: solVaultPDA,
          systemProgram: SystemProgram.programId,
        })
        .rpc();
      expect.fail("Should have thrown an error");
    } catch (err) {
      expect(err.toString()).to.include("InvalidAmount");
    }
  });

  it("Enforces rate limiting", async () => {
    const testNullifier = new Uint8Array(32);
    if (typeof window !== "undefined" && window.crypto) {
      window.crypto.getRandomValues(testNullifier);
    } else {
      const crypto = await import("crypto");
      crypto.randomFillSync(testNullifier);
    }

    const [testDropPDA] = PublicKey.findProgramAddressSync(
      [Buffer.from("drop"), Buffer.from(testNullifier)],
      program.programId
    );

    const [rateLimitPDA] = PublicKey.findProgramAddressSync(
      [Buffer.from("rate_limit"), authority.publicKey.toBuffer()],
      program.programId
    );

    const clock = await provider.connection.getSlot();
    const blockTime = await provider.connection.getBlockTime(clock);
    const expiresAt = new BN(blockTime! + 3600);

    // First drop should succeed
    await program.methods
      .depositPool(
        Array.from(testNullifier),
        new BN(1000000),
        0,
        expiresAt
      )
      .accounts({
        drop: testDropPDA,
        nullifierAccount: nullifierPDA,
        config: configPDA,
        rateLimitAccount: rateLimitPDA,
        payer: authority.publicKey,
        solVault: solVaultPDA,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    // Second drop immediately after should fail
    const testNullifier2 = new Uint8Array(32);
    if (typeof window !== "undefined" && window.crypto) {
      window.crypto.getRandomValues(testNullifier2);
    } else {
      const crypto = await import("crypto");
      crypto.randomFillSync(testNullifier2);
    }

    const [testDropPDA2] = PublicKey.findProgramAddressSync(
      [Buffer.from("drop"), Buffer.from(testNullifier2)],
      program.programId
    );

    try {
      await program.methods
        .depositPool(
          Array.from(testNullifier2),
          new BN(1000000),
          0,
          expiresAt
        )
        .accounts({
          drop: testDropPDA2,
          nullifierAccount: nullifierPDA,
          config: configPDA,
          rateLimitAccount: rateLimitPDA,
          payer: authority.publicKey,
          solVault: solVaultPDA,
          systemProgram: SystemProgram.programId,
        })
        .rpc();
      expect.fail("Should have thrown an error");
    } catch (err) {
      expect(err.toString()).to.include("RateLimitExceeded");
    }
  });
});

