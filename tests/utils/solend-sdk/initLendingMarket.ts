import { TOKEN_PROGRAM_ID } from "@solana/spl-token";
import {
  PublicKey,
  SYSVAR_RENT_PUBKEY,
  TransactionInstruction,
} from "@solana/web3.js";
import * as Layout from "./layout";
import { LendingInstruction } from "./instruction";

const BufferLayout = require("buffer-layout");

export const initLendingMarketInstruction = (
  owner: PublicKey,
  quoteCurrency: Buffer,
  lendingMarket: PublicKey,
  lendingProgramId: PublicKey,
  oracleProgramId: PublicKey,
  switchboardProgramId: PublicKey
): TransactionInstruction => {
  const dataLayout = BufferLayout.struct([
    BufferLayout.u8("instruction"),
    Layout.publicKey("owner"),
    BufferLayout.blob(32, "quoteCurrency"),
  ]);

  const data = Buffer.alloc(dataLayout.span);
  dataLayout.encode(
    {
      instruction: LendingInstruction.InitLendingMarket,
      owner,
      quoteCurrency,
    },
    data
  );

  const keys = [
    { pubkey: lendingMarket, isSigner: false, isWritable: true },
    { pubkey: SYSVAR_RENT_PUBKEY, isSigner: false, isWritable: false },
    { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
    { pubkey: oracleProgramId, isSigner: false, isWritable: false },
    { pubkey: switchboardProgramId, isSigner: false, isWritable: false },
  ];

  return new TransactionInstruction({
    keys,
    programId: lendingProgramId,
    data,
  });
};
