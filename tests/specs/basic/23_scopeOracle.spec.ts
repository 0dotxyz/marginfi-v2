import { BN, Program } from "@coral-xyz/anchor";
import { Keypair, PublicKey, Transaction } from "@solana/web3.js";
import { Marginfi } from "../../../target/types/marginfi";
import {
  addBank,
  configureBankOracleScope,
  groupInitialize,
} from "../../utils/group-instructions";
import {
  accountInit,
  composeRemainingAccounts,
  depositIx,
  healthPulse,
  pulseBankPrice,
} from "../../utils/user-instructions";
import {
  bankrunContext,
  bankrunProgram,
  bankRunProvider,
  ecosystem,
  groupAdmin,
  users,
} from "../../rootHooks";
import {
  assertI80F48Approx,
  expectFailedTxWithError,
} from "../../utils/genericTests";
import {
  defaultBankConfig,
  HEALTH_CACHE_HEALTHY,
  HEALTH_CACHE_ORACLE_OK,
} from "../../utils/types";
import { bytesToF64, getBankrunTime } from "../../utils/tools";
import { wrappedI80F48toBigNumber } from "@mrgnlabs/mrgn-common";
import { assert } from "chai";
import { createMintToInstruction } from "@solana/spl-token";

const SCOPE_PROGRAM = new PublicKey(
  "HFn8GnPADiny6XqUoWE8uRPPxb29ikn4yTuPa9MF2fWJ"
);
/** sha256("account:OraclePrices")[..8] */
const ORACLE_PRICES_DISCRIMINATOR = Buffer.from([
  89, 128, 118, 221, 6, 72, 180, 146,
]);
const PRICES_OFFSET = 40;
const DATED_PRICE_SIZE = 56;
const MAX_ENTRIES = 512;
const ORACLE_PRICES_SIZE = PRICES_OFFSET + MAX_ENTRIES * DATED_PRICE_SIZE;

type Entry = { index: number; value: bigint; exp: bigint; timestamp: number };

/**
 * Builds a scope `OraclePrices` buffer: discriminator, the oracle_mappings pubkey, then 512
 * `DatedPrice { value: u64, exp: u64, last_updated_slot: u64, unix_timestamp: u64, _pad: [u8;24] }`.
 * Unlisted entries stay all-zero, i.e. "never refreshed".
 */
const makePrices = (entries: Entry[]) => {
  const data = Buffer.alloc(ORACLE_PRICES_SIZE);
  ORACLE_PRICES_DISCRIMINATOR.copy(data, 0);
  for (const e of entries) {
    const off = PRICES_OFFSET + e.index * DATED_PRICE_SIZE;
    data.writeBigUInt64LE(e.value, off);
    data.writeBigUInt64LE(e.exp, off + 8);
    data.writeBigUInt64LE(BigInt(e.timestamp), off + 16); // last_updated_slot (unused)
    data.writeBigUInt64LE(BigInt(e.timestamp), off + 24);
  }
  return data;
};

const scopeGroup = Keypair.generate();
const scopeBank = Keypair.generate();
const feed = Keypair.generate().publicKey;

/** entry 42 = 103.445108, entry 7 = 1.25 — distinct so we can prove the index is honoured. */
const ENTRY_A = { index: 42, value: 10_344_510_800n, exp: 8n };
const ENTRY_B = { index: 7, value: 1_250_000_000n, exp: 9n };

let program: Program<Marginfi>;

