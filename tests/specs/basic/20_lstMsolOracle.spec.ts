import { BN, Program } from "@coral-xyz/anchor";
import {
  Keypair,
  LAMPORTS_PER_SOL,
  PublicKey,
  SystemProgram,
  Transaction,
} from "@solana/web3.js";
import { Marginfi } from "../../../target/types/marginfi";
import {
  addBank,
  configureBankOracle,
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
  banksClient,
  ecosystem,
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
  ORACLE_SETUP_PYTH_LST,
  ORACLE_SETUP_PYTH_MSOL,
  ORACLE_SETUP_PYTH_PUSH,
} from "../../utils/types";
import { refreshPullOraclesBankrun } from "../../utils/bankrun-oracles";
import {
  divI80,
  mintToTokenAccount,
  mulI80,
  toI80Scaled,
} from "../../utils/tools";
import { wrappedI80F48toBigNumber } from "@mrgnlabs/mrgn-common";
import { assert } from "chai";

// Mainnet fixtures, one per stake-pool owner program (see tests/fixtures)
const MSOL_STATE = new PublicKey(
  "8szGkuLTAux9XMgZ2vtY39jVSowEcpBfFfD8hXSEqdGC",
);
const SANCTUM_SPL_POOL = new PublicKey(
  "9mhGNSPArRMHpLDMSmxAvuoizBqtBGqYdT8WGuqgxNdn",
);
const JUPSOL_POOL = new PublicKey(
  "8VpRhuxa7sUUepdY3kQiTmX9rS5vx4WgaXiAnXq4KCtr",
);

// Mirrors `MinimalStakePool` in price.rs
const decodeStakePool = (data: Uint8Array) => {
  const buf = Buffer.from(data);
  return {
    totalLamports: buf.readBigUInt64LE(258),
    poolTokenSupply: buf.readBigUInt64LE(266),
  };
};

// Canonical SOL/mSOL rate from Marinade's balances, matching `marinade_price_multiplier`:
// total_virtual_staked_lamports / msol_supply. Byte offsets match `MinimalMarinadeState`.
const decodeMarinadeRate = (data: Uint8Array): number => {
  const b = Buffer.from(data);
  const u = (o: number) => b.readBigUInt64LE(o);
  const underControl = u(376) + u(226) + u(568) + u(496); // active + delayed + emergency + reserve
  const tvsl = underControl - u(528); // - circulating_ticket_balance
  return Number(tvsl) / Number(u(504)); // / msol_supply
};

const lstGroup = Keypair.generate();
const lstBank = Keypair.generate();

let program: Program<Marginfi>;
/** SOL/USD as the program reports it, captured from a plain Pyth pulse. */
let baseSolPrice: number;

