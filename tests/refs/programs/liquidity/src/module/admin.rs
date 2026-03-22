use anchor_lang::prelude::*;
use anchor_spl::token_interface::Mint;
use std::collections::HashMap;

use crate::{constants::*, errors::ErrorCodes, events::*, state::*};
use library::{
    math::{casting::*, safe_math::*},
    structs::{AddressBool, TokenTransferParams},
    token::*,
};

fn check_token_decimals_range(token_: &InterfaceAccount<'_, Mint>) -> Result<()> {
    let decimals_: u8 = decimals(&token_)?;

    if decimals_ < MIN_TOKEN_DECIMALS || decimals_ > MAX_TOKEN_DECIMALS {
        return Err(ErrorCodes::InvalidParams.into());
    }

    Ok(())
}

pub fn init_liquidity(
    context: Context<InitLiquidity>,
    authority: Pubkey,
    revenue_collector: Pubkey,
) -> Result<()> {
    if authority == Pubkey::default() || revenue_collector == Pubkey::default() {
        return Err(ErrorCodes::InvalidParams.into());
    }

    context
        .accounts
        .liquidity
        .init(authority, revenue_collector, context.bumps.liquidity)?;

    context.accounts.auth_list.init(authority)?;

    Ok(())
}

pub fn init_token_reserve(context: Context<InitTokenReserve>) -> Result<()> {
    check_token_decimals_range(&context.accounts.mint)?;
    check_for_token_extensions(&context.accounts.mint, &context.accounts.vault)?;

    if context.accounts.mint.key() != WSOL
        && total_supply(&context.accounts.mint)?.cast::<u128>()? > MAX_TOKEN_AMOUNT_CAP / 2
    {
        // in case any token ever needs to get listed with an unexpected big total supply, must double check deeper first
        // and possibly raise MAX_TOKEN_AMOUNT_CAP.
        return Err(ErrorCodes::InvalidParams.into());
    }

    let mut token_reserve = context.accounts.token_reserve.load_init()?;
    token_reserve.init(context.accounts.mint.key(), context.accounts.vault.key())?;

    let mut rate_model = context.accounts.rate_model.load_init()?;
    rate_model.init(context.accounts.mint.key())?;

    Ok(())
}

pub fn init_new_protocol(
    context: Context<InitNewProtocol>,
    supply_mint: Pubkey,
    borrow_mint: Pubkey,
    protocol: Pubkey,
) -> Result<()> {
    let mut user_supply_position = context.accounts.user_supply_position.load_init()?;
    user_supply_position.init(protocol, supply_mint)?;

    let mut user_borrow_position = context.accounts.user_borrow_position.load_init()?;
    user_borrow_position.init(protocol, borrow_mint)?;

    Ok(())
}

/// @notice adds/removes auths. Auths generally could be contracts which can have restricted actions defined on contract.
///         auths can be helpful in reducing governance overhead where it's not needed.
/// @param auth_status array of structs setting allowed status for an address.
///                    status true => add auth, false => remove auth
pub fn update_auths(context: Context<UpdateAuths>, auth_status: Vec<AddressBool>) -> Result<()> {
    let mut auth_map: HashMap<Pubkey, bool> = context
        .accounts
        .auth_list
        .auth_users
        .iter()
        .map(|addr: &Pubkey| (*addr, true))
        .collect();

    let default_pubkey: Pubkey = Pubkey::default();

    for auth in auth_status.iter() {
        if auth.addr == default_pubkey
            || (auth.addr == context.accounts.liquidity.authority && !auth.value)
        {
            return Err(ErrorCodes::InvalidParams.into());
        }

        auth_map.insert(auth.addr, auth.value);
    }

    context.accounts.auth_list.auth_users = auth_map
        .into_iter()
        .filter(|(_, value)| *value)
        .map(|(addr, _)| addr)
        .collect();

    if context.accounts.auth_list.auth_users.len() > MAX_AUTH_COUNT {
        return Err(ErrorCodes::MaxAuthCountReached.into());
    }

    emit!(LogUpdateAuths {
        auth_status: auth_status.clone()
    });

    Ok(())
}

