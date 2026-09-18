import { Program } from "@coral-xyz/anchor";
import { Keypair, PublicKey, Transaction } from "@solana/web3.js";
import { Marginfi } from "../../../target/types/marginfi";
import {
  bankrunProgram,
  groupAdmin,
  kaminoAccounts,
  KAMINO_USDC_BANK,
  oracles,
} from "../../rootHooks";
import { configureBankOracle } from "../../utils/group-instructions";
import {
  assertKeysEqual,
  expectFailedTxWithError,
} from "../../utils/genericTests";
import {
  ORACLE_SETUP_JUPLEND_LST,
  ORACLE_SETUP_KAMINO_LST,
  ORACLE_SETUP_KAMINO_MSOL,
} from "../../utils/types";
import { ORACLE_SETUP_KAMINO_PYTH_PUSH } from "../../utils/kamino-utils";
import { assert } from "chai";

const MSOL_STATE = new PublicKey(
  "8szGkuLTAux9XMgZ2vtY39jVSowEcpBfFfD8hXSEqdGC",
);
const BSOL_POOL = new PublicKey("stk9ApL5HeVAwPLr3TLhDXdZS8ptVu7zp6ov8HFDuMi");

let program: Program<Marginfi>;
let bank: PublicKey;
let reserve: PublicKey;
let origOracle: PublicKey;

describe("Kamino LST / mSOL oracle setups", () => {
  before(async () => {
    program = bankrunProgram;
    bank = kaminoAccounts.get(KAMINO_USDC_BANK);
    const { config } = await program.account.bank.fetch(bank);
    origOracle = config.oracleKeys[0];
    reserve = config.oracleKeys[1];
  });

  const setOracle = async (
    type: number,
    oracle: PublicKey,
    remaining: PublicKey[],
  ) => {
    const ix = await configureBankOracle(groupAdmin.mrgnProgram, {
      bank,
      type,
      oracle,
      remaining,
    });
    return groupAdmin.mrgnProgram.provider.sendAndConfirm!(
      new Transaction().add(ix),
    );
  };

  // This is a USDC bank, so a bSOL stake pool / mSOL State is a mint mismatch, which config
  // validation rejects.
  it("(admin) tries to configure KaminoLST with a stake pool for the wrong mint - fails", async () => {
    await expectFailedTxWithError(
      async () => {
        await setOracle(ORACLE_SETUP_KAMINO_LST, oracles.wsolOracle.publicKey, [
          reserve,
          BSOL_POOL,
        ]);
      },
      "StakePoolValidationFailed",
      6048,
    );
  });

  it("(admin) tries to configure KaminoMSOL with a State for the wrong mint - fails", async () => {
    await expectFailedTxWithError(
      async () => {
        await setOracle(ORACLE_SETUP_KAMINO_MSOL, oracles.wsolOracle.publicKey, [
          reserve,
          MSOL_STATE,
        ]);
      },
      "MarinadeStateValidationFailed",
      6136,
    );
  });

  it("(admin) tries to configure KaminoLST with wrong oracle setup - fails", async () => {
    await expectFailedTxWithError(
      async () => {
        await setOracle(
          ORACLE_SETUP_JUPLEND_LST,
          oracles.wsolOracle.publicKey,
          [reserve, BSOL_POOL],
        );
      },
      "InvalidOracleSetup",
      6025,
    );
  });

  it("(admin) tries to configure KaminoLST with wrong number of accounts - fails", async () => {
    await expectFailedTxWithError(
      async () => {
        await setOracle(ORACLE_SETUP_KAMINO_LST, oracles.wsolOracle.publicKey, [
          reserve,
        ]);
      },
      "WrongNumberOfOracleAccounts",
      6051,
    );
  });

  it("(admin) tries to configure KaminoLST with a mismatched reserve - fails", async () => {
    await expectFailedTxWithError(
      async () => {
        await setOracle(ORACLE_SETUP_KAMINO_LST, oracles.wsolOracle.publicKey, [
          Keypair.generate().publicKey,
          BSOL_POOL,
        ]);
      },
      "KaminoReserveValidationFailed",
      6210,
    );
  });

  it("(admin) tries to configure KaminoLST with a non-stake-pool account - fails", async () => {
    await expectFailedTxWithError(
      async () => {
        await setOracle(ORACLE_SETUP_KAMINO_LST, oracles.wsolOracle.publicKey, [
          reserve,
          Keypair.generate().publicKey,
        ]);
      },
      "StakePoolValidationFailed",
      6048,
    );
  });

  it("(admin) restores the bank oracle", async () => {
    await setOracle(ORACLE_SETUP_KAMINO_PYTH_PUSH, origOracle, [reserve]);
    const { config } = await program.account.bank.fetch(bank);
    assert.deepEqual(config.oracleSetup, { kaminoPythPush: {} });
    assertKeysEqual(config.oracleKeys[0], origOracle);
  });
});
