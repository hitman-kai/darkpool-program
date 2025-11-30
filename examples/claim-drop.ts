import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { PublicKey, Keypair } from "@solana/web3.js";

/**
 * Example: Claim a drop
 * 
 * This demonstrates how to claim a drop using the nullifier.
 * The claimer must know the nullifier to claim the drop.
 */
export async function claimDrop(
  program: Program,
  claimer: Keypair,
  nullifier: Uint8Array
) {
  // Derive PDAs
  const [dropPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("drop"), Buffer.from(nullifier)],
    program.programId
  );

  const [nullifierPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("nullifier"), Buffer.from(nullifier)],
    program.programId
  );

  // Fetch drop to verify it exists and is active
  const drop = await program.account.dropAccount.fetch(dropPDA);
  console.log("Drop status:", drop.status);
  console.log("Amount:", drop.amount.toString());
  console.log("Recipient:", drop.recipient.toString());

  // Claim the drop
  const tx = await program.methods
    .claimDrop(Array.from(nullifier))
    .accounts({
      drop: dropPDA,
      nullifierAccount: nullifierPDA,
      claimer: claimer.publicKey,
    })
    .signers([claimer])
    .rpc();

  console.log("Drop claimed!");
  console.log("Transaction:", tx);

  return tx;
}