pub fn update_authority(context: Context<UpdateAuthority>, new_authority: Pubkey) -> Result<()> {
    if context.accounts.authority.key() != context.accounts.liquidity.authority {
        // second check on top of context.rs to be extra sure
        return Err(ErrorCodes::OnlyLiquidityAuthority.into());
    }

    if new_authority != GOVERNANCE_MS {
        return Err(ErrorCodes::InvalidParams.into());
    }

    let old_authority = context.accounts.liquidity.authority.clone();

    context.accounts.liquidity.authority = new_authority;

    let mut auth_map: HashMap<Pubkey, bool> = context
        .accounts
        .auth_list
        .auth_users
        .iter()
        .map(|addr: &Pubkey| (*addr, true))
        .collect();

    auth_map.remove(&old_authority);
    auth_map.insert(new_authority, true);

    context.accounts.auth_list.auth_users = auth_map
        .into_iter()
        .filter(|(_, value)| *value)
        .map(|(addr, _)| addr)
        .collect();

    let mut guadians_map: HashMap<Pubkey, bool> = context
        .accounts
        .auth_list
        .guardians
        .iter()
        .map(|addr: &Pubkey| (*addr, true))
        .collect();

    guadians_map.remove(&old_authority);
    guadians_map.insert(new_authority, true);

    context.accounts.auth_list.guardians = guadians_map
        .into_iter()
        .filter(|(_, value)| *value)
        .map(|(addr, _)| addr)
        .collect();

    emit!(LogUpdateAuthority {
        new_authority: new_authority,
    });

    Ok(())
}

/// @notice adds/removes guardians. Only callable by Governance.
/// @param guardian_status array of structs setting allowed status for an address.
///                         status true => add guardian, false => remove guardian
pub fn update_guardians(
    context: Context<UpdateAuths>,
    guardian_status: Vec<AddressBool>,
) -> Result<()> {
    let mut guardian_map: HashMap<Pubkey, bool> = context
        .accounts
        .auth_list
        .guardians
        .iter()
        .map(|addr: &Pubkey| (*addr, true))
        .collect();

    let default_pubkey: Pubkey = Pubkey::default();

    for guardian in guardian_status.iter() {
        if guardian.addr == default_pubkey
            || (guardian.addr == context.accounts.liquidity.authority && !guardian.value)
        {
            return Err(ErrorCodes::InvalidParams.into());
        }

        guardian_map.insert(guardian.addr, guardian.value);
    }

    context.accounts.auth_list.guardians = guardian_map
        .into_iter()
        .filter(|(_, value)| *value)
        .map(|(addr, _)| addr)
        .collect();

    if context.accounts.auth_list.guardians.len() > MAX_AUTH_COUNT {
        return Err(ErrorCodes::MaxAuthCountReached.into());
    }

    emit!(LogUpdateGuardians {
        guardian_status: guardian_status.clone()
    });

    Ok(())
}

/// @notice changes the revenue collector address (contract that is sent revenue). Only callable by Governance.
/// @param revenue_collector new revenue collector address
pub fn update_revenue_collector(
    context: Context<UpdateRevenueCollector>,
    revenue_collector: Pubkey,
) -> Result<()> {
    if revenue_collector == Pubkey::default() {
        return Err(ErrorCodes::InvalidParams.into());
    }

    context.accounts.liquidity.revenue_collector = revenue_collector;

    emit!(LogUpdateRevenueCollector {
        revenue_collector: revenue_collector
    });

    Ok(())
}

/// @notice         collects revenue for tokens to configured revenueCollector address.
pub fn collect_revenue(context: Context<CollectRevenue>) -> Result<()> {
    let liquidity_token_balance: u128 =
        balance_of(&context.accounts.vault.to_account_info())?.cast()?;

    let token_reserve = context.accounts.token_reserve.load()?;
    let revenue_amount: u128 = token_reserve.calc_revenue(liquidity_token_balance)?;

    let liquidity_seeds: &[&[u8]] = &[LIQUIDITY_SEED, &[context.accounts.liquidity.bump]];

    if revenue_amount > 0 {
        transfer_spl_tokens(TokenTransferParams {
            source: context.accounts.vault.to_account_info(),
            destination: context.accounts.revenue_collector_account.to_account_info(),
            authority: context.accounts.liquidity.to_account_info(),
            amount: revenue_amount.cast()?,
            token_program: context.accounts.token_program.to_account_info(),
            signer_seeds: Some(&[&liquidity_seeds]),
            mint: context.accounts.mint.clone(),
        })?;

        emit!(LogCollectRevenue {
            token: token_reserve.mint,
            revenue_amount: revenue_amount
        });
    }

    Ok(())
}

