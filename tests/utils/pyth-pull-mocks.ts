import { Keypair, PublicKey } from "@solana/web3.js";
import BN from "bn.js";
import { createMockAccount, Oracles, storeMockAccount } from "./mocks";
import {
  ASSET_TAG_DRIFT,
  ASSET_TAG_KAMINO,
  ASSET_TAG_SOLEND,
  ORACLE_CONF_INTERVAL,
} from "./types";
import { Mocks } from "../../target/types/mocks";
import { Marginfi } from "../../target/types/marginfi";
import { Program, Wallet, workspace } from "@coral-xyz/anchor";
import { printBuffers } from "../rootHooks";
import { ProgramTestContext, BanksClient } from "solana-bankrun";

type VerificationLevel =
  | { kind: "Partial"; num_signatures: number }
  | { kind: "Full" };

interface PriceFeedMessage {
  feed_id: PublicKey; // 32 bytes
  price: BN; // i64
  conf: BN; // u64
  exponent: number; // i32
  publish_time: BN; // i64 (timestamp in seconds)
  prev_publish_time: BN; // i64
  ema_price: BN; // i64
  ema_conf: BN; // u64
}

interface PriceUpdateV2 {
  write_authority: PublicKey; // 32 bytes
  verification_level: VerificationLevel; // 2 bytes (1 byte tag + 1 byte value/padding)
  price_message: PriceFeedMessage; // 84 bytes
  posted_slot: BN; // u64 (8 bytes)
}

/**
 * Decodes a base64 string (Borsh-serialized PriceUpdateV2) into the PriceUpdateV2 structure.
 *
 * Note: The first 8 bytes are assumed to be the Anchor account discriminator.
 */
export function decodePriceUpdateV2(base64Data: string): PriceUpdateV2 {
  const buffer = Buffer.from(base64Data, "base64");
  let offset = 0;

  // Skip the 8-byte discriminator. (34 241 35 99 157 126 244 205)

  //   const discrim = buffer.subarray(offset, offset + 8);
  //   for (let i = 0; i < discrim.length; i++) {
  //     console.log(i + " " + discrim[i]);
  //   }
  offset += 8;

  // 1. write_authority (32 bytes)
  const write_authority = new PublicKey(buffer.subarray(offset, offset + 32));
  offset += 32;

  // 2. verification_level (2 bytes)
  const verTag = buffer.readUInt8(offset);
  offset += 1;
  let verification_level: VerificationLevel;
  if (verTag === 0) {
    // "Partial" variant: the next byte is the num_signatures.
    const num_signatures = buffer.readUInt8(offset);
    offset += 1;
    verification_level = { kind: "Partial", num_signatures };
  } else if (verTag === 1) {
    // "Full" variant: the next byte is NOT SKIPPED, it is just one byte shorter!
    // offset += 1;
    verification_level = { kind: "Full" };
  } else {
    throw new Error(`Unknown verification level tag: ${verTag}`);
  }

  // 3. PriceFeedMessage
  // - feed_id: 32 bytes
  const feed_id = new PublicKey(buffer.subarray(offset, offset + 32));
  offset += 32;
  // - price: i64 (8 bytes, little-endian)
  const priceBN = new BN(buffer.subarray(offset, offset + 8), "le");
  offset += 8;
  // - conf: u64 (8 bytes)
  const conf = new BN(buffer.subarray(offset, offset + 8), "le");
  offset += 8;
  // - exponent: i32 (4 bytes)
  const exponent = buffer.readInt32LE(offset);
  offset += 4;
  // - publish_time: i64 (8 bytes)
  const publishTimeBN = new BN(buffer.subarray(offset, offset + 8), "le");
  offset += 8;
  // - prev_publish_time: i64 (8 bytes)
  const prevPublishTimeBN = new BN(buffer.subarray(offset, offset + 8), "le");
  offset += 8;
  // - ema_price: i64 (8 bytes)
  const emaPriceBN = new BN(buffer.subarray(offset, offset + 8), "le");
  offset += 8;
  // - ema_conf: u64 (8 bytes)
  const ema_conf = new BN(buffer.subarray(offset, offset + 8), "le");
  offset += 8;

  const price_message: PriceFeedMessage = {
    feed_id,
    price: priceBN,
    conf,
    exponent,
    publish_time: publishTimeBN,
    prev_publish_time: prevPublishTimeBN,
    ema_price: emaPriceBN,
    ema_conf,
  };

  // 4. posted_slot: u64 (8 bytes)
  const posted_slot = new BN(buffer.subarray(offset, offset + 8), "le");
  offset += 8;

  return {
    write_authority,
    verification_level,
    price_message,
    posted_slot,
  };
}

