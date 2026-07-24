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
  ORACLE_SETUP_PTSOL,
  ORACLE_SETUP_PYTH_PUSH,
} from "../../utils/types";
import { refreshPullOraclesBankrun } from "../../utils/bankrun-oracles";
import { getBankrunTime } from "../../utils/tools";
import { wrappedI80F48toBigNumber } from "@mrgnlabs/mrgn-common";
import { assert } from "chai";

const EXPONENT_PROGRAM = new PublicKey(
  "ExponentnaRg3CQbW6dqQNZKXp7gtZ9DGMp1cwC4HAS7",
);
// Real mainnet Exponent vault (already past maturity -> price pinned at par)
const REAL_VAULT = new PublicKey("9YbaicMsXrtupkpD72pdWBfU6R7EJfSByw75sEpDM1uH");
const VAULT_DISCRIMINATOR = Buffer.from([211, 8, 232, 43, 2, 152, 117, 119]);

// Minimal Exponent vault bytes: discriminator + start_ts @264 + duration @268
const makeVault = (startTs: number, duration: number) => {
  const data = Buffer.alloc(272);
  VAULT_DISCRIMINATOR.copy(data, 0);
  data.writeUInt32LE(startTs, 264);
  data.writeUInt32LE(duration, 268);
  return data;
};

const ptGroup = Keypair.generate();
const ptBank = Keypair.generate();

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
  });

  const setVault = (pubkey: PublicKey, data: Buffer) =>
    bankrunContext.setAccount(pubkey, {
      lamports: 1_000_000_000,
      data,
      owner: EXPONENT_PROGRAM,
      executable: false,
      rentEpoch: 0,
    });

  const configurePtsol = async () => {
    const ix = await configureBankOracle(groupAdmin.mrgnProgram, {
      bank: ptBank.publicKey,
      type: ORACLE_SETUP_PTSOL,
      oracle: oracles.wsolOracle.publicKey,
    });
    return groupAdmin.mrgnProgram.provider.sendAndConfirm!(
      new Transaction().add(ix),
    );
  };

  const setStartPrice = async (price: number, vault: PublicKey) => {
    const ix = await setFixedPrice(groupAdmin.mrgnProgram, {
      bank: ptBank.publicKey,
      price,
      remaining: [vault],
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

  it("(admin) configures PTSOL - happy path", async () => {
    await configurePtsol();
    const { config } = await program.account.bank.fetch(ptBank.publicKey);
    assert.deepEqual(config.oracleSetup, { ptsol: {} });
    assertKeysEqual(config.oracleKeys[0], oracles.wsolOracle.publicKey);
  });

  it("(admin) sets the PT start price + vault - happy path", async () => {
    await setStartPrice(0.9, REAL_VAULT);
    const { config } = await program.account.bank.fetch(ptBank.publicKey);
    assertI80F48Approx(config.fixedPrice, 0.9);
    assertKeysEqual(config.oracleKeys[1], REAL_VAULT);
  });

  it("(admin) tries to set the PT price - fails with a non-vault account", async () => {
    await expectFailedTxWithError(
      async () => {
        await setStartPrice(0.9, Keypair.generate().publicKey);
      },
      "ExponentVaultValidationFailed",
      6601,
    );
  });

  it("(admin) tries to set the PT price - fails with a start price above par", async () => {
    await expectFailedTxWithError(
      async () => {
        await setStartPrice(1.5, REAL_VAULT);
      },
      "InvalidPtStartPrice",
      6602,
    );
  });

  it("prices a matured vault at par (SOL/USD)", async () => {
    const now = await getBankrunTime(bankrunContext);
    const vault = Keypair.generate().publicKey;
    setVault(vault, makeVault(now - 10_000, 5_000)); // maturity = now - 5000, already past
    await setStartPrice(0.9, vault);

    const cache = await pulseCache([vault]);
    // At/after maturity -> multiplier is exactly 1, so PT price == SOL price.
    assertI80F48Approx(cache.lastOraclePrice, baseSolPrice);
    assertI80F48Approx(cache.priceMultiplier, 1);
  });

  it("prices a not-yet-started vault at the start price", async () => {
    const now = await getBankrunTime(bankrunContext);
    const vault = Keypair.generate().publicKey;
    setVault(vault, makeVault(now + 100_000, 1_000));
    await setStartPrice(0.85, vault);

    const cache = await pulseCache([vault]);
    // now < start_ts -> multiplier clamps to start_price 0.85
    assertI80F48Approx(cache.lastOraclePrice, baseSolPrice * 0.85);
  });

  it("prices a mid-life vault along the linear curve", async () => {
    const now = await getBankrunTime(bankrunContext);
    const startTs = now - 5_000;
    const duration = 10_000;
    const startPrice = 0.8;
    const vault = Keypair.generate().publicKey;
    setVault(vault, makeVault(startTs, duration));
    await setStartPrice(startPrice, vault);

    const cache = await pulseCache([vault]);
    const at = await getBankrunTime(bankrunContext);
    const progress = (at - startTs) / duration;
    const expectedMult = startPrice + (1 - startPrice) * progress;
    assertI80F48Approx(cache.lastOraclePrice, baseSolPrice * expectedMult);
  });
});
