use crate::{
    bank_authority_seed, bank_seed,
    state::{
        bank::BankVaultType,
        bank_config::BankConfigImpl,
        marginfi_account::get_remaining_accounts_per_bank,
        price::{
            OraclePriceFeedAdapter, OraclePriceType, OraclePriceWithConfidence, PriceAdapter,
            PriceBias,
        },
    },
    MarginfiError, MarginfiResult,
};
use anchor_lang::prelude::*;
use anchor_spl::{
    token::Token,
    token_2022::spl_token_2022::{
        self,
        extension::{
            transfer_fee::{TransferFee, TransferFeeConfig},
            BaseStateWithExtensions, StateWithExtensions,
        },
    },
    token_interface::Mint,
};
use fixed::types::I80F48;
use marginfi_type_crate::{
    constants::{
        ASSET_TAG_DEFAULT, ASSET_TAG_DRIFT, ASSET_TAG_KAMINO, ASSET_TAG_SOL, ASSET_TAG_SOLEND,
        ASSET_TAG_STAKED,
    },
    types::{Bank, BankOperationalState, MarginfiAccount, WrappedI80F48},
};

pub fn find_bank_vault_pda(bank_pk: &Pubkey, vault_type: BankVaultType) -> (Pubkey, u8) {
    Pubkey::find_program_address(bank_seed!(vault_type, bank_pk), &crate::ID)
}

pub fn find_bank_vault_authority_pda(bank_pk: &Pubkey, vault_type: BankVaultType) -> (Pubkey, u8) {
    Pubkey::find_program_address(bank_authority_seed!(vault_type, bank_pk), &crate::ID)
}

pub trait NumTraitsWithTolerance<T> {
    fn is_zero_with_tolerance(&self, t: T) -> bool;
    fn is_positive_with_tolerance(&self, t: T) -> bool;
}

impl<T> NumTraitsWithTolerance<T> for I80F48
where
    I80F48: PartialOrd<T>,
{
    fn is_zero_with_tolerance(&self, t: T) -> bool {
        self.abs() < t
    }

    fn is_positive_with_tolerance(&self, t: T) -> bool {
        self.gt(&t)
    }
}

pub fn calculate_pre_fee_spl_deposit_amount(
    mint_ai: AccountInfo,
    post_fee_amount: u64,
    epoch: u64,
) -> MarginfiResult<u64> {
    if mint_ai.owner.eq(&Token::id()) {
        return Ok(post_fee_amount);
    }

    let mint_data = mint_ai.try_borrow_data()?;
    let mint = StateWithExtensions::<spl_token_2022::state::Mint>::unpack(&mint_data)?;

    match mint.get_extension::<TransferFeeConfig>() {
        Ok(transfer_fee_config) => {
            let epoch_fee = transfer_fee_config.get_epoch_fee(epoch);
            let pre_fee_amount = calculate_pre_fee_amount(epoch_fee, post_fee_amount).unwrap();
            Ok(pre_fee_amount)
        }
        Err(_) => Ok(post_fee_amount),
    }
}

pub fn calculate_post_fee_spl_deposit_amount(
    mint_ai: AccountInfo,
    input_amount: u64,
    epoch: u64,
) -> MarginfiResult<u64> {
    if mint_ai.owner.eq(&Token::id()) {
        return Ok(input_amount);
    }

    let mint_data = mint_ai.try_borrow_data()?;
    let mint = StateWithExtensions::<spl_token_2022::state::Mint>::unpack(&mint_data)?;

    let fee = if let Ok(transfer_fee_config) = mint.get_extension::<TransferFeeConfig>() {
        transfer_fee_config
            .calculate_epoch_fee(epoch, input_amount)
            .unwrap()
    } else {
        0
    };

    let output_amount = input_amount
        .checked_sub(fee)
        .ok_or(MarginfiError::MathError)?;

    Ok(output_amount)
}