// ----- slop ------

/**
 * Pretty print oracle account data for debugging
 */
export async function debugPrintOracleData(
  banksClient: BanksClient,
  oracleAccount: PublicKey,
  oracleName: string = "Oracle",
) {
  const account = await banksClient.getAccount(oracleAccount);
  if (!account) {
    console.log(`=== ${oracleName} Oracle Account ===`);
    console.log("❌ Oracle account does not exist!");
    return;
  }

  const oracleData = decodePriceUpdateV2(
    Buffer.from(account.data).toString("base64"),
  );
  const price = oracleData.price_message.price.toNumber();
  const conf = oracleData.price_message.conf.toNumber();
  const exponent = oracleData.price_message.exponent;
  const publishTime = oracleData.price_message.publish_time.toNumber();
  const postedSlot = oracleData.posted_slot.toNumber();

  // Calculate human-readable values
  const priceInDollars = price * Math.pow(10, exponent);
  const confInDollars = conf * Math.pow(10, exponent);
  const confPercentage = (conf / price) * 100;

  console.log(`\n=== ${oracleName} Oracle Account Data ===`);
  console.log(`Oracle Address: ${oracleAccount.toString()}`);
  console.log(`Price (raw): ${price}`);
  console.log(`Price (human): $${priceInDollars.toFixed(6)}`);
  console.log(`Confidence (raw): ${conf}`);
  console.log(`Confidence (human): $${confInDollars.toFixed(6)}`);
  console.log(`Confidence %: ${confPercentage.toFixed(4)}%`);
  console.log(`Exponent: ${exponent}`);
  console.log(`Publish time: ${new Date(publishTime * 1000).toISOString()}`);
  console.log(`Posted slot: ${postedSlot}`);
  console.log(`Verification level: ${oracleData.verification_level.kind}`);
  console.log(`Feed ID: ${oracleData.price_message.feed_id.toString()}`);
  console.log(`==========================================\n`);
}

/**
 * Pretty print bank configuration for debugging
 */
export async function debugPrintBankConfig(
  program: Program<Marginfi>,
  bankPubkey: PublicKey,
  bankName: string = "Bank",
) {
  try {
    const bank = await program.account.bank.fetch(bankPubkey);

    console.log(`\n=== ${bankName} Bank Configuration ===`);
    console.log(`Bank Address: ${bankPubkey.toString()}`);

    // Oracle configuration
    console.log(`Oracle Setup: ${JSON.stringify(bank.config.oracleSetup)}`);
    console.log(`Oracle Max Confidence: ${bank.config.oracleMaxConfidence}`);
    console.log(`Oracle Max Age: ${bank.config.oracleMaxAge}`);
    console.log(`Oracle Keys[0]: ${bank.config.oracleKeys[0].toString()}`);
    console.log(`Oracle Keys[1]: ${bank.config.oracleKeys[1].toString()}`);

    // Bank type specific fields
    console.log(`Asset Tag: ${bank.config.assetTag}`);
    console.log(
      `Operational State: ${JSON.stringify(bank.config.operationalState)}`,
    );

    // Check which type of bank it is
    const isDrift = bank.config.assetTag === ASSET_TAG_DRIFT;
    const isSolend = bank.config.assetTag === ASSET_TAG_SOLEND;
    const isKamino = bank.config.assetTag === ASSET_TAG_KAMINO;

    console.log(
      `Bank Type: ${
        isDrift
          ? "Drift"
          : isSolend
          ? "Solend"
          : isKamino
          ? "Kamino"
          : "Regular"
      }`,
    );

    if (isKamino) {
      console.log(`Kamino Reserve: ${bank.integrationAcc1.toString()}`);
      console.log(`Kamino Obligation: ${bank.integrationAcc2.toString()}`);
    }

    if (isDrift) {
      console.log(`Drift Spot Market: ${bank.integrationAcc1.toString()}`);
      console.log(`Drift User: ${bank.integrationAcc2.toString()}`);
    }

    if (isSolend) {
      console.log(`Solend Reserve: ${bank.integrationAcc1.toString()}`);
      console.log(`Solend Obligation: ${bank.integrationAcc2.toString()}`);
    }

    console.log(`========================================\n`);
  } catch (error) {
    console.log(`\n=== ${bankName} Bank Configuration ===`);
    console.log(`❌ Error fetching bank: ${error.message}`);
    console.log(`========================================\n`);
  }
}