/// @notice changes current status, e.g. for pausing or unpausing all user operations. Only callable by Auths.
/// @param new_status new status
///        status = true -> pause, status = false -> resume.
pub fn change_status(context: Context<ChangeStatus>, status: bool) -> Result<()> {
    if context.accounts.liquidity.status == status {
        return Err(ErrorCodes::StatusAlreadySet.into());
    }

    context.accounts.liquidity.status = status;
    Ok(emit!(LogChangeStatus { new_status: status }))
}

/// @notice                  update tokens rate data version 1. Only callable by Auths.
/// @param rate_data          RateDataV1Params with rate data for specific token
pub fn update_rate_data_v1(
    context: Context<UpdateRateData>,
    rate_data: RateDataV1Params,
) -> Result<()> {
    let mut token_reserve = context.accounts.token_reserve.load_mut()?;
    token_reserve.update_exchange_price()?;

    let rate_data_clone: RateDataV1Params = rate_data.clone();

    let mut rate_model = context.accounts.rate_model.load_mut()?;
    rate_model.set_rate_v1(rate_data)?;

    token_reserve.update_exchange_prices_and_rates(&rate_model)?;

    Ok(emit!(LogUpdateRateDataV1 {
        token: context.accounts.mint.key(),
        rate_data: rate_data_clone,
    }))
}

/// @notice                  update tokens rate data version 2. Only callable by Auths.
/// @param rate_data          RateDataV2Params with rate data for specific token
pub fn update_rate_data_v2(
    context: Context<UpdateRateData>,
    rate_data: RateDataV2Params,
) -> Result<()> {
    let mut token_reserve = context.accounts.token_reserve.load_mut()?;
    token_reserve.update_exchange_price()?;

    let rate_data_clone: RateDataV2Params = rate_data.clone();

    let mut rate_model = context.accounts.rate_model.load_mut()?;
    rate_model.set_rate_v2(rate_data)?;

    token_reserve.update_exchange_prices_and_rates(&rate_model)?;

    Ok(emit!(LogUpdateRateDataV2 {
        token: context.accounts.mint.key(),
        rate_data: rate_data_clone,
    }))
}

/// @notice updates token configs: fee charge on borrowers interest & storage update utilization threshold.
///         Only callable by Auths.
/// @param token_config contains token address, fee & utilization threshold
pub fn update_token_config(
    context: Context<UpdateTokenConfig>,
    token_config: TokenConfig,
) -> Result<()> {
    if token_config.token != context.accounts.mint.key() {
        return Err(ErrorCodes::InvalidParams.into());
    }

    let rate_model = context.accounts.rate_model.load()?;

    // rate config should be set before token config
    if rate_model.version == 0 {
        return Err(ErrorCodes::InvalidConfigOrder.into());
    }

    if token_config.fee > FOUR_DECIMALS {
        // fee can not be > 100%
        return Err(ErrorCodes::InvalidParams.into());
    }

    if token_config.max_utilization > FOUR_DECIMALS || token_config.max_utilization == 0 {
        // borrows above 100% should never be possible
        return Err(ErrorCodes::InvalidParams.into());
    }

    let mut token_reserve = context.accounts.token_reserve.load_mut()?;
    token_reserve.update_exchange_price()?;

    token_reserve.fee_on_interest = token_config.fee.cast()?;
    token_reserve.max_utilization = token_config.max_utilization.cast()?;

    token_reserve.update_exchange_prices_and_rates(&rate_model)?;

    Ok(emit!(LogUpdateTokenConfigs {
        token_config: token_config.clone()
    }))
}

