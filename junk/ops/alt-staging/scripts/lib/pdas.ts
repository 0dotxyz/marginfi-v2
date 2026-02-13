import { BN } from "@coral-xyz/anchor";
import { PublicKey } from "@solana/web3.js";

export function deriveFeeState(programId: PublicKey): PublicKey {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("feestate", "utf-8")],
    programId
  )[0];
}

export function deriveBankWithSeed(
  programId: PublicKey,
  group: PublicKey,
  bankMint: PublicKey,
  bankSeed: BN
): PublicKey {
  return PublicKey.findProgramAddressSync(
    [group.toBuffer(), bankMint.toBuffer(), u64le(bankSeed)],
    programId
  )[0];
}

export function deriveLiquidityVaultAuthority(programId: PublicKey, bank: PublicKey): PublicKey {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("liquidity_vault_auth", "utf-8"), bank.toBuffer()],
    programId
  )[0];
}

export function deriveLiquidityVault(programId: PublicKey, bank: PublicKey): PublicKey {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("liquidity_vault", "utf-8"), bank.toBuffer()],
    programId
  )[0];
}

export function deriveInsuranceVaultAuthority(programId: PublicKey, bank: PublicKey): PublicKey {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("insurance_vault_auth", "utf-8"), bank.toBuffer()],
    programId
  )[0];
}

export function deriveInsuranceVault(programId: PublicKey, bank: PublicKey): PublicKey {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("insurance_vault", "utf-8"), bank.toBuffer()],
    programId
  )[0];
}

export function deriveFeeVaultAuthority(programId: PublicKey, bank: PublicKey): PublicKey {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("fee_vault_auth", "utf-8"), bank.toBuffer()],
    programId
  )[0];
}

export function deriveFeeVault(programId: PublicKey, bank: PublicKey): PublicKey {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("fee_vault", "utf-8"), bank.toBuffer()],
    programId
  )[0];
}

/**
 * Derive the marginfi account PDA used by `marginfiAccountInitializePda`.
 */
export function deriveMarginfiAccountPda(
  programId: PublicKey,
  group: PublicKey,
  authority: PublicKey,
  accountIndex: number,
  thirdPartyId: number
): PublicKey {
  const accountIndexBuf = u16le(accountIndex);
  const thirdPartyBuf = u16le(thirdPartyId);

  return PublicKey.findProgramAddressSync(
    [
      Buffer.from("marginfi_account", "utf-8"),
      group.toBuffer(),
      authority.toBuffer(),
      accountIndexBuf,
      thirdPartyBuf,
    ],
    programId
  )[0];
}

function u64le(n: BN): Buffer {
  // Anchor uses u64 seeds as 8-byte LE buffers
  return n.toArrayLike(Buffer, "le", 8);
}

function u16le(n: number): Buffer {
  if (!Number.isInteger(n) || n < 0 || n > 65535) {
    throw new Error(`u16le: value out of range: ${n}`);
  }
  const b = Buffer.alloc(2);
  b.writeUInt16LE(n, 0);
  return b;
}
