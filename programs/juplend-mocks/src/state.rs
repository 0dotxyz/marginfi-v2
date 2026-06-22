use anchor_lang::prelude::*;
use fixed::types::I80F48;

// Account discriminator from JupLend IDL for `Lending`.
// Anchor discriminator = sha256("account:Lending")[0..8].
pub const LENDING_DISCRIMINATOR: [u8; 8] = [135, 199, 82, 16, 249, 131, 182, 241];

// Anchor discriminator = sha256("account:TokenReserve")[0..8].
pub const TOKEN_RESERVE_DISCRIMINATOR: [u8; 8] = [21, 18, 59, 135, 120, 20, 31, 12];

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
#[account(zero_copy, discriminator = &LENDING_DISCRIMINATOR)]
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
    /// Note that we allow last_update_timestamp to be in the *future* compared to the current timestamp.
    /// This is useful for the external callers of the functions we expose, such as try_from_bank(),
    /// because they do not have to synchronize their clock before every call.
    #[inline]
    pub fn is_stale(&self, current_timestamp: i64) -> bool {
        (self.last_update_timestamp as i64) < current_timestamp
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

const _: () = assert!(core::mem::size_of::<TokenReserve>() == 184);

/// Minimal mirror of the Juplend's liquidity-layer `TokenReserve` account — the rate-bearing account a
/// JupLend `Lending` references via `token_reserves_liquidity`.
/// https://github.com/Instadapp/fluid-solana-programs/blob/master/programs/liquidity/src/state/token_reserve.rs#L14-L40
#[zero_copy]
#[repr(C, packed)]
pub struct TokenReserve {
    pub mint: Pubkey,
    pub vault: Pubkey,

    /// Stored borrow rate (1e2: 100% == 10_000).
    pub borrow_rate: u16,
    /// Fee taken on interest (1e2: 100% == 10_000).
    pub fee_on_interest: u16,
    /// Last stored utilization (1e2: 100% == 10_000).
    pub last_utilization: u16,
    pub last_update_timestamp: u64,
    /// Supply exchange price (1e12).
    pub supply_exchange_price: u64,
    /// Borrow exchange price (1e12).
    pub borrow_exchange_price: u64,

    pub max_utilization: u16,

    pub total_supply_with_interest: u64,
    pub total_supply_interest_free: u64,
    pub total_borrow_with_interest: u64,
    pub total_borrow_interest_free: u64,
    pub total_claim_amount: u64,

    pub interacting_protocol: Pubkey,
    pub interacting_timestamp: u64,
    pub interacting_balance: u64,
}

impl TokenReserve {
    /// True when the reserve's rate/exchange prices were not updated for `current_timestamp`.
    /// A future `last_update_timestamp` is treated as fresh (mirrors `Lending::is_stale`).
    #[inline]
    pub fn is_stale(&self, current_timestamp: i64) -> bool {
        (self.last_update_timestamp as i64) < current_timestamp
    }

    /// Decode a `TokenReserve` from raw Fluid account data: an 8-byte Anchor discriminator followed by
    /// the packed body. Because the struct is `#[repr(C, packed)]` (matching Fluid's exact byte layout)
    /// it can't be borrowed zero-copy via `AccountLoader`, so the body is copied out with an unaligned
    /// read. Returns `None` on a length or discriminator mismatch.
    pub fn from_account_data(data: &[u8]) -> Option<Self> {
        const LEN: usize = core::mem::size_of::<TokenReserve>();
        if data.len() < 8 + LEN || data[..8] != TOKEN_RESERVE_DISCRIMINATOR {
            return None;
        }
        bytemuck::try_pod_read_unaligned(&data[8..8 + LEN]).ok()
    }

    /// Fluid liquidity-layer supply rate (I80F48, 1.0 == 100%) from the lagged stored fields. The
    /// caller must ensure the reserve was refreshed this slot (see [`TokenReserve::is_stale`]).
    /// Yields zero for an uninitialized reserve and `None` on overflow. Mirrors the supply branch of
    /// `TokenReserve::calculate_exchange_prices`:
    /// https://github.com/Instadapp/fluid-solana-programs/blob/master/programs/liquidity/src/state/token_reserve.rs#L362-L539
    pub fn supply_rate(&self) -> Option<I80F48> {
        juplend_supply_rate_from_parts(
            u128::from(self.borrow_rate),
            u128::from(self.fee_on_interest),
            u128::from(self.last_utilization),
            u128::from(self.supply_exchange_price),
            u128::from(self.borrow_exchange_price),
            u128::from(self.total_supply_with_interest),
            u128::from(self.total_supply_interest_free),
            u128::from(self.total_borrow_with_interest),
            u128::from(self.total_borrow_interest_free),
        )
    }
}

/// Fluid liquidity-layer scale constants: `FOUR_DECIMALS` (1e4) and
/// `EXCHANGE_PRICE_RATE_OUTPUT_DECIMALS` (1e17).
const FOUR_DECIMALS: u128 = 10_000;
const EXCHANGE_PRICE_RATE_OUTPUT_DECIMALS: u128 = 100_000_000_000_000_000;

/// Mirrors Fluid `get_with_interest_vs_free_ratio`: the smaller-over-larger ratio scaled to
/// `FOUR_DECIMALS`.
/// https://github.com/Instadapp/fluid-solana-programs/blob/master/programs/liquidity/src/state/token_reserve.rs#L67-L94
fn get_with_interest_vs_free_ratio(with_interest: u128, interest_free: u128) -> u128 {
    if with_interest > interest_free {
        interest_free * FOUR_DECIMALS / with_interest
    } else if with_interest < interest_free {
        with_interest * FOUR_DECIMALS / interest_free
    } else if with_interest > 0 {
        FOUR_DECIMALS
    } else {
        0
    }
}

/// Juplend's supply-rate computation from `TokenReserve` parts (all 1e2/1e12 native units)
/// Returns the supply APR as I80F48 (zero when uninitialized, `None` on overflow). Mirrors the `ratio_supply_yield` +
/// `supply_rate` block of `TokenReserve::calculate_exchange_prices` (`get_supply_ratio` /
/// `get_borrow_ratio` are `get_with_interest_vs_free_ratio` at the supply / borrow totals):
/// https://github.com/Instadapp/fluid-solana-programs/blob/master/programs/liquidity/src/state/token_reserve.rs#L422-L520
#[allow(clippy::too_many_arguments)]
pub fn juplend_supply_rate_from_parts(
    borrow_rate: u128,
    fee_on_interest: u128,
    utilization: u128,
    supply_exchange_price: u128,
    borrow_exchange_price: u128,
    total_supply_with_interest: u128,
    total_supply_interest_free: u128,
    total_borrow_with_interest: u128,
    total_borrow_interest_free: u128,
) -> Option<I80F48> {
    if borrow_rate == 0
        || total_borrow_with_interest == 0
        || total_supply_with_interest == 0
        || supply_exchange_price == 0
        || borrow_exchange_price == 0
    {
        return Some(I80F48::ZERO);
    }
    // `get_supply_ratio`
    let supply_ratio =
        get_with_interest_vs_free_ratio(total_supply_with_interest, total_supply_interest_free);

    // step 1: ratio_supply_yield without the borrow_ratio part.
    let mut ratio_supply_yield = if total_supply_with_interest < total_supply_interest_free {
        if supply_ratio == 0 {
            return Some(I80F48::ZERO);
        }

        let supply_ratio = EXCHANGE_PRICE_RATE_OUTPUT_DECIMALS * FOUR_DECIMALS / supply_ratio;
        utilization.saturating_mul(EXCHANGE_PRICE_RATE_OUTPUT_DECIMALS + supply_ratio)
            / FOUR_DECIMALS
    } else {
        utilization
            .saturating_mul(EXCHANGE_PRICE_RATE_OUTPUT_DECIMALS)
            .saturating_mul(FOUR_DECIMALS + supply_ratio)
            / (FOUR_DECIMALS * FOUR_DECIMALS)
    };

    // `get_borrow_ratio`, then x of borrowers paying yield (scaled to EXCHANGE_PRICE_RATE_OUTPUT_DECIMALS).
    let borrow_ratio =
        get_with_interest_vs_free_ratio(total_borrow_with_interest, total_borrow_interest_free);
    let borrow_ratio = if total_borrow_with_interest < total_borrow_interest_free {
        borrow_ratio * EXCHANGE_PRICE_RATE_OUTPUT_DECIMALS / (FOUR_DECIMALS + borrow_ratio)
    } else {
        EXCHANGE_PRICE_RATE_OUTPUT_DECIMALS
            - borrow_ratio * EXCHANGE_PRICE_RATE_OUTPUT_DECIMALS / (FOUR_DECIMALS + borrow_ratio)
    };

    // safe_multiply_divide(ratio_supply_yield, borrow_ratio, E17), then * FOUR_DECIMALS / E17. Fluid
    // uses u256 here; we checked-mul in u128 (None on overflow is fine for a ranking signal).
    ratio_supply_yield = ratio_supply_yield.checked_mul(borrow_ratio)?
        / EXCHANGE_PRICE_RATE_OUTPUT_DECIMALS
        * FOUR_DECIMALS
        / EXCHANGE_PRICE_RATE_OUTPUT_DECIMALS;

    // supply_rate = borrow_rate * ratio_supply_yield * (FOUR_DECIMALS - fee_on_interest).
    let supply_rate = borrow_rate
        .checked_mul(ratio_supply_yield)?
        .checked_mul(FOUR_DECIMALS.saturating_sub(fee_on_interest))?;
    Some(I80F48::from_num(supply_rate) / I80F48::from_num(1_000_000_000_000u128))
}

#[cfg(test)]
mod rate_tests {
    use super::*;

    fn approx(actual: I80F48, expected: f64) {
        let a = actual.to_num::<f64>();
        assert!((a - expected).abs() < 1e-5, "got {a}, expected {expected}");
    }

    #[test]
    fn supply_rate_from_real_mainnet_values() {
        // USDC TokenReserve (94vK29np...): borrow_rate 4.42%, fee 10%, util 83.57%. With no
        // interest-free split the formula reduces to borrow * util * (1 - fee) ≈ 0.033244.
        let r = juplend_supply_rate_from_parts(
            442,
            1000,
            8357,
            1_029_996_710_353,
            1_000_000_000_000,
            401_387_174_957_279,
            0,
            100_000_000_000,
            0,
        );
        approx(r.unwrap(), 0.033244);
    }

    #[test]
    fn supply_rate_uninitialized_reserve_is_zero() {
        let r = juplend_supply_rate_from_parts(0, 0, 0, 0, 0, 0, 0, 0, 0).unwrap();
        assert_eq!(r, I80F48::ZERO);
    }

    /// The `supply_rate()` method must forward `TokenReserve` fields to
    /// `juplend_supply_rate_from_parts` in the right order.
    #[test]
    fn supply_rate_method_matches_from_parts() {
        use bytemuck::Zeroable;
        let mut tr = TokenReserve::zeroed();
        tr.borrow_rate = 442;
        tr.fee_on_interest = 1000;
        tr.last_utilization = 8357;
        tr.supply_exchange_price = 1_029_996_710_353;
        tr.borrow_exchange_price = 1_000_000_000_000;
        tr.total_supply_with_interest = 401_387_174_957_279;
        tr.total_borrow_with_interest = 100_000_000_000;
        assert_eq!(
            tr.supply_rate(),
            juplend_supply_rate_from_parts(
                442,
                1000,
                8357,
                1_029_996_710_353,
                1_000_000_000_000,
                401_387_174_957_279,
                0,
                100_000_000_000,
                0
            )
        );
    }

    /// `from_account_data` must round-trip a valid `[discriminator][body]` buffer and reject a wrong
    /// discriminator or a truncated body.
    #[test]
    fn from_account_data_round_trips_and_rejects_bad_input() {
        use bytemuck::Zeroable;
        let mut tr = TokenReserve::zeroed();
        tr.borrow_rate = 442;
        tr.last_update_timestamp = 1_700_000_000;

        let mut buf = TOKEN_RESERVE_DISCRIMINATOR.to_vec();
        buf.extend_from_slice(bytemuck::bytes_of(&tr));

        let decoded = TokenReserve::from_account_data(&buf).unwrap();
        assert_eq!({ decoded.borrow_rate }, 442);
        assert_eq!({ decoded.last_update_timestamp }, 1_700_000_000);

        let mut wrong_discriminator = buf.clone();
        wrong_discriminator[0] ^= 0xFF;
        assert!(TokenReserve::from_account_data(&wrong_discriminator).is_none());

        assert!(TokenReserve::from_account_data(&buf[..buf.len() - 1]).is_none());
    }
}
