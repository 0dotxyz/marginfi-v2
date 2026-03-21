import { BN } from "@coral-xyz/anchor";
import { PublicKey, Transaction } from "@solana/web3.js";
import { createAssociatedTokenAccountIdempotentInstruction } from "@solana/spl-token";
import {
  bankrunContext,
  bankrunProgram,
  banksClient,
  ecosystem,
  groupAdmin,
  oracles,
} from "../rootHooks";
import {
  configureJuplendProtocolPermissions,
  initJuplendGlobals,
  initJuplendPool,
} from "./juplend/jlr-pool-setup";
import { deriveBankWithSeed } from "./pdas";
import { processBankrunTransaction, mintToTokenAccount } from "./tools";
import {
  addJuplendBankIx,
  makeJuplendInitPositionIx,
} from "./juplend/group-instructions";
import {
  defaultJuplendBankConfig,
  DEFAULT_BORROW_CONFIG_MIN,
  JuplendPoolKeys,
} from "./juplend/types";
import {
  deriveJuplendGlobalKeys,
  deriveJuplendPoolKeys,
} from "./juplend/juplend-pdas";
import { deriveLiquidityVaultAuthority } from "./pdas";

const hasAccount = async (address: PublicKey) => {
  const account = await banksClient.getAccount(address);
  return account !== null;
};

let juplendTokenAPoolPromise: Promise<JuplendPoolKeys> | null = null;

const ensureJuplendTokenAPoolSetupInner =
  async (): Promise<JuplendPoolKeys> => {
    const mint = ecosystem.tokenAMint.publicKey;
    const pool = deriveJuplendPoolKeys({ mint });
    const globals = deriveJuplendGlobalKeys();

    if (!(await hasAccount(globals.liquidity))) {
      await initJuplendGlobals({ admin: groupAdmin.wallet });
    }

    if (!(await hasAccount(pool.lending))) {
      await initJuplendPool({
        admin: groupAdmin.wallet,
        mint,
        symbol: "mlTokenA",
        decimals: ecosystem.tokenADecimals,
      });
    }

    await configureJuplendProtocolPermissions({
      admin: groupAdmin.wallet,
      mint,
      lending: pool.lending,
      rateModel: pool.rateModel,
      tokenReserve: pool.tokenReserve,
      supplyPositionOnLiquidity: pool.supplyPositionOnLiquidity,
      borrowPositionOnLiquidity: pool.borrowPositionOnLiquidity,
      tokenProgram: pool.tokenProgram,
      borrowConfig: DEFAULT_BORROW_CONFIG_MIN,
    });

    return pool;
  };

export const ensureJuplendTokenAPoolSetup =
  async (): Promise<JuplendPoolKeys> => {
    if (!juplendTokenAPoolPromise) {
      juplendTokenAPoolPromise = ensureJuplendTokenAPoolSetupInner();
    }

    try {
      return await juplendTokenAPoolPromise;
    } catch (error) {
      juplendTokenAPoolPromise = null;
      throw error;
    }
  };

export const addJuplendBanksForGroup = async (args: {
  group: PublicKey;
  numberOfBanks: number;
  startingSeed: number;
  oracle?: PublicKey;
  useSwitchboardOracle?: boolean;
}): Promise<{ juplendBanks: PublicKey[]; pool: JuplendPoolKeys }> => {
  const pool = await ensureJuplendTokenAPoolSetup();
  if (args.numberOfBanks <= 0) {
    return { juplendBanks: [], pool };
  }

  const seedDepositAmount = new BN(10).pow(new BN(ecosystem.tokenADecimals));
  await mintToTokenAccount(
    ecosystem.tokenAMint.publicKey,
    groupAdmin.tokenAAccount,
    seedDepositAmount.mul(new BN(args.numberOfBanks + 1))
  );

  const juplendBanks: PublicKey[] = [];
  const bankOracle = args.oracle ?? oracles.tokenAOracle.publicKey;
  const useSwitchboardOracle = args.useSwitchboardOracle ?? false;
  for (let i = 0; i < args.numberOfBanks; i += 1) {
    const bankSeed = new BN(args.startingSeed + i);
    const config = defaultJuplendBankConfig(
      bankOracle,
      ecosystem.tokenADecimals
    );
    if (useSwitchboardOracle) {
      config.oracleSetup = { juplendSwitchboardPull: {} };
    }
    const addIx = await addJuplendBankIx(groupAdmin.mrgnBankrunProgram, {
      group: args.group,
      feePayer: groupAdmin.wallet.publicKey,
      bankMint: ecosystem.tokenAMint.publicKey,
      bankSeed,
      oracle: bankOracle,
      jupLendingState: pool.lending,
      fTokenMint: pool.fTokenMint,
      config,
      tokenProgram: pool.tokenProgram,
    });
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(addIx),
      [groupAdmin.wallet],
      false,
      true
    );

    const [bankPk] = deriveBankWithSeed(
      bankrunProgram.programId,
      args.group,
      ecosystem.tokenAMint.publicKey,
      bankSeed
    );
    const [liquidityVaultAuthority] = deriveLiquidityVaultAuthority(
      bankrunProgram.programId,
      bankPk
    );
    const createWithdrawIntermediaryAtaIx =
      createAssociatedTokenAccountIdempotentInstruction(
        groupAdmin.wallet.publicKey,
        (await bankrunProgram.account.bank.fetch(bankPk)).integrationAcc3,
        liquidityVaultAuthority,
        ecosystem.tokenAMint.publicKey,
        pool.tokenProgram
      );
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(createWithdrawIntermediaryAtaIx),
      [groupAdmin.wallet],
      false,
      true
    );

    const initPositionIx = await makeJuplendInitPositionIx(
      groupAdmin.mrgnBankrunProgram,
      {
        feePayer: groupAdmin.wallet.publicKey,
        signerTokenAccount: groupAdmin.tokenAAccount,
        bank: bankPk,
        pool,
        seedDepositAmount,
        tokenProgram: pool.tokenProgram,
      }
    );
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(initPositionIx),
      [groupAdmin.wallet],
      false,
      true
    );

    juplendBanks.push(bankPk);
  }

  return { juplendBanks, pool };
};
