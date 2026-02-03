use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar::instructions as ix_sysvar;
use anchor_lang::solana_program::sysvar::instructions::load_instruction_at_checked;
use fixed::types::I80F48;
use fixed_macro::types::I80F48;
use pyth_solana_receiver_sdk::price_update::VerificationLevel;

use crate::MarginfiResult;

// This file should only contain the constants which couldn't be moved to type-crate:
// 1. the constants used for testing/internal purposes
// 2. or the ones dependant on some 3rd party crates which are not part of type-crate dependency tree

/// Mocks program ID for third-party ID restrictions
pub const MOCKS_PROGRAM_ID: Pubkey = pubkey!("rec5EKMGg6MxZYaMdyBfgwp4d5rB9T1VQH5pJv5LtFJ");

pub const DRIFT_USER_SEED: &str = "user";
pub const DRIFT_USER_STATS_SEED: &str = "user_stats";

pub const SOLEND_OBLIGATION_SEED: &str = "solend_obligation";

/// Used for the health cache to track which version of the program generated it.
/// * 0 = invalid
/// * 1 = 0.1.3
/// * 2 = 0.1.4
/// * 3 = 0.1.5
/// * others = invalid
pub const PROGRAM_VERSION: u8 = 3;

cfg_if::cfg_if! {
    if #[cfg(feature = "devnet")] {
        pub const PYTH_ID: Pubkey = pubkey!("gSbePebfvPy7tRqimPoVecS2UsBvYv46ynrzWocc92s");
    } else if #[cfg(any(feature = "mainnet-beta", feature = "staging", feature = "stagingalt"))] {
        pub const PYTH_ID: Pubkey = pubkey!("FsJ3A3u2vn5cTVofAjvy6y5kwABJAqYWpe4975bi2epH");
        // NOTE: There is some weirdness around addresses used by Drift and Solend integrations
        // that we haven't quite figured out yet. The mock program key below may be needed:
        // pub const PYTH_ID: Pubkey = pubkey!("rec5EKMGg6MxZYaMdyBfgwp4d5rB9T1VQH5pJv5LtFJ");
    } else {
        // The key of the mock program on localnet (see its declared id)
        pub const PYTH_ID: Pubkey = pubkey!("rec5EKMGg6MxZYaMdyBfgwp4d5rB9T1VQH5pJv5LtFJ");
    }
}

// TODO update to the actual deployment key on mainnet/devnet/staging
cfg_if::cfg_if! {
    if #[cfg(feature = "devnet")] {
        pub const SPL_SINGLE_POOL_ID: Pubkey = pubkey!("SVSPxpvHdN29nkVg9rPapPNDddN5DipNLRUFhyjFThE");
    } else if #[cfg(any(feature = "mainnet-beta", feature = "staging", feature = "stagingalt"))] {
        pub const SPL_SINGLE_POOL_ID: Pubkey = pubkey!("SVSPxpvHdN29nkVg9rPapPNDddN5DipNLRUFhyjFThE");
    } else {
        pub const SPL_SINGLE_POOL_ID: Pubkey = pubkey!("SVSPxpvHdN29nkVg9rPapPNDddN5DipNLRUFhyjFThE");
    }
}

cfg_if::cfg_if! {
    if #[cfg(feature = "devnet")] {
        pub const SWITCHBOARD_PULL_ID: Pubkey = pubkey!("Aio4gaXjXzJNVLtzwtNVmSqGKpANtXhybbkhtAC94ji2");
    } else {
        pub const SWITCHBOARD_PULL_ID: Pubkey = pubkey!("SBondMDrcV3K4kxZR1HNVT7osZxAHVHgYXL5Ze1oMUv");
    }
}

pub const COMPUTE_PROGRAM_KEY: Pubkey = pubkey!("ComputeBudget111111111111111111111111111111");
pub const JUP_KEY: Pubkey = pubkey!("JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4");
pub const TITAN_KEY: Pubkey = pubkey!("T1TANpTeScyeqVzzgNViGDNrkQ6qHz9KrSBS4aNXvGT");
pub const ASSOCIATED_TOKEN_KEY: Pubkey = pubkey!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");

// Note: We mock Kamino/Kamino Farms with the same keys on localnet
pub const KAMINO_PROGRAM_ID: Pubkey = pubkey!("KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD");
pub const FARMS_PROGRAM_ID: Pubkey = pubkey!("FarmsPZpWu9i7Kky8tPN37rs2TpmMrAZrC7S7vJa91Hr");

pub const DRIFT_PROGRAM_ID: Pubkey = pubkey!("dRiftyHA39MWEi3m9aunc5MzRF1JYuBsbn6VPcn33UH");

/// Drift uses a fixed 9 decimal precision for all spot market scaled balances,
/// regardless of the underlying token's decimals
pub const DRIFT_SCALED_BALANCE_DECIMALS: u8 = 9;

pub const SOLEND_PROGRAM_ID: Pubkey = pubkey!("So1endDq2YkqhipRh3WViPa8hdiSpxWy6z3Z6tMCpAo");
pub const NATIVE_STAKE_ID: Pubkey = pubkey!("Stake11111111111111111111111111111111111111");

