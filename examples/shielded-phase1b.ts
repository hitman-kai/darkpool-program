import * as fs from "fs";
import crypto from "crypto";
import {
  Connection,
  PublicKey,
  Keypair,
  SystemProgram,
  ComputeBudgetProgram,
  Transaction,
  TransactionInstruction,
  sendAndConfirmTransaction,
} from "@solana/web3.js";

const RPC_URL = process.env.RPC_URL || "https://api.devnet.solana.com";
const PROGRAM_ID = new PublicKey(
  process.env.DARKPOOL_PROGRAM_ID ||
    "EPpgM9ogD8wTVESMmin8kwemTmkVPQhPq9w1Mpz8Gxb7"
);
const KEYPAIR_PATH =
  process.env.DARKPOOL_KEYPAIR ||
  process.env.ANCHOR_WALLET ||
  "D:\\Dev\\Keys\\darkpool-deployer.json";
const RECIPIENT = process.env.RECIPIENT;

function loadKeypair(path: string): Keypair {
  const raw = JSON.parse(fs.readFileSync(path, "utf8"));
  return Keypair.fromSecretKey(new Uint8Array(raw));
}

function discriminator(name: string): Buffer {
  const hash = crypto.createHash("sha256").update(`global:${name}`).digest();
  return hash.subarray(0, 8);
}

function u64le(value: number | bigint): Buffer {
  const buf = Buffer.alloc(8);
  buf.writeBigUInt64LE(BigInt(value), 0);
  return buf;
}

function u32le(value: number): Buffer {
  const buf = Buffer.alloc(4);
  buf.writeUInt32LE(value >>> 0, 0);
  return buf;
}

function decodeShieldedConfig(data: Buffer) {
  let offset = 8;
  const authority = new PublicKey(data.subarray(offset, offset + 32));
  offset += 32;
  const isInitialized = data.readUInt8(offset) === 1;
  offset += 1;
  const treeDepth = data.readUInt8(offset);
  offset += 1;
  const vaultBump = data.readUInt8(offset);
  offset += 1;
  const currentRoot = data.subarray(offset, offset + 32);
  offset += 32;
  const nextLeafIndex = data.readUInt32LE(offset);
  return { authority, isInitialized, treeDepth, vaultBump, currentRoot, nextLeafIndex };
}

function decodeShieldedTree(data: Buffer) {
  let offset = 8;
  const depth = data.readUInt8(offset);
  offset += 1;
  const filledSubtrees = data.subarray(offset, offset + 20 * 32);
  offset += 20 * 32;
  const zeroes = data.subarray(offset, offset + 20 * 32);
  offset += 20 * 32;
  const root = data.subarray(offset, offset + 32);
  offset += 32;
  const nextLeafIndex = data.readUInt32LE(offset);
  return { depth, filledSubtrees, zeroes, root, nextLeafIndex };
}

function decodeShieldedNullifier(data: Buffer) {
  let offset = 8;
  const nullifier = data.subarray(offset, offset + 32);
  offset += 32;
  const isUsed = data.readUInt8(offset) === 1;
  return { nullifier, isUsed };
}

function assert(condition: boolean, message: string) {
  if (!condition) {
    throw new Error(message);
  }
}