// ------- slop -------

/**
 * Constructs a PriceUpdateV2 buffer (with discriminator) and writes it to the mock account. Pass `existingAccount` to update an existing feed.
 *
 * Layout:
 *   - 8 bytes: Discriminator (34, 241, 35, 99, 157, 126, 244, 205)
 *   - 32 bytes: write_authority (PublicKey)
 *   - 2 bytes: verification_level (Full => [1, 0])
 *   - 32 bytes: feed_id (PublicKey)
 *   - 8 bytes: price (i64)
 *   - 8 bytes: conf (u64)
 *   - 4 bytes: exponent (i32)
 *   - 8 bytes: publish_time (i64)
 *   - 8 bytes: prev_publish_time (i64)
 *   - 8 bytes: ema_price (i64)
 *   - 8 bytes: ema_conf (u64)
 *   - 8 bytes: posted_slot (u64)
 *
 * Total: 134 bytes.
 */
export async function initOrUpdatePriceUpdateV2(
  wallet: Wallet,
  feed_id: PublicKey,
  price: BN,
  conf: BN,
  time: number,
  exponent: number,
  // Use after setup to update existing account
  existingAccount?: Keypair,
  // Use to give a deterministic keypair during initial setup instead of a random one
  oracleKeypair?: Keypair,
  bankrunContext?: ProgramTestContext,
  verbose: boolean = false,
  publishTime?: number,
) {
  const space = 134;
  // Compute publish times.
  const now = publishTime ?? Math.floor(Date.now() / 1000);
  if (verbose) {
    if (existingAccount) {
      console.log(
        "publish price to " +
          existingAccount.publicKey.toString() +
          " at: " +
          now,
      );
    } else {
      console.log("publish price to a new feed at " + now);
    }
    let nowActually = Math.floor(Date.now() / 1000);
    console.log("your system clock reads: " + nowActually);
    if (bankrunContext) {
      let clock = await bankrunContext.banksClient.getClock();
      console.log("bankrun thinks the time is: " + clock.unixTimestamp);
    }
  }
  const publish_time = new BN(now);
  const prev_publish_time = new BN(now - 1);
  // Allocate a 134-byte buffer.
  const buf = Buffer.alloc(space);
  let offset = 0;
  // Write the 8-byte discriminator.
  const discriminator = Buffer.from([34, 241, 35, 99, 157, 126, 244, 205]);
  discriminator.copy(buf, offset);
  offset += 8;
  // Write the write_authority (32 bytes).
  // Don't care about this key, use any value.
  const writeAuthority = PublicKey.unique().toBuffer();
  writeAuthority.copy(buf, offset);
  offset += 32;
  // Write verification_level (2 bytes) - "Full": tag 1 and dummy 0.
  buf.writeUInt8(1, offset); // tag for Full
  offset += 1;

  // PriceFeedMessage:
  // feed_id (32 bytes)
  feed_id.toBuffer().copy(buf, offset);
  offset += 32;
  // price (i64, 8 bytes)
  price.toArrayLike(Buffer, "le", 8).copy(buf, offset);
  offset += 8;
  // conf (u64, 8 bytes)
  conf.toArrayLike(Buffer, "le", 8).copy(buf, offset);
  offset += 8;
  // exponent (i32, 4 bytes)
  buf.writeInt32LE(exponent, offset);
  offset += 4;
  // publish_time (i64, 8 bytes)
  publish_time.toArrayLike(Buffer, "le", 8).copy(buf, offset);
  offset += 8;
  // prev_publish_time (i64, 8 bytes)
  prev_publish_time.toArrayLike(Buffer, "le", 8).copy(buf, offset);
  offset += 8;
  // ema_price (i64, 8 bytes)
  price.toArrayLike(Buffer, "le", 8).copy(buf, offset);
  offset += 8;
  // ema_conf (u64, 8 bytes)
  conf.toArrayLike(Buffer, "le", 8).copy(buf, offset);
  offset += 8;
  // posted_slot (u64, 8 bytes)
  new BN(0).toArrayLike(Buffer, "le", 8).copy(buf, offset);
  offset += 8;

  if (printBuffers) {
    console.log("PriceUpdateV2 Buffer (base64):", buf.toString("base64"));
  }

  // Write the buffer to the mock account
  const mockProgram: Program<Mocks> = workspace.Mocks;
  if (existingAccount) {
    await storeMockAccount(
      mockProgram,
      wallet,
      existingAccount,
      0,
      buf,
      bankrunContext,
    );
    return existingAccount;
  } else {
    let account = await createMockAccount(
      mockProgram,
      space,
      wallet,
      oracleKeypair,
      bankrunContext,
    );
    await storeMockAccount(
      mockProgram,
      wallet,
      account,
      0,
      buf,
      bankrunContext,
    );
    return account;
  }
}

