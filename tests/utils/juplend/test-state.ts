// Helps alias in which test an account originates and gives it a cleaner name
export const JUPLEND_STATE_KEYS = {
  jlr01Group: "jlr01_group",
  jlr01BankUsdc: "jlr01_bank_usdc",
  jlr01BankTokenA: "jlr01_bank_token_a",
  jlr01BankWsol: "jlr01_bank_wsol",
  jlr01RegularBankUsdc: "jlr01_regular_bank_usdc",
  jlr01RegularBankTokenB: "jlr01_regular_bank_token_b",
  jlr01RegularBankLst: "jlr01_regular_bank_lst",
  jlr02User0MarginfiAccount: "jlr02_user0_marginfi_account",
  jlr05User1MarginfiAccount: "jlr05_user1_marginfi_account",
} as const;

export const jlr01BankStateKey = (bankName: string) => {
  if (bankName === "USDC") return JUPLEND_STATE_KEYS.jlr01BankUsdc;
  if (bankName === "TokenA") return JUPLEND_STATE_KEYS.jlr01BankTokenA;
  if (bankName === "WSOL") return JUPLEND_STATE_KEYS.jlr01BankWsol;

  throw new Error(`Unsupported jlr01 bank name: ${bankName}`);
};

export const jlr01RegularBankStateKey = (bankName: string) => {
  if (bankName === "USDC") return JUPLEND_STATE_KEYS.jlr01RegularBankUsdc;
  if (bankName === "TokenB") return JUPLEND_STATE_KEYS.jlr01RegularBankTokenB;
  if (bankName === "LST") return JUPLEND_STATE_KEYS.jlr01RegularBankLst;

  throw new Error(`Unsupported jlr01 regular bank name: ${bankName}`);
};