/// @notice updates user classes: 0 is for new protocols, 1 is for established protocols.
///         Only callable by Auths.
/// @param user_class array of structs of AddressU8 for each user address
pub fn update_user_class(
    context: Context<UpdateUserClass>,
    user_class: Vec<AddressU8>,
) -> Result<()> {
    let mut user_class_map: HashMap<Pubkey, u8> = context
        .accounts
        .auth_list
        .user_classes
        .iter()
        .map(|user_class: &UserClass| (user_class.addr, user_class.class))
        .collect();

    let default_pubkey: Pubkey = Pubkey::default();

    for user_class in user_class.iter() {
        if user_class.value > 1 {
            return Err(ErrorCodes::InvalidParams.into());
        }

        if user_class.addr == default_pubkey {
            return Err(ErrorCodes::InvalidParams.into());
        }

        user_class_map.insert(user_class.addr, user_class.value);
    }

    context.accounts.auth_list.user_classes = user_class_map
        .into_iter()
        .map(|(addr, class)| UserClass { addr, class })
        .collect();

    if context.accounts.auth_list.user_classes.len() > MAX_USER_CLASSES {
        return Err(ErrorCodes::MaxUserClassesReached.into());
    }

    Ok(emit!(LogUpdateUserClass {
        user_class: user_class.clone()
    }))
}

/// @notice sets a new withdrawal limit as the current limit for a certain user
/// @param new_limit new limit until which user supply can decrease to.
///                  Important: input in raw. Must account for exchange price in input param calculation.
///                  Note any limit that is < max expansion or > current user supply will set max expansion limit or
///                  current user supply as limit respectively.
///                  - set 0 to make maximum possible withdrawable: instant full expansion, and if that goes
///                  below base limit then fully down to 0.
///                  - set u128::MAX to make current withdrawable 0 (sets current user supply as limit).
pub fn update_user_withdrawal_limit(
    context: Context<UpdateUserWithdrawalLimit>,
    new_limit: u128,
    _protocol: Pubkey,
    _mint: Pubkey,
) -> Result<()> {
    // @dev no need to check if protocol is contract, because we are using protocol as a signer for operate instruction
    // and using PDA to sign the instruction

    let mut user_supply_position = context.accounts.user_supply_position.load_mut()?;

    let user_supply_amount: u128 = user_supply_position.get_amount()?;
    let user_supply_expand_pct: u128 = user_supply_position.expand_pct.cast()?;
    let max_withdrawal_limit: u128 = user_supply_amount.safe_sub(
        user_supply_amount
            .safe_mul(user_supply_expand_pct)?
            .safe_div(FOUR_DECIMALS)?,
    )?;

    let mut limit: u128 = new_limit;

    if limit == 0 || limit < max_withdrawal_limit {
        // instant full expansion, and if that goes below base limit then fully down to 0.
        // if we were to set a limit that goes below max expansion limit, then after 1 deposit or 1 withdrawal it would
        // become based on the max expansion limit again (unless it goes below base limit), which can be confusing.
        // Also updating base limit here to avoid the change after 1 interaction might have undesired effects.
        // So limiting update to max. full expansion. If more is desired, this must be called again after some withdraws.
        limit = max_withdrawal_limit;
    } else if new_limit == u128::MAX || new_limit > user_supply_amount {
        // current withdrawable 0 (sets current user supply as limit).
        limit = user_supply_amount;
    }

    let base_limit: u128 = user_supply_position.get_base_withdrawal_limit()?;

    if user_supply_amount < base_limit {
        limit = 0;
        // Note if new limit goes below base limit, it follows default behavior: first there must be a withdrawal
        // that brings user supply below base limit, then the limit will be set to 0.
        // otherwise we would have the same problem as described above after 1 interaction.
    }

    user_supply_position.set_withdrawal_limit(limit)?;
    user_supply_position.last_update = Clock::get()?.unix_timestamp.cast()?;

    Ok(emit!(LogUpdateUserWithdrawalLimit {
        user: user_supply_position.protocol,
        token: user_supply_position.mint,
        new_limit: limit.clone()
    }))
}

