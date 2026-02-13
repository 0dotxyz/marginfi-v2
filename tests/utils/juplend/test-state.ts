export const JUPLEND_STATE_KEYS = {
  jlr01Group: "jlr01_group",
  jlr01BankUsdc: "jlr01_bank_usdc",
  jlr01BankTokenA: "jlr01_bank_token_a",
  jlr01BankWsol: "jlr01_bank_wsol",
} as const;

export const jlr01BankStateKey = (bankName: string) => {
  if (bankName === "USDC") return JUPLEND_STATE_KEYS.jlr01BankUsdc;
  if (bankName === "TokenA") return JUPLEND_STATE_KEYS.jlr01BankTokenA;
  if (bankName === "WSOL") return JUPLEND_STATE_KEYS.jlr01BankWsol;

  throw new Error(`Unsupported jlr01 bank name: ${bankName}`);
};
