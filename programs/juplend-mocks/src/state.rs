use anchor_lang::prelude::*;

// Account discriminator from JupLend IDL for `Lending`.
// Anchor discriminator = sha256("account:Lending")[0..8].
pub const LENDING_DISCRIMINATOR: [u8; 8] = [135, 199, 82, 16, 249, 131, 182, 241];

/// Precision used for exchange prices in JupLend (1e12).
///
/// Source: JupLend lending program constant `EXCHANGE_PRICES_PRECISION`.
pub const EXCHANGE_PRICES_PRECISION: u128 = 1_000_000_000_000;

/// Pure helper for JupLend withdraw preview math.
///
/// Formula (1e12 precision): `shares = ceil(assets * 1e12 / token_exchange_price)`.
#[inline]
pub fn expected_shares_for_withdraw_from_rate(
    assets: u64,
    token_exchange_price: u64,
) -> Option<u64> {
    let token_exchange_price = token_exchange_price as u128;
    if token_exchange_price == 0 {
        return None;
    }

    let numerator = (assets as u128)
        .checked_mul(EXCHANGE_PRICES_PRECISION)?
        .checked_add(token_exchange_price - 1)?;

    let shares_u128 = numerator.checked_div(token_exchange_price)?;

    shares_u128.try_into().ok()
}

/// Pure helper for JupLend redeem preview math.
///
/// Formula (1e12 precision): `assets = floor(shares * token_exchange_price / 1e12)`.
#[inline]
pub fn expected_assets_for_redeem_from_rate(shares: u64, token_exchange_price: u64) -> Option<u64> {
    let token_exchange_price = token_exchange_price as u128;
    if token_exchange_price == 0 {
        return None;
    }

    let assets_u128 = (shares as u128)
        .checked_mul(token_exchange_price)?
        .checked_div(EXCHANGE_PRICES_PRECISION)?;

    assets_u128.try_into().ok()
}

/// Pure helper for JupLend deposit preview math.
///
/// Mirrors lending + liquidity two-step conversion:
/// ```text
/// raw    = floor(assets * 1e12 / liquidity_exchange_price)
/// norm   = floor(raw * liquidity_exchange_price / 1e12)
/// shares = floor(norm * 1e12 / token_exchange_price)
/// ```
#[inline]
pub fn expected_shares_for_deposit_from_rates(
    assets: u64,
    liquidity_exchange_price: u64,
    token_exchange_price: u64,
) -> Option<u64> {
    let liquidity_ex_price = liquidity_exchange_price as u128;
    let token_ex_price = token_exchange_price as u128;
    if liquidity_ex_price == 0 || token_ex_price == 0 {
        return None;
    }

    let registered_amount_raw = (assets as u128)
        .checked_mul(EXCHANGE_PRICES_PRECISION)?
        .checked_div(liquidity_ex_price)?;

    let registered_amount = registered_amount_raw
        .checked_mul(liquidity_ex_price)?
        .checked_div(EXCHANGE_PRICES_PRECISION)?;

    let shares_u128 = registered_amount
        .checked_mul(EXCHANGE_PRICES_PRECISION)?
        .checked_div(token_ex_price)?;

    shares_u128.try_into().ok()
}

/// Minimal representation of the on-chain JupLend `Lending` account.
///
/// Notes:
/// - We intentionally use a **zero-copy** layout here to match how other integrations load large
///   external accounts (and to avoid paying Borsh (de)serialization cost on every access).
/// - `repr(C, packed)` keeps the byte layout identical to a field-by-field serialization
///   (i.e. no implicit padding). This is important because `Pubkey` has alignment=1 while `u64`
///   has alignment=8; using plain `repr(C)` would insert padding before the first `u64`.
#[account(zero_copy(unsafe), discriminator = &LENDING_DISCRIMINATOR)]
#[repr(C, packed)]
pub struct Lending {
    pub mint: Pubkey,
    pub f_token_mint: Pubkey,

    pub lending_id: u16,

    /// number of decimals for the fToken, same as underlying mint
    pub decimals: u8,