/// @notice sets user supply configs per token basis. Eg: with interest or interest-free and automated limits.
///         Only callable by Auths.
/// @param user_supply_config struct array containing user supply config, see `UserSupplyConfig` struct for more info
pub fn update_user_supply_config(
    context: Context<UpdateUserSupplyConfig>,
    user_supply_config: UserSupplyConfig,
) -> Result<()> {
    let mut token_reserve = context.accounts.token_reserve.load_mut()?;
    if token_reserve.max_utilization == 0 {
        // must set token config first
        return Err(ErrorCodes::InvalidConfigOrder.into());
    }

    if user_supply_config.mode > 1 {
        return Err(ErrorCodes::InvalidParams.into());
    }

    if user_supply_config.expand_percent > FOUR_DECIMALS {
        return Err(ErrorCodes::InvalidParams.into());
    }

    if user_supply_config.expand_duration > X24 || user_supply_config.expand_duration == 0 {
        return Err(ErrorCodes::InvalidParams.into());
    }

    if user_supply_config.base_withdrawal_limit == 0 {
        // base withdrawal limit can not be 0. As a side effect, this ensures that there is no supply config
        // where all values would be 0, so configured users can be differentiated in the mapping.
        return Err(ErrorCodes::InvalidParams.into());
    }

    let mut user_supply_position = context.accounts.user_supply_position.load_mut()?;

    // If configs are not set, and this is first time setting configs
    if user_supply_position.configs_not_set() {
        user_supply_position.set_status_as_active()?;
        user_supply_position.set_interest_mode(user_supply_config.mode)?;
        user_supply_position.set_expand_pct(user_supply_config.expand_percent)?;
        user_supply_position.set_expand_duration(user_supply_config.expand_duration)?;
        user_supply_position.set_base_withdrawal_limit(user_supply_config.base_withdrawal_limit)?;
    } else {
        if !user_supply_position.with_interest() && user_supply_config.mode == 0
            || user_supply_position.with_interest() && user_supply_config.mode == 1
        {
            user_supply_position.set_interest_mode(user_supply_config.mode)?;
            user_supply_position.set_expand_pct(user_supply_config.expand_percent)?;
            user_supply_position.set_expand_duration(user_supply_config.expand_duration)?;
            user_supply_position
                .set_base_withdrawal_limit(user_supply_config.base_withdrawal_limit)?;
        } else {
            // mode changes -> values have to be converted from raw <> normal etc.
            // update exchange prices for the token in storage up to now
            let (supply_exchange_price, _) = token_reserve.update_exchange_price()?;

            let mut total_supply_raw_interest: u128 =
                token_reserve.get_total_supply_with_interest()?;
            let mut total_supply_interest_free: u128 =
                token_reserve.get_total_supply_interest_free()?;

            // read current user supply & withdraw limit values
            // here supplyConversion_ = user supply amount
            let mut supply_conversion: u128 = user_supply_position.get_amount()?;
            let mut withdraw_limit_conversion: u128 =
                user_supply_position.get_withdrawal_limit()?;

            // conversion of balance and limit according to the mode change

            if !user_supply_position.with_interest() && user_supply_config.mode == 1 {
                // Changing balance from interest free to with interest -> normal amounts to raw amounts
                // -> must divide by exchange price.

                // decreasing interest free total supply
                total_supply_interest_free =
                    total_supply_interest_free.saturating_sub(supply_conversion);

                supply_conversion = supply_conversion
                    .safe_mul(EXCHANGE_PRICES_PRECISION)?
                    .safe_div(supply_exchange_price)?;

                withdraw_limit_conversion = withdraw_limit_conversion
                    .safe_mul(EXCHANGE_PRICES_PRECISION)?
                    .safe_div(supply_exchange_price)?;

                // increasing raw (with interest) total supply
                total_supply_raw_interest =
                    total_supply_raw_interest.safe_add(supply_conversion)?;
            } else if user_supply_position.with_interest() && user_supply_config.mode == 0 {
                // Changing balance from with interest to interest free-> raw amounts to normal amounts
                // -> must multiply by exchange price.

                // decreasing raw (with interest) supply
                total_supply_raw_interest =
                    total_supply_raw_interest.saturating_sub(supply_conversion);

                supply_conversion = supply_conversion
                    .safe_mul(supply_exchange_price)?
                    .safe_div(EXCHANGE_PRICES_PRECISION)?;

                withdraw_limit_conversion = withdraw_limit_conversion
                    .safe_mul(supply_exchange_price)?
                    .safe_div(EXCHANGE_PRICES_PRECISION)?;

                // increasing interest free total supply
                total_supply_interest_free =
                    total_supply_interest_free.safe_add(supply_conversion)?;
            }

            user_supply_position.set_interest_mode(user_supply_config.mode)?;
            user_supply_position.set_expand_pct(user_supply_config.expand_percent)?;
            user_supply_position.set_expand_duration(user_supply_config.expand_duration)?;

            user_supply_position.set_amount(supply_conversion)?;
            user_supply_position.set_withdrawal_limit(withdraw_limit_conversion)?;
            user_supply_position
                .set_base_withdrawal_limit(user_supply_config.base_withdrawal_limit)?;

            token_reserve.set_total_supply_with_interest(total_supply_raw_interest)?;
            token_reserve.set_total_supply_interest_free(total_supply_interest_free)?;

            let rate_model = context.accounts.rate_model.load()?;
            // trigger update borrow rate, utilization, ratios etc.
            token_reserve.update_exchange_prices_and_rates(&rate_model)?;
        }
    }

    Ok(emit!(LogUpdateUserSupplyConfigs {
        user: context.accounts.protocol.key(),
        token: context.accounts.mint.key(),
        user_supply_config: user_supply_config.clone()
    }))
}

