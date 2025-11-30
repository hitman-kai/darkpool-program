import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { PublicKey, Keypair, SystemProgram } from "@solana/web3.js";
import { BN } from "@coral-xyz/anchor";
/**
 * Example: Create a drop
 * 
 * This demonstrates how to create a privacy-focused token drop
 * using nullifier-based verification.
 */
export async function createDrop(
  program: Program,
  authority: Keypair,
  recipient: PublicKey,
  amount: number, // in lamports
  assetType: number = 0 // 0 = SOL, 1 = USDC
) {
  // Generate nullifier (in production, use secure random generation)
  // For browser: use crypto.getRandomValues()
  // For Node.js: use crypto.randomBytes()
  const nullifier = new Uint8Array(32);
  if (typeof window !== "undefined" && window.crypto) {
    window.crypto.getRandomValues(nullifier);
  } else {
    // Node.js fallback
    const crypto = await import("crypto");
    crypto.randomFillSync(nullifier);
  }

  // Derive PDAs
  const [configPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("config")],
    program.programId
  );

  const [dropPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("drop"), Buffer.from(nullifier)],
    program.programId
  );

  const [nullifierPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("nullifier"), Buffer.from(nullifier)],
    program.programId
  );

  const [rateLimitPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("rate_limit"), authority.publicKey.toBuffer()],
    program.programId
  );

  // Calculate expiration (1 hour from now)
  const now = Math.floor(Date.now() / 1000);
  const expiresAt = new BN(now + 3600);

  // Create drop
  const tx = await program.methods
    .createDrop(
      Array.from(nullifier),
      recipient,
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
      systemProgram: SystemProgram.programId,
    })
    .signers([authority])
    .rpc();

  console.log("Drop created!");
  console.log("Transaction:", tx);
  console.log("Nullifier:", Buffer.from(nullifier).toString("hex"));
  console.log("Drop PDA:", dropPDA.toString());

  return {
    nullifier: Array.from(nullifier),
    dropPDA,
    nullifierPDA,
    tx,
  };
}

