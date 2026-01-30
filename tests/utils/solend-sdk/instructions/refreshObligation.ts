import {
  PublicKey,
  TransactionInstruction,
  SYSVAR_CLOCK_PUBKEY,
} from "@solana/web3.js";
import { LendingInstruction } from "../instruction";
const BufferLayout = require("buffer-layout");

/// Refresh an obligation"s accrued interest and collateral and liquidity prices. Requires
/// refreshed reserves, as all obligation collateral deposit reserves in order, followed by all
/// liquidity borrow reserves in order.
///
/// Accounts expected by this instruction:
///
///   0. `[writable]` Obligation account.
///   1. `[]` Clock sysvar.
///   .. `[]` Collateral deposit reserve accounts - refreshed, all, in order.
///   .. `[]` Liquidity borrow reserve accounts - refreshed, all, in order.
export const refreshObligationInstruction = (
  obligation: PublicKey,
  depositReserves: PublicKey[],
  borrowReserves: PublicKey[],
  solendProgramAddress: PublicKey
): TransactionInstruction => {
  const dataLayout = BufferLayout.struct([BufferLayout.u8("instruction")]);

  const data = Buffer.alloc(dataLayout.span);
  dataLayout.encode(
    { instruction: LendingInstruction.RefreshObligation },
    data
  );

  const keys = [{ pubkey: obligation, isSigner: false, isWritable: true }];

  depositReserves.forEach((depositReserve) =>
    keys.push({
      pubkey: depositReserve,
      isSigner: false,
      isWritable: false,
    })
  );
  borrowReserves.forEach((borrowReserve) =>
    keys.push({
      pubkey: borrowReserve,
      isSigner: false,
      isWritable: false,
    })
  );
  return new TransactionInstruction({
    keys,
    programId: solendProgramAddress,
    data,
  });
};