pub fn nonzero_fee(mint_ai: AccountInfo, epoch: u64) -> MarginfiResult<bool> {
    if mint_ai.owner.eq(&Token::id()) {
        return Ok(false);
    }

    let mint_data = mint_ai.try_borrow_data()?;
    let mint = StateWithExtensions::<spl_token_2022::state::Mint>::unpack(&mint_data)?;

    if let Ok(transfer_fee_config) = mint.get_extension::<TransferFeeConfig>() {
        return Ok(u16::from(
            transfer_fee_config
                .get_epoch_fee(epoch)
                .transfer_fee_basis_points,
        ) != 0);
    }

    Ok(false)
}

/// Checks if first account is a mint account. If so, updates remaining_account -> &remaining_account[1..]
///
/// Ok(None) if Tokenkeg
pub fn maybe_take_bank_mint<'info>(
    remaining_accounts: &mut &'info [AccountInfo<'info>],
    bank: &Bank,
    token_program: &Pubkey,
) -> MarginfiResult<Option<InterfaceAccount<'info, Mint>>> {
    match *token_program {
        anchor_spl::token::ID => Ok(None),
        anchor_spl::token_2022::ID => {
            let (maybe_mint, remaining) = remaining_accounts
                .split_first()
                .ok_or(MarginfiError::T22MintRequired)?;
            *remaining_accounts = remaining;

            if bank.mint != *maybe_mint.key {
                return err!(MarginfiError::T22MintRequired);
            }

            InterfaceAccount::try_from(maybe_mint)
                .map(Option::Some)
                .map_err(|e| {
                    msg!("failed to parse mint account: {:?}", e);
                    MarginfiError::T22MintRequired.into()
                })
        }

        _ => panic!("unsupported token program"),
    }
}

const ONE_IN_BASIS_POINTS: u128 = 10_000;
/// backported fix from
/// https://github.com/solana-labs/solana-program-library/commit/20e6792179fc7f1251579c1c33a4a0feec48e15e
pub fn calculate_pre_fee_amount(transfer_fee: &TransferFee, post_fee_amount: u64) -> Option<u64> {
    let maximum_fee = u64::from(transfer_fee.maximum_fee);
    let transfer_fee_basis_points = u16::from(transfer_fee.transfer_fee_basis_points) as u128;
    match (transfer_fee_basis_points, post_fee_amount) {
        // no fee, same amount
        (0, _) => Some(post_fee_amount),
        // 0 zero out, 0 in
        (_, 0) => Some(0),
        // 100%, cap at max fee
        (ONE_IN_BASIS_POINTS, _) => maximum_fee.checked_add(post_fee_amount),
        _ => {
            let numerator = (post_fee_amount as u128).checked_mul(ONE_IN_BASIS_POINTS)?;
            let denominator = ONE_IN_BASIS_POINTS.checked_sub(transfer_fee_basis_points)?;
            let raw_pre_fee_amount = ceil_div(numerator, denominator)?;

            if raw_pre_fee_amount.checked_sub(post_fee_amount as u128)? >= maximum_fee as u128 {
                post_fee_amount.checked_add(maximum_fee)
            } else {
                // should return `None` if `pre_fee_amount` overflows
                u64::try_from(raw_pre_fee_amount).ok()
            }
        }
    }
}

// Private function from spl-program-library
fn ceil_div(numerator: u128, denominator: u128) -> Option<u128> {
    numerator
        .checked_add(denominator)?
        .checked_sub(1)?
        .checked_div(denominator)
}

/// A minimal tool to convert a hex string like "22f123639" into the byte equivalent.
#[cfg(test)]
pub fn hex_to_bytes(hex: &str) -> Vec<u8> {
    if hex.len() % 2 != 0 {
        panic!("hex string odd size");
    }
    hex.as_bytes()
        .chunks(2)
        .map(|chunk| {
            let high = chunk[0] as char;
            let low = chunk[1] as char;
            let high = high.to_digit(16).expect("Invalid hex character") as u8;
            let low = low.to_digit(16).expect("Invalid hex character") as u8;
            (high << 4) | low
        })
        .collect()
}

