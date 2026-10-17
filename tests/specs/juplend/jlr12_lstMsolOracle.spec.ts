import { Program } from "@coral-xyz/anchor";
import { Keypair, PublicKey, Transaction } from "@solana/web3.js";
import { Marginfi } from "../../../target/types/marginfi";
import {
  bankrunProgram,
  bankrunContext,
  banksClient,
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
  ORACLE_SETUP_KAMINO_LST,
} from "../../utils/types";
import { JUPLEND_STATE_KEYS } from "../../utils/juplend/test-state";
import { findJuplendLiquidityPda } from "../../utils/juplend/juplend-pdas";
import { assert } from "chai";

const MSOL_STATE = new PublicKey(
  "8szGkuLTAux9XMgZ2vtY39jVSowEcpBfFfD8hXSEqdGC",
);
const BSOL_POOL = new PublicKey("stk9ApL5HeVAwPLr3TLhDXdZS8ptVu7zp6ov8HFDuMi");

let program: Program<Marginfi>;
let bank: PublicKey;
let lending: PublicKey;
let origOracle: PublicKey;
const [liquidityPda] = findJuplendLiquidityPda();

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

  // This is a USDC bank, so a bSOL stake pool / mSOL State is a mint mismatch, which config
  // validation rejects.
  it("(admin) tries to configure JuplendLST with a stake pool for the wrong mint - fails", async () => {
    await expectFailedTxWithError(
      async () => {
        await setOracle(ORACLE_SETUP_JUPLEND_LST, oracles.wsolOracle.publicKey, [
          lending,
          BSOL_POOL,
          liquidityPda,
        ]);
      },
      "StakePoolValidationFailed",
      6048,
    );
  });

  it("(admin) tries to configure JuplendMSOL with a State for the wrong mint - fails", async () => {
    await expectFailedTxWithError(
      async () => {
        await setOracle(ORACLE_SETUP_JUPLEND_MSOL, oracles.wsolOracle.publicKey, [
          lending,
          MSOL_STATE,
          liquidityPda,
        ]);
      },
      "MarinadeStateValidationFailed",
      6136,
    );
  });

  it("(admin) tries to configure JuplendLST with wrong oracle setup - fails", async () => {
    await expectFailedTxWithError(
      async () => {
        await setOracle(ORACLE_SETUP_KAMINO_LST, oracles.wsolOracle.publicKey, [
          lending,
          BSOL_POOL,
        ]);
      },
      "InvalidOracleSetup",
      6025,
    );
  });

  it("(admin) tries to configure JuplendLST with wrong number of accounts - fails", async () => {
    await expectFailedTxWithError(
      async () => {
        await setOracle(
          ORACLE_SETUP_JUPLEND_LST,
          oracles.wsolOracle.publicKey,
          [lending, BSOL_POOL], 
        );
      },
      "WrongNumberOfOracleAccounts",
      6051,
    );
  });

  it("(admin) tries to configure JuplendLST with a mismatched lending account - fails", async () => {
    await expectFailedTxWithError(
      async () => {
        await setOracle(
          ORACLE_SETUP_JUPLEND_LST,
          oracles.wsolOracle.publicKey,
          [Keypair.generate().publicKey, BSOL_POOL, liquidityPda],
        );
      },
      "JuplendLendingValidationFailed",
      6501,
    );
  });

  it("(admin) tries to configure JuplendLST with a non-stake-pool account - fails", async () => {
    await expectFailedTxWithError(
      async () => {
        await setOracle(
          ORACLE_SETUP_JUPLEND_LST,
          oracles.wsolOracle.publicKey,
          [lending, Keypair.generate().publicKey, liquidityPda],
        );
      },
      "StakePoolValidationFailed",
      6048,
    );
  });

  it("(admin) successfully configures JuplendLST with valid accounts - succeeds", async () => {
    
    const usdcBank = await program.account.bank.fetch(bank);
    const usdcMint = usdcBank.mint;

    const tempStakePool = Keypair.generate();
    const bsolPoolAcc = await banksClient.getAccount(BSOL_POOL);
    if (!bsolPoolAcc) {
      throw new Error("BSOL_POOL account not found");
    }

    const data = Buffer.from(bsolPoolAcc.data);
    usdcMint.toBuffer().copy(data, 162); 

    bankrunContext.setAccount(tempStakePool.publicKey, {
      lamports: bsolPoolAcc.lamports,
      data,
      owner: bsolPoolAcc.owner,
      executable: bsolPoolAcc.executable,
      rentEpoch: bsolPoolAcc.rentEpoch,
    });

    await setOracle(ORACLE_SETUP_JUPLEND_LST, oracles.wsolOracle.publicKey, [
      lending,
      tempStakePool.publicKey,
      liquidityPda,
    ]);

    const { config } = await program.account.bank.fetch(bank);
    assert.deepEqual(config.oracleSetup, { juplendLst: {} });
    assertKeysEqual(config.oracleKeys[0], oracles.wsolOracle.publicKey);
    assertKeysEqual(config.oracleKeys[1], lending);
    assertKeysEqual(config.oracleKeys[2], tempStakePool.publicKey);
  });

  it("(admin) restores the bank oracle", async () => {
    await setOracle(ORACLE_SETUP_JUPLEND_PYTH_PULL, origOracle, [lending, liquidityPda]);
    const { config } = await program.account.bank.fetch(bank);
    assert.deepEqual(config.oracleSetup, { juplendPythPull: {} });
    assertKeysEqual(config.oracleKeys[0], origOracle);
  });
});
