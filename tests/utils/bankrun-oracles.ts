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
import { SWITCHBOARD_PULL_WSOL_FEED_HEX } from "../fixtures/switchboard_pull_wsol_fixture";

/** Default Pyth receiver program ID (mocks program) */
export const PYTH_RECEIVER_PROGRAM_ID = new PublicKey(
  "rec5EKMGg6MxZYaMdyBfgwp4d5rB9T1VQH5pJv5LtFJ"
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
  owner: PublicKey
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
    })
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
  owner: PublicKey
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
    })
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
    "rec5EKMGg6MxZYaMdyBfgwp4d5rB9T1VQH5pJv5LtFJ"
  )
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
      "Account does not exist, not creating because this causes bankrun issues"
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
    "rec5EKMGg6MxZYaMdyBfgwp4d5rB9T1VQH5pJv5LtFJ"
  )
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
    owner
  );

  await setPythPullOraclePrice(
    bankrunContext,
    banksClient,
    oracles.wsolOracle.publicKey,
    oracles.wsolOracleFeed.publicKey,
    oracles.wsolPrice,
    oracles.wsolDecimals,
    ORACLE_CONF_INTERVAL,
    owner
  );

  await setPythPullOraclePrice(
    bankrunContext,
    banksClient,
    oracles.usdcOracle.publicKey,
    oracles.usdcOracleFeed.publicKey,
    oracles.usdcPrice,
    oracles.usdcDecimals,
    ORACLE_CONF_INTERVAL,
    owner
  );

  await setPythPullOraclePrice(
    bankrunContext,
    banksClient,
    oracles.tokenAOracle.publicKey,
    oracles.tokenAOracleFeed.publicKey,
    oracles.tokenAPrice,
    oracles.tokenADecimals,
    ORACLE_CONF_INTERVAL,
    owner
  );

  await setPythPullOraclePrice(
    bankrunContext,
    banksClient,
    oracles.tokenBOracle.publicKey,
    oracles.tokenBOracleFeed.publicKey,
    oracles.tokenBPrice,
    oracles.tokenBDecimals,
    ORACLE_CONF_INTERVAL,
    owner
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
  verbose: boolean = false
): Promise<
  Omit<
    Oracles,
    | "wsolOracleSwb"
    | "wsolPriceSwb"
    | "wsolDecimalsSwb"
    | "tokenAOracleSwb"
    | "tokenAPriceSwb"
    | "tokenADecimalsSwb"
    | "lstAlphaOracleSwb"
    | "lstAlphaPriceSwb"
    | "lstAlphaDecimalsSwb"
  >
