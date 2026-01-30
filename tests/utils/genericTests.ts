import type { AnchorProvider } from "@coral-xyz/anchor";
import { WrappedI80F48, wrappedI80F48toBigNumber } from "@mrgnlabs/mrgn-common";
import type { RawAccount } from "@solana/spl-token";
import { AccountLayout } from "@solana/spl-token";
import { PublicKey } from "@solana/web3.js";
import { BankrunProvider } from "anchor-bankrun";
import BigNumber from "bignumber.js";
import BN from "bn.js";
import { assert } from "chai";
import {
  BanksTransactionMeta,
  BanksTransactionResultWithMeta,
} from "solana-bankrun";
import {
  WrappedI68F60 as WrappedU68F60,
  wrappedU68F60toBigNumber,
} from "./kamino-utils";

/**
 * Shorthand for `assert.equal(a.toString(), b.toString())`
 * @param a
 * @param b
 */
export const assertKeysEqual = (a: PublicKey, b: PublicKey) => {
  assert.equal(a.toString(), b.toString());
};

/**
 * Shorthand for `assert.equal(a.toString(), PublicKey.default.toString())`
 * @param a
 */
export const assertKeyDefault = (a: PublicKey) => {
  assert.equal(a.toString(), PublicKey.default.toString());
};

/**
 * Shorthand for `assert.equal(a.toString(), b.toString())`
 * @param a - a BN
 * @param b - a BN or number
 */
export const assertBNEqual = (a: BN, b: BN | number) => {
  if (typeof b === "number") {
    b = new BN(b);
  }
  assert.equal(a.toString(), b.toString());
};

/**
 * Shorthand for assert that a BN is greater than another BN
 * @param a - The value to check
 * @param b - The value to compare against
 * @param message - Optional message to display if the assertion fails
 */
export const assertBNGreaterThan = (
  a: BN,
  b: BN | number,
  message?: string
) => {
  if (typeof b === "number") {
    b = new BN(b);
  }
  const aB = BigInt(a.toString());
  const bB = BigInt(b.toString());

  if (!(aB > bB)) {
    throw new Error(
      message || `Expected ${aB.toString()} to be greater than ${bB.toString()}`
    );
  }
};

/**
 * Shorthand to convert I80F48 to a string and compare against a BN, number, or other WrappedI80F48
 *
 * Generally, use `assertI80F48Approx` instead if the expected value is not a whole number or zero.
 * @param a
 * @param b
 */
export const assertI80F48Equal = (
  a: WrappedI80F48,
  b: WrappedI80F48 | BN | number
) => {
  const bigA = wrappedI80F48toBigNumber(a);
  let bigB: BigNumber;

  if (typeof b === "number") {
    bigB = new BigNumber(b);
  } else if (b instanceof BN) {
    bigB = new BigNumber(b.toString());
  } else if (isWrappedI80F48(b)) {
    bigB = wrappedI80F48toBigNumber(b);
  } else {
    throw new Error("Unsupported type for comparison");
  }

  if (bigA.isNaN() || bigB.isNaN()) {
    throw new Error("One of the values is NaN");
  }

  assert.equal(bigA.toString(), bigB.toString());
};

/**
 * Shorthand to convert I68F60 to a string and compare against a BN, number, or other WrappedI68F60
 *
 * Generally, use `assertI68F60Approx` instead if the expected value is not a whole number or zero.
 * @param a
 * @param b
 */
export const assertI68F60Equal = (
  a: WrappedU68F60 | BN,
  b: WrappedU68F60 | BN | number
) => {
  const bigA = wrappedU68F60toBigNumber(a);
  let bigB: BigNumber;

  if (typeof b === "number") {
    bigB = new BigNumber(b);
  } else if (b instanceof BN) {
    bigB = new BigNumber(b.toString());
  } else if (isWrappedU68F60(b)) {
    bigB = wrappedU68F60toBigNumber(b);
  } else {
    throw new Error("Unsupported type for comparison");
  }

  if (bigA.isNaN() || bigB.isNaN()) {
    throw new Error("One of the values is NaN");
  }

  assert.equal(bigA.toString(), bigB.toString());
};

/**
 * Shorthand to convert I80F48 to a BigNumber and compare against a BN, number, or other WrappedI80F48 within a given tolerance
 * @param a
 * @param b
 * @param tolerance - the allowed difference between the two values (default .000001)
 */
