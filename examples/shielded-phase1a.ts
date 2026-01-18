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
const TREE_DEPTH = Number(process.env.SHIELDED_TREE_DEPTH || "16");
const FORCE_INIT = ["1", "true", "yes"].includes(
  (process.env.FORCE_INIT || "").toLowerCase()
);

function loadKeypair(path: string): Keypair {
  const raw = JSON.parse(fs.readFileSync(path, "utf8"));
  return Keypair.fromSecretKey(new Uint8Array(raw));
}

function discriminator(name: string): Buffer {
  const hash = crypto.createHash("sha256").update(`global:${name}`).digest();
  return hash.subarray(0, 8);
}

function u8(value: number): Buffer {
  return Buffer.from([value & 0xff]);
}

function u64le(value: number | bigint): Buffer {
  const buf = Buffer.alloc(8);
  buf.writeBigUInt64LE(BigInt(value), 0);
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

function assert(condition: boolean, message: string) {
  if (!condition) {
    throw new Error(message);
  }
}

function splitInto32(buf: Buffer, depth: number) {
  const out: Buffer[] = [];
  for (let i = 0; i < depth; i += 1) {
    out.push(buf.subarray(i * 32, (i + 1) * 32));
  }
  return out;
}

function hashPair(left: Buffer, right: Buffer) {
  return crypto
    .createHash("sha256")
    .update(Buffer.concat([left, right]))
    .digest();
}

function computeExpectedRoot(
  tree: ReturnType<typeof decodeShieldedTree>,
  commitment: Buffer
) {
  const depth = tree.depth;
  const zeroes = splitInto32(tree.zeroes, depth);
  const filled = splitInto32(tree.filledSubtrees, depth);
  let index = tree.nextLeafIndex;
  let current = Buffer.from(commitment);
  for (let level = 0; level < depth; level += 1) {
    if (index % 2 === 0) {
      current = hashPair(current, zeroes[level]);
    } else {
      current = hashPair(filled[level], current);
    }
    index = Math.floor(index / 2);
  }
  return current;
}

async function main() {
  const connection = new Connection(RPC_URL, "confirmed");
  const authority = loadKeypair(KEYPAIR_PATH);

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

  console.log("Authority:", authority.publicKey.toBase58());
  console.log("Shielded config:", configPDA.toBase58());
  console.log("Shielded tree:", treePDA.toBase58());
  console.log("Shielded vault:", vaultPDA.toBase58());

  const configInfo = await connection.getAccountInfo(configPDA, "confirmed");
  const decodedConfig = configInfo?.data
    ? decodeShieldedConfig(configInfo.data)
    : null;
  const shouldInit =
    !configInfo || FORCE_INIT || decodedConfig?.treeDepth !== TREE_DEPTH;

  if (shouldInit) {
    console.log("\n== initialize_shielded ==");
    if (decodedConfig) {
      console.log(
        `Re-initializing (depth ${decodedConfig.treeDepth} -> ${TREE_DEPTH})`
      );
    }
    const data = Buffer.concat([
      discriminator("initialize_shielded"),
      u8(TREE_DEPTH),
    ]);
    const ix = new TransactionInstruction({
      programId: PROGRAM_ID,
      keys: [
        { pubkey: configPDA, isSigner: false, isWritable: true },
        { pubkey: authority.publicKey, isSigner: true, isWritable: true },
        { pubkey: vaultPDA, isSigner: false, isWritable: true },
        { pubkey: treePDA, isSigner: false, isWritable: true },
        { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
      ],
      data,
    });
    const initializeTx = new Transaction().add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 10_000_000 }),
      ix
    );
    await sendAndConfirmTransaction(connection, initializeTx, [authority]);
    console.log("initialize_shielded done");
  } else if (decodedConfig) {
    console.log("\n== initialize_shielded ==");
    console.log("Already initialized");
    console.log("Tree depth:", decodedConfig.treeDepth);
    console.log(
      "Current root:",
      Buffer.from(decodedConfig.currentRoot).toString("hex")
    );
    console.log("Next leaf index:", decodedConfig.nextLeafIndex);
  }

  console.log("\n== deposit_shielded ==");
  const treeBeforeDeposit1 = await connection.getAccountInfo(treePDA, "confirmed");
  assert(treeBeforeDeposit1?.data, "Shielded tree missing before first deposit");
  const treeBeforeDecoded1 = decodeShieldedTree(treeBeforeDeposit1.data);
  const commitment = crypto
    .createHash("sha256")
    .update("darkpool-deposit-1")
    .digest();
  const amount = 1_000_000;
  const depositData = Buffer.concat([
    discriminator("deposit_shielded"),
    commitment,
    u64le(amount),
  ]);
  const depositIx = new TransactionInstruction({
    programId: PROGRAM_ID,
    keys: [
      { pubkey: configPDA, isSigner: false, isWritable: true },
      { pubkey: treePDA, isSigner: false, isWritable: true },
      { pubkey: vaultPDA, isSigner: false, isWritable: true },
      { pubkey: authority.publicKey, isSigner: true, isWritable: true },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    data: depositData,
  });
  const depositTx = new Transaction().add(
    ComputeBudgetProgram.setComputeUnitLimit({ units: 10_000_000 }),
    depositIx
  );
  await sendAndConfirmTransaction(connection, depositTx, [authority]);
  console.log("deposit_shielded done");
  console.log("Commitment:", commitment.toString("hex"));

  const configAfter = await connection.getAccountInfo(configPDA, "confirmed");
  const treeAfter = await connection.getAccountInfo(treePDA, "confirmed");
  if (configAfter?.data) {
    const decoded = decodeShieldedConfig(configAfter.data);
    console.log("New root:", Buffer.from(decoded.currentRoot).toString("hex"));
    console.log("Next leaf index:", decoded.nextLeafIndex);
  }
  if (treeAfter?.data) {
    const decoded = decodeShieldedTree(treeAfter.data);
    console.log("Tree root:", Buffer.from(decoded.root).toString("hex"));
  }

  assert(treeAfter?.data, "Shielded tree missing after deposit");
  const expectedRoot1 = computeExpectedRoot(treeBeforeDecoded1, commitment);
  assert(
    Buffer.compare(Buffer.from(treeBeforeDecoded1.root), expectedRoot1) !== 0,
    "Expected root to change after first deposit"
  );
  if (configAfter?.data) {
    const decoded = decodeShieldedConfig(configAfter.data);
    assert(
      Buffer.compare(Buffer.from(decoded.currentRoot), expectedRoot1) === 0,
      "Root mismatch after first deposit"
    );
    assert(
      decoded.nextLeafIndex === treeBeforeDecoded1.nextLeafIndex + 1,
      `Expected next_leaf_index ${treeBeforeDecoded1.nextLeafIndex + 1}, got ${decoded.nextLeafIndex}`
    );
  }

  console.log("\n== deposit_shielded #2 ==");
  const commitment2 = crypto
    .createHash("sha256")
    .update("darkpool-deposit-2")
    .digest();
  const depositData2 = Buffer.concat([
    discriminator("deposit_shielded"),
    commitment2,
    u64le(amount),
  ]);
  const depositIx2 = new TransactionInstruction({
    programId: PROGRAM_ID,
    keys: [
      { pubkey: configPDA, isSigner: false, isWritable: true },
      { pubkey: treePDA, isSigner: false, isWritable: true },
      { pubkey: vaultPDA, isSigner: false, isWritable: true },
      { pubkey: authority.publicKey, isSigner: true, isWritable: true },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    data: depositData2,
  });
  const depositTx2 = new Transaction().add(
    ComputeBudgetProgram.setComputeUnitLimit({ units: 10_000_000 }),
    depositIx2
  );
  await sendAndConfirmTransaction(connection, depositTx2, [authority]);
  console.log("deposit_shielded #2 done");
  console.log("Commitment #2:", commitment2.toString("hex"));

  const configAfter2 = await connection.getAccountInfo(configPDA, "confirmed");
  const treeAfter2 = await connection.getAccountInfo(treePDA, "confirmed");
  assert(treeAfter?.data && treeAfter2?.data, "Missing tree data for deposit #2");
  const treeBeforeDeposit2 = decodeShieldedTree(treeAfter.data);
  const expectedRoot2 = computeExpectedRoot(treeBeforeDeposit2, commitment2);
  assert(
    Buffer.compare(Buffer.from(treeBeforeDeposit2.root), expectedRoot2) !== 0,
    "Expected root to change after second deposit"
  );
  if (configAfter2?.data) {
    const decoded = decodeShieldedConfig(configAfter2.data);
    assert(
      Buffer.compare(Buffer.from(decoded.currentRoot), expectedRoot2) === 0,
      "Root mismatch after second deposit"
    );
    assert(
      decoded.nextLeafIndex === treeBeforeDeposit2.nextLeafIndex + 1,
      `Expected next_leaf_index ${treeBeforeDeposit2.nextLeafIndex + 1}, got ${decoded.nextLeafIndex}`
    );
  }
}

main().catch((err) => {
  console.error("Phase 1A test failed:", err);
  process.exit(1);
});
