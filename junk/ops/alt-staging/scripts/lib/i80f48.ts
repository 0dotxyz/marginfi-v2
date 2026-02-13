/**
 * Minimal I80F48 encoder compatible with the on-chain `WrappedI80F48` (16-byte bytemuck).
 *
 * Marginfi uses I80F48 (signed i128 with 48 fractional bits) for many config values.
 */

export type WrappedI80F48 = { value: number[] };

const SCALE = 1n << 48n;
const TWO_POW_128 = 1n << 128n;

export const I80F48_ZERO: WrappedI80F48 = { value: new Array(16).fill(0) };
export const I80F48_ONE: WrappedI80F48 = i80f48FromString("1");

/**
 * Convert a decimal string (e.g. "0.8", "1.25") into WrappedI80F48.
 *
 * Notes:
 * - This performs *floor* rounding for the fractional part.
 * - Supports negative values.
 */
export function i80f48FromString(input: string): WrappedI80F48 {
  const s = input.trim();
  if (s.length === 0) {
    throw new Error("i80f48FromString: empty string");
  }

  // Basic scientific notation fallback
  if (/[eE]/.test(s)) {
    return i80f48FromNumber(Number(s));
  }

  let negative = false;
  let t = s;
  if (t.startsWith("-")) {
    negative = true;
    t = t.slice(1);
  } else if (t.startsWith("+")) {
    t = t.slice(1);
  }

  const [intPartRaw, fracPartRaw = ""] = t.split(".");
  const intPart = intPartRaw === "" ? "0" : intPartRaw;
  if (!/^\d+$/.test(intPart)) {
    throw new Error(`i80f48FromString: invalid integer part: ${input}`);
  }

  let scaled = BigInt(intPart) * SCALE;

  const fracPart = fracPartRaw.replace(/_/g, "");
  if (fracPart.length > 0) {
    if (!/^\d+$/.test(fracPart)) {
      throw new Error(`i80f48FromString: invalid fractional part: ${input}`);
    }
    const fracInt = BigInt(fracPart);
    const denom = 10n ** BigInt(fracPart.length);
    scaled += (fracInt * SCALE) / denom;
  }

  if (negative) scaled = -scaled;

  return { value: i128ToLeBytes(scaled) };
}

/**
 * Convert a JS number into WrappedI80F48.
 *
 * Prefer i80f48FromString for exact decimals.
 */
export function i80f48FromNumber(n: number): WrappedI80F48 {
  if (!Number.isFinite(n)) {
    throw new Error(`i80f48FromNumber: non-finite number: ${n}`);
  }
  // Convert through string for readability; this is still lossy for some decimals.
  return i80f48FromString(n.toString());
}

function i128ToLeBytes(value: bigint): number[] {
  // two's complement for negative values
  let v = value;
  if (v < 0) {
    v = TWO_POW_128 + v;
  }

  const out: number[] = new Array(16);
  for (let i = 0; i < 16; i++) {
    out[i] = Number((v >> BigInt(8 * i)) & 0xffn);
  }
  return out;
}