describe("LST / mSOL internal oracle setups", () => {
  before(async () => {
    program = bankrunProgram;
    const admin = groupAdmin.mrgnProgram;

    await admin.provider.sendAndConfirm!(
      new Transaction().add(
        await groupInitialize(admin, {
          marginfiGroup: lstGroup.publicKey,
          admin: groupAdmin.wallet.publicKey,
        }),
      ),
      [lstGroup],
    );

    await admin.provider.sendAndConfirm!(
      new Transaction().add(
        await addBank(admin, {
          marginfiGroup: lstGroup.publicKey,
          feePayer: groupAdmin.wallet.publicKey,
          bankMint: ecosystem.wsolMint.publicKey,
          bank: lstBank.publicKey,
          config: defaultBankConfig(),
        }),
      ),
      [lstBank],
    );

    await admin.provider.sendAndConfirm!(
      new Transaction().add(
        await configureBankOracle(admin, {
          bank: lstBank.publicKey,
          type: ORACLE_SETUP_PYTH_PUSH,
          oracle: oracles.wsolOracle.publicKey,
        }),
      ),
    );

    // Config validation requires the pricing account's mint to equal the bank's mint. The bank
    // holds wsol while the fixtures mint their own LSTs, so point each fixture's embedded mint at
    // the bank's. `pool_mint` sits at byte 162 of an SPL StakePool; `msol_mint` is Marinade State's
    // first field (byte 8, past the disc).
    await patchMint(SANCTUM_SPL_POOL, 162);
    await patchMint(JUPSOL_POOL, 162);
    await patchMint(MSOL_STATE, 8);

    const base = await pulseCache([]);
    baseSolPrice = wrappedI80F48toBigNumber(base.lastOraclePrice).toNumber();
  });

  const patchData = async (
    account: PublicKey,
    patch: (data: Buffer) => void,
  ) => {
    const acc = await banksClient.getAccount(account);
    const data = Buffer.from(acc!.data);
    patch(data);
    bankrunContext.setAccount(account, {
      lamports: acc!.lamports,
      data,
      owner: acc!.owner,
      executable: acc!.executable,
      rentEpoch: acc!.rentEpoch,
    });
  };

  // Overwrite the mint pubkey embedded at `offset` in a pricing account with the bank's mint.
  const patchMint = (account: PublicKey, offset: number) =>
    patchData(account, (data) => {
      ecosystem.wsolMint.publicKey.toBuffer().copy(data, offset);
    });

  // Marinade State's `paused` flag sits at byte 608 (600 past the disc).
  const setMsolPaused = (paused: boolean) =>
    patchData(MSOL_STATE, (data) => {
      data[608] = Number(paused);
    });

  const stakePoolRate = async (pool: PublicKey) => {
    const acc = await banksClient.getAccount(pool);
    const { totalLamports, poolTokenSupply } = decodeStakePool(acc!.data);
    return Number(totalLamports) / Number(poolTokenSupply);
  };

  const marinadeRate = async (state: PublicKey) => {
    const acc = await banksClient.getAccount(state);
    return decodeMarinadeRate(acc!.data);
  };

  const setOracle = async (type: number, remaining: PublicKey[]) => {
    const ix = await configureBankOracle(groupAdmin.mrgnProgram, {
      bank: lstBank.publicKey,
      type,
      oracle: oracles.wsolOracle.publicKey,
      remaining,
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
          bank: lstBank.publicKey,
          remaining: [oracles.wsolOracle.publicKey, ...multiplierAccounts],
        }),
      ),
    );
    return (await program.account.bank.fetch(lstBank.publicKey)).cache;
  };

  it("(admin) configures PythLST - happy path", async () => {
    await setOracle(ORACLE_SETUP_PYTH_LST, [SANCTUM_SPL_POOL]);
    const { config } = await program.account.bank.fetch(lstBank.publicKey);
    assert.deepEqual(config.oracleSetup, { pythLst: {} });
    assertKeysEqual(config.oracleKeys[0], oracles.wsolOracle.publicKey);
    assertKeysEqual(config.oracleKeys[1], SANCTUM_SPL_POOL);
  });

  it("(admin) configures PythMSOL - happy path", async () => {
    await setOracle(ORACLE_SETUP_PYTH_MSOL, [MSOL_STATE]);
    const { config } = await program.account.bank.fetch(lstBank.publicKey);
    assert.deepEqual(config.oracleSetup, { pythMsol: {} });
    assertKeysEqual(config.oracleKeys[1], MSOL_STATE);
  });

  it("(admin) tries to configure PythLST - fails with wrong number of accounts", async () => {
    await expectFailedTxWithError(
      async () => {
        await setOracle(ORACLE_SETUP_PYTH_LST, []);
      },
      "WrongNumberOfOracleAccounts",
      6051,
    );
  });

  it("(admin) tries to configure PythLST - fails with a non-stake-pool account", async () => {
    await expectFailedTxWithError(
      async () => {
        await setOracle(ORACLE_SETUP_PYTH_LST, [Keypair.generate().publicKey]);
      },
      "StakePoolValidationFailed",
      6048,
    );
  });

  it("(admin) tries to configure PythMSOL - fails with a non-marinade account", async () => {
    await expectFailedTxWithError(
      async () => {
        await setOracle(ORACLE_SETUP_PYTH_MSOL, [SANCTUM_SPL_POOL]);
      },
      "MarinadeStateValidationFailed",
      6136,
    );
  });

  it("prices a Sanctum-SPL LST via PythLST", async () => {
    await setOracle(ORACLE_SETUP_PYTH_LST, [SANCTUM_SPL_POOL]);
    const rate = await stakePoolRate(SANCTUM_SPL_POOL);
    const cache = await pulseCache([SANCTUM_SPL_POOL]);
    // Rate is baked into the price; the wrapper multiplier stays 1.
    assertI80F48Approx(cache.lastOraclePrice, baseSolPrice * rate);
    assertI80F48Approx(cache.priceMultiplier, 1);
  });

  it("prices jupSOL (Sanctum-Multi owner) via PythLST", async () => {
    await setOracle(ORACLE_SETUP_PYTH_LST, [JUPSOL_POOL]);
    const rate = await stakePoolRate(JUPSOL_POOL);
    const cache = await pulseCache([JUPSOL_POOL]);
    assertI80F48Approx(cache.lastOraclePrice, baseSolPrice * rate);
  });

  it("prices mSOL via PythMSOL", async () => {
    await setOracle(ORACLE_SETUP_PYTH_MSOL, [MSOL_STATE]);
    const rate = await marinadeRate(MSOL_STATE);
    const cache = await pulseCache([MSOL_STATE]);
    assertI80F48Approx(cache.lastOraclePrice, baseSolPrice * rate);
    assertI80F48Approx(cache.priceMultiplier, 1);
  });

  it("(user 0) paused Marinade zeroes mSOL's Initial value only", async () => {
    await setOracle(ORACLE_SETUP_PYTH_MSOL, [MSOL_STATE]);
    const user = users[0];
    const userAccount = Keypair.generate();
    const amount = new BN(LAMPORTS_PER_SOL);
    await mintToTokenAccount(
      ecosystem.wsolMint.publicKey,
      user.wsolAccount,
      amount,
    );
    await user.mrgnProgram.provider.sendAndConfirm!(
      new Transaction().add(
        await accountInit(user.mrgnProgram, {
          marginfiGroup: lstGroup.publicKey,
          marginfiAccount: userAccount.publicKey,
          authority: user.wallet.publicKey,
          feePayer: user.wallet.publicKey,
        }),
      ),
      [userAccount],
    );
    await user.mrgnProgram.provider.sendAndConfirm!(
      new Transaction().add(
        await depositIx(user.mrgnProgram, {
          marginfiAccount: userAccount.publicKey,
          bank: lstBank.publicKey,
          tokenAccount: user.wsolAccount,
          amount,
        }),
      ),
    );

    const pulseHealth = async () => {
      await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);
      await user.mrgnProgram.provider.sendAndConfirm!(
        new Transaction().add(
          await healthPulse(user.mrgnProgram, {
            marginfiAccount: userAccount.publicKey,
            remaining: composeRemainingAccounts([
              [lstBank.publicKey, oracles.wsolOracle.publicKey, MSOL_STATE],
            ]),
          }),
        ),
      );
      const { healthCache } = await program.account.marginfiAccount.fetch(
        userAccount.publicKey,
      );
      return {
        init: toI80Scaled(healthCache.assetValue),
        maint: toI80Scaled(healthCache.assetValueMaint),
      };
    };

    // The mock feed sets EMA equal to spot, so Initial prices at the cached spot price too.
    await pulseCache([MSOL_STATE]);
    const { cache, config, assetShareValue } = await program.account.bank.fetch(
      lstBank.publicKey,
    );
    const { lendingAccount } = await program.account.marginfiAccount.fetch(
      userAccount.publicKey,
    );
    const assets = mulI80(
      toI80Scaled(lendingAccount.balances[0].assetShares),
      toI80Scaled(assetShareValue),
    );
    const lowPrice =
      toI80Scaled(cache.lastOraclePrice) -
      toI80Scaled(cache.lastOraclePriceConfidence);
    const scale = toI80Scaled(10n ** BigInt(ecosystem.wsolDecimals));
    const valueAt = (weight: bigint) =>
      divI80(mulI80(mulI80(assets, weight), lowPrice), scale);
    const initValue = valueAt(toI80Scaled(config.assetWeightInit));
    const maintValue = valueAt(toI80Scaled(config.assetWeightMaint));

    const healthy = await pulseHealth();
    assert.equal(healthy.init, initValue);
    assert.equal(healthy.maint, maintValue);

    await setMsolPaused(true);
    const paused = await pulseHealth();
    assert.equal(paused.init, 0n);
    assert.equal(paused.maint, maintValue);

    await setMsolPaused(false);
    const resumed = await pulseHealth();
    assert.equal(resumed.init, initValue);
  });

  it("PythLST tracks pool appreciation", async () => {
    await setOracle(ORACLE_SETUP_PYTH_LST, [SANCTUM_SPL_POOL]);
    const { poolTokenSupply } = decodeStakePool(
      (await banksClient.getAccount(SANCTUM_SPL_POOL))!.data,
    );
    const before = wrappedI80F48toBigNumber(
      (await pulseCache([SANCTUM_SPL_POOL])).lastOraclePrice,
    ).toNumber();

    // Simulate rewards: add 777 SOL of backing lamports, leaving the supply untouched.
    const addedLamports = 777 * LAMPORTS_PER_SOL;
    await patchData(SANCTUM_SPL_POOL, (data) => {
      data.writeBigUInt64LE(
        decodeStakePool(data).totalLamports + BigInt(addedLamports),
        258,
      );
    });

    // Price rises by exactly baseSolPrice * addedLamports / supply.
    const expectedGain =
      (baseSolPrice * addedLamports) / Number(poolTokenSupply);
    const after = await pulseCache([SANCTUM_SPL_POOL]);
    assertI80F48Approx(after.lastOraclePrice, before + expectedGain);
  });

  it("(admin) tries to price PythLST with a wrong-owner pool - fails", async () => {
    await setOracle(ORACLE_SETUP_PYTH_LST, [JUPSOL_POOL]);
    const acc = await banksClient.getAccount(JUPSOL_POOL);
    bankrunContext.setAccount(JUPSOL_POOL, {
      lamports: acc!.lamports,
      data: Buffer.from(acc!.data),
      owner: SystemProgram.programId,
      executable: acc!.executable,
      rentEpoch: acc!.rentEpoch,
    });
    await expectFailedTxWithError(
      async () => {
        await pulseCache([JUPSOL_POOL]);
      },
      "StakePoolValidationFailed",
      6048,
    );
  });
});
