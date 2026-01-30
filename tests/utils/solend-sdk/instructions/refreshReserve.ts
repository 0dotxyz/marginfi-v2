import {
  PublicKey,
  TransactionInstruction,
  SYSVAR_CLOCK_PUBKEY,
} from "@solana/web3.js";
import { LendingInstruction } from "../instruction";
const BufferLayout = require("buffer-layout");
/// Accrue interest and update market price of liquidity on a reserve.
///
/// Accounts expected by this instruction:
///
///   0. `[writable]` Reserve account.
///   1. `[]` Clock sysvar.
///   2. `[optional]` Reserve liquidity oracle account.
///                     Required if the reserve currency is not the lending market quote
///                     currency.
export const refreshReserveInstruction = (
  reserve: PublicKey,
  solendProgramAddress: PublicKey,
  oracle: PublicKey,
  switchboardFeedAddress?: PublicKey,
  extraOracle?: PublicKey
): TransactionInstruction => {
  const dataLayout = BufferLayout.struct([BufferLayout.u8("instruction")]);

  const data = Buffer.alloc(dataLayout.span);
  dataLayout.encode({ instruction: LendingInstruction.RefreshReserve }, data);

  const keys = [{ pubkey: reserve, isSigner: false, isWritable: true }];

  keys.push({ pubkey: oracle, isSigner: false, isWritable: false });

  if (switchboardFeedAddress) {
    keys.push({
      pubkey: switchboardFeedAddress,
      isSigner: false,
      isWritable: false,
    });
  }

  if (extraOracle) {
    keys.push({
      pubkey: extraOracle,
      isSigner: false,
      isWritable: false,
    });
  }

  return new TransactionInstruction({
    keys,
    programId: solendProgramAddress,
    data,
  });
};
