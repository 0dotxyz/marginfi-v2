import { Program } from "@coral-xyz/anchor";
import {
  Keypair,
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
import { wrappedI80F48toBigNumber } from "@mrgnlabs/mrgn-common";
import { assert } from "chai";

// Mainnet fixtures, one per stake-pool owner program (see tests/fixtures)
const MSOL_STATE = new PublicKey(
  "8szGkuLTAux9XMgZ2vtY39jVSowEcpBfFfD8hXSEqdGC",
);
const BSOL_POOL = new PublicKey("stk9ApL5HeVAwPLr3TLhDXdZS8ptVu7zp6ov8HFDuMi");
const SANCTUM_SPL_POOL = new PublicKey(
  "9mhGNSPArRMHpLDMSmxAvuoizBqtBGqYdT8WGuqgxNdn",
);
const JUPSOL_POOL = new PublicKey(
  "8VpRhuxa7sUUepdY3kQiTmX9rS5vx4WgaXiAnXq4KCtr",
);
const MSOL_PRICE_PRECISION = 2 ** 32;

const lstGroup = Keypair.generate();
const lstBank = Keypair.generate();

let program: Program<Marginfi>;

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
  });

  const readU64 = (data: Uint8Array, offset: number) =>
    Buffer.from(data).readBigUInt64LE(offset);

  const stakePoolRate = async (pool: PublicKey) => {
    const acc = await banksClient.getAccount(pool);
    return Number(readU64(acc!.data, 258)) / Number(readU64(acc!.data, 266));
  };

  const marinadeRate = async (state: PublicKey) => {
    const acc = await banksClient.getAccount(state);
    return Number(readU64(acc!.data, 512)) / MSOL_PRICE_PRECISION;
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

  const pulseAndRead = async (multiplierAccounts: PublicKey[]) => {
    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);
    await groupAdmin.mrgnProgram.provider.sendAndConfirm!(
      new Transaction().add(
        await pulseBankPrice(groupAdmin.mrgnProgram, {
          bank: lstBank.publicKey,
          remaining: [oracles.wsolOracle.publicKey, ...multiplierAccounts],
        }),
      ),
    );
    const bank = await program.account.bank.fetch(lstBank.publicKey);
    return {
      price: wrappedI80F48toBigNumber(bank.cache.lastOraclePrice).toNumber(),
      multiplier: wrappedI80F48toBigNumber(
        bank.cache.priceMultiplier,
      ).toNumber(),
    };
  };

  it("(admin) configures PythLST - happy path", async () => {
    await setOracle(ORACLE_SETUP_PYTH_LST, [BSOL_POOL]);
    const { config } = await program.account.bank.fetch(lstBank.publicKey);
    assert.deepEqual(config.oracleSetup, { pythLst: {} });
    assertKeysEqual(config.oracleKeys[0], oracles.wsolOracle.publicKey);
    assertKeysEqual(config.oracleKeys[1], BSOL_POOL);
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
        await setOracle(ORACLE_SETUP_PYTH_MSOL, [BSOL_POOL]);
      },
      "MarinadeStateValidationFailed",
      6600,
    );
  });

  it("prices bSOL via PythLST (vanilla SPL owner)", async () => {
    await setOracle(ORACLE_SETUP_PYTH_LST, [BSOL_POOL]);
    const rate = await stakePoolRate(BSOL_POOL);
    const { price, multiplier } = await pulseAndRead([BSOL_POOL]);
    const expected = oracles.wsolPrice * rate;
    assert.approximately(price, expected, expected * 0.02);
    assert.approximately(multiplier, 1, 0.0001);
  });

  it("prices a Sanctum-SPL LST via PythLST", async () => {
    await setOracle(ORACLE_SETUP_PYTH_LST, [SANCTUM_SPL_POOL]);
    const rate = await stakePoolRate(SANCTUM_SPL_POOL);
    const { price } = await pulseAndRead([SANCTUM_SPL_POOL]);
    const expected = oracles.wsolPrice * rate;
    assert.approximately(price, expected, expected * 0.02);
  });

  it("prices jupSOL (Sanctum-Multi owner) via PythLST", async () => {
    await setOracle(ORACLE_SETUP_PYTH_LST, [JUPSOL_POOL]);
    const rate = await stakePoolRate(JUPSOL_POOL);
    const { price } = await pulseAndRead([JUPSOL_POOL]);
    const expected = oracles.wsolPrice * rate;
    assert.approximately(price, expected, expected * 0.02);
  });

  it("prices mSOL via PythMSOL", async () => {
    await setOracle(ORACLE_SETUP_PYTH_MSOL, [MSOL_STATE]);
    const rate = await marinadeRate(MSOL_STATE);
    const { price, multiplier } = await pulseAndRead([MSOL_STATE]);
    const expected = oracles.wsolPrice * rate;
    assert.approximately(price, expected, expected * 0.02);
    assert.approximately(multiplier, 1, 0.0001);
  });

  it("PythLST price tracks pool appreciation", async () => {
    await setOracle(ORACLE_SETUP_PYTH_LST, [SANCTUM_SPL_POOL]);
    const before = await pulseAndRead([SANCTUM_SPL_POOL]);

    // Bump total_lamports by 10%, leaving supply untouched
    const acc = await banksClient.getAccount(SANCTUM_SPL_POOL);
    const data = Buffer.from(acc!.data);
    data.writeBigUInt64LE((readU64(data, 258) * 110n) / 100n, 258);
    bankrunContext.setAccount(SANCTUM_SPL_POOL, {
      lamports: acc!.lamports,
      data,
      owner: acc!.owner,
      executable: acc!.executable,
      rentEpoch: acc!.rentEpoch,
    });

    const after = await pulseAndRead([SANCTUM_SPL_POOL]);
    assert.isAbove(after.price, before.price);
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
        await pulseAndRead([JUPSOL_POOL]);
      },
      "StakePoolValidationFailed",
      6048,
    );
  });
});
