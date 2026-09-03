import { Program } from "@coral-xyz/anchor";
import { Keypair, PublicKey, Transaction } from "@solana/web3.js";
import * as fs from "fs";
import * as path from "path";
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
  groupAdmin,
} from "../../rootHooks";
import {
  defaultBankConfig,
  ORACLE_CONF_INTERVAL,
  ORACLE_SETUP_PYTH_LST,
  ORACLE_SETUP_SWITCHBOARD_PULL,
} from "../../utils/types";
import {
  createBankrunPythFeedAccount,
  createBankrunPythOracleAccount,
  setPythPullOraclePrice,
  setupSwitchboardPullOracleFromTemplate,
  refreshSwitchboardPullOracleBankrun,
} from "../../utils/bankrun-oracles";
import { wrappedI80F48toBigNumber } from "@mrgnlabs/mrgn-common";
import { assert } from "chai";

// Real mainnet accounts (see tests/fixtures). These three fixtures + the SOL/USD and bSOL/USD
// prices decoded below form ONE consistent snapshot — if any is re-captured, re-capture all.
const BSOL_MINT = new PublicKey("bSo13r4TkiE4KumL71LsHTPpL2euBYLFx6h9HP3piy1");
const BSOL_POOL = new PublicKey("stk9ApL5HeVAwPLr3TLhDXdZS8ptVu7zp6ov8HFDuMi");
const PYTH_RECEIVER = new PublicKey(
  "rec5EKMGg6MxZYaMdyBfgwp4d5rB9T1VQH5pJv5LtFJ",
);

const readFixture = (file: string) =>
  JSON.parse(
    fs.readFileSync(path.resolve(__dirname, "../../fixtures", file), "utf8"),
  );
const fixtureBytes = (file: string): Buffer =>
  Buffer.from(readFixture(file).account.data[0], "base64");
const loadFixtureAccount = (file: string) => {
  const j = readFixture(file);
  bankrunContext.setAccount(new PublicKey(j.pubkey), {
    lamports: Number(j.account.lamports),
    data: Buffer.from(j.account.data[0], "base64"),
    owner: new PublicKey(j.account.owner),
    executable: j.account.executable,
    rentEpoch: Number(j.account.rentEpoch ?? 0),
  });
};

// Pyth PriceUpdateV2 (verification level Full): price i64 @73, exponent i32 @89.
const decodePythPrice = (file: string): number => {
  const b = fixtureBytes(file);
  return Number(b.readBigInt64LE(73)) * 10 ** b.readInt32LE(89);
};
// Switchboard PullFeedAccountData: aggregated result value i128 (1e18 precision) @56.
const decodeSwbPrice = (file: string): number => {
  const b = fixtureBytes(file);
  const v = b.readBigUInt64LE(56) + (b.readBigUInt64LE(64) << 64n);
  return Number(v) / 1e18;
};

/** Real mainnet SOL/USD (Pyth) at snapshot time. */
const SOL_USD = decodePythPrice("pyth_sol_usd.json");
/** Real mainnet bSOL/USD (Switchboard) at snapshot time — the independent reference price. */
const BSOL_USD = decodeSwbPrice("swb_bsol_usd.json");

const parityGroup = Keypair.generate();
const parityBank = Keypair.generate();
// Dedicated oracle accounts so we don't disturb the shared ones.
const solUsdPyth = Keypair.generate();
const solUsdPythFeed = Keypair.generate();
const bsolUsdSwb = Keypair.generate();

let program: Program<Marginfi>;