> {
  const owner = PYTH_RECEIVER_PROGRAM_ID;

  // Deterministic keypairs (same as pyth_mocks.ts)
  const wsolPythPullOracle = Keypair.fromSeed(
    Buffer.from("ORACLE_SEED_00000000000000F_WSOL")
  );
  const wsolPythPullOracleFeed = Keypair.fromSeed(
    Buffer.from("ORACLE_SEED_0000000000000ID_WSOL")
  );
  const usdcPythPullOracle = Keypair.fromSeed(
    Buffer.from("ORACLE_SEED_00000000000000F_USDC")
  );
  const usdcPythPullOracleFeed = Keypair.fromSeed(
    Buffer.from("ORACLE_SEED_0000000000000ID_USDC")
  );
  const fakeUsdcPythPullOracle = Keypair.fromSeed(
    Buffer.from("ORACLE_SEED_00000000000001F_USDC")
  );
  const fakeUsdcPythPullOracleFeed = Keypair.fromSeed(
    Buffer.from("ORACLE_SEED_0000000000001ID_USDC")
  );
  const tokenAPythPullOracle = Keypair.fromSeed(
    Buffer.from("ORACLE_SEED_00000000000001F_00TA")
  );
  const tokenAPythPullOracleFeed = Keypair.fromSeed(
    Buffer.from("ORACLE_SEED_0000000000001ID_00TA")
  );
  const tokenBPythPullOracle = Keypair.fromSeed(
    Buffer.from("ORACLE_SEED_00000000000001F_00TB")
  );
  const tokenBPythPullOracleFeed = Keypair.fromSeed(
    Buffer.from("ORACLE_SEED_0000000000001ID_00TB")
  );
  const lstPythPullOracle = Keypair.fromSeed(
    Buffer.from("ORACLE_SEED_00000000000001F_0LST")
  );
  const lstPythPullOracleFeed = Keypair.fromSeed(
    Buffer.from("ORACLE_SEED_0000000000001ID_0LST")
  );

  // Create all feed accounts
  await createBankrunPythFeedAccount(
    bankrunContext,
    banksClient,
    wsolPythPullOracleFeed,
    owner
  );
  await createBankrunPythFeedAccount(
    bankrunContext,
    banksClient,
    usdcPythPullOracleFeed,
    owner
  );
  await createBankrunPythFeedAccount(
    bankrunContext,
    banksClient,
    fakeUsdcPythPullOracleFeed,
    owner
  );
  await createBankrunPythFeedAccount(
    bankrunContext,
    banksClient,
    tokenAPythPullOracleFeed,
    owner
  );
  await createBankrunPythFeedAccount(
    bankrunContext,
    banksClient,
    tokenBPythPullOracleFeed,
    owner
  );
  await createBankrunPythFeedAccount(
    bankrunContext,
    banksClient,
    lstPythPullOracleFeed,
    owner
  );

  // Create all oracle accounts
  await createBankrunPythOracleAccount(
    bankrunContext,
    banksClient,
    wsolPythPullOracle,
    owner
  );
  await createBankrunPythOracleAccount(
    bankrunContext,
    banksClient,
    usdcPythPullOracle,
    owner
  );
  await createBankrunPythOracleAccount(
    bankrunContext,
    banksClient,
    fakeUsdcPythPullOracle,
    owner
  );
  await createBankrunPythOracleAccount(
    bankrunContext,
    banksClient,
    tokenAPythPullOracle,
    owner
  );
  await createBankrunPythOracleAccount(
    bankrunContext,
    banksClient,
    tokenBPythPullOracle,
    owner
  );
  await createBankrunPythOracleAccount(
    bankrunContext,
    banksClient,
    lstPythPullOracle,
    owner
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
    owner
  );
  await setPythPullOraclePrice(
    bankrunContext,
    banksClient,
    usdcPythPullOracle.publicKey,
    usdcPythPullOracleFeed.publicKey,
    usdcPrice,
    usdcDecimals,
    ORACLE_CONF_INTERVAL,
    owner
  );
  await setPythPullOraclePrice(
    bankrunContext,
    banksClient,
    fakeUsdcPythPullOracle.publicKey,
    fakeUsdcPythPullOracleFeed.publicKey,
    usdcPrice,
    usdcDecimals,
    ORACLE_CONF_INTERVAL,
    owner
  );
  await setPythPullOraclePrice(
    bankrunContext,
    banksClient,
    tokenAPythPullOracle.publicKey,
    tokenAPythPullOracleFeed.publicKey,
    tokenAPrice,
    tokenADecimals,
    ORACLE_CONF_INTERVAL,
    owner
  );
  await setPythPullOraclePrice(
    bankrunContext,
    banksClient,
    tokenBPythPullOracle.publicKey,
    tokenBPythPullOracleFeed.publicKey,
    tokenBPrice,
    tokenBDecimals,
    ORACLE_CONF_INTERVAL,
    owner
  );
  await setPythPullOraclePrice(
    bankrunContext,
    banksClient,
    lstPythPullOracle.publicKey,
    lstPythPullOracleFeed.publicKey,
    lstAlphaPrice,
    lstAlphaDecimals,
    ORACLE_CONF_INTERVAL,
    owner
  );

  if (verbose) {
    console.log("Mock Pyth Pull price oracles (bankrun):");
    console.log("wsol:    \t" + wsolPythPullOracle.publicKey);
    console.log("usdc:    \t" + usdcPythPullOracle.publicKey);
    console.log("token a: \t" + tokenAPythPullOracle.publicKey);
    console.log("token b: \t" + tokenBPythPullOracle.publicKey);
    console.log("lst:     \t" + lstPythPullOracle.publicKey);
  }

  const oracles: Omit<
    Oracles,
    | "wsolOracleSwb"
    | "wsolPriceSwb"
    | "wsolDecimalsSwb"
    | "tokenAOracleSwb"
    | "tokenAPriceSwb"
    | "tokenADecimalsSwb"
    | "lstAlphaOracleSwb"
    | "lstAlphaPriceSwb"
    | "lstAlphaDecimalsSwb"
  > = {
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

// ---------------------------------------------------------------------------
// Switchboard Pull Oracle Helpers
// ---------------------------------------------------------------------------

export const SWITCHBOARD_PULL_PROGRAM_ID = new PublicKey(
  "SBondMDrcV3K4kxZR1HNVT7osZxAHVHgYXL5Ze1oMUv"
);

/**
 * Switchboard Pull feed discriminator (8 bytes).
 *
 * This matches `PullFeedAccountData::DISCRIMINATOR` from the `switchboard_on_demand` crate.
 *
 * Source of truth: the Rust unit test fixture in `programs/marginfi/src/state/price.rs`
 * (`swb_pull_get_price_1`) starts with these 8 bytes.
 */
export const SWITCHBOARD_PULL_FEED_DISCRIMINATOR = Buffer.from([
  0xc4, 0x1b, 0x6c, 0xc4, 0x0a, 0xd7, 0xdb, 0x28,
]);

/**
 * Byte offset (from the *start* of the feed account data) for `last_update_timestamp: i64`.
 *
 * We use this to "refresh" Switchboard Pull oracles in bankrun by directly editing the account
 * bytes, similar to how we refresh Pyth Pull oracles.
 *
 * IMPORTANT:
 * - This offset assumes the 8-byte discriminator is present at `data[0..8]`.
 * - If you accidentally strip the discriminator (making the buffer 3200 bytes), you will corrupt
 *   unrelated fields (including `result.value`) and Marginfi may read price=0.
 *
 * Derived from the Rust fixture (`swb_pull_get_price_1`) by locating the embedded timestamp
 * `0x0000000066b69e9e` (1723244190) in little-endian, which occurs at byte 2216.
 */
export const SWITCHBOARD_PULL_LAST_UPDATE_TIMESTAMP_OFFSET = 2216;

/**
 * Byte offset (from the start of account data, including discriminator) for
 * `result.value: i128` in `PullFeedAccountData`.
 *
 * Used to patch fixture prices for bankrun tests while keeping the rest of the
 * account layout intact.
 */
export const SWITCHBOARD_PULL_RESULT_VALUE_OFFSET = 2264;

/**
 * Byte offset (from the start of account data, including discriminator) for
 * `result.std_dev: i128` in `PullFeedAccountData`.
 */
export const SWITCHBOARD_PULL_RESULT_STD_DEV_OFFSET = 2280;

/**
 * Minimum expected size of a Switchboard Pull feed account.
 *
 * Currently: 8-byte discriminator + 3200-byte `PullFeedAccountData` = 3208 bytes.
 *
 * If this changes upstream, Marginfi's `SwitchboardPullPriceFeed::load_checked` will also change.
 */
export const SWITCHBOARD_PULL_MIN_FEED_ACCOUNT_DATA_LEN = 3208;

function assertValidSwitchboardPullFeedData(data: Buffer): void {
  if (data.length < SWITCHBOARD_PULL_MIN_FEED_ACCOUNT_DATA_LEN) {
    throw new Error(
      `Switchboard Pull feed buffer too small: got ${data.length}, expected >= ${SWITCHBOARD_PULL_MIN_FEED_ACCOUNT_DATA_LEN}`
    );
  }

  const disc = data.subarray(0, 8);
  if (!disc.equals(SWITCHBOARD_PULL_FEED_DISCRIMINATOR)) {
    throw new Error(
      `Switchboard Pull feed discriminator mismatch. Got ${disc.toString(
        "hex"
      )}, expected ${SWITCHBOARD_PULL_FEED_DISCRIMINATOR.toString("hex")}`
    );
  }
}

const scaledDecimalToI128 = (value: number, scale: number): bigint => {
  const normalized = value.toFixed(scale);
  const [wholeRaw, fracRaw = ""] = normalized.split(".");
  const negative = wholeRaw.startsWith("-");
  const whole = negative ? wholeRaw.slice(1) : wholeRaw;
  const frac = fracRaw.padEnd(scale, "0").slice(0, scale);
  const combined = `${whole}${frac}`;
  const magnitude = BigInt(combined.length > 0 ? combined : "0");
  return negative ? -magnitude : magnitude;
};

const writeI128LE = (buffer: Buffer, offset: number, value: bigint): void => {
  const min = -(1n << 127n);
  const max = (1n << 127n) - 1n;
  if (value < min || value > max) {
    throw new Error(`i128 overflow: ${value.toString()}`);
  }

  let v = value < 0 ? (1n << 128n) + value : value;
  for (let i = 0; i < 16; i += 1) {
    buffer[offset + i] = Number(v & 0xffn);
    v >>= 8n;
  }
};

/**
 * Build a Switchboard Pull feed buffer by patching the default WSOL fixture.
 *
 * `price` and `stdDev` are encoded with Switchboard's fixed precision (1e18).
 */
export const buildSwitchboardPullFixtureFromTemplate = (args: {
  price: number;
  stdDev?: number;
  templateHex?: string;
}): Buffer => {
  const { price, stdDev, templateHex = SWITCHBOARD_PULL_WSOL_FEED_HEX } = args;
  const raw = Buffer.from(templateHex, "hex");
  const data =
    raw.length >= SWITCHBOARD_PULL_MIN_FEED_ACCOUNT_DATA_LEN
      ? Buffer.from(raw)
      : Buffer.concat([
          raw,
          Buffer.alloc(SWITCHBOARD_PULL_MIN_FEED_ACCOUNT_DATA_LEN - raw.length),
        ]);

  assertValidSwitchboardPullFeedData(data);
  writeI128LE(
    data,
    SWITCHBOARD_PULL_RESULT_VALUE_OFFSET,
    scaledDecimalToI128(price, 18)
  );
  if (stdDev !== undefined) {
    writeI128LE(
      data,
      SWITCHBOARD_PULL_RESULT_STD_DEV_OFFSET,
      scaledDecimalToI128(stdDev, 18)
    );
  }

  return data;
};

/**
 * Creates a blank Switchboard Pull feed account in bankrun.
 *
 * The account data must be a valid `PullFeedAccountData` (Switchboard On-Demand).
 * For tests, we typically create the account and then `setAccount` with a fixture buffer.
 *
 * @param space - The size (in bytes) of the PullFeedAccountData buffer you will write later.
 * @param owner - The program that owns this feed account (defaults to SWITCHBOARD_PULL_PROGRAM_ID).
 */
export async function createBankrunSwitchboardPullFeedAccount(
  bankrunContext: ProgramTestContext,
  banksClient: BanksClient,
  feedKeypair: Keypair,
  space: number,
  owner: PublicKey = SWITCHBOARD_PULL_PROGRAM_ID
): Promise<Keypair> {
  const rent = await banksClient.getRent();
  const lamports = Number(rent.minimumBalance(BigInt(space)));

  const tx = new Transaction().add(
    SystemProgram.createAccount({
      fromPubkey: bankrunContext.payer.publicKey,
      newAccountPubkey: feedKeypair.publicKey,
      lamports,
      space,
      programId: owner,
    })
  );

  await processBankrunTransaction(bankrunContext, tx, [
    bankrunContext.payer,
    feedKeypair,
  ]);

  return feedKeypair;
}

/**
 * Create or overwrite a deterministic Switchboard Pull oracle account from the
 * default fixture template, patching only price/std_dev.
 */
export async function setupSwitchboardPullOracleFromTemplate(
  bankrunContext: ProgramTestContext,
  banksClient: BanksClient,
  feedKeypair: Keypair,
  args: {
    price: number;
    stdDev?: number;
    verbose?: boolean;
    label?: string;
  }
): Promise<Keypair> {
  const { price, stdDev, verbose = false, label } = args;
  const feedData = buildSwitchboardPullFixtureFromTemplate({ price, stdDev });

  const existing = await banksClient.getAccount(feedKeypair.publicKey);
  if (!existing) {
    await createBankrunSwitchboardPullFeedAccount(
      bankrunContext,
      banksClient,
      feedKeypair,
      feedData.length
    );
  }

  await setSwitchboardPullFeedAccountData(
    bankrunContext,
    banksClient,
    feedKeypair.publicKey,
    feedData
  );
  await refreshSwitchboardPullOracleBankrun(
    bankrunContext,
    banksClient,
    feedKeypair.publicKey
  );

  if (verbose) {
    const prefix = label ? `${label}: ` : "";
    console.log(
      `${prefix}${feedKeypair.publicKey.toBase58()} (price=${price.toFixed(8)})`
    );
  }

  return feedKeypair;
}

/**
 * Overwrite an existing Switchboard Pull feed account's data buffer in bankrun.
 *
 * NOTE: We create the account via SystemProgram::create_account first to avoid bankrun "Account in use"
 * edge cases. Then we use setAccount to write the real bytes.
 */
export async function setSwitchboardPullFeedAccountData(
  bankrunContext: ProgramTestContext,
  banksClient: BanksClient,
  feedAccount: PublicKey,
  data: Buffer
): Promise<void> {
  const existing = await banksClient.getAccount(feedAccount);

  if (!existing) {
    console.log(
      "Switchboard Pull feed account does not exist (did you create it first?)"
    );
    return;
  }

  bankrunContext.setAccount(feedAccount, {
    lamports: existing.lamports,
    data,
    owner: existing.owner, // Preserve existing owner
    executable: existing.executable,
    rentEpoch: existing.rentEpoch,
  });
}

/**
 * Refresh a Switchboard Pull feed's staleness timestamp in bankrun by editing bytes directly.
 *
 * This mirrors `refreshPullOraclesBankrun` (Pyth Pull) but for Switchboard Pull feeds.
 *
 * @param feedAccount - The Switchboard Pull feed account pubkey.
 * @param unixTimestampOverride - If provided, write this value instead of the current bankrun clock.
 */
export async function refreshSwitchboardPullOracleBankrun(
  bankrunContext: ProgramTestContext,
  banksClient: BanksClient,
  feedAccount: PublicKey,
  unixTimestampOverride?: number
): Promise<void> {
  const existing = await banksClient.getAccount(feedAccount);

  if (!existing) {
    console.log(
      "Switchboard Pull feed account does not exist (did you create it first?)"
    );
    return;
  }

  const data = Buffer.from(existing.data);
  assertValidSwitchboardPullFeedData(data);

  const clock = await banksClient.getClock();
  const ts = unixTimestampOverride ?? Number(clock.unixTimestamp);

  // Write i64 LE at the known field offset.
  new BN(ts)
    .toArrayLike(Buffer, "le", 8)
    .copy(data, SWITCHBOARD_PULL_LAST_UPDATE_TIMESTAMP_OFFSET);

  bankrunContext.setAccount(feedAccount, {
    lamports: existing.lamports,
    data,
    owner: existing.owner,
    executable: existing.executable,
    rentEpoch: existing.rentEpoch,
  });
}

/**
 * Create a deterministic Switchboard Pull SOL/USD oracle account in bankrun and seed it with the
 * hardcoded fixture bytes.
 */
export async function setupSwbOraclesBankrun(
  bankrunContext: ProgramTestContext,
  banksClient: BanksClient,
  prices: {
    wsolPrice: number;
    wsolDecimals: number;
    tokenAPrice: number;
    tokenADecimals: number;
    lstAlphaPrice: number;
    lstAlphaDecimals: number;
  },
  verbose: boolean = false
): Promise<
  Pick<
    Oracles,
    | "wsolOracleSwb"
    | "wsolPriceSwb"
    | "wsolDecimalsSwb"
    | "tokenAOracleSwb"
    | "tokenAPriceSwb"
    | "tokenADecimalsSwb"
    | "lstAlphaOracleSwb"
    | "lstAlphaPriceSwb"
    | "lstAlphaDecimalsSwb"
  >
> {
  const {
    wsolPrice,
    wsolDecimals,
    tokenAPrice,
    tokenADecimals,
    lstAlphaPrice,
    lstAlphaDecimals,
  } = prices;
  const wsolOracleSwb = Keypair.fromSeed(
    Buffer.from("ORACLE_SEED_00000000000000F_SWB0")
  );
  const tokenAOracleSwb = Keypair.fromSeed(
    Buffer.from("ORACLE_SEED_00000000000000F_SWB1")
  );
  const lstAlphaOracleSwb = Keypair.fromSeed(
    Buffer.from("ORACLE_SEED_00000000000000F_SWB2")
  );

  await setupSwitchboardPullOracleFromTemplate(
    bankrunContext,
    banksClient,
    wsolOracleSwb,
    { price: wsolPrice, verbose, label: "wsol swb" }
  );
  await setupSwitchboardPullOracleFromTemplate(
    bankrunContext,
    banksClient,
    tokenAOracleSwb,
    { price: tokenAPrice, verbose, label: "tokenA swb" }
  );
  await setupSwitchboardPullOracleFromTemplate(
    bankrunContext,
    banksClient,
    lstAlphaOracleSwb,
    { price: lstAlphaPrice, verbose, label: "lstAlpha swb" }
  );

  return {
    wsolOracleSwb,
    wsolPriceSwb: wsolPrice,
    wsolDecimalsSwb: wsolDecimals,
    tokenAOracleSwb,
    tokenAPriceSwb: tokenAPrice,
    tokenADecimalsSwb: tokenADecimals,
    lstAlphaOracleSwb,
    lstAlphaPriceSwb: lstAlphaPrice,
    lstAlphaDecimalsSwb: lstAlphaDecimals,
  };
}