export const assertI80F48Approx = (
  a: WrappedI80F48,
  b: WrappedI80F48 | BN | number,
  tolerance: number = 0.000001
) => {
  const bigA = wrappedI80F48toBigNumber(a);
  let bigB: BigNumber;

  if (typeof b === "number") {
    bigB = new BigNumber(b);
  } else if (b instanceof BN) {
    bigB = new BigNumber(b.toString());
  } else if (isWrappedI80F48(b)) {
    bigB = wrappedI80F48toBigNumber(b);
  } else {
    throw new Error("Unsupported type for comparison");
  }

  if (bigA.isNaN() || bigB.isNaN()) {
    throw new Error("One of the values is NaN");
  }

  const diff = bigA.minus(bigB).abs();
  const allowedDifference = new BigNumber(tolerance);

  if (diff.isGreaterThan(allowedDifference)) {
    throw new Error(
      `Values are not approximately equal. A: ${bigA.toString()} B: ${bigB.toString()} 
      Difference: ${diff.toString()}, Allowed Tolerance: ${tolerance}`
    );
  }
};

/**
 * Shorthand to convert U68F60 to a BigNumber and compare against a BN, number, or other WrappedU68F60 within a given tolerance
 * @param a
 * @param b
 * @param tolerance - the allowed difference between the two values (default .000001)
 */
export const assertU68F60Approx = (
  a: WrappedU68F60 | BN,
  b: WrappedU68F60 | BN | number,
  tolerance: number = 0.000001
) => {
  const bigA = wrappedU68F60toBigNumber(a);
  let bigB: BigNumber;

  if (typeof b === "number") {
    bigB = new BigNumber(b);
  } else if (b instanceof BN) {
    bigB = new BigNumber(b.toString());
  } else if (isWrappedU68F60(b)) {
    bigB = wrappedU68F60toBigNumber(b);
  } else {
    throw new Error("Unsupported type for comparison");
  }

  if (bigA.isNaN() || bigB.isNaN()) {
    throw new Error("One of the values is NaN");
  }

  const diff = bigA.minus(bigB).abs();
  const allowedDifference = new BigNumber(tolerance);

  if (diff.isGreaterThan(allowedDifference)) {
    throw new Error(
      `Values are not approximately equal. A: ${bigA.toString()} B: ${bigB.toString()} 
      Difference: ${diff.toString()}, Allowed Tolerance: ${tolerance}`
    );
  }
};

/**
 * Type guard to check if a value is WrappedI80F48
 * @param value
 * @returns
 */
function isWrappedI80F48(value: any): value is WrappedI80F48 {
  return value && typeof value === "object" && Array.isArray(value.value);
}

/**
 * Type guard to check if a value is WrappedU68F60
 * @param value
 * @returns
 */
function isWrappedU68F60(value: any): value is WrappedU68F60 {
  return value && typeof value === "object" && Array.isArray(value.value);
}

/**
 * Shorthand for `assert.approximately(a, b, tolerance)` for two BNs. Safe from Integer overflow
 * @param a
 * @param b
 * @param tolerance
 */
export const assertBNApproximately = (
  a: BN,
  b: BN | number,
  tolerance: BN | number
) => {
  const aB = BigNumber(a.toString());
  const bB = BigNumber(b.toString());
  const toleranceB = BigNumber(tolerance.toString());
  const diff = aB.minus(bB).abs();

  if (diff.isGreaterThan(toleranceB)) {
    throw new Error(
      `Values are not approximately equal. A: ${aB.toString()} B: ${bB.toString()} 
      Difference: ${diff.toString()}, Allowed Tolerance: ${tolerance}`
    );
  }
};

/**
 * Returns the balance of a token account, in whatever currency the account is in.
 * @param provider
 * @param account
 * @returns
 */
export const getTokenBalance = async (
  provider: AnchorProvider | BankrunProvider,
  account: PublicKey
) => {
  const accountInfo = await provider.connection.getAccountInfo(account);
  if (!accountInfo) {
    console.error("Tried to balance of acc that doesn't exist");
    return 0;
  }
  const data: RawAccount = AccountLayout.decode(accountInfo.data);
  if (data === undefined || data.amount === undefined) {
    console.error("Token account has no data");
    console.error("owned by: " + accountInfo.owner);
    return 0;
  }
  const amount: BigInt = data.amount;
  return Number(amount);
};

/**
 * Waits until the given time
 * @param time - in seconds (e.g. Date.now()/1000)
 * @param silenceWarning - (optional) set to true to silence the warning if the time is in the past
 */
export const waitUntil = async (
  time: number,
  silenceWarning: boolean = false
) => {
  const now = Date.now() / 1000;
  if (time > now + 500) {
    console.error("Tried to wait a very long time, aborted");
    return;
  }
  if (now > time) {
    if (!silenceWarning) {
      console.error(
        "Tried to wait for a time that's in the past. You probably need to adjust test timings."
      );
      console.error("now: " + now + " and tried waiting until: " + time);
    }
    return new Promise((r) => setTimeout(r, 1)); //waits 1 ms
  }
  const toWait = Math.ceil(time - now) * 1000;
  await new Promise((r) => setTimeout(r, toWait));
};