/// Validate that after a deposit to Bank, the users's account contains either all Default/SOL
/// balances, or all Staked/Sol balances. Default and Staked assets cannot mix.
pub fn validate_asset_tags(bank: &Bank, marginfi_account: &MarginfiAccount) -> MarginfiResult {
    let mut has_default_asset = false;
    let mut has_staked_asset = false;

    let is_default_like = |asset_tag: u8| {
        matches!(
            asset_tag,
            ASSET_TAG_DEFAULT | ASSET_TAG_KAMINO | ASSET_TAG_DRIFT | ASSET_TAG_SOLEND
        )
    };

    for balance in marginfi_account.lending_account.balances.iter() {
        if balance.is_active() {
            match balance.bank_asset_tag {
                ASSET_TAG_DEFAULT => has_default_asset = true,
                ASSET_TAG_SOL => { /* Do nothing, SOL can mix with any asset type */ }
                ASSET_TAG_STAKED => has_staked_asset = true,
                // Kamino/Drift/Solend assets behave like default assets
                ASSET_TAG_KAMINO | ASSET_TAG_DRIFT | ASSET_TAG_SOLEND => has_default_asset = true,
                _ => panic!("unsupported asset tag"),
            }
        }
    }

    // 1. Default-like assets cannot mix with Staked assets
    if is_default_like(bank.config.asset_tag) && has_staked_asset {
        return err!(MarginfiError::AssetTagMismatch);
    }

    // 2. Staked SOL cannot mix with Default-like assets
    if bank.config.asset_tag == ASSET_TAG_STAKED && has_default_asset {
        return err!(MarginfiError::AssetTagMismatch);
    }

    Ok(())
}

/// Validate that two banks are compatible based on their asset tags. See the following combinations
/// (* is wildcard, e.g. any tag):
///
/// Allowed:
/// 1) Default/Default
/// 2) Sol/*
/// 3) Staked/Staked
///
/// Forbidden:
/// 1) Default/Staked
///
/// Returns an error if the two banks have mismatching asset tags according to the above.
pub fn validate_bank_asset_tags(bank_a: &Bank, bank_b: &Bank) -> MarginfiResult {
    let is_default_like = |asset_tag: u8| {
        matches!(
            asset_tag,
            ASSET_TAG_DEFAULT | ASSET_TAG_KAMINO | ASSET_TAG_DRIFT | ASSET_TAG_SOLEND
        )
    };

    let is_bank_a_default = is_default_like(bank_a.config.asset_tag);
    let is_bank_a_staked = bank_a.config.asset_tag == ASSET_TAG_STAKED;
    let is_bank_b_default = is_default_like(bank_b.config.asset_tag);
    let is_bank_b_staked = bank_b.config.asset_tag == ASSET_TAG_STAKED;
    // Note: Sol is compatible with all other tags and doesn't matter...

    // 1. Default assets cannot mix with Staked assets
    if is_bank_a_default && is_bank_b_staked {
        return err!(MarginfiError::AssetTagMismatch);
    }
    if is_bank_a_staked && is_bank_b_default {
        return err!(MarginfiError::AssetTagMismatch);
    }

    Ok(())
}

#[derive(Debug, Clone, Copy)]
pub enum InstructionKind {
    /// Only fails if the bank is in `BankKilledByBankruptcy`, technically doesn't exist (yet)
    Unrestricted,
    /// E.g. withdraw, repay
    FailsInReduceState,
    /// E.g. liquidation
    FailsInPausedState,
    /// E.g. borrow, deposit
    FailsIfPausedOrReduceState,
}

