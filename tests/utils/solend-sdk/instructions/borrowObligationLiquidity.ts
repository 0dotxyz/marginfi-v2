import { PublicKey, TransactionInstruction } from "@solana/web3.js";
import BN from "bn.js";
import { TOKEN_PROGRAM_ID } from "@solana/spl-token";
import * as Layout from "../layout";
import { LendingInstruction } from "../instruction";

const BufferLayout = require("buffer-layout");

/// Borrow liquidity from a reserve by depositing collateral tokens. Requires a refreshed
/// obligation and reserve.
///
/// Accounts expected by this instruction:
///
///   0. `[writable]` Source borrow reserve liquidity supply SPL Token account.
///   1. `[writable]` Destination liquidity token account.
///                     Minted by borrow reserve liquidity mint.
///   2. `[writable]` Borrow reserve account - refreshed.
///   3. `[writable]` Borrow reserve liquidity fee receiver account.
///                     Must be the fee account specified at InitReserve.
///   4. `[writable]` Obligation account - refreshed.
///   5. `[]` Lending market account.
///   6. `[]` Derived lending market authority.
///   7. `[signer]` Obligation owner.
///   8. `[]` Clock sysvar.
///   9. `[]` Token program id.
///   10 `[optional, writable]` Host fee receiver account.
export const borrowObligationLiquidityInstruction = (
  liquidityAmount: number | BN,
  sourceLiquidity: PublicKey,
  destinationLiquidity: PublicKey,
  borrowReserve: PublicKey,
  borrowReserveLiquidityFeeReceiver: PublicKey,
  obligation: PublicKey,
  lendingMarket: PublicKey,
  lendingMarketAuthority: PublicKey,
  obligationOwner: PublicKey,
  solendProgramAddress: PublicKey,
  depositReserves: Array<PublicKey>,
  hostFeeReceiver?: PublicKey
): TransactionInstruction => {
  const dataLayout = BufferLayout.struct([
    BufferLayout.u8("instruction"),
    Layout.uint64("liquidityAmount"),
  ]);

  const data = Buffer.alloc(dataLayout.span);
  dataLayout.encode(
    {
      instruction: LendingInstruction.BorrowObligationLiquidity,
      liquidityAmount: new BN(liquidityAmount),
    },
    data
  );

  const keys = [
    { pubkey: sourceLiquidity, isSigner: false, isWritable: true },
    { pubkey: destinationLiquidity, isSigner: false, isWritable: true },
    { pubkey: borrowReserve, isSigner: false, isWritable: true },
    {
      pubkey: borrowReserveLiquidityFeeReceiver,
      isSigner: false,
      isWritable: true,
    },
    { pubkey: obligation, isSigner: false, isWritable: true },
    { pubkey: lendingMarket, isSigner: false, isWritable: true },
    { pubkey: lendingMarketAuthority, isSigner: false, isWritable: false },
    { pubkey: obligationOwner, isSigner: true, isWritable: false },
    { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
    ...depositReserves.map((reserve) => ({
      pubkey: reserve,
      isSigner: false,
      isWritable: true,
    })),
  ];

  if (hostFeeReceiver) {
    keys.push({ pubkey: hostFeeReceiver, isSigner: false, isWritable: true });
  }

  return new TransactionInstruction({
    keys,
    programId: solendProgramAddress,
    data,
  });
};