describe("Scope oracle", () => {
  const setFeed = (pubkey: PublicKey, data: Buffer, owner = SCOPE_PROGRAM) =>
    bankrunContext.setAccount(pubkey, {
      executable: false,
      owner,
      lamports: 1_000_000_000,
      data,
      rentEpoch: 0,
    });

  const freshFeed = async (extra: Partial<Entry>[] = []) => {
    const now = await getBankrunTime(bankrunContext);
    return makePrices([
      { ...ENTRY_A, timestamp: now },
      { ...ENTRY_B, timestamp: now },
      ...(extra as Entry[]),
    ]);
  };

  const pulse = async (bank: PublicKey) => {
    const admin = groupAdmin.mrgnProgram;
    await admin.provider.sendAndConfirm!(
      new Transaction().add(
        await pulseBankPrice(admin, { bank, remaining: [feed] })
      )
    );
    return program.account.bank.fetch(bank);
  };

  before(async () => {
    program = bankrunProgram;
    const admin = groupAdmin.mrgnProgram;

    setFeed(feed, await freshFeed());

    await admin.provider.sendAndConfirm!(
      new Transaction().add(
        await groupInitialize(admin, {
          marginfiGroup: scopeGroup.publicKey,
          admin: groupAdmin.wallet.publicKey,
        })
      ),
      [scopeGroup]
    );
    await admin.provider.sendAndConfirm!(
      new Transaction().add(
        await addBank(admin, {
          marginfiGroup: scopeGroup.publicKey,
          feePayer: groupAdmin.wallet.publicKey,
          bankMint: ecosystem.wsolMint.publicKey,
          bank: scopeBank.publicKey,
          config: defaultBankConfig(),
        })
      ),
      [scopeBank]
    );
  });

  it("configures a bank against a scope entry and prices from it", async () => {
    const admin = groupAdmin.mrgnProgram;
    await admin.provider.sendAndConfirm!(
      new Transaction().add(
        await configureBankOracleScope(admin, {
          bank: scopeBank.publicKey,
          oracle: feed,
          entryIndex: ENTRY_A.index,
        })
      )
    );

    const bank = await program.account.bank.fetch(scopeBank.publicKey);
    assert.deepEqual(bank.config.oracleSetup, { scope: {} });
    assert.equal(bank.config.scopeEntryIndex, ENTRY_A.index);
    assert.equal(bank.config.oracleKeys[0].toString(), feed.toString());

    const pulsed = await pulse(scopeBank.publicKey);
    assertI80F48Approx(pulsed.cache.lastOraclePrice, 103.445108, 0.000001);
  });

  it("reads the configured entry, not a neighbouring one", async () => {
    const admin = groupAdmin.mrgnProgram;
    await admin.provider.sendAndConfirm!(
      new Transaction().add(
        await configureBankOracleScope(admin, {
          bank: scopeBank.publicKey,
          oracle: feed,
          entryIndex: ENTRY_B.index,
        })
      )
    );
    const pulsed = await pulse(scopeBank.publicKey);
    assertI80F48Approx(pulsed.cache.lastOraclePrice, 1.25, 0.000001);
    assert.notEqual(
      wrappedI80F48toBigNumber(pulsed.cache.lastOraclePrice).toNumber(),
      103.445108
    );

    // restore
    await admin.provider.sendAndConfirm!(
      new Transaction().add(
        await configureBankOracleScope(admin, {
          bank: scopeBank.publicKey,
          oracle: feed,
          entryIndex: ENTRY_A.index,
        })
      )
    );
  });

  it("rejects a feed owned by another program", async () => {
    setFeed(feed, await freshFeed(), Keypair.generate().publicKey);
    await expectFailedTxWithError(
      async () => {
        await pulse(scopeBank.publicKey);
      },
      "ScopeInvalidAccount",
      6700
    );
    setFeed(feed, await freshFeed());
  });

  it("rejects a corrupted discriminator", async () => {
    const data = await freshFeed();
    data[0] ^= 0xff;
    setFeed(feed, data);
    await expectFailedTxWithError(
      async () => {
        await pulse(scopeBank.publicKey);
      },
      "ScopeInvalidAccount",
      6700
    );
    setFeed(feed, await freshFeed());
  });

  it("rejects a feed of the wrong size", async () => {
    setFeed(feed, (await freshFeed()).subarray(0, ORACLE_PRICES_SIZE - 1));
    await expectFailedTxWithError(
      async () => {
        await pulse(scopeBank.publicKey);
      },
      "ScopeInvalidAccount",
      6700
    );
    setFeed(feed, await freshFeed());
  });

  it("rejects an entry that has never been refreshed", async () => {
    const admin = groupAdmin.mrgnProgram;
    // entry 300 is all zeroes in every fixture above
    await admin.provider.sendAndConfirm!(
      new Transaction().add(
        await configureBankOracleScope(admin, {
          bank: scopeBank.publicKey,
          oracle: feed,
          entryIndex: 300,
        })
      )
    ).then(
      () => assert.fail("expected configure to reject an unrefreshed entry"),
      () => {}
    );
  });

  it("rejects a stale price", async () => {
    const now = await getBankrunTime(bankrunContext);
    const maxAge = defaultBankConfig().oracleMaxAge;
    setFeed(feed, makePrices([{ ...ENTRY_A, timestamp: now - (maxAge + 60) }]));
    await expectFailedTxWithError(
      async () => {
        await pulse(scopeBank.publicKey);
      },
      "ScopeStalePrice",
      6702
    );
    setFeed(feed, await freshFeed());
  });

  it("rejects an exponent past the power-of-ten table", async () => {
    const now = await getBankrunTime(bankrunContext);
    setFeed(feed, makePrices([{ ...ENTRY_A, exp: 24n, timestamp: now }]));
    await expectFailedTxWithError(
      async () => {
        await pulse(scopeBank.publicKey);
      },
      "ScopeInvalidEntry",
      6701
    );
    setFeed(feed, await freshFeed());
  });

  it("rejects a different account than the configured feed", async () => {
    const admin = groupAdmin.mrgnProgram;
    const impostor = Keypair.generate().publicKey;
    setFeed(impostor, await freshFeed());
    await expectFailedTxWithError(
      async () => {
        await admin.provider.sendAndConfirm!(
          new Transaction().add(
            await pulseBankPrice(admin, {
              bank: scopeBank.publicKey,
              remaining: [impostor],
            })
          )
        );
      },
      "WrongOracleAccountKeys",
      6052
    );
  });

  it("re-prices when the feed is cranked with a new value", async () => {
    const now = await getBankrunTime(bankrunContext);
    setFeed(
      feed,
      makePrices([{ ...ENTRY_A, value: 20_000_000_000n, timestamp: now }])
    );
    const pulsed = await pulse(scopeBank.publicKey);
    assertI80F48Approx(pulsed.cache.lastOraclePrice, 200.0, 0.000001);
    setFeed(feed, await freshFeed());
  });

  it("accepts exp 0 (integer-valued entry)", async () => {
    const now = await getBankrunTime(bankrunContext);
    setFeed(
      feed,
      makePrices([{ ...ENTRY_A, value: 250n, exp: 0n, timestamp: now }])
    );
    const pulsed = await pulse(scopeBank.publicKey);
    assertI80F48Approx(pulsed.cache.lastOraclePrice, 250.0, 0.000001);
    setFeed(feed, await freshFeed());
  });

  it("accepts exp 23, the last slot of the power-of-ten table", async () => {
    const now = await getBankrunTime(bankrunContext);
    setFeed(
      feed,
      makePrices([
        {
          ...ENTRY_A,
          value: 12_345_678_900_000_000_000n,
          exp: 23n,
          timestamp: now,
        },
      ])
    );
    const pulsed = await pulse(scopeBank.publicKey);
    assertI80F48Approx(pulsed.cache.lastOraclePrice, 1.23456789e-4, 1e-12);
    setFeed(feed, await freshFeed());
  });

  it("accepts a price exactly at max_age and rejects one second past it", async () => {
    const maxAge = defaultBankConfig().oracleMaxAge;
    let now = await getBankrunTime(bankrunContext);
    setFeed(feed, makePrices([{ ...ENTRY_A, timestamp: now - maxAge }]));
    const pulsed = await pulse(scopeBank.publicKey);
    assertI80F48Approx(pulsed.cache.lastOraclePrice, 103.445108, 0.000001);

    now = await getBankrunTime(bankrunContext);
    setFeed(feed, makePrices([{ ...ENTRY_A, timestamp: now - (maxAge + 1) }]));
    await expectFailedTxWithError(
      async () => {
        await pulse(scopeBank.publicKey);
      },
      "ScopeStalePrice",
      6702
    );
    setFeed(feed, await freshFeed());
  });

  it("rejects a zero value carrying a fresh timestamp", async () => {
    const now = await getBankrunTime(bankrunContext);
    setFeed(feed, makePrices([{ ...ENTRY_A, value: 0n, timestamp: now }]));
    await expectFailedTxWithError(
      async () => {
        await pulse(scopeBank.publicKey);
      },
      "ScopeInvalidEntry",
      6701
    );
    setFeed(feed, await freshFeed());
  });

  it("rejects a non-zero value with a zero timestamp", async () => {
    setFeed(feed, makePrices([{ ...ENTRY_A, timestamp: 0 }]));
    await expectFailedTxWithError(
      async () => {
        await pulse(scopeBank.publicKey);
      },
      "ScopeInvalidEntry",
      6701
    );
    setFeed(feed, await freshFeed());
  });

  it("rejects an entry index past the end of the price array", async () => {
    const admin = groupAdmin.mrgnProgram;
    await expectFailedTxWithError(
      async () => {
        await admin.provider.sendAndConfirm!(
          new Transaction().add(
            await configureBankOracleScope(admin, {
              bank: scopeBank.publicKey,
              oracle: feed,
              entryIndex: 512,
            })
          )
        );
      },
      "ScopeInvalidEntry",
      6701
    );
  });

  it("feeds the scope price into the health engine", async () => {
    const user = users[0];
    const acc = Keypair.generate();

    // Self-contained funding: this spec must pass when run on its own, so it does not rely
    // on 07_deposit having minted to the users first.
    await bankRunProvider.sendAndConfirm!(
      new Transaction().add(
        createMintToInstruction(
          ecosystem.wsolMint.publicKey,
          user.wsolAccount,
          bankrunContext.payer.publicKey,
          10 * 10 ** ecosystem.wsolDecimals
        )
      )
    );

    // The group admin funds the rent; the user stays the account authority.
    await groupAdmin.mrgnProgram.provider.sendAndConfirm!(
      new Transaction().add(
        await accountInit(groupAdmin.mrgnProgram, {
          marginfiGroup: scopeGroup.publicKey,
          marginfiAccount: acc.publicKey,
          authority: user.wallet.publicKey,
          feePayer: groupAdmin.wallet.publicKey,
        })
      ),
      [acc, user.wallet]
    );

    const deposit = new BN(2 * 10 ** ecosystem.wsolDecimals);
    await user.mrgnProgram.provider.sendAndConfirm!(
      new Transaction().add(
        await depositIx(user.mrgnProgram, {
          marginfiAccount: acc.publicKey,
          bank: scopeBank.publicKey,
          tokenAccount: user.wsolAccount,
          amount: deposit,
        })
      )
    );

    await user.mrgnProgram.provider.sendAndConfirm!(
      new Transaction().add(
        await healthPulse(user.mrgnProgram, {
          marginfiAccount: acc.publicKey,
          remaining: composeRemainingAccounts([[scopeBank.publicKey, feed]]),
        })
      )
    );

    const after = await program.account.marginfiAccount.fetch(acc.publicKey);
    const cache = after.healthCache;

    // The engine read the scope entry, not a fallback or a zero.
    assert.approximately(bytesToF64(cache.prices[0]), 103.445108, 0.00001);
    assert.approximately(
      wrappedI80F48toBigNumber(cache.assetValueEquity).toNumber(),
      2 * 103.445108,
      0.0001
    );
    assert.equal(wrappedI80F48toBigNumber(cache.liabilityValue).toNumber(), 0);
    assert.equal(cache.mrgnErr, 0);
    assert.equal(cache.internalErr, 0);
    assert.isAbove(cache.flags & HEALTH_CACHE_ORACLE_OK, 0);
    assert.isAbove(cache.flags & HEALTH_CACHE_HEALTHY, 0);
  });
});
