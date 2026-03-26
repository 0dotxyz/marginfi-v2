export const INTEGRATION_PROTOCOL_ACCOUNT_COUNTS = {
  drift: {
    deposit: 8,
    withdraw: 15,
  },
  juplend: {
    deposit: 14,
    withdraw: 16,
  },
  kamino: {
    deposit: 13,
    withdraw: 13,
  },
  solend: {
    deposit: 11,
    withdraw: 9,
  },
} as const;

export const assertProtocolAccountCount = (
  integration: string,
  direction: "deposit" | "withdraw",
  count: number,
  expected: number,
): void => {
  if (count !== expected) {
    throw new Error(
      `${integration} ${direction} protocol account count mismatch: expected ${expected}, got ${count}`,
    );
  }
};
