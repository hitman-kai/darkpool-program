import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Connection, Keypair, PublicKey, clusterApiUrl } from "@solana/web3.js";
import { Wallet } from "@coral-xyz/anchor";
import { createDrop } from "./create-drop";
import { claimDrop } from "./claim-drop";

/**
 * Full end-to-end example
 * 
 * This demonstrates the complete flow:
 * 1. Initialize program (if needed)
 * 2. Create a drop
 * 3. Claim the drop
 */
async function main() {
  // Setup connection
  const connection = new Connection(clusterApiUrl("devnet"), "confirmed");

  // Load or create keypairs
  // In production, load from secure storage
  const authority = Keypair.generate();
  const recipient = Keypair.generate();
  const claimer = Keypair.generate(); // Same as recipient in real scenario

  // Airdrop SOL for testing (devnet only)
  console.log("Requesting airdrop...");
  const airdropSig = await connection.requestAirdrop(
    authority.publicKey,
    2 * anchor.web3.LAMPORTS_PER_SOL
  );
  await connection.confirmTransaction(airdropSig);

  // Load program
  const wallet = new Wallet(authority);
  const provider = new anchor.AnchorProvider(connection, wallet, {
    commitment: "confirmed",
  });
  anchor.setProvider(provider);

  // Replace with your program ID
  const programId = new PublicKey(
    "EPpgM9ogD8wTVESMmin8kwemTmkVPQhPq9w1Mpz8Gxb7"
  );
  const program = await anchor.Program.at(programId, provider);

  // Step 1: Initialize program (if not already initialized)
  try {
    const [configPDA] = PublicKey.findProgramAddressSync(
      [Buffer.from("config")],
      program.programId
    );

    await program.methods
      .initialize()
      .accounts({
        config: configPDA,
        authority: authority.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .rpc();

    console.log("Program initialized");
  } catch (err) {
    console.log("Program already initialized or error:", err);
  }

  // Step 2: Create a drop
  console.log("\n=== Creating Drop ===");
  const dropResult = await createDrop(
    program,
    authority,
    recipient.publicKey,
    1000000, // 0.001 SOL
    0 // SOL
  );

  console.log("\n=== Drop Created ===");
  console.log("Nullifier:", Buffer.from(dropResult.nullifier).toString("hex"));
  console.log("Save this nullifier to claim the drop later");

  // Step 3: Claim the drop
  console.log("\n=== Claiming Drop ===");
  await claimDrop(program, claimer, new Uint8Array(dropResult.nullifier));

  console.log("\n=== Complete ===");
  console.log("Drop successfully created and claimed!");
}

main()
  .then(() => process.exit(0))
  .catch((error) => {
    console.error(error);
    process.exit(1);
  });

