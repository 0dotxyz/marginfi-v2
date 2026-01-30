import {
  Keypair,
  PublicKey,
  SystemProgram,
  Transaction,
} from "@solana/web3.js";
import { BN } from "@coral-xyz/anchor";
import { BanksClient, ProgramTestContext } from "solana-bankrun";
import { Oracles } from "./mocks";
import {
  ORACLE_CONF_INTERVAL,
  DRIFT_ORACLE_RECEIVER_PROGRAM_ID,
} from "./types";
import { processBankrunTransaction } from "./tools";

/** Default Pyth receiver program ID (mocks program) */
export const PYTH_RECEIVER_PROGRAM_ID = new PublicKey(
  "rec5EKMGg6MxZYaMdyBfgwp4d5rB9T1VQH5pJv5LtFJ",
);

/**
 * Creates a blank pyth feed account in bankrun (300 bytes).
 *
 * @param owner - The program that owns this feed account. For Drift oracles, use DRIFT_ORACLE_RECEIVER_PROGRAM_ID.
 */
export async function createBankrunPythFeedAccount(
  bankrunContext: ProgramTestContext,
  banksClient: BanksClient,
  feedKeypair: Keypair,
  owner: PublicKey,
): Promise<Keypair> {
  const space = 300;
  const rent = await banksClient.getRent();
  const lamports = Number(rent.minimumBalance(BigInt(space)));

  const tx = new Transaction().add(
    SystemProgram.createAccount({
      fromPubkey: bankrunContext.payer.publicKey,
      newAccountPubkey: feedKeypair.publicKey,
      lamports,
      space,
      programId: owner,
    }),
  );

  await processBankrunTransaction(bankrunContext, tx, [
    bankrunContext.payer,
    feedKeypair,
  ]);

  return feedKeypair;
}

/**
 * Creates a blank pyth oracle account in bankrun with specified owner (134 bytes).
 *
 * @param owner - The program that owns this oracle account. For Drift oracles, use DRIFT_ORACLE_RECEIVER_PROGRAM_ID.
 */
export async function createBankrunPythOracleAccount(
  bankrunContext: ProgramTestContext,
  banksClient: BanksClient,
  oracleKeypair: Keypair,
  owner: PublicKey,
): Promise<Keypair> {
  const space = 134;
  const rent = await banksClient.getRent();
  const lamports = Number(rent.minimumBalance(BigInt(space)));

  const tx = new Transaction().add(
    SystemProgram.createAccount({
      fromPubkey: bankrunContext.payer.publicKey,
      newAccountPubkey: oracleKeypair.publicKey,
      lamports,
      space,
      programId: owner,
    }),
  );

  await processBankrunTransaction(bankrunContext, tx, [
    bankrunContext.payer,
    oracleKeypair,
  ]);

  return oracleKeypair;
}

/**
 * Sets a Pyth Pull oracle price directly using bankrun, bypassing transactions.
 * This avoids "Account in use" errors from concurrent transactions.
 *
 * @param bankrunContext - The bankrun context
 * @param banksClient - The banks client to get clock from
 * @param oracleAccount - The PriceUpdateV2 account to update
 * @param feedAccount - The feed account referenced in the price update
 * @param price - The price value (will be multiplied by 10^decimals)
 * @param decimals - Number of decimals for the price
 * @param confidence - Confidence value as a multiplier (e.g., 0.01 for 1%)
 * @param owner - The program that owns the oracle account (defaults to pyth-solana-receiver)
 */
