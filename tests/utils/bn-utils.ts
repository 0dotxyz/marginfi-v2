import BN from "bn.js";
import BigNumber from "bignumber.js";
import type { WrappedI80F48 } from "@mrgnlabs/mrgn-common";

export type IntegerLike = BN | number | bigint;
export type NumericLike = BN | number | bigint;
export type I80F48Like = WrappedI80F48 | IntegerLike;

export const I80F48_FRACTIONAL_BITS = 48n;
export const I80F48_TOTAL_BITS = 128n;
export const I80F48_SCALE = 1n << I80F48_FRACTIONAL_BITS;
const I80F48_MOD = 1n << I80F48_TOTAL_BITS;

export const bnToBigIntSafe = (value: BN): bigint => {
  const bytes = Uint8Array.from(value.abs().toArray("be"));
  let out = 0n;
  for (const byte of bytes) {
    out = (out << 8n) | BigInt(byte);
  }
  return value.isNeg() ? -out : out;
};

export const bnToDecimalStringSafe = (value: BN): string =>
  bnToBigIntSafe(value).toString();

export const integerToBigInt = (value: IntegerLike): bigint => {
  if (typeof value === "bigint") return value;
  if (typeof value === "number") {
    if (!Number.isSafeInteger(value)) {
      throw new Error(`Unsafe integer value: ${value}`);
    }
    return BigInt(value);
  }
  return bnToBigIntSafe(value);
};

export const integerToBigNumber = (value: IntegerLike): BigNumber =>
  new BigNumber(integerToBigInt(value).toString());

export const numericToBigNumber = (value: NumericLike): BigNumber => {
  if (typeof value === "number") {
    if (!Number.isFinite(value)) {
      throw new Error(`Invalid numeric value: ${value}`);
    }
    return new BigNumber(value.toString());
  }
  return integerToBigNumber(value);
};

export const isWrappedI80F48 = (value: unknown): value is WrappedI80F48 => {
  if (!value || typeof value !== "object") return false;
  if (!("value" in value)) return false;
  const bytes = (value as { value?: unknown }).value;
  return Array.isArray(bytes) || ArrayBuffer.isView(bytes);
};

const bytesToUnsignedBigIntLE = (bytesLike: number[] | Uint8Array): bigint => {
  const bytes = Array.isArray(bytesLike) ? bytesLike : Array.from(bytesLike);
  let value = 0n;
  for (let i = 0; i < bytes.length; i++) {
    value |= BigInt(bytes[i] & 0xff) << (8n * BigInt(i));
  }
  return value;
};

const decimalToI80Scaled = (value: number): bigint => {
  if (!Number.isFinite(value)) {
    throw new Error(`Unsupported numeric value for I80F48: ${value}`);
  }
  if (Number.isInteger(value)) {
    return BigInt(value) * I80F48_SCALE;
  }

  const scaled = new BigNumber(value.toString())
    .times(new BigNumber(2).pow(Number(I80F48_FRACTIONAL_BITS)))
    .integerValue(BigNumber.ROUND_HALF_UP);
  return BigInt(scaled.toString());
};

export const toI80Scaled = (value: I80F48Like): bigint => {
  if (isWrappedI80F48(value)) {
    const bytes = Array.from(value.value);
    if (bytes.length !== 16) {
      throw new Error(`Invalid WrappedI80F48 length: ${bytes.length}`);
    }

    const raw = bytesToUnsignedBigIntLE(bytes);
    const signBit = 1n << (I80F48_TOTAL_BITS - 1n);
    return raw & signBit ? raw - I80F48_MOD : raw;
  }

  if (typeof value === "number") {
    return decimalToI80Scaled(value);
  }

  return integerToBigInt(value) * I80F48_SCALE;
};

export const fromI80Scaled = (scaled: bigint): WrappedI80F48 => {
  if (
    scaled < -(1n << (I80F48_TOTAL_BITS - 1n)) ||
    scaled >= 1n << (I80F48_TOTAL_BITS - 1n)
  ) {
    throw new Error(`I80F48 scaled value out of range: ${scaled.toString()}`);
  }

  let raw = scaled < 0 ? scaled + I80F48_MOD : scaled;
  const bytes: number[] = new Array(16);
  for (let i = 0; i < 16; i++) {
    bytes[i] = Number(raw & 0xffn);
    raw >>= 8n;
  }

  return { value: bytes };
};

export const mulI80 = (lhsScaled: bigint, rhsScaled: bigint): bigint =>
  (lhsScaled * rhsScaled) >> I80F48_FRACTIONAL_BITS;

export const divI80 = (lhsScaled: bigint, rhsScaled: bigint): bigint =>
  (lhsScaled << I80F48_FRACTIONAL_BITS) / rhsScaled;

export const nativeToI80Scaled = (native: BN): bigint =>
  integerToBigInt(native) * I80F48_SCALE;

export const addNativeAmountToI80 = (
  base: WrappedI80F48 | null | undefined,
  amount: BN
): WrappedI80F48 =>
  fromI80Scaled((base ? toI80Scaled(base) : 0n) + nativeToI80Scaled(amount));

/** Shorthand to convert an I80F48 to BN, truncating fractional ticks. */
export const toBnFromI80 = (value: WrappedI80F48): BN =>
  new BN((toI80Scaled(value) >> I80F48_FRACTIONAL_BITS).toString());

/** Shorthand to cast BN/number/bigint as BN. */
export const toBn = (value: IntegerLike): BN => {
  if (typeof value === "bigint") return new BN(value.toString());
  if (typeof value === "number") return new BN(value);
  return value;
};