/// The default fee, in native SOL in native decimals (i.e. lamports) used in testing
pub const INIT_BANK_ORIGINATION_FEE_DEFAULT: u32 = 10000;
/// The default fee, in native SOL in native decimals (i.e. lamports) used in testing
pub const LIQUIDATION_FLAT_FEE_DEFAULT: u32 = 5000;
/// Liquidators can claim at least this premium, as a percent, when liquidating an asset in
/// receivership liquidation, e.g. (1 + this) * amount repaid <= asset seized
/// * This is the minimum value the program allows for the above, if fee state is set below this,
///   the program will use this instead.
pub const LIQUIDATION_BONUS_FEE_MINIMUM: I80F48 = I80F48!(0.05);
/// Liquidators can consume/close out the entire account with essentially no limits (e.g. regardless
/// of liquidation bonus, etc) if it has net assets worth less than this amount in dollars. This
/// roughly covers the fee to open a liquidation record plus a little extra.
pub const LIQUIDATION_CLOSEOUT_DOLLAR_THRESHOLD: I80F48 = I80F48!(5);
/// Maximum order execution fee as a percent of the order size
/// * This value is used together with the slippage set by the user.
pub const ORDER_EXECUTION_MAX_FEE: I80F48 = I80F48!(0.05); // 5%
/// The default fee, in native SOL in native decimals (i.e. lamports) used in testing
pub const ORDER_INIT_FLAT_FEE_DEFAULT: u32 = 100_000;
/// Maximum basis points (100%)
pub const MAX_BPS: u16 = 10_000;

pub const MIN_PYTH_PUSH_VERIFICATION_LEVEL: VerificationLevel = VerificationLevel::Full;

// TODO move this to the global fee wallet eventually
/// A nominal fee paid to the global wallet when intiating an account transfer. Primarily intended
/// to avoid spamming account migration, which is mildly annoying to backend systems that track the
/// state of accounts.
/// * Should be ~ $0.50 or around that magnitude
/// * In lamports
pub const ACCOUNT_TRANSFER_FEE: u64 = 5_000_000;

/// When creating a mrgn account using a PDA, programs that wish to specify a third_party_id must be
/// registered here. This confers no other benefits. Creating accounts with third_party_id = 0 or
/// (the default) or id < PDA_FREE_THRESHOLD is freely available to any caller.
///
/// This enables third-parties (who have registered) to quickly sort all mrgn accounts that are
/// relevant to their use-case by memcmp without loading the entire mrgn ecosystem.
///
/// Registration is free, we will include your registration in the next program update (roughly
/// monthly). Feel free to request multiple.
///
/// Contact us or open a GH issue to register.
pub const THIRD_PARTY_CPI_RULES: &[(u16, Pubkey)] = &[
    (10_001, MOCKS_PROGRAM_ID),
    (
        11_111,
        pubkey!("AsgardVwpApNc9DgEsDAHJcvupPuRtLQSKDP9MQNw16N"),
    ),
    (
        11_123,
        pubkey!("6HyT8NQDpXY5wGkvX7haQVJ5nGUBVXQSkaT6Nf7fbsuJ"),
    ),
    (
        11_124,
        pubkey!("save8RQVPMWNTzU18t3GBvBkN9hT7jsGjiCQ28FpD9H"),
    ),
];

/// * IDs < PDA_FREE_THRESHOLD are "free" (no special CPI restriction), just go ahead and use them
///
/// * IDs >= PDA_FREE_THRESHOLD are "restricted": must contact us to register first.
pub const PDA_FREE_THRESHOLD: u16 = 10_000;

// TODO move to ix_utils after liquidation_remix merged into 0.1.5
/// third_party_id > PDA_FREE_THRESHOLD are restricted, contact us to secure one.
///
///
/// Returns:
/// - Ok(true)  => it *is* a CPI from the allowed program for `third_party_id`, or uses an
///   unrestricted seed that isn't subject to any limits.
/// - Ok(false) => not a CPI (direct call) OR CPI from a different program that has not registered
///   that seed.
pub fn is_allowed_cpi_for_third_party_id(
    sysvar_info: &AccountInfo,
    third_party_id: u16,
) -> MarginfiResult<bool> {
    // Free tier: no gating at all.
    if third_party_id < PDA_FREE_THRESHOLD {
        return Ok(true);
    }

    // Restricted tier: must have a rule.
    let allowed_program = match THIRD_PARTY_CPI_RULES
        .iter()
        .find(|(id, _)| *id == third_party_id)
        .map(|(_, program_id)| *program_id)
    {
        Some(p) => p,
        None => {
            return Ok(false);
        }
    };

    let current_ix_index = ix_sysvar::load_current_index_checked(sysvar_info)?;
    let current_ixn = load_instruction_at_checked(current_ix_index as usize, sysvar_info)?;

    // If the current (top-level) instruction is *this* program, it's a direct call (not CPI) -> no
    // "third party" id allowed in the restricted zone.
    if current_ixn.program_id == crate::ID {
        return Ok(false);
    }

    Ok(current_ixn.program_id == allowed_program)
}