export async function setPythPullOraclePrice(
  bankrunContext: ProgramTestContext,
  banksClient: BanksClient,
  oracleAccount: PublicKey,
  feedAccount: PublicKey,
  price: number,
  decimals: number,
  confidence: number,
  owner: PublicKey = new PublicKey(
    "rec5EKMGg6MxZYaMdyBfgwp4d5rB9T1VQH5pJv5LtFJ",
  ),
) {
  // Get current clock for slot and timestamp
  const clock = await banksClient.getClock();
  const slot = new BN(Number(clock.slot));
  const publishTime = Number(clock.unixTimestamp);

  // Convert price to BN with decimals
  const priceBN = new BN(price * 10 ** decimals);
  const confBN = new BN(price * confidence * 10 ** decimals);

  // Build PriceUpdateV2 buffer (134 bytes)
  const buffer = Buffer.alloc(134);
  let offset = 0;

  // Discriminator (8 bytes)
  Buffer.from([34, 241, 35, 99, 157, 126, 244, 205]).copy(buffer, offset);
  offset += 8;

  // Write authority (32 bytes) - can be any pubkey
  PublicKey.unique().toBuffer().copy(buffer, offset);
  offset += 32;

  // Verification level - Full (1 byte)
  buffer.writeUInt8(1, offset);
  offset += 1;

  // Feed ID (32 bytes)
  feedAccount.toBuffer().copy(buffer, offset);
  offset += 32;

  // Price (i64, 8 bytes)
  priceBN.toArrayLike(Buffer, "le", 8).copy(buffer, offset);
  offset += 8;

  // Confidence (u64, 8 bytes)
  confBN.toArrayLike(Buffer, "le", 8).copy(buffer, offset);
  offset += 8;

  // Exponent (i32, 4 bytes)
  buffer.writeInt32LE(-decimals, offset);
  offset += 4;

  // Publish time (i64, 8 bytes)
  new BN(publishTime).toArrayLike(Buffer, "le", 8).copy(buffer, offset);
  offset += 8;

  // Previous publish time (i64, 8 bytes)
  new BN(publishTime - 1).toArrayLike(Buffer, "le", 8).copy(buffer, offset);
  offset += 8;

  // EMA price (i64, 8 bytes) - use same as price
  priceBN.toArrayLike(Buffer, "le", 8).copy(buffer, offset);
  offset += 8;

  // EMA conf (u64, 8 bytes) - use same as conf
  confBN.toArrayLike(Buffer, "le", 8).copy(buffer, offset);
  offset += 8;

  // Posted slot (u64, 8 bytes)
  slot.toArrayLike(Buffer, "le", 8).copy(buffer, offset);

  // Get existing account or create if it doesn't exist
  const existing = await banksClient.getAccount(oracleAccount);

  if (!existing) {
    console.log(
      "Account does not exist, not creating because this causes bankrun issues",
    );
    return;
  } else {
    // Update existing account with new data
    bankrunContext.setAccount(oracleAccount, {
      lamports: existing.lamports,
      data: buffer,
      owner: existing.owner, // Preserve existing owner
      executable: existing.executable,
      rentEpoch: existing.rentEpoch,
    });
  }
}

/**
 * Updates all Pyth Pull oracles in the oracles object.
 * This is a drop-in replacement for refreshPullOracles that avoids "Account in use" errors.
 *
 * @param oracles - The oracles object containing all oracle accounts and price data
 * @param bankrunContext - The bankrun context
 * @param banksClient - The banks client to get clock from
 * @param owner - The program that owns the oracle accounts (defaults to pyth-solana-receiver)
 */
export async function refreshPullOraclesBankrun(
  oracles: Oracles,
  bankrunContext: ProgramTestContext,
  banksClient: BanksClient,
  owner: PublicKey = new PublicKey(
    "rec5EKMGg6MxZYaMdyBfgwp4d5rB9T1VQH5pJv5LtFJ",
  ),
) {
  // Update each oracle sequentially to avoid any race conditions
  await setPythPullOraclePrice(
    bankrunContext,
    banksClient,
    oracles.pythPullLst.publicKey,
    oracles.pythPullLstOracleFeed.publicKey,
    oracles.lstAlphaPrice,
    oracles.lstAlphaDecimals,
    ORACLE_CONF_INTERVAL,
    owner,
  );

  await setPythPullOraclePrice(
    bankrunContext,
    banksClient,
    oracles.wsolOracle.publicKey,
    oracles.wsolOracleFeed.publicKey,
    oracles.wsolPrice,
    oracles.wsolDecimals,
    ORACLE_CONF_INTERVAL,
    owner,
  );

  await setPythPullOraclePrice(
    bankrunContext,
    banksClient,
    oracles.usdcOracle.publicKey,
    oracles.usdcOracleFeed.publicKey,
    oracles.usdcPrice,
    oracles.usdcDecimals,
    ORACLE_CONF_INTERVAL,
    owner,
  );

  await setPythPullOraclePrice(
    bankrunContext,
    banksClient,
    oracles.tokenAOracle.publicKey,
    oracles.tokenAOracleFeed.publicKey,
    oracles.tokenAPrice,
    oracles.tokenADecimals,
    ORACLE_CONF_INTERVAL,
    owner,
  );

  await setPythPullOraclePrice(
    bankrunContext,
    banksClient,
    oracles.tokenBOracle.publicKey,
    oracles.tokenBOracleFeed.publicKey,
    oracles.tokenBPrice,
    oracles.tokenBDecimals,
    ORACLE_CONF_INTERVAL,
    owner,
  );
}