/// @notice setting user borrow configs per token basis. Eg: with interest or interest-free and automated limits.
///         Only callable by Auths.
/// @param user_borrow_config struct array containing user borrow config, see `UserBorrowConfig` struct for more info
pub fn update_user_borrow_config(
    context: Context<UpdateUserBorrowConfig>,
    user_borrow_config: UserBorrowConfig,
) -> Result<()> {
    let mut token_reserve = context.accounts.token_reserve.load_mut()?;
    if token_reserve.max_utilization == 0 {
        // must set token config first
        return Err(ErrorCodes::InvalidConfigOrder.into());
    }

    if user_borrow_config.mode > 1 {
        return Err(ErrorCodes::InvalidParams.into());
    }

    if user_borrow_config.base_debt_ceiling > user_borrow_config.max_debt_ceiling {
        return Err(ErrorCodes::InvalidParams.into());
    }

    // Skip this check for WSOL, as totalSupply for wSOL is 0
    if context.accounts.mint.key() != WSOL
        && user_borrow_config.max_debt_ceiling
            > 10u128.safe_mul(total_supply(&context.accounts.mint)?.cast()?)?
    {
        // cap at 10x of total token supply
        return Err(ErrorCodes::InvalidParams.into());
    }

    if user_borrow_config.expand_percent > X14 {
        // expandPercent is max 14 bits
        return Err(ErrorCodes::InvalidParams.into());
    }

    if user_borrow_config.expand_duration > X24 || user_borrow_config.expand_duration == 0 {
        // duration is max 24 bits
        return Err(ErrorCodes::InvalidParams.into());
    }

    if user_borrow_config.base_debt_ceiling == 0 || user_borrow_config.max_debt_ceiling == 0 {
        // limits can not be 0. As a side effect, this ensures that there is no borrow config
        // where all values would be 0, so configured users can be differentiated in the mapping.
        return Err(ErrorCodes::LimitsCannotBeZero.into());
    }

    let mut user_borrow_position = context.accounts.user_borrow_position.load_mut()?;

    // If configs are not set, and this is first time setting configs
    if user_borrow_position.configs_not_set() {
        user_borrow_position.set_status_as_active()?;
        user_borrow_position.set_interest_mode(user_borrow_config.mode)?;
        user_borrow_position.set_expand_pct(user_borrow_config.expand_percent)?;
        user_borrow_position.set_expand_duration(user_borrow_config.expand_duration)?;
        user_borrow_position.set_base_debt_ceiling(user_borrow_config.base_debt_ceiling)?;
        user_borrow_position.set_max_debt_ceiling(user_borrow_config.max_debt_ceiling)?;
    } else {
        if !user_borrow_position.with_interest() && user_borrow_config.mode == 0
            || user_borrow_position.with_interest() && user_borrow_config.mode == 1
        {
            user_borrow_position.set_interest_mode(user_borrow_config.mode)?;
            user_borrow_position.set_expand_pct(user_borrow_config.expand_percent)?;
            user_borrow_position.set_expand_duration(user_borrow_config.expand_duration)?;
            user_borrow_position.set_base_debt_ceiling(user_borrow_config.base_debt_ceiling)?;
            user_borrow_position.set_max_debt_ceiling(user_borrow_config.max_debt_ceiling)?;
        } else {
            // mode changes -> values have to be converted from raw <> normal etc.
            // update exchange prices for the token in storage up to now
            let (_, borrow_exchange_price) = token_reserve.update_exchange_price()?;

            let mut total_borrow_raw_interest: u128 =
                token_reserve.get_total_borrow_with_interest()?;
            let mut total_borrow_interest_free: u128 =
                token_reserve.get_total_borrow_interest_free()?;

            // read current user borrow & debt ceiling values
            let mut borrow_conversion: u128 = user_borrow_position.get_amount()?;
            let mut debt_ceiling_conversion: u128 = user_borrow_position.get_debt_ceiling()?;

            // conversion of balance and limit according to the mode change
            if !user_borrow_position.with_interest() && user_borrow_config.mode == 1 {
                // Changing balance from interest free to with interest -> normal amounts to raw amounts
                // -> must divide by exchange price.

                // decreasing interest free total borrow
                total_borrow_interest_free =
                    total_borrow_interest_free.saturating_sub(borrow_conversion);

                borrow_conversion = borrow_conversion
                    .safe_mul(EXCHANGE_PRICES_PRECISION)?
                    .safe_div_ceil(borrow_exchange_price)?; // ROUND UP

                debt_ceiling_conversion = debt_ceiling_conversion
                    .safe_mul(EXCHANGE_PRICES_PRECISION)?
                    .safe_div(borrow_exchange_price)?;

                // increasing raw (with interest) total borrow
                total_borrow_raw_interest =
                    total_borrow_raw_interest.safe_add(borrow_conversion)?;
            } else if user_borrow_position.with_interest() && user_borrow_config.mode == 0 {
                // Changing balance from with interest to interest free -> raw amounts to normal amounts
                // -> must multiply by exchange price.

                // decreasing raw (with interest) borrow
                total_borrow_raw_interest =
                    total_borrow_raw_interest.saturating_sub(borrow_conversion);

                borrow_conversion = borrow_conversion
                    .safe_mul(borrow_exchange_price)?
                    .safe_div_ceil(EXCHANGE_PRICES_PRECISION)?; // ROUND UP

                debt_ceiling_conversion = debt_ceiling_conversion
                    .safe_mul(borrow_exchange_price)?
                    .safe_div(EXCHANGE_PRICES_PRECISION)?;

                // increasing interest free total borrow
                total_borrow_interest_free =
                    total_borrow_interest_free.safe_add(borrow_conversion)?;
            }

            // Update user borrow position with new values
            user_borrow_position.set_interest_mode(user_borrow_config.mode)?;
            user_borrow_position.set_amount(borrow_conversion)?;

            user_borrow_position.set_expand_pct(user_borrow_config.expand_percent)?;
            user_borrow_position.set_expand_duration(user_borrow_config.expand_duration)?;

            user_borrow_position.set_debt_ceiling(debt_ceiling_conversion)?;
            user_borrow_position.set_base_debt_ceiling(user_borrow_config.base_debt_ceiling)?;
            user_borrow_position.set_max_debt_ceiling(user_borrow_config.max_debt_ceiling)?;

            // Update token reserve total borrowings
            token_reserve.set_total_borrow_with_interest(total_borrow_raw_interest)?;
            token_reserve.set_total_borrow_interest_free(total_borrow_interest_free)?;

            let rate_model = context.accounts.rate_model.load()?;
            token_reserve.update_exchange_prices_and_rates(&rate_model)?;
        }
    }

    Ok(emit!(LogUpdateUserBorrowConfigs {
        user: context.accounts.protocol.key(),
        token: context.accounts.mint.key(),
        user_borrow_config: user_borrow_config.clone()
    }))
}