async function main() {
  const connection = new Connection(RPC_URL, "confirmed");
  const spender = loadKeypair(KEYPAIR_PATH);
  const rentLamports = await connection.getMinimumBalanceForRentExemption(8 + 32 + 1);
  const minBalance = rentLamports + 500_000_000;
  const spenderBalance = await connection.getBalance(spender.publicKey, "confirmed");
  console.log("Spender balance:", spenderBalance);
  if (spenderBalance < minBalance) {
    const sig = await connection.requestAirdrop(spender.publicKey, 5_000_000_000);
    const latest = await connection.getLatestBlockhash("confirmed");
    await connection.confirmTransaction(
      {
        signature: sig,
        blockhash: latest.blockhash,
        lastValidBlockHeight: latest.lastValidBlockHeight,
      },
      "confirmed"
    );
    console.log("Airdrop signature:", sig);
  }
  const spenderBalanceAfter = await connection.getBalance(
    spender.publicKey,
    "confirmed"
  );
  console.log("Spender balance after:", spenderBalanceAfter);
  if (spenderBalanceAfter < minBalance) {
    throw new Error("Insufficient SOL for rent/fees after airdrop.");
  }

  const [configPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("shielded_config")],
    PROGRAM_ID
  );
  const [treePDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("shielded_tree")],
    PROGRAM_ID
  );
  const [vaultPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("shielded_vault")],
    PROGRAM_ID
  );

  const configInfo = await connection.getAccountInfo(configPDA, "confirmed");
  if (!configInfo) {
    throw new Error("Shielded config not initialized");
  }
  const decoded = decodeShieldedConfig(configInfo.data);
  if (!decoded.isInitialized) {
    throw new Error("Shielded config not initialized");
  }

  const treeInfo = await connection.getAccountInfo(treePDA, "confirmed");
  console.log("Config lamports:", configInfo.lamports, "data len:", configInfo.data.length);
  console.log(
    "Config rent min:",
    await connection.getMinimumBalanceForRentExemption(configInfo.data.length)
  );
  if (treeInfo?.data) {
    console.log("Tree lamports:", treeInfo.lamports, "data len:", treeInfo.data.length);
    console.log(
      "Tree rent min:",
      await connection.getMinimumBalanceForRentExemption(treeInfo.data.length)
    );
  }
  const vaultInfo = await connection.getAccountInfo(vaultPDA, "confirmed");
  console.log("Vault owner:", vaultInfo?.owner?.toBase58());
  console.log("Vault data len:", vaultInfo?.data?.length ?? 0);
  console.log("Vault lamports:", vaultInfo?.lamports ?? 0);

  async function runSpend(
    label: string,
    recipient: PublicKey,
    amount: number,
    expectFailure: boolean
  ) {
    console.log(`\n== spend_shielded (${label}) ==`);
    const recipientInfo = await connection.getAccountInfo(recipient, "confirmed");
    if (recipientInfo) {
      console.log(
        "Recipient owner:",
        recipientInfo.owner.toBase58(),
        "lamports:",
        recipientInfo.lamports,
        "data len:",
        recipientInfo.data.length
      );
      console.log(
        "Recipient rent min:",
        await connection.getMinimumBalanceForRentExemption(recipientInfo.data.length)
      );
    } else {
      console.log("Recipient account: not found (will be created by transfer)");
    }

    // TODO: replace with zk proof verification + PDA-backed nullifier.
    const nullifier = crypto.randomBytes(32);
    const nullifierKeypair = Keypair.generate();

    const nullifierInfoBefore = await connection.getAccountInfo(
      nullifierKeypair.publicKey,
      "confirmed"
    );
    assert(!nullifierInfoBefore, "Nullifier already used");

    const treeInfoBefore = await connection.getAccountInfo(treePDA, "confirmed");
    assert(!!treeInfoBefore?.data, "Shielded tree missing before spend");
    const treeBefore = decodeShieldedTree(treeInfoBefore!.data);
    assert(
      Buffer.compare(treeBefore.root, decoded.currentRoot) === 0,
      "Config root mismatch before spend"
    );

    const vaultBalanceBefore = await connection.getBalance(vaultPDA, "confirmed");
    const recipientBalanceBefore = await connection.getBalance(
      recipient,
      "confirmed"
    );

    const proof = Buffer.alloc(32, 1);
    const data = Buffer.concat([
      discriminator("spend_shielded_with_proof"),
      nullifier,
      u64le(amount),
      decoded.currentRoot,
      u32le(proof.length),
      proof,
    ]);

    const ix = new TransactionInstruction({
      programId: PROGRAM_ID,
      keys: [
        { pubkey: configPDA, isSigner: false, isWritable: true },
        { pubkey: treePDA, isSigner: false, isWritable: true },
        { pubkey: vaultPDA, isSigner: false, isWritable: true },
        { pubkey: nullifierKeypair.publicKey, isSigner: true, isWritable: true },
        { pubkey: recipient, isSigner: false, isWritable: true },
        { pubkey: spender.publicKey, isSigner: true, isWritable: true },
        { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
      ],
      data,
    });

    const tx = new Transaction().add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 1_000_000 }),
      ix
    );
    const latestForLog = await connection.getLatestBlockhash("confirmed");
    tx.recentBlockhash = latestForLog.blockhash;
    tx.feePayer = spender.publicKey;
    const message = tx.compileMessage();
    console.log("Tx account keys:");
    message.accountKeys.forEach((key, index) => {
      console.log(`  [${index}] ${key.toBase58()}`);
    });

    try {
      await sendAndConfirmTransaction(connection, tx, [spender, nullifierKeypair]);
      if (expectFailure) {
        throw new Error("Expected spend_shielded to fail but it succeeded.");
      }
    } catch (err) {
      if (!expectFailure) {
        throw err;
      }
      const msg = String(err);
      console.log("Expected failure:", msg);
      return;
    }

    console.log("spend_shielded done");
    console.log("Recipient:", recipient.toBase58());
    console.log("Nullifier:", nullifier.toString("hex"));
    console.log("Amount:", amount);
    console.log("Root:", Buffer.from(decoded.currentRoot).toString("hex"));

    const vaultBalanceAfter = await connection.getBalance(vaultPDA, "confirmed");
    const recipientBalanceAfter = await connection.getBalance(
      recipient,
      "confirmed"
    );
    if (!recipient.equals(spender.publicKey)) {
      assert(
        recipientBalanceAfter - recipientBalanceBefore === amount,
        "Recipient balance did not increase by amount"
      );
    }
    assert(
      vaultBalanceBefore - vaultBalanceAfter === amount,
      "Vault balance did not decrease by amount"
    );

    const nullifierInfoAfter = await connection.getAccountInfo(
      nullifierKeypair.publicKey,
      "confirmed"
    );
    assert(!!nullifierInfoAfter?.data, "Nullifier account missing after spend");
    const decodedNullifier = decodeShieldedNullifier(nullifierInfoAfter!.data);
    assert(
      Buffer.compare(decodedNullifier.nullifier, nullifier) === 0,
      "Nullifier stored mismatch"
    );
    assert(decodedNullifier.isUsed, "Nullifier not marked used");

    const configAfter = await connection.getAccountInfo(configPDA, "confirmed");
    const treeAfter = await connection.getAccountInfo(treePDA, "confirmed");
    assert(!!configAfter?.data && !!treeAfter?.data, "Missing state after spend");
    const decodedAfter = decodeShieldedConfig(configAfter!.data);
    const treeAfterDecoded = decodeShieldedTree(treeAfter!.data);
    assert(
      Buffer.compare(decodedAfter.currentRoot, decoded.currentRoot) === 0,
      "Config root changed during spend"
    );
    assert(
      Buffer.compare(treeAfterDecoded.root, decoded.currentRoot) === 0,
      "Tree root changed during spend"
    );
  }

  const rentMin0 = await connection.getMinimumBalanceForRentExemption(0);
  console.log("Rent min (0 data):", rentMin0);

  const freshBelow = Keypair.generate().publicKey;
  const freshAbove = Keypair.generate().publicKey;
  const existingRecipient = spender.publicKey;

  const belowAmount = Math.max(1, rentMin0 - 1);
  const aboveAmount = rentMin0 + 1;
  const existingAmount = 1_000;

  await runSpend("fresh recipient below rent-min (expect failure)", freshBelow, belowAmount, true);
  await runSpend("fresh recipient above rent-min (expect success)", freshAbove, aboveAmount, false);
  await runSpend("existing recipient (expect success)", existingRecipient, existingAmount, false);
}

main().catch((err) => {
  console.error("Phase 1B test failed:", err);
  process.exit(1);
});