// TODO remove redundant checks for these elsewhere in the program (they are nested many laters deep
// in various value delta functions)
/// Validate the bank's state does not forbid the execution of an instruction
pub fn validate_bank_state(bank: &Bank, kind: InstructionKind) -> MarginfiResult {
    if bank.config.operational_state == BankOperationalState::KilledByBankruptcy {
        return err!(MarginfiError::BankKilledByBankruptcy);
    }

    match kind {
        InstructionKind::FailsInReduceState
            if bank.config.operational_state == BankOperationalState::ReduceOnly =>
        {
            return err!(MarginfiError::BankReduceOnly);
        }

        InstructionKind::FailsInPausedState
            if bank.config.operational_state == BankOperationalState::Paused =>
        {
            return err!(MarginfiError::BankPaused);
        }

        InstructionKind::FailsIfPausedOrReduceState
            if matches!(
                bank.config.operational_state,
                BankOperationalState::Paused | BankOperationalState::ReduceOnly
            ) =>
        {
            return match bank.config.operational_state {
                BankOperationalState::Paused => {
                    err!(MarginfiError::BankPaused)
                }
                BankOperationalState::ReduceOnly => {
                    err!(MarginfiError::BankReduceOnly)
                }
                _ => unreachable!(),
            };
        }
        _ => {}
    }

    Ok(())
}

pub fn wrapped_i80f48_to_f64(n: WrappedI80F48) -> f64 {
    let as_i80: I80F48 = n.into();
    let as_f64: f64 = as_i80.to_num();
    as_f64
}

/// Fetch a low-biased price for a given bank from a properly structured remaining accounts slice as
/// passed to any risk check.
///
/// * Errors if bank not found or bank/oracles don't appear in the slice in the correct order
/// * If a RiskEngine available, consider `get_unbiased_price_for_bank` instead
pub fn fetch_asset_price_for_bank_low_bias<'info>(
    bank_key: &Pubkey,
    bank: &Bank,
    clock: &Clock,
    remaining_accounts: &'info [AccountInfo<'info>],
) -> Result<I80F48> {
    let oracle_ais = oracle_accounts_for_bank(bank_key, bank, remaining_accounts)?;
    let pf = OraclePriceFeedAdapter::try_from_bank(bank, oracle_ais, clock)?;
    let price = pf.get_price_of_type(
        OraclePriceType::RealTime,
        Some(PriceBias::Low),
        bank.config.oracle_max_confidence,
    )?;

    Ok(price)
}

/// Fetch an unbiased oracle price (no safety bias) for a given bank.
///
/// * Errors if bank not found or bank/oracles don't appear in the slice in the correct order
/// * If a RiskEngine available, consider `get_unbiased_price_for_bank` instead
pub fn fetch_unbiased_price_for_bank<'info>(
    bank_key: &Pubkey,
    bank: &Bank,
    clock: &Clock,
    remaining_accounts: &'info [AccountInfo<'info>],
) -> Result<OraclePriceWithConfidence> {
    let oracle_ais = oracle_accounts_for_bank(bank_key, bank, remaining_accounts)?;
    let pf = OraclePriceFeedAdapter::try_from_bank(bank, oracle_ais, clock)?;
    let price = pf.get_price_and_confidence_of_type(
        OraclePriceType::RealTime,
        bank.config.oracle_max_confidence,
    )?;

    Ok(price)
}

/// Fetch a rate-limit price for inflow accounting (deposit/repay).
///
/// Prefers a live oracle price when available, but falls back to the cached price
/// to avoid blocking inflows when oracles are omitted. Cached prices must be fresh.
pub fn fetch_rate_limit_price_for_inflow<'info>(
    bank_key: &Pubkey,
    bank: &Bank,
    clock: &Clock,
    remaining_accounts: &'info [AccountInfo<'info>],
) -> MarginfiResult<I80F48> {
    if !remaining_accounts.is_empty() {
        if let Ok(price) =
            fetch_asset_price_for_bank_low_bias(bank_key, bank, clock, remaining_accounts)
        {
            if price > I80F48::ZERO {
                return Ok(price);
            }
        }
    }

    let cached_price: I80F48 = bank.cache.last_oracle_price.into();
    let cached_ts = bank.cache.last_oracle_price_timestamp;
    let max_age = bank.config.get_oracle_max_age() as i64;
    let age = clock.unix_timestamp.saturating_sub(cached_ts);

    if cached_price <= I80F48::ZERO || cached_ts == 0 || age < 0 || age > max_age {
        return err!(MarginfiError::InvalidRateLimitPrice);
    }

    Ok(cached_price)
}