/// @notice pause operations for a particular user in class 0 (class 1 users can't be paused by guardians).
/// Only callable by Guardians.
#[instruction(protocol: Pubkey, mint: Pubkey)]
pub fn pause_user(
    context: Context<PauseUser>,
    protocol: Pubkey,
    supply_mint: Pubkey,
    borrow_mint: Pubkey,
    supply_status: Option<u8>,
    borrow_status: Option<u8>,
) -> Result<()> {
    let user_class_value = match context
        .accounts
        .auth_list
        .user_classes
        .iter()
        .find(|user_class| user_class.addr == protocol)
    {
        Some(user_class) => user_class.class,
        None => 0, // Default to class 0
    };

    if user_class_value == 1 {
        return Err(ErrorCodes::UserClassNotPausable.into());
    }

    // Only pause supply if status is 1
    // @dev In future we can add more status values like 3 to pause withdrawals
    if supply_status.is_some() {
        if supply_status.unwrap() != 1 {
            return Err(ErrorCodes::InvalidParams.into());
        }
        let mut user_supply_position = context.accounts.user_supply_position.load_mut()?;
        // @dev do not allow to pause if configs are not set
        if user_supply_position.mint != supply_mint || user_supply_position.configs_not_set() {
            return Err(ErrorCodes::InvalidParams.into());
        }
        if !user_supply_position.is_active() {
            return Err(ErrorCodes::UserAlreadyPaused.into());
        }
        user_supply_position.status = 1;

        emit!(LogPauseUser {
            user: protocol,
            mint: supply_mint,
            status: 1
        })
    }

    // Only pause borrow if status is 1
    // @dev In future we can add more status values like 4 to pause paybacks
    if borrow_status.is_some() {
        if borrow_status.unwrap() != 1 {
            return Err(ErrorCodes::InvalidParams.into());
        }
        let mut user_borrow_position = context.accounts.user_borrow_position.load_mut()?;
        // @dev do not allow to pause if configs are not set
        if user_borrow_position.mint != borrow_mint || user_borrow_position.configs_not_set() {
            return Err(ErrorCodes::InvalidParams.into());
        }
        if !user_borrow_position.is_active() {
            return Err(ErrorCodes::UserAlreadyPaused.into());
        }
        user_borrow_position.status = 1;

        emit!(LogPauseUser {
            user: protocol,
            mint: borrow_mint,
            status: 1,
        })
    }

    Ok(())
}

