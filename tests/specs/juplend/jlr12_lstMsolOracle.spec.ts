import { Program } from "@coral-xyz/anchor";
import { Keypair, PublicKey, Transaction } from "@solana/web3.js";
import { Marginfi } from "../../../target/types/marginfi";
import {
  bankrunProgram,
  groupAdmin,
  juplendAccounts,
  oracles,
} from "../../rootHooks";
import { configureBankOracle } from "../../utils/group-instructions";
import {
  assertKeysEqual,
  expectFailedTxWithError,
} from "../../utils/genericTests";
import {
  ORACLE_SETUP_JUPLEND_LST,
  ORACLE_SETUP_JUPLEND_MSOL,
  ORACLE_SETUP_JUPLEND_PYTH_PULL,
} from "../../utils/types";
import { JUPLEND_STATE_KEYS } from "../../utils/juplend/test-state";
import { assert } from "chai";

const MSOL_STATE = new PublicKey(
  "8szGkuLTAux9XMgZ2vtY39jVSowEcpBfFfD8hXSEqdGC",
);
const BSOL_POOL = new PublicKey("stk9ApL5HeVAwPLr3TLhDXdZS8ptVu7zp6ov8HFDuMi");

let program: Program<Marginfi>;
let bank: PublicKey;
let lending: PublicKey;
let origOracle: PublicKey;

describe("JupLend LST / mSOL oracle setups", () => {
  before(async () => {
    program = bankrunProgram;
    bank = juplendAccounts.get(JUPLEND_STATE_KEYS.jlr01BankUsdc);
    const { config } = await program.account.bank.fetch(bank);
    origOracle = config.oracleKeys[0];
    lending = config.oracleKeys[1];
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

  it("(admin) configures JuplendLST - happy path", async () => {
    await setOracle(ORACLE_SETUP_JUPLEND_LST, oracles.wsolOracle.publicKey, [
      lending,
      BSOL_POOL,
    ]);
    const { config } = await program.account.bank.fetch(bank);
    assert.deepEqual(config.oracleSetup, { juplendLst: {} });
    assertKeysEqual(config.oracleKeys[0], oracles.wsolOracle.publicKey);
    assertKeysEqual(config.oracleKeys[1], lending); // lending preserved, not rewritten
    assertKeysEqual(config.oracleKeys[2], BSOL_POOL);
  });

  it("(admin) configures JuplendMSOL - happy path", async () => {
    await setOracle(ORACLE_SETUP_JUPLEND_MSOL, oracles.wsolOracle.publicKey, [
      lending,
      MSOL_STATE,
    ]);
    const { config } = await program.account.bank.fetch(bank);
    assert.deepEqual(config.oracleSetup, { juplendMsol: {} });
    assertKeysEqual(config.oracleKeys[2], MSOL_STATE);
  });

  it("(admin) tries to configure JuplendLST - fails with wrong number of accounts", async () => {
    await expectFailedTxWithError(
      async () => {
        await setOracle(
          ORACLE_SETUP_JUPLEND_LST,
          oracles.wsolOracle.publicKey,
          [lending],
        );
      },
      "WrongNumberOfOracleAccounts",
      6051,
    );
  });

  it("(admin) tries to configure JuplendLST - fails with a mismatched lending account", async () => {
    await expectFailedTxWithError(
      async () => {
        await setOracle(
          ORACLE_SETUP_JUPLEND_LST,
          oracles.wsolOracle.publicKey,
          [Keypair.generate().publicKey, BSOL_POOL],
        );
      },
      "JuplendLendingValidationFailed",
      6501,
    );
  });

  it("(admin) tries to configure JuplendLST - fails with a non-stake-pool account", async () => {
    await expectFailedTxWithError(
      async () => {
        await setOracle(
          ORACLE_SETUP_JUPLEND_LST,
          oracles.wsolOracle.publicKey,
          [lending, Keypair.generate().publicKey],
        );
      },
      "StakePoolValidationFailed",
      6048,
    );
  });

  it("(admin) restores the bank oracle", async () => {
    await setOracle(ORACLE_SETUP_JUPLEND_PYTH_PULL, origOracle, [lending]);
    const { config } = await program.account.bank.fetch(bank);
    assert.deepEqual(config.oracleSetup, { juplendPythPull: {} });
    assertKeysEqual(config.oracleKeys[0], origOracle);
  });
});