/// Locate a bank's oracle information from a properly formatted slice of remaining accounts.
fn oracle_accounts_for_bank<'info>(
    bank_key: &Pubkey,
    bank: &Bank,
    remaining_accounts: &'info [AccountInfo<'info>],
) -> Result<&'info [AccountInfo<'info>]> {
    let accs_needed = get_remaining_accounts_per_bank(bank)? - 1;

    let bank_idx = remaining_accounts
        .iter()
        .position(|ai| ai.key == bank_key)
        .ok_or_else(|| error!(MarginfiError::BankAccountNotFound))?;

    let start = bank_idx + 1;
    let end = start + accs_needed;

    require!(
        end <= remaining_accounts.len(),
        MarginfiError::WrongNumberOfOracleAccounts
    );

    Ok(&remaining_accounts[start..end])
}

#[macro_export]
macro_rules! assert_eq_with_tolerance {
    ($test_val:expr, $val:expr, $tolerance:expr) => {
        assert!(
            ($test_val - $val).abs() <= $tolerance,
            "assertion failed: `({} - {}) <= {}`",
            $test_val,
            $val,
            $tolerance
        );
    };
}

/// Helper function for constraint validation - checks if asset tag is valid for MarginFi operations
pub fn is_marginfi_asset_tag(asset_tag: u8) -> bool {
    matches!(
        asset_tag,
        ASSET_TAG_DEFAULT | ASSET_TAG_SOL | ASSET_TAG_STAKED
    )
}

/// Helper function for constraint validation - checks if asset tag is valid for Kamino operations
pub fn is_kamino_asset_tag(asset_tag: u8) -> bool {
    asset_tag == ASSET_TAG_KAMINO
}

/// Helper function for constraint validation - checks if asset tag is valid for Drift operations
pub fn is_drift_asset_tag(asset_tag: u8) -> bool {
    asset_tag == ASSET_TAG_DRIFT
}

/// Helper function for constraint validation - checks if asset tag is valid for Solend operations
pub fn is_solend_asset_tag(asset_tag: u8) -> bool {
    asset_tag == ASSET_TAG_SOLEND
}

/// Helper function - checks if asset tag is an integration type (Kamino, Drift, or Solend)
/// These integrations share a position limit due to their 3-account-per-position overhead
pub fn is_integration_asset_tag(asset_tag: u8) -> bool {
    matches!(
        asset_tag,
        ASSET_TAG_KAMINO | ASSET_TAG_DRIFT | ASSET_TAG_SOLEND
    )
}

#[cfg(test)]
mod tests {
    use super::validate_asset_tags;
    use crate::MarginfiError;
    use bytemuck::Zeroable;
    use marginfi_type_crate::constants::{
        ASSET_TAG_DEFAULT, ASSET_TAG_DRIFT, ASSET_TAG_KAMINO, ASSET_TAG_SOL, ASSET_TAG_SOLEND,
        ASSET_TAG_STAKED,
    };
    use marginfi_type_crate::types::{Bank, MarginfiAccount};

    fn bank_with_tag(asset_tag: u8) -> Bank {
        let mut bank = Bank::zeroed();
        bank.config.asset_tag = asset_tag;
        bank
    }

    fn account_with_tag(asset_tag: u8) -> MarginfiAccount {
        let mut account = MarginfiAccount::zeroed();
        account.lending_account.balances[0].active = 1;
        account.lending_account.balances[0].bank_asset_tag = asset_tag;
        account
    }

    fn account_with_tags(asset_tags: &[u8]) -> MarginfiAccount {
        let mut account = MarginfiAccount::zeroed();
        for (idx, tag) in asset_tags.iter().enumerate() {
            account.lending_account.balances[idx].active = 1;
            account.lending_account.balances[idx].bank_asset_tag = *tag;
        }
        account
    }