/// @notice unpause operations for a particular user in class 0 (class 1 users can't be paused by guardians).
/// Only callable by Guardians.
pub fn unpause_user(
    context: Context<PauseUser>,
    protocol: Pubkey,
    supply_mint: Pubkey,
    borrow_mint: Pubkey,
    supply_status: Option<u8>,
    borrow_status: Option<u8>,
) -> Result<()> {
    // Only unpause supply if status is 0
    if supply_status.is_some() {
        if supply_status.unwrap() != 0 {
            return Err(ErrorCodes::InvalidParams.into());
        }
        let mut user_supply_position = context.accounts.user_supply_position.load_mut()?;
        if user_supply_position.is_active() {
            return Err(ErrorCodes::UserAlreadyUnpaused.into());
        }
        if user_supply_position.mint != supply_mint {
            return Err(ErrorCodes::InvalidParams.into());
        }

        user_supply_position.status = 0;
        emit!(LogUnpauseUser {
            user: protocol,
            mint: supply_mint,
            status: 0,
        })
    }

    // Only unpause borrow if status is 0
    if borrow_status.is_some() {
        if borrow_status.unwrap() != 0 {
            return Err(ErrorCodes::InvalidParams.into());
        }
        let mut user_borrow_position = context.accounts.user_borrow_position.load_mut()?;
        if user_borrow_position.is_active() {
            return Err(ErrorCodes::UserAlreadyUnpaused.into());
        }
        if user_borrow_position.mint != borrow_mint {
            return Err(ErrorCodes::InvalidParams.into());
        }

        user_borrow_position.status = 0;
        emit!(LogUnpauseUser {
            user: protocol,
            mint: borrow_mint,
            status: 0,
        })
    }

    Ok(())
}
