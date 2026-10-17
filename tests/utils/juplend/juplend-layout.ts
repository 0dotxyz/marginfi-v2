import { PublicKey } from "@solana/web3.js";

export enum JuplendOracleSetupType {
  FixedJuplend = 0,
  JuplendPythPull = 1,
  JuplendSwitchboardPull = 2,
  ScopeJuplend = 3,
  JuplendMSOL = 4,
  JuplendLST = 5,
}

export interface JuplendLayout {
  lending: number;
  liquidity: number;
  len: number;
}

export function getJuplendLayout(setup: JuplendOracleSetupType): JuplendLayout {
  switch (setup) {
    case JuplendOracleSetupType.FixedJuplend:
      return {
        lending: 0,
        liquidity: 1,
        len: 2,
      };
    case JuplendOracleSetupType.JuplendPythPull:
    case JuplendOracleSetupType.JuplendSwitchboardPull:
    case JuplendOracleSetupType.ScopeJuplend:
      return {
        lending: 1,
        liquidity: 2,
        len: 3,
      };
    case JuplendOracleSetupType.JuplendMSOL:
    case JuplendOracleSetupType.JuplendLST:
      return {
        lending: 1,
        liquidity: 3,
        len: 4,
      };
  }
}

/**
 * Centralized JupLend layout constants for all supported setups.
 * Built from the Rust definitions via getJuplendLayout().
 */
export const JUPLEND_LAYOUTS = {
  [JuplendOracleSetupType.FixedJuplend]: getJuplendLayout(JuplendOracleSetupType.FixedJuplend),
  [JuplendOracleSetupType.JuplendPythPull]: getJuplendLayout(JuplendOracleSetupType.JuplendPythPull),
  [JuplendOracleSetupType.JuplendSwitchboardPull]: getJuplendLayout(JuplendOracleSetupType.JuplendSwitchboardPull),
  [JuplendOracleSetupType.ScopeJuplend]: getJuplendLayout(JuplendOracleSetupType.ScopeJuplend),
  [JuplendOracleSetupType.JuplendMSOL]: getJuplendLayout(JuplendOracleSetupType.JuplendMSOL),
  [JuplendOracleSetupType.JuplendLST]: getJuplendLayout(JuplendOracleSetupType.JuplendLST),
};

/**
 * Builds the required oracle accounts for a JupLend bank.
 * Filters out default pubkeys and appends the liquidity PDA.
 *
 * @param bank - Bank account with config.oracleKeys
 * @param liquidityPda - JupLend Liquidity PDA
 * @returns Array of oracle keys (filtered) + liquidity PDA
 *
 * Usage:
 *   const oracleAccounts = buildJuplendOracleAccounts(bank, liquidityPda);
 *   const remaining = [bankPk, ...oracleAccounts];
 */
export function buildJuplendOracleAccounts(
  bank: { config: { oracleKeys: PublicKey[] } },
  liquidityPda: PublicKey,
): PublicKey[] {
  return [
    ...bank.config.oracleKeys.filter((key) => !key.equals(PublicKey.default)),
    liquidityPda,
  ];
}