describe("LST oracle price parity (SwitchboardPull vs PythLST, real bSOL fixtures)", () => {
  before(async () => {
    program = bankrunProgram;
    const admin = groupAdmin.mrgnProgram;

    // Load the real bSOL mint so a bank can be created against it (bank.mint == pool.pool_mint).
    loadFixtureAccount("bsol_mint.json");

    // Seed the two oracles with the real captured prices.
    await createBankrunPythOracleAccount(
      bankrunContext,
      banksClient,
      solUsdPyth,
      PYTH_RECEIVER,
    );
    await createBankrunPythFeedAccount(
      bankrunContext,
      banksClient,
      solUsdPythFeed,
      PYTH_RECEIVER,
    );
    await setPythPullOraclePrice(
      bankrunContext,
      banksClient,
      solUsdPyth.publicKey,
      solUsdPythFeed.publicKey,
      SOL_USD,
      8,
      ORACLE_CONF_INTERVAL,
    );
    await setupSwitchboardPullOracleFromTemplate(
      bankrunContext,
      banksClient,
      bsolUsdSwb,
      { price: BSOL_USD, label: "bSOL/USD swb" },
    );

    await admin.provider.sendAndConfirm!(
      new Transaction().add(
        await groupInitialize(admin, {
          marginfiGroup: parityGroup.publicKey,
          admin: groupAdmin.wallet.publicKey,
        }),
      ),
      [parityGroup],
    );
    await admin.provider.sendAndConfirm!(
      new Transaction().add(
        await addBank(admin, {
          marginfiGroup: parityGroup.publicKey,
          feePayer: groupAdmin.wallet.publicKey,
          bankMint: BSOL_MINT,
          bank: parityBank.publicKey,
          config: defaultBankConfig(),
        }),
      ),
      [parityBank],
    );
  });

  const priceViaSwitchboard = async (): Promise<number> => {
    await groupAdmin.mrgnProgram.provider.sendAndConfirm!(
      new Transaction().add(
        await configureBankOracle(groupAdmin.mrgnProgram, {
          bank: parityBank.publicKey,
          type: ORACLE_SETUP_SWITCHBOARD_PULL,
          oracle: bsolUsdSwb.publicKey,
        }),
      ),
    );
    await refreshSwitchboardPullOracleBankrun(
      bankrunContext,
      banksClient,
      bsolUsdSwb.publicKey,
    );
    await groupAdmin.mrgnProgram.provider.sendAndConfirm!(
      new Transaction().add(
        await pulseBankPrice(groupAdmin.mrgnProgram, {
          bank: parityBank.publicKey,
          remaining: [bsolUsdSwb.publicKey],
        }),
      ),
    );
    const { cache } = await program.account.bank.fetch(parityBank.publicKey);
    return wrappedI80F48toBigNumber(cache.lastOraclePrice).toNumber();
  };

  const priceViaPythLst = async (): Promise<number> => {
    await groupAdmin.mrgnProgram.provider.sendAndConfirm!(
      new Transaction().add(
        await configureBankOracle(groupAdmin.mrgnProgram, {
          bank: parityBank.publicKey,
          type: ORACLE_SETUP_PYTH_LST,
          oracle: solUsdPyth.publicKey,
          remaining: [BSOL_POOL],
        }),
      ),
    );
    // Re-stamp the Pyth price fresh for the pulse.
    await setPythPullOraclePrice(
      bankrunContext,
      banksClient,
      solUsdPyth.publicKey,
      solUsdPythFeed.publicKey,
      SOL_USD,
      8,
      ORACLE_CONF_INTERVAL,
    );
    await groupAdmin.mrgnProgram.provider.sendAndConfirm!(
      new Transaction().add(
        await pulseBankPrice(groupAdmin.mrgnProgram, {
          bank: parityBank.publicKey,
          remaining: [solUsdPyth.publicKey, BSOL_POOL],
        }),
      ),
    );
    const { cache } = await program.account.bank.fetch(parityBank.publicKey);
    return wrappedI80F48toBigNumber(cache.lastOraclePrice).toNumber();
  };

  it("prices the same bSOL bank identically via SwitchboardPull and PythLST", async () => {
    // The bSOL stake pool's real rate; PythLST derives bSOL/USD = SOL/USD * this rate.
    const poolAcc = await banksClient.getAccount(BSOL_POOL);
    const poolRate =
      Number(Buffer.from(poolAcc!.data).readBigUInt64LE(258)) /
      Number(Buffer.from(poolAcc!.data).readBigUInt64LE(266));

    const pSwb = await priceViaSwitchboard();
    const pLst = await priceViaPythLst();

    console.log(
      `  SwitchboardPull bSOL/USD = ${pSwb.toFixed(4)} | ` +
        `PythLST (SOL/USD ${SOL_USD.toFixed(2)} x rate ${poolRate.toFixed(
          5,
        )}) = ${pLst.toFixed(4)} | ` +
        `diff ${((Math.abs(pSwb - pLst) / pSwb) * 100).toFixed(3)}%`,
    );

    // Sanity: each path reports exactly what we expect from the real inputs.
    assert.approximately(pSwb, BSOL_USD, 1e-6);
    assert.approximately(pLst, SOL_USD * poolRate, 1e-6);

    // The point of the test: both oracles price the same bank to within 0.1%.
    const relDiff = Math.abs(pSwb - pLst) / pSwb;
    assert.isBelow(
      relDiff,
      0.001,
      `SwitchboardPull=${pSwb} vs PythLST=${pLst} differ by ${(
        relDiff * 100
      ).toFixed(4)}%`,
    );
  });
});
