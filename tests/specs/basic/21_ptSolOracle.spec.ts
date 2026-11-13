import { Program } from "@coral-xyz/anchor";
import { Keypair, PublicKey, Transaction } from "@solana/web3.js";
import { Marginfi } from "../../../target/types/marginfi";
import {
  addBank,
  configureBankOracle,
  groupInitialize,
  setFixedPrice,
} from "../../utils/group-instructions";
import {
  accountInit,
  borrowIx,
  composeRemainingAccounts,
  depositIx,
  pulseBankPrice,
} from "../../utils/user-instructions";
import {
  bankrunContext,
  bankrunProgram,
  banksClient,
  ecosystem,
  globalProgramAdmin,
  groupAdmin,
  oracles,
  users,
} from "../../rootHooks";
import {
  assertI80F48Approx,
  assertKeysEqual,
  expectFailedTxWithError,
} from "../../utils/genericTests";
import {
  defaultBankConfig,
  ORACLE_SETUP_PT_FIXED,
  ORACLE_SETUP_PT_PYTH,
  ORACLE_SETUP_PYTH_PUSH,
} from "../../utils/types";
import { refreshPullOraclesBankrun } from "../../utils/bankrun-oracles";
import { getBankrunTime, processBankrunTransaction } from "../../utils/tools";
import { BN } from "@coral-xyz/anchor";
import { createMintToInstruction } from "@solana/spl-token";
import { wrappedI80F48toBigNumber } from "@mrgnlabs/mrgn-common";
import { assert } from "chai";

const EXPONENT_PROGRAM = new PublicKey(
  "ExponentnaRg3CQbW6dqQNZKXp7gtZ9DGMp1cwC4HAS7",
);
// Real mainnet Exponent vault. Its `mint_pt` is a real PT mint, so it is only usable where a mint
// mismatch is the expected outcome.
const REAL_VAULT = new PublicKey(
  "9YbaicMsXrtupkpD72pdWBfU6R7EJfSByw75sEpDM1uH",
);
const VAULT_DISCRIMINATOR = Buffer.from([211, 8, 232, 43, 2, 152, 117, 119]);

/** Precision of Exponent's `Number` type. */
const SY_RATE_PRECISION = 1_000_000_000_000n;
/** Exponent `Vault.status` with every operation enabled; bit 1 is `STATUS_CAN_MERGE`. */
const STATUS_ALL = 0b11_1111;
const STATUS_CAN_MERGE = 0b10;

// Minimal Exponent vault bytes, at absolute offsets 104 / 264 / 268 / 337 / 369 / 441 / 449 / 523.
// Defaults describe a fully-backed vault (sy_for_pt = pt_supply / max(watermark, sy_rate), as
// Exponent sets it), whose redemption rate is 1.0 unless the rate sits below the watermark.
const makeVault = (
  startTs: number,
  duration: number,
  opts: {
    mintPt?: PublicKey;
    syRate?: bigint;
    allTimeHigh?: bigint;
    syForPt?: bigint;
    ptSupply?: bigint;
    status?: number;
  } = {},
) => {
  const syRate = opts.syRate ?? 2n * SY_RATE_PRECISION;
  const allTimeHigh = opts.allTimeHigh ?? syRate;
  const ptSupply = opts.ptSupply ?? 1_000_000_000_000n;
  const watermark = allTimeHigh > syRate ? allTimeHigh : syRate;
  const syForPt = opts.syForPt ?? (ptSupply * SY_RATE_PRECISION) / watermark;

  const data = Buffer.alloc(524);
  VAULT_DISCRIMINATOR.copy(data, 0);
  (opts.mintPt ?? ecosystem.wsolMint.publicKey).toBuffer().copy(data, 104);
  data.writeUInt32LE(startTs, 264);
  data.writeUInt32LE(duration, 268);
  data.writeBigUInt64LE(syRate, 337);
  data.writeBigUInt64LE(allTimeHigh, 369);
  data.writeBigUInt64LE(syForPt, 441);
  data.writeBigUInt64LE(ptSupply, 449);
  data.writeUInt8(opts.status ?? STATUS_ALL, 523);
  return data;
};

const ptGroup = Keypair.generate();
const ptBank = Keypair.generate();
/** Synthetic vault carrying the bank's own PT mint, used for the setup happy paths. */
const okVault = Keypair.generate().publicKey;

let program: Program<Marginfi>;
/** SOL/USD as the program reports it, captured from a plain Pyth pulse. */
let baseSolPrice: number;