/**
 * Assert a bankrun Tx executed with `tryProcessTransaction` failed with the expected error code.
 * Throws an error if the tx succeeded or a different error was found.
 * @param result
 * @param expectedErrorCode - In hex or human readable number, as you see in Anchor logs, e.g. for
 * error 6047 pass `0x179f` (a string) or 6047 (a number)
 */
export const assertBankrunTxFailed = (
  result: BanksTransactionResultWithMeta | BanksTransactionMeta,
  expectedErrorCode: string | number
) => {
  if (!("result" in result)) {
    throw new Error("TX succeeded when it should have failed");
  }

  // Convert decimal number to hex string if necessary,
  // otherwise assume it's already a hex string.
  const codeHex =
    typeof expectedErrorCode === "number"
      ? "0x" + expectedErrorCode.toString(16)
      : expectedErrorCode.toLocaleLowerCase();
  assert(result.meta && result.meta.logMessages, "no log");
  assert(result.meta.logMessages.length > 0, "empty log");
  assert(result.result, "TX succeeded when it should have failed");
  const lastLog = result.meta.logMessages.pop();
  assert(
    lastLog.includes(codeHex),
    "\nExpected code " + codeHex + " but got: " + lastLog
  );
};

/**
 * Typically used when catching the result of a tx sendAndConfirm. Asserts that the logs contain the
 * given error code.
 * @param logs
 * @param errorCode
 * @returns
 */
export function logContainsError(logs: string[], errorCode: string): boolean {
  if (!logs || !Array.isArray(logs)) {
    throw new Error("Invalid logs provided for verification:" + logs);
  }

  return logs.some((log) => log.includes(`Error Code: ${errorCode}`));
}

/**
 * Asserts that the contained transaction failed with the given error code. Fails if the tx did not
 * fail or fails with the wrong error code (if logs are undefined, searches for error number instead).
 *
 * Invalid if not awaited. MAKE SURE TO CALL WITH AWAIT.
 * @param transactionFn
 * @param errorCode - an enum value of the error, e.g. "CannotCloseOutstandingEmissions"
 * @param errorNumber - the error code as a number, e.g. 6033
 */
export async function expectFailedTxWithError(
  transactionFn: () => Promise<void>,
  errorCode: string,
  errorNumber: number
): Promise<void> {
  let failed = false;
  try {
    await transactionFn();
  } catch (err) {
    if (!err.logs || !Array.isArray(err.logs)) {
      // If logs are not available, check for error number
      const parsedNumber = extractCustomErrorCode(err.toString());
      assert.equal(
        parsedNumber,
        errorNumber,
        `Expected error code ${errorNumber} but got ${parsedNumber}`
      );
      return;
    }
    assert.ok(
      logContainsError(err.logs, errorCode),
      `Expected error code '${errorCode}' was not found in logs. Log dump: ${err.logs}`
    );
    failed = true;
  }
  assert.ok(failed, "Transaction succeeded when it should have failed");
}

function extractCustomErrorCode(errorMessage) {
  const match = errorMessage.match(/"Custom":\s*(\d+)/);
  return match ? parseInt(match[1], 10) : null;
}

/**
 * Asserts that the contained transaction failed with the given error code. Fails if the tx did not
 * fail or fails without containing the given string
 *
 * Invalid if not awaited. MAKE SURE TO CALL WITH AWAIT.
 *
 * Generally, use `expectFailedTxWithError` unless catching a generic error like a panic or
 * signature violations.
 * @param transactionFn
 * @param expectedString
 */
export async function expectFailedTxWithMessage(
  transactionFn: () => Promise<void>,
  expectedString: string
): Promise<void> {
  let failed = false;
  try {
    await transactionFn();
    console.log("tx exec");
  } catch (err) {
    let errString = err.toString();
    // Also check logs array if available (bankrun puts detailed logs there)
    const logsString = err.logs ? err.logs.join("\n") : "";
    const fullString = errString + " " + logsString;
    assert.ok(
      fullString.includes(expectedString),
      `Expected error code '${expectedString}' was not found in logs. Log dump: ${err} or ${err.logs}`
    );

    failed = true;
  }
  assert.ok(failed, "Transaction succeeded when it should have failed");
}

/**
 * Converts an APR value (as a number) into a `u32`-compatible integer representation,
 * assuming the maximum APR is 1000% (i.e. 10.0). Values above 1000% are clamped to u32::MAX.
 *
 * The mapping is linear: 0 → 0, 10.0 → u32::MAX.
 *
 * @param value - APR as a number (expected to be >= 0)
 * @returns A u32-style number in the range [0, 4294967295]
 */
export function aprToU32(value: number): number {
  const MAX_PERCENT = 10.0;
  const MAX_U32 = 0xffffffff; // 2^32 - 1

  const clamped = Math.min(value, MAX_PERCENT);
  const ratio = clamped / MAX_PERCENT;
  return Math.floor(ratio * MAX_U32);
}
