import { BN, Program } from "@coral-xyz/anchor";
import {
  AccountMeta,
  PublicKey,
  TransactionInstruction,
} from "@solana/web3.js";
import { TOKEN_PROGRAM_ID } from "@solana/spl-token";

import { Marginfi } from "../../../target/types/marginfi";
import type { JuplendConfigCompact } from "./types";
import type { JuplendPoolKeys } from "./types";
import { JUPLEND_LIQUIDITY_PROGRAM_ID } from "./juplend-pdas";

export type AddJuplendBankAccounts = {
  group: PublicKey;
  feePayer: PublicKey;
  /** Must match the mint of the jupLendingState */
  bankMint: PublicKey;
  bankSeed: BN;
  /** Pyth price update account (oracle_keys[0]) */
  oracle: PublicKey;
  /** JupLend lending state (oracle_keys[1]) */
  jupLendingState: PublicKey;
  /**
   * Can be read from `jupLendingState`
   * * Note: Although anchor believes it can infer this, it cannot. The limitation is probably
   *   because integrationAcc1 (jupLendingState) does not belong to our program and the has_one
   *   inference doesn't work here. You must pass this in `accountsPartial` or you will get the
   *   generic `Reached maximum depth for account resolution` error
   * */
  fTokenMint: PublicKey;
  config: JuplendConfigCompact;
  /** Optional explicit token program; defaults to SPL Token classic. T22 assets must use T22 */
  tokenProgram?: PublicKey;
};

export const addJuplendBankIx = async (
  program: Program<Marginfi>,
  accounts: AddJuplendBankAccounts,
): Promise<TransactionInstruction> => {
  const tokenProgram = accounts.tokenProgram ?? TOKEN_PROGRAM_ID;

  const remainingAccounts: AccountMeta[] = [
    { pubkey: accounts.oracle, isSigner: false, isWritable: false },
    { pubkey: accounts.jupLendingState, isSigner: false, isWritable: false },
  ];

  return program.methods
    .lendingPoolAddBankJuplend(accounts.config, accounts.bankSeed)
    .accounts({
      group: accounts.group,
      feePayer: accounts.feePayer,
      bankMint: accounts.bankMint,
      integrationAcc1: accounts.jupLendingState,
      tokenProgram,
    })
    .accountsPartial({
      fTokenMint: accounts.fTokenMint,
    })
    .remainingAccounts(remainingAccounts)
    .instruction();
};

export type JuplendInitPositionAccounts = {
  feePayer: PublicKey;
  signerTokenAccount: PublicKey;
  bank: PublicKey;
  pool: JuplendPoolKeys;
  seedDepositAmount: BN;
  tokenProgram?: PublicKey;
};

/**
 * Build `juplend_init_position`.
 */
export const makeJuplendInitPositionIx = async (
  program: Program<Marginfi>,
  accounts: JuplendInitPositionAccounts,
): Promise<TransactionInstruction> => {
  return (
    program.methods
      .juplendInitPosition(accounts.seedDepositAmount)
      .accounts({
        feePayer: accounts.feePayer,
        signerTokenAccount: accounts.signerTokenAccount,
        bank: accounts.bank,
        lendingAdmin: accounts.pool.lendingAdmin,
        supplyTokenReservesLiquidity: accounts.pool.tokenReserve,
        lendingSupplyPositionOnLiquidity: accounts.pool.supplyPositionOnLiquidity,
        rateModel: accounts.pool.rateModel,
        vault: accounts.pool.vault,
        liquidity: accounts.pool.liquidity,
        liquidityProgram: JUPLEND_LIQUIDITY_PROGRAM_ID,
        rewardsRateModel: accounts.pool.lendingRewardsRateModel,
        tokenProgram: accounts.tokenProgram ?? TOKEN_PROGRAM_ID,
      })
      // Still required: Anchor cannot infer `fTokenMint` via `integration_acc_1`
      // because that account is external to marginfi.
      .accountsPartial({
        fTokenMint: accounts.pool.fTokenMint,
      })
      .instruction()
  );
};