/**
 * Refreshes any oracle in Oracles that is EXACTLY NAMED  *OracleFeed to the same
 * price/conf/etc it currently has but the given slot/time
 * @param oracles
 * @param wallet
 * @param publishTime
 * @param bankrunContext
 * @param verbose
 */
export async function refreshOracles(
  oracles: Oracles,
  wallet: Keypair,
  slot: BN,
  publishTime: number,
  bankrunContext?: ProgramTestContext,
  verbose: boolean = false
) {
  // Discover all "*OracleFeed" "*Pull" "*Price" "*Decimals" entries
  const feeds = (Object.keys(oracles) as Array<keyof Oracles>)
    .filter((k) => k.endsWith("OracleFeed"))
    .map((feedKey) => {
      const baseKey = feedKey.replace(/OracleFeed$/, "") as keyof Oracles;
      const feedId: PublicKey = (oracles[feedKey] as Keypair).publicKey;
      const account = oracles[`${baseKey as string}Oracle` as keyof Oracles] as Keypair;

      const tokenName = (baseKey as string).replace(/Pull$/, "");
      const priceKey = `${tokenName}Price` as keyof Oracles;
      const decimalsKey = `${tokenName}Decimals` as keyof Oracles;

      const priceNum = (oracles as any)[priceKey] as number;
      const dec = (oracles as any)[decimalsKey] as number;
      const price = new BN(priceNum * 10 ** dec);
      const conf = new BN(priceNum * ORACLE_CONF_INTERVAL * 10 ** dec);
      const emaPrice = price.clone();
      const emaConf = conf.clone();
      const exponent = -dec;

      return {
        base: baseKey,
        feedId,
        account,
        price,
        conf,
        emaPrice,
        emaConf,
        exponent,
      };
    });

  const tasks = feeds.map(
    ({ base, feedId, account, price, conf, emaPrice, emaConf, exponent }) => {
      if (verbose) {
        console.log(
          `[batchUpdate] ${base}: price=${price.toString()}, conf=${conf.toString()}, slot=${slot.toString()}, exp=${exponent}`
        );
      }
      return initOrUpdatePriceUpdateV2(
        new Wallet(wallet),
        feedId,
        price,
        conf,
        // emaPrice,
        // emaConf,
        slot.toNumber(),
        exponent,
        account,
        undefined,
        bankrunContext,
        verbose,
        publishTime
      );
    }
  );

  await Promise.all(tasks);
}

// TODO use this in roothooks instead of the custom implementation...
/**
 * Refreshes any oracle in Oracles that is EXACTLY NAMED *Pull with *OracleFeed to the same
 * price/conf/etc it currently has but the given slot/time
 * @param oracles
 * @param wallet
 * @param publishTime
 * @param bankrunContext
 * @param verbose
 */
