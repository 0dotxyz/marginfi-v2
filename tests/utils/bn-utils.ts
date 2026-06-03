import BN from "bn.js";

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
