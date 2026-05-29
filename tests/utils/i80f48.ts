import type { WrappedI80F48 } from "@mrgnlabs/mrgn-common";

const I80F48_FRACTIONAL_BITS = 48n;
const I80F48_TOTAL_BITS = 128n;
const I80F48_SCALE = 1n << I80F48_FRACTIONAL_BITS;
const I80F48_MOD = 1n << I80F48_TOTAL_BITS;

const expandExponential = (value: string): string => {
  if (!/[eE]/.test(value)) {
    return value;
  }

  const sign = value.startsWith("-") ? "-" : "";
  const unsigned = value.replace(/^[+-]/, "");
  const [coefficient, exponentRaw] = unsigned.toLowerCase().split("e");
  const exponent = Number(exponentRaw);
  if (!Number.isInteger(exponent)) {
    throw new Error(`Invalid decimal exponent: ${value}`);
  }

  const [integerPart, fractionalPart = ""] = coefficient.split(".");
  const digits = `${integerPart}${fractionalPart}`.replace(/^0+(?=\d)/, "");
  const decimalIndex = integerPart.length + exponent;

  if (decimalIndex <= 0) {
    return `${sign}0.${"0".repeat(-decimalIndex)}${digits}`;
  }

  if (decimalIndex >= digits.length) {
    return `${sign}${digits}${"0".repeat(decimalIndex - digits.length)}`;
  }

  return `${sign}${digits.slice(0, decimalIndex)}.${digits.slice(
    decimalIndex,
  )}`;
};

const decimalToScaledI80 = (
  value: number | string | { toString(): string },
): bigint => {
  const normalized = expandExponential(value.toString().trim());
  if (!/^[+-]?\d+(\.\d+)?$/.test(normalized)) {
    throw new Error(`Invalid decimal value: ${value.toString()}`);
  }

  const isNegative = normalized.startsWith("-");
  const unsigned = normalized.replace(/^[+-]/, "");
  const [integerPart, fractionalPart = ""] = unsigned.split(".");
  const digits = `${integerPart}${fractionalPart}`.replace(/^0+(?=\d)/, "");
  const numerator = BigInt(digits || "0") * I80F48_SCALE;
  const denominator = 10n ** BigInt(fractionalPart.length);

  let scaled = numerator / denominator;
  const remainder = numerator % denominator;
  if (remainder * 2n >= denominator) {
    scaled += 1n;
  }

  return isNegative ? -scaled : scaled;
};

export const bigNumberToWrappedI80F48 = (
  value: number | string | { toString(): string },
): WrappedI80F48 => {
  let scaled = decimalToScaledI80(value);
  if (scaled < 0) {
    scaled = I80F48_MOD + scaled;
  }

  const bytes: number[] = [];
  for (let i = 0n; i < I80F48_TOTAL_BITS / 8n; i++) {
    bytes.push(Number((scaled >> (8n * i)) & 0xffn));
  }

  return { value: bytes };
};
