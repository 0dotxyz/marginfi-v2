// Helps alias in which test an account originates and gives it a cleaner name
export const JUPLEND_STATE_KEYS = {
  jlr01Group: "jlr01_group",
  jlr01BankUsdc: "jlr01_bank_usdc",
  jlr01BankTokenA: "jlr01_bank_token_a",
  jlr01BankWsol: "jlr01_bank_wsol",
  jlr01RegularBankUsdc: "jlr01_regular_bank_usdc",
  jlr01RegularBankTokenB: "jlr01_regular_bank_token_b",
  jlr01RegularBankLst: "jlr01_regular_bank_lst",
  jlr01LookupTable: "jlr01_lookup_table",
  jlr02User0MarginfiAccount: "jlr02_user0_marginfi_account",
  jlr05User1MarginfiAccount: "jlr05_user1_marginfi_account",
  jlr07BankUsdcFixed: "jlr07_bank_usdc_fixed",
  jlr08BankUsdcSwitchboard: "jlr08_swb_usdc",
  jlr12BankMsolJuplend: "jlr12_bank_msol_juplend",
  jlr12BankLstJuplend: "jlr12_bank_lst_juplend",
  jlr13BankUsdcScope: "jlr13_bank_usdc_scope",
  jlr14BankMsolJuplend: "jlr14_bank_msol_juplend",
  jlr14BankLstJuplend: "jlr14_bank_lst_juplend",
} as const;

const JLR01_BANK_STATE_LOOKUP: Record<string, string> = {
  USDC: JUPLEND_STATE_KEYS.jlr01BankUsdc,
  TOKENA: JUPLEND_STATE_KEYS.jlr01BankTokenA,
  WSOL: JUPLEND_STATE_KEYS.jlr01BankWsol,
};

export const jlr01BankStateKey = (bankName: string) => {
  const upper = bankName.toUpperCase();
  const key = JLR01_BANK_STATE_LOOKUP[upper];
  if (key) return key;
  throw new Error(`Unsupported jlr01 bank name: ${bankName}`);
};

const JLR01_REGULAR_BANK_STATE_LOOKUP: Record<string, string> = {
  USDC: JUPLEND_STATE_KEYS.jlr01RegularBankUsdc,
  TOKENB: JUPLEND_STATE_KEYS.jlr01RegularBankTokenB,
  LST: JUPLEND_STATE_KEYS.jlr01RegularBankLst,
};

export const jlr01RegularBankStateKey = (bankName: string) => {
  const upper = bankName.toUpperCase();
  const key = JLR01_REGULAR_BANK_STATE_LOOKUP[upper];
  if (key) return key;
  throw new Error(`Unsupported jlr01 regular bank name: ${bankName}`);
};