/**
 * Bankrun-native version of setupPythOracles.
 * Creates all oracle accounts via bankrun transactions and sets initial prices.
 *
 * NOTE: This uses setAccount to set oracle data, which may cause issues with warpToSlot.
 * For tests that use warpToSlot, consider using generateOracleInitialAccounts instead.
 */
export async function setupPythOraclesBankrun(
  bankrunContext: ProgramTestContext,
  banksClient: BanksClient,
  wsolPrice: number,
  wsolDecimals: number,
  usdcPrice: number,
  usdcDecimals: number,
  tokenAPrice: number,
  tokenADecimals: number,
  tokenBPrice: number,
  tokenBDecimals: number,
  lstAlphaPrice: number,
  lstAlphaDecimals: number,
  verbose: boolean = false,
): Promise<Oracles> {
  const owner = PYTH_RECEIVER_PROGRAM_ID;

  // Deterministic keypairs (same as pyth_mocks.ts)
  const wsolPythPullOracle = Keypair.fromSeed(
    Buffer.from("ORACLE_SEED_00000000000000F_WSOL"),
  );
  const wsolPythPullOracleFeed = Keypair.fromSeed(
    Buffer.from("ORACLE_SEED_0000000000000ID_WSOL"),
  );
  const usdcPythPullOracle = Keypair.fromSeed(
    Buffer.from("ORACLE_SEED_00000000000000F_USDC"),
  );
  const usdcPythPullOracleFeed = Keypair.fromSeed(
    Buffer.from("ORACLE_SEED_0000000000000ID_USDC"),
  );
  const fakeUsdcPythPullOracle = Keypair.fromSeed(
    Buffer.from("ORACLE_SEED_00000000000001F_USDC"),
  );
  const fakeUsdcPythPullOracleFeed = Keypair.fromSeed(
    Buffer.from("ORACLE_SEED_0000000000001ID_USDC"),
  );
  const tokenAPythPullOracle = Keypair.fromSeed(
    Buffer.from("ORACLE_SEED_00000000000001F_00TA"),
  );
  const tokenAPythPullOracleFeed = Keypair.fromSeed(
    Buffer.from("ORACLE_SEED_0000000000001ID_00TA"),
  );
  const tokenBPythPullOracle = Keypair.fromSeed(
    Buffer.from("ORACLE_SEED_00000000000001F_00TB"),
  );
  const tokenBPythPullOracleFeed = Keypair.fromSeed(
    Buffer.from("ORACLE_SEED_0000000000001ID_00TB"),
  );
  const lstPythPullOracle = Keypair.fromSeed(
    Buffer.from("ORACLE_SEED_00000000000001F_0LST"),
  );
  const lstPythPullOracleFeed = Keypair.fromSeed(
    Buffer.from("ORACLE_SEED_0000000000001ID_0LST"),
  );

  // Create all feed accounts
  await createBankrunPythFeedAccount(
    bankrunContext,
    banksClient,
    wsolPythPullOracleFeed,
    owner,
  );
  await createBankrunPythFeedAccount(
    bankrunContext,
    banksClient,
    usdcPythPullOracleFeed,
    owner,
  );
  await createBankrunPythFeedAccount(
    bankrunContext,
    banksClient,
    fakeUsdcPythPullOracleFeed,
    owner,
  );
  await createBankrunPythFeedAccount(
    bankrunContext,
    banksClient,
    tokenAPythPullOracleFeed,
    owner,
  );
  await createBankrunPythFeedAccount(
    bankrunContext,
    banksClient,
    tokenBPythPullOracleFeed,
    owner,
  );
  await createBankrunPythFeedAccount(
    bankrunContext,
    banksClient,
    lstPythPullOracleFeed,
    owner,
  );

  // Create all oracle accounts
  await createBankrunPythOracleAccount(
    bankrunContext,
    banksClient,
    wsolPythPullOracle,
    owner,
  );
  await createBankrunPythOracleAccount(
    bankrunContext,
    banksClient,
    usdcPythPullOracle,
    owner,
  );
  await createBankrunPythOracleAccount(
    bankrunContext,
    banksClient,
    fakeUsdcPythPullOracle,
    owner,
  );
  await createBankrunPythOracleAccount(
    bankrunContext,
    banksClient,
    tokenAPythPullOracle,
    owner,
  );
  await createBankrunPythOracleAccount(
    bankrunContext,
    banksClient,
    tokenBPythPullOracle,
    owner,
  );
  await createBankrunPythOracleAccount(
    bankrunContext,
    banksClient,
    lstPythPullOracle,
    owner,
  );

  // Set prices using setAccount (WARNING: may break warpToSlot)
  await setPythPullOraclePrice(
    bankrunContext,
    banksClient,
    wsolPythPullOracle.publicKey,
    wsolPythPullOracleFeed.publicKey,
    wsolPrice,
    wsolDecimals,
    ORACLE_CONF_INTERVAL,
    owner,
  );
  await setPythPullOraclePrice(
    bankrunContext,
    banksClient,
    usdcPythPullOracle.publicKey,
    usdcPythPullOracleFeed.publicKey,
    usdcPrice,
    usdcDecimals,
    ORACLE_CONF_INTERVAL,
    owner,
  );
  await setPythPullOraclePrice(
    bankrunContext,
    banksClient,
    fakeUsdcPythPullOracle.publicKey,
    fakeUsdcPythPullOracleFeed.publicKey,
    usdcPrice,
    usdcDecimals,
    ORACLE_CONF_INTERVAL,
    owner,
  );
  await setPythPullOraclePrice(
    bankrunContext,
    banksClient,
    tokenAPythPullOracle.publicKey,
    tokenAPythPullOracleFeed.publicKey,
    tokenAPrice,
    tokenADecimals,
    ORACLE_CONF_INTERVAL,
    owner,
  );
  await setPythPullOraclePrice(
    bankrunContext,
    banksClient,
    tokenBPythPullOracle.publicKey,
    tokenBPythPullOracleFeed.publicKey,
    tokenBPrice,
    tokenBDecimals,
    ORACLE_CONF_INTERVAL,
    owner,
  );
  await setPythPullOraclePrice(
    bankrunContext,
    banksClient,
    lstPythPullOracle.publicKey,
    lstPythPullOracleFeed.publicKey,
    lstAlphaPrice,
    lstAlphaDecimals,
    ORACLE_CONF_INTERVAL,
    owner,
  );

  if (verbose) {
    console.log("Mock Pyth Pull price oracles (bankrun):");
    console.log("wsol:    \t" + wsolPythPullOracle.publicKey);
    console.log("usdc:    \t" + usdcPythPullOracle.publicKey);
    console.log("token a: \t" + tokenAPythPullOracle.publicKey);
    console.log("token b: \t" + tokenBPythPullOracle.publicKey);
    console.log("lst:     \t" + lstPythPullOracle.publicKey);
  }

  const oracles: Oracles = {
    wsolOracle: wsolPythPullOracle,
    wsolOracleFeed: wsolPythPullOracleFeed,
    wsolDecimals: wsolDecimals,
    usdcOracle: usdcPythPullOracle,
    usdcOracleFeed: usdcPythPullOracleFeed,
    usdcDecimals: usdcDecimals,
    tokenAOracle: tokenAPythPullOracle,
    tokenAOracleFeed: tokenAPythPullOracleFeed,
    tokenADecimals: tokenADecimals,
    tokenBOracle: tokenBPythPullOracle,
    tokenBOracleFeed: tokenBPythPullOracleFeed,
    tokenBDecimals: tokenBDecimals,
    wsolPrice: wsolPrice,
    usdcPrice: usdcPrice,
    tokenAPrice: tokenAPrice,
    tokenBPrice: tokenBPrice,
    lstAlphaPrice: lstAlphaPrice,
    lstAlphaDecimals: lstAlphaDecimals,
    fakeUsdc: fakeUsdcPythPullOracle.publicKey,
    fakeUsdcFeed: fakeUsdcPythPullOracleFeed.publicKey,
    pythPullLst: lstPythPullOracle,
    pythPullLstOracleFeed: lstPythPullOracleFeed,
  };

  return oracles;
}
