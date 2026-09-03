import { Program } from "@coral-xyz/anchor";
import { Keypair, PublicKey, Transaction } from "@solana/web3.js";
import { Marginfi } from "../../../target/types/marginfi";
import {
  addBank,
  configureBankOracle,
  groupInitialize,
  setFixedPrice,
} from "../../utils/group-instructions";
import { pulseBankPrice } from "../../utils/user-instructions";
import {
  bankrunContext,
  bankrunProgram,
  banksClient,
  ecosystem,
  groupAdmin,
  oracles,
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
import { getBankrunTime } from "../../utils/tools";
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

// Minimal Exponent vault bytes, at absolute offsets 104 / 264 / 268 / 337 / 441 / 449. Defaults
// describe a fully-backed vault (sy_for_pt = pt_supply / sy_rate), whose redemption rate is 1.0
// and so never caps the linear price.
const makeVault = (
  startTs: number,
  duration: number,
  opts: {
    mintPt?: PublicKey;
    syRate?: bigint;
    syForPt?: bigint;
    ptSupply?: bigint;
  } = {},
) => {
  const syRate = opts.syRate ?? 2n * SY_RATE_PRECISION;
  const ptSupply = opts.ptSupply ?? 1_000_000_000_000n;
  const syForPt = opts.syForPt ?? (ptSupply * SY_RATE_PRECISION) / syRate;

  const data = Buffer.alloc(457);
  VAULT_DISCRIMINATOR.copy(data, 0);
  (opts.mintPt ?? ecosystem.wsolMint.publicKey).toBuffer().copy(data, 104);
  data.writeUInt32LE(startTs, 264);
  data.writeUInt32LE(duration, 268);
  data.writeBigUInt64LE(syRate, 337);
  data.writeBigUInt64LE(syForPt, 441);
  data.writeBigUInt64LE(ptSupply, 449);
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