export async function refreshPullOracles(
  oracles: Oracles,
  wallet: Keypair,
  slot: BN,
  publishTime: number,
  bankrunContext?: ProgramTestContext,
  verbose: boolean = false,
) {
  // Discover all "*PullOracleFeed" "*Pull" "*Price" "*Decimals" entries
  const feeds = (Object.keys(oracles) as Array<keyof Oracles>)
    .filter((k) => k.endsWith("PullOracleFeed"))
    .map((feedKey) => {
      const baseKey = feedKey.replace(/OracleFeed$/, "") as keyof Oracles;
      const feedId: PublicKey = (oracles[feedKey] as Keypair).publicKey;
      const account = oracles[baseKey] as Keypair;

      const tokenName = (baseKey as string).replace(/Pull$/, "");
      const priceKey = `${tokenName}Price` as keyof Oracles;
      const decimalsKey = `${tokenName}Decimals` as keyof Oracles;

      const priceNum = (oracles as any)[priceKey] as number;
      const dec = (oracles as any)[decimalsKey] as number;
      const price = new BN(priceNum * 10 ** dec);
      const conf = new BN(priceNum * ORACLE_CONF_INTERVAL * 10 ** dec);
      const emaPrice = price.clone();
      const emaConf = conf.clone();
      const exponent = -dec;

      return {
        base: baseKey,
        feedId,
        account,
        price,
        conf,
        emaPrice,
        emaConf,
        exponent,
      };
    });

  const tasks = feeds.map(
    ({ base, feedId, account, price, conf, emaPrice, emaConf, exponent }) => {
      if (verbose) {
        console.log(
          `[batchUpdate] ${base}: price=${price.toString()}, conf=${conf.toString()}, slot=${slot.toString()}, exp=${exponent}`,
        );
      }
      return initOrUpdatePriceUpdateV2(
        new Wallet(wallet),
        feedId,
        price,
        conf,
        publishTime ?? Math.floor(Date.now() / 1000),
        exponent,
        account,
        undefined,
        bankrunContext,
        verbose,
        publishTime,
      );
    },
  );

  await Promise.all(tasks);
}

/**
 * Price updates expect a `valid` feed, but don't actually read anything from it. If suffices to
 * create an account of the correct size with "PYTH" as the owner. Pass this key as the `feed_id` to
 * create price updates for an asset. This size of this account also doesn't matter, nor does the
 * discriminator, etc. It can literally be any account owned by "PYTH". On mainnet this is the
 * receiver program, on localnet we let the mocks program own both the oracles and these feeds.
 * @param wallet
 * @returns
 */
export async function initBlankOracleFeed(wallet: Wallet, keypair?: Keypair) {
  const space = 300;
  const buf = Buffer.alloc(space);

  // Write the buffer to the mock account
  const mockProgram: Program<Mocks> = workspace.Mocks;
  let account = await createMockAccount(mockProgram, space, wallet, keypair);
  await storeMockAccount(mockProgram, wallet, account, 0, buf);

  return account;
}

// Sample usage (run with `ts-node pyth-pull-mocks.ts`)

// const base64Data = "IvEjY51+9M0AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAQETiBzPres/ehv0qThWXLPfQ9hD11F3I/i9W1bPF+o7/QAW0L4oAAAAAMOd0AAAAAD3////vBLSZwAAAAC7EtJnAAAAAAAW0L4oAAAAAMOd0AAAAAAAAAAAAAAAAAA=";
// //const base64Data =
// //  "IvEjY51+9M1gMUcENA3t3zcf1CRyFI8kjp0abRpesqw6zYt/1dayQwHvDYtv2izrpB2hXUCV0do5Kg0vjtDGx7wPTPrIwoC1bW2njwUFAAAARTozAQAAAAD4////1wN5ZwAAAADXA3lnAAAAAIRKaggFAAAAk1kTAQAAAAA8qpUSAAAAAAA=";
// const priceUpdate = decodePriceUpdateV2(base64Data);

// // For debugging, output fields in a readable format.
// console.log("Write Authority (hex):", priceUpdate.write_authority.toString());
// console.log("Verification Level:", priceUpdate.verification_level);
// console.log("Price Feed Message:", {
//   feed_id: priceUpdate.price_message.feed_id.toString(),
//   price: priceUpdate.price_message.price.toString(),
//   conf: priceUpdate.price_message.conf.toString(),
//   exponent: priceUpdate.price_message.exponent,
//   publish_time: priceUpdate.price_message.publish_time.toString(),
//   prev_publish_time: priceUpdate.price_message.prev_publish_time.toString(),
//   ema_price: priceUpdate.price_message.ema_price.toString(),
//   ema_conf: priceUpdate.price_message.ema_conf.toString(),
// });
// console.log("Posted Slot:", priceUpdate.posted_slot.toString());