describe("PT-SOL internal oracle setup", () => {
  before(async () => {
    program = bankrunProgram;
    const admin = groupAdmin.mrgnProgram;

    await admin.provider.sendAndConfirm!(
      new Transaction().add(
        await groupInitialize(admin, {
          marginfiGroup: ptGroup.publicKey,
          admin: groupAdmin.wallet.publicKey,
        }),
      ),
      [ptGroup],
    );
    await admin.provider.sendAndConfirm!(
      new Transaction().add(
        await addBank(admin, {
          marginfiGroup: ptGroup.publicKey,
          feePayer: groupAdmin.wallet.publicKey,
          bankMint: ecosystem.wsolMint.publicKey,
          bank: ptBank.publicKey,
          config: defaultBankConfig(),
        }),
      ),
      [ptBank],
    );
    await admin.provider.sendAndConfirm!(
      new Transaction().add(
        await configureBankOracle(admin, {
          bank: ptBank.publicKey,
          type: ORACLE_SETUP_PYTH_PUSH,
          oracle: oracles.wsolOracle.publicKey,
        }),
      ),
    );

    const base = await pulseCache([]);
    baseSolPrice = wrappedI80F48toBigNumber(base.lastOraclePrice).toNumber();

    // Matured, fully backed, carrying the bank's mint.
    const now = await getBankrunTime(bankrunContext);
    setVault(okVault, makeVault(now - 10_000, 5_000));
  });

  const setVault = (pubkey: PublicKey, data: Buffer) =>
    bankrunContext.setAccount(pubkey, {
      lamports: 1_000_000_000,
      data,
      owner: EXPONENT_PROGRAM,
      executable: false,
      rentEpoch: 0,
    });

  // Self-contained PT-SOL setup: [Pyth SOL/USD, Exponent vault] + start price.
  const setPtsol = async (price: number, vault: PublicKey) => {
    const ix = await setFixedPrice(groupAdmin.mrgnProgram, {
      bank: ptBank.publicKey,
      price,
      setup: ORACLE_SETUP_PT_PYTH,
      remaining: [oracles.wsolOracle.publicKey, vault],
    });
    return groupAdmin.mrgnProgram.provider.sendAndConfirm!(
      new Transaction().add(ix),
    );
  };

  const pulseCache = async (multiplierAccounts: PublicKey[]) => {
    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);
    await groupAdmin.mrgnProgram.provider.sendAndConfirm!(
      new Transaction().add(
        await pulseBankPrice(groupAdmin.mrgnProgram, {
          bank: ptBank.publicKey,
          remaining: [oracles.wsolOracle.publicKey, ...multiplierAccounts],
        }),
      ),
    );
    return (await program.account.bank.fetch(ptBank.publicKey)).cache;
  };

  it("(admin) sets up PTPyth (setup + start price + vault) - happy path", async () => {
    await setPtsol(0.9, okVault);
    const { config } = await program.account.bank.fetch(ptBank.publicKey);
    assert.deepEqual(config.oracleSetup, { ptPyth: {} });
    assertKeysEqual(config.oracleKeys[0], oracles.wsolOracle.publicKey);
    assertKeysEqual(config.oracleKeys[1], okVault);
    assertI80F48Approx(config.fixedPrice, 0.9);
  });

  it("(admin) tries to set up PTPyth - fails with a non-vault account", async () => {
    await expectFailedTxWithError(
      async () => {
        await setPtsol(0.9, Keypair.generate().publicKey);
      },
      "ExponentVaultValidationFailed",
      6137,
    );
  });

  it("(admin) tries to set up PTPyth - fails with a start price above par", async () => {
    await expectFailedTxWithError(
      async () => {
        await setPtsol(1.5, okVault);
      },
      "InvalidPtStartPrice",
      6138,
    );
  });

  it("(admin) tries to set up PTPyth - fails when the vault's PT mint is not the bank mint", async () => {
    // A genuine mainnet vault, so it clears the owner/discriminator checks and is rejected purely
    // on the mint.
    await expectFailedTxWithError(
      async () => {
        await setPtsol(0.9, REAL_VAULT);
      },
      "ExponentVaultValidationFailed",
      6137,
    );
  });

  it("prices a matured vault at par (SOL/USD)", async () => {
    const now = await getBankrunTime(bankrunContext);
    const vault = Keypair.generate().publicKey;
    setVault(vault, makeVault(now - 10_000, 5_000)); // maturity = now - 5000, already past
    await setPtsol(0.9, vault);

    const cache = await pulseCache([vault]);
    // At/after maturity -> multiplier is exactly 1, so PT price == SOL price.
    assertI80F48Approx(cache.lastOraclePrice, baseSolPrice);
    assertI80F48Approx(cache.priceMultiplier, 1);
  });

  it("prices a not-yet-started vault at the start price", async () => {
    const now = await getBankrunTime(bankrunContext);
    const vault = Keypair.generate().publicKey;
    setVault(vault, makeVault(now + 100_000, 1_000));
    await setPtsol(0.85, vault);

    const cache = await pulseCache([vault]);
    // now < start_ts -> multiplier clamps to start_price 0.85
    assertI80F48Approx(cache.lastOraclePrice, baseSolPrice * 0.85);
  });

  it("caps an under-backed matured vault at its redemption rate", async () => {
    const now = await getBankrunTime(bankrunContext);
    const vault = Keypair.generate().publicKey;
    // Escrow covers only 45% of the SY needed for PT supply => 0.9 asset per PT, so the cap wins
    // over the linear model's par at maturity.
    setVault(
      vault,
      makeVault(now - 10_000, 5_000, {
        syRate: 2n * SY_RATE_PRECISION,
        ptSupply: 1_000_000_000_000n,
        syForPt: 450_000_000_000n,
      }),
    );
    await setPtsol(0.9, vault);

    const cache = await pulseCache([vault]);
    assertI80F48Approx(cache.lastOraclePrice, baseSolPrice * 0.9);
    assertI80F48Approx(cache.priceMultiplier, 1);
  });

  it("prices a mid-life vault along the linear curve", async () => {
    const now = await getBankrunTime(bankrunContext);
    const startTs = now - 5_000;
    const duration = 10_000;
    const startPrice = 0.8;
    const vault = Keypair.generate().publicKey;
    setVault(vault, makeVault(startTs, duration));
    await setPtsol(startPrice, vault);

    const cache = await pulseCache([vault]);
    const at = await getBankrunTime(bankrunContext);
    const progress = (at - startTs) / duration;
    const expectedMult = startPrice + (1 - startPrice) * progress;
    assertI80F48Approx(cache.lastOraclePrice, baseSolPrice * expectedMult);
  });

  it("prices a vault below its watermark at what its PT redeems for (negative yield)", async () => {
    const now = await getBankrunTime(bankrunContext);
    const vault = Keypair.generate().publicKey;
    // Live rate 2.0 under a 2.5 watermark: Exponent sets aside pt_supply / 2.5 SY, so a matured PT
    // redeems for 2.0 / 2.5 = 0.8 SOL.
    setVault(
      vault,
      makeVault(now - 10_000, 5_000, {
        syRate: 2n * SY_RATE_PRECISION,
        allTimeHigh: (5n * SY_RATE_PRECISION) / 2n,
      }),
    );
    await setPtsol(0.9, vault);

    const cache = await pulseCache([vault]);
    assertI80F48Approx(cache.lastOraclePrice, baseSolPrice * 0.8);
  });

  it("PT backs no new borrows while Exponent has merging paused", async () => {
    const borrower = users[0];
    const lender = users[1];
    const borrowerAcc = Keypair.generate();
    const lenderAcc = Keypair.generate();
    const debtBank = Keypair.generate();
    const vault = Keypair.generate().publicKey;
    const wsol = (n: number) => new BN(n * 10 ** ecosystem.wsolDecimals);
    const usdc = (n: number) => new BN(n * 10 ** ecosystem.usdcDecimals);

    const now = await getBankrunTime(bankrunContext);
    const setMergeable = (mergeable: boolean) =>
      setVault(
        vault,
        makeVault(now - 10_000, 5_000, {
          status: mergeable ? STATUS_ALL : STATUS_ALL & ~STATUS_CAN_MERGE,
        }),
      );
    setMergeable(false);
    await setPtsol(0.9, vault);

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await addBank(groupAdmin.mrgnBankrunProgram, {
          marginfiGroup: ptGroup.publicKey,
          feePayer: groupAdmin.wallet.publicKey,
          bankMint: ecosystem.usdcMint.publicKey,
          bank: debtBank.publicKey,
          config: defaultBankConfig(),
        }),
      ),
      [groupAdmin.wallet, debtBank],
    );
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await configureBankOracle(groupAdmin.mrgnBankrunProgram, {
          bank: debtBank.publicKey,
          type: ORACLE_SETUP_PYTH_PUSH,
          oracle: oracles.usdcOracle.publicKey,
        }),
      ),
      [groupAdmin.wallet],
    );
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        createMintToInstruction(
          ecosystem.wsolMint.publicKey,
          borrower.wsolAccount,
          globalProgramAdmin.wallet.publicKey,
          2n * 10n ** BigInt(ecosystem.wsolDecimals),
        ),
        createMintToInstruction(
          ecosystem.usdcMint.publicKey,
          lender.usdcAccount,
          globalProgramAdmin.wallet.publicKey,
          1_000n * 10n ** BigInt(ecosystem.usdcDecimals),
        ),
      ),
      [globalProgramAdmin.wallet],
    );
    for (const [user, acc, bank, tokenAccount, amount] of [
      [lender, lenderAcc, debtBank.publicKey, lender.usdcAccount, usdc(1_000)],
      [borrower, borrowerAcc, ptBank.publicKey, borrower.wsolAccount, wsol(2)],
    ] as const) {
      await processBankrunTransaction(
        bankrunContext,
        new Transaction().add(
          await accountInit(user.mrgnBankrunProgram, {
            marginfiGroup: ptGroup.publicKey,
            marginfiAccount: acc.publicKey,
            authority: user.wallet.publicKey,
            feePayer: user.wallet.publicKey,
          }),
        ),
        [user.wallet, acc],
      );
      await processBankrunTransaction(
        bankrunContext,
        new Transaction().add(
          await depositIx(user.mrgnBankrunProgram, {
            marginfiAccount: acc.publicKey,
            bank,
            tokenAccount,
            amount,
          }),
        ),
        [user.wallet],
      );
    }

    const borrow = async () =>
      processBankrunTransaction(
        bankrunContext,
        new Transaction().add(
          await borrowIx(borrower.mrgnBankrunProgram, {
            marginfiAccount: borrowerAcc.publicKey,
            bank: debtBank.publicKey,
            tokenAccount: borrower.usdcAccount,
            remaining: composeRemainingAccounts([
              [ptBank.publicKey, oracles.wsolOracle.publicKey, vault],
              [debtBank.publicKey, oracles.usdcOracle.publicKey],
            ]),
            amount: usdc(50),
          }),
        ),
        [borrower.wallet],
      );

    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);
    await expectFailedTxWithError(
      async () => {
        await borrow();
      },
      "RiskEngineInitRejected",
      6009,
    );

    setMergeable(true);
    await borrow();
  });

  // --- PT-hyUSD: same Exponent lerp, but no base feed (hyUSD ~= $1, so the rate is the price) ---
  const setPthyusd = async (price: number, vault: PublicKey) => {
    const ix = await setFixedPrice(groupAdmin.mrgnProgram, {
      bank: ptBank.publicKey,
      price,
      setup: ORACLE_SETUP_PT_FIXED,
      remaining: [vault], // vault only -> PT-Fixed ($1 base)
    });
    return groupAdmin.mrgnProgram.provider.sendAndConfirm!(
      new Transaction().add(ix),
    );
  };

  const pulseHyusd = async (vault: PublicKey) => {
    await groupAdmin.mrgnProgram.provider.sendAndConfirm!(
      new Transaction().add(
        await pulseBankPrice(groupAdmin.mrgnProgram, {
          bank: ptBank.publicKey,
          remaining: [vault],
        }),
      ),
    );
    return (await program.account.bank.fetch(ptBank.publicKey)).cache;
  };

  it("(admin) sets up PTFixed (vault only) - happy path", async () => {
    await setPthyusd(0.95, okVault);
    const { config } = await program.account.bank.fetch(ptBank.publicKey);
    assert.deepEqual(config.oracleSetup, { ptFixed: {} });
    assertKeysEqual(config.oracleKeys[0], okVault);
    assertI80F48Approx(config.fixedPrice, 0.95);
  });

  it("(admin) tries to set up PTFixed - fails with a start price above par", async () => {
    await expectFailedTxWithError(
      async () => {
        await setPthyusd(1.5, okVault);
      },
      "InvalidPtStartPrice",
      6138,
    );
  });

  it("prices a matured PT-hyUSD vault at par ($1)", async () => {
    const now = await getBankrunTime(bankrunContext);
    const vault = Keypair.generate().publicKey;
    setVault(vault, makeVault(now - 10_000, 5_000));
    await setPthyusd(0.9, vault);
    // The lerp value is the USD price directly; past maturity -> par.
    assertI80F48Approx((await pulseHyusd(vault)).lastOraclePrice, 1.0);
  });

  it("prices a not-yet-started PT-hyUSD vault at the start price", async () => {
    const now = await getBankrunTime(bankrunContext);
    const vault = Keypair.generate().publicKey;
    setVault(vault, makeVault(now + 100_000, 1_000));
    await setPthyusd(0.85, vault);
    assertI80F48Approx((await pulseHyusd(vault)).lastOraclePrice, 0.85);
  });
});
