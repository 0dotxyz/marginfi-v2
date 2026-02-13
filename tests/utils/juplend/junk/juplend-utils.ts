// import { BN } from "@coral-xyz/anchor";
// import { PublicKey } from "@solana/web3.js";
// import BigNumber from "bignumber.js";

// import { bigNumberToWrappedI80F48, WrappedI80F48 } from "@mrgnlabs/mrgn-common";

// /**
//  * Compact bank config for the JupLend integration.
//  *
//  * Mirrors the Rust type: `JuplendConfigCompact`.
//  *
//  * Note: JupLend banks always start in `Paused` state. Only `juplend_init_position`
//  * can activate them to `Operational`.
//  */
// export interface JuplendConfigCompact {
//   oracle: PublicKey;

//   assetWeightInit: WrappedI80F48;
//   assetWeightMaint: WrappedI80F48;

//   depositLimit: BN;

//   /**
//    * Oracle setup enum.
//    *
//    * For JupLend we use either:
//    * - { juplendPythPull: {} }
//    * - { juplendSwitchboardPull: {} }
//    */
//   oracleSetup: { juplendPythPull: {} } | { juplendSwitchboardPull: {} };

//   riskTier: { collateral: {} } | { isolated: {} };
//   configFlags: number;

//   totalAssetValueInitLimit: BN;
//   oracleMaxAge: number;
//   oracleMaxConfidence: number;
// }

// /**
//  * Default JupLend bank config used in tests.
//  *
//  * Notes:
//  * - JupLend banks always start `Paused` and are activated via `juplendInitPosition`.
//  * - We use `juplendPythPull` oracle setup which expects:
//  *   - oracleKeys[0] = Pyth PriceUpdateV2
//  *   - oracleKeys[1] = JupLend `lending` state account
//  */
// export const defaultJuplendBankConfig = (
//   oracle: PublicKey,
//   decimals: number
// ): JuplendConfigCompact => {
//   return {
//     oracle,
//     assetWeightInit: bigNumberToWrappedI80F48(new BigNumber(0.8)),
//     assetWeightMaint: bigNumberToWrappedI80F48(new BigNumber(0.9)),
//     depositLimit: new BN(1_000_000).mul(new BN(10).pow(new BN(decimals))),
//     oracleSetup: { juplendPythPull: {} },
//     riskTier: { collateral: {} },
//     configFlags: 1,
//     totalAssetValueInitLimit: new BN(1_000_000_000).mul(
//       new BN(10).pow(new BN(decimals))
//     ),
//     oracleMaxAge: 60,
//     oracleMaxConfidence: 0,
//   };
// };