    #[test]
    fn staked_blocks_kamino() {
        let bank = bank_with_tag(ASSET_TAG_KAMINO);
        let account = account_with_tag(ASSET_TAG_STAKED);
        let result = validate_asset_tags(&bank, &account);
        assert_eq!(
            result.err().unwrap(),
            MarginfiError::AssetTagMismatch.into()
        );
    }

    #[test]
    fn staked_blocks_drift() {
        let bank = bank_with_tag(ASSET_TAG_DRIFT);
        let account = account_with_tag(ASSET_TAG_STAKED);
        let result = validate_asset_tags(&bank, &account);
        assert_eq!(
            result.err().unwrap(),
            MarginfiError::AssetTagMismatch.into()
        );
    }

    #[test]
    fn staked_blocks_solend() {
        let bank = bank_with_tag(ASSET_TAG_SOLEND);
        let account = account_with_tag(ASSET_TAG_STAKED);
        let result = validate_asset_tags(&bank, &account);
        assert_eq!(
            result.err().unwrap(),
            MarginfiError::AssetTagMismatch.into()
        );
    }

    #[test]
    fn staked_blocks_default_like_inverse_cases() {
        let bank = bank_with_tag(ASSET_TAG_STAKED);

        let account = account_with_tag(ASSET_TAG_KAMINO);
        let result = validate_asset_tags(&bank, &account);
        assert_eq!(
            result.err().unwrap(),
            MarginfiError::AssetTagMismatch.into()
        );

        let account = account_with_tag(ASSET_TAG_DRIFT);
        let result = validate_asset_tags(&bank, &account);
        assert_eq!(
            result.err().unwrap(),
            MarginfiError::AssetTagMismatch.into()
        );

        let account = account_with_tag(ASSET_TAG_SOLEND);
        let result = validate_asset_tags(&bank, &account);
        assert_eq!(
            result.err().unwrap(),
            MarginfiError::AssetTagMismatch.into()
        );
    }

    #[test]
    fn staked_blocks_default() {
        let bank = bank_with_tag(ASSET_TAG_DEFAULT);
        let account = account_with_tag(ASSET_TAG_STAKED);
        let result = validate_asset_tags(&bank, &account);
        assert_eq!(
            result.err().unwrap(),
            MarginfiError::AssetTagMismatch.into()
        );

        let bank = bank_with_tag(ASSET_TAG_STAKED);
        let account = account_with_tag(ASSET_TAG_DEFAULT);
        let result = validate_asset_tags(&bank, &account);
        assert_eq!(
            result.err().unwrap(),
            MarginfiError::AssetTagMismatch.into()
        );
    }

    #[test]
    fn sol_and_staked_can_coexist() {
        let bank = bank_with_tag(ASSET_TAG_SOL);
        let account = account_with_tags(&[ASSET_TAG_STAKED, ASSET_TAG_SOL]);
        assert!(validate_asset_tags(&bank, &account).is_ok());

        let bank = bank_with_tag(ASSET_TAG_STAKED);
        let account = account_with_tags(&[ASSET_TAG_SOL]);
        assert!(validate_asset_tags(&bank, &account).is_ok());
    }

    #[test]
    fn default_like_tags_can_coexist_with_each_other_and_sol() {
        let account = account_with_tags(&[
            ASSET_TAG_DEFAULT,
            ASSET_TAG_KAMINO,
            ASSET_TAG_DRIFT,
            ASSET_TAG_SOLEND,
            ASSET_TAG_SOL,
        ]);

        let bank = bank_with_tag(ASSET_TAG_DEFAULT);
        assert!(validate_asset_tags(&bank, &account).is_ok());

        let bank = bank_with_tag(ASSET_TAG_KAMINO);
        assert!(validate_asset_tags(&bank, &account).is_ok());

        let bank = bank_with_tag(ASSET_TAG_DRIFT);
        assert!(validate_asset_tags(&bank, &account).is_ok());

        let bank = bank_with_tag(ASSET_TAG_SOLEND);
        assert!(validate_asset_tags(&bank, &account).is_ok());
    }
}