    /// PDA of rewards rate model (LRRM)
    pub rewards_rate_model: Pubkey,

    /// exchange price in the liquidity layer (no rewards)
    pub liquidity_exchange_price: u64,

    /// exchange price between fToken and underlying (with rewards)
    pub token_exchange_price: u64,

    /// unix timestamp when exchange prices were updated last
    pub last_update_timestamp: u64,

    pub token_reserves_liquidity: Pubkey,
    pub supply_position_on_liquidity: Pubkey,

    pub bump: u8,
}

impl Lending {
    /// Returns true if the lending exchange rate is not updated for the current timestamp.
    ///
    /// Marginfi uses a strict equality check (same-slot/same-time) to ensure exact math.
    #[inline]
    pub fn is_stale(&self, current_timestamp: i64) -> bool {
        self.last_update_timestamp as i64 != current_timestamp
    }

    /// Expected fToken shares minted when depositing `assets` underlying.
    ///
    /// Mirrors JupLend's actual deposit flow: **round down** via the liquidity layer.
    ///
    /// The deposit goes through a two-step conversion in the liquidity layer before
    /// computing shares. The intermediate floor divisions can cause up to 1 unit of
    /// rounding loss vs the naive single-step formula when exchange prices != 1e12.
    ///
    /// Formula (1e12 precision):
    /// ```text
    /// raw   = floor(assets * 1e12 / liquidity_exchange_price)
    /// norm  = floor(raw * liquidity_exchange_price / 1e12)
    /// shares = floor(norm * 1e12 / token_exchange_price)
    /// ```
    /// https://github.com/Instadapp/fluid-solana-programs/blob/830458299be42eaeb6e1fe8fef6aa23444430a10/programs/lending/src/utils/deposit.rs#L68-L86
    #[inline]
    pub fn expected_shares_for_deposit(&self, assets: u64) -> Option<u64> {
        expected_shares_for_deposit_from_rates(
            assets,
            self.liquidity_exchange_price,
            self.token_exchange_price,
        )
    }

    /// Expected fToken shares burned when withdrawing `assets` underlying.
    ///
    /// Mirrors JupLend's ERC-4626 style `preview_withdraw` semantics: **round up**.
    ///
    /// Formula (1e12 precision): `shares = ceil(assets * 1e12 / token_exchange_price)`.
    ///
    /// # Ceiling Division Implementation
    ///
    /// Uses the standard integer ceiling division identity:
    /// ```text
    /// ceil(a / b) = floor((a + b - 1) / b)
    /// ```
    ///
    /// The `+ (b - 1)` bumps the numerator into the next bucket when there's any
    /// remainder, but has no effect when `a` is exactly divisible by `b`.
    ///
    /// JupLend uses `safe_div_ceil()` which is mathematically equivalent.
    /// https://github.com/Instadapp/fluid-solana-programs/blob/830458299be42eaeb6e1fe8fef6aa23444430a10/programs/lending/src/utils/withdraw.rs#L52-L59
    #[inline]
    pub fn expected_shares_for_withdraw(&self, assets: u64) -> Option<u64> {
        expected_shares_for_withdraw_from_rate(assets, self.token_exchange_price)
    }

    /// Expected underlying assets returned when redeeming `shares` fTokens.
    ///
    /// Mirrors JupLend's ERC-4626 style `preview_redeem` semantics: **round down**.
    ///
    /// Formula (1e12 precision): `assets = floor(shares * token_exchange_price / 1e12)`.
    /// https://github.com/Instadapp/fluid-solana-programs/blob/830458299be42eaeb6e1fe8fef6aa23444430a10/programs/lending/src/state/context.rs#L399-L411
    /// https://github.com/Instadapp/fluid-solana-programs/blob/830458299be42eaeb6e1fe8fef6aa23444430a10/programs/lending/src/utils/helpers.rs#L37-L41
    #[inline]
    pub fn expected_assets_for_redeem(&self, shares: u64) -> Option<u64> {
        expected_assets_for_redeem_from_rate(shares, self.token_exchange_price)
    }
}
