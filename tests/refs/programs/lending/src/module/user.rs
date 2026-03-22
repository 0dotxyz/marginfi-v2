use anchor_lang::prelude::*;

use crate::errors::*;
use crate::state::context::*;
use crate::utils::{
    deposit::{deposit_internal, mint_internal},
    withdraw::{redeem_internal, withdraw_internal},
};

///////////////////////////////////////////////////////////////
//                            DEPOSIT
///////////////////////////////////////////////////////////////

/// @notice If `amount` equals u64::MAX then the whole balance of `signer` is deposited.
///         Recommended to use `deposit_with_min_amount_out()` with a `min_amount_out` param instead to set acceptable limit.
/// @return shares_ actually minted shares
pub fn deposit(ctx: Context<Deposit>, amount: u64) -> Result<u64> {
    let shares_minted: u64 = deposit_internal(ctx, amount)?;

    Ok(shares_minted)
}

/// @notice same as deposit() but with an additional setting for minimum output amount.
/// reverts with `FTokenMinAmountOut()` if `min_amount_out` of shares is not reached
pub fn deposit_with_min_amount_out(
    ctx: Context<Deposit>,
    amount: u64,
    min_amount_out: u64,
) -> Result<()> {
    let shares_minted: u64 = deposit_internal(ctx, amount)?;

    if shares_minted < min_amount_out {
        return Err(ErrorCodes::FTokenMinAmountOut.into());
    }

    Ok(())
}

///////////////////////////////////////////////////////////////
//                            MINT
///////////////////////////////////////////////////////////////
/// @notice If `shares` equals u64::MAX then the whole balance of `msg.sender` is deposited.
///         Note there might be tiny inaccuracies between requested `shares` and actually received shares amount.
///         Recommended to use `deposit()` over mint because it is more gas efficient and less likely to revert.
///         Recommended to use `mint()` with a `minAmountOut` param instead to set acceptable limit.
/// @return assets deposited assets amount
///
pub fn mint(ctx: Context<Deposit>, shares: u64) -> Result<u64> {
    let assets = mint_internal(ctx, shares)?;
    Ok(assets)
}

/// @notice same as mint() but with an additional setting for maximum assets input amount.
/// reverts with `FTokenMaxAmount()` if `maxAssets` of assets is surpassed to mint `shares`.
pub fn mint_with_max_assets(ctx: Context<Deposit>, shares: u64, max_assets: u64) -> Result<u64> {
    let assets = mint_internal(ctx, shares)?;

    if max_assets != 0 && assets > max_assets {
        return Err(ErrorCodes::FTokenMaxAmount.into());
    }

    Ok(assets)
}

///////////////////////////////////////////////////////////////
//                            WITHDRAW
///////////////////////////////////////////////////////////////

/// @notice If `assets` equals u64::MAX then the whole fToken balance of `owner_` is withdrawn. This does not
///         consider withdrawal limit at Liquidity so best to check with max withdrawable amount before.
///         Note there might be tiny inaccuracies between requested `assets` and actually received assets amount.
///         Recommended to use `withdraw()` with a `minAmountOut` param instead to set acceptable limit.
/// @return shares_ burned shares
pub fn withdraw(ctx: Context<Withdraw>, assets: u64) -> Result<u64> {
    let shares_burned = withdraw_internal(ctx, assets)?;

    Ok(shares_burned)
}

/// @notice same as withdraw() but with an additional setting for maximum shares burned.
/// reverts with `FTokenMaxAmount()` if `maxSharesBurn` of shares burned is surpassed.
pub fn withdraw_with_max_shares_burn(
    ctx: Context<Withdraw>,
    assets: u64,
    max_shares_burn: u64,
) -> Result<u64> {
    let shares_burned = withdraw_internal(ctx, assets)?;

    if max_shares_burn != 0 && shares_burned > max_shares_burn {
        return Err(ErrorCodes::FTokenMaxAmount.into());
    }

    Ok(shares_burned)
}

///////////////////////////////////////////////////////////////
//                            REDEEM
///////////////////////////////////////////////////////////////

/// @notice If `shares` equals u64::MAX then the whole balance of `owner` is withdrawn.
///         Recommended to use `withdraw()` over redeem because it is more gas efficient and can set specific amount.
///         Recommended to use `redeem()` with a `minAmountOut` param instead to set acceptable limit.
/// @return assets withdrawn assets amount
pub fn redeem(ctx: Context<Withdraw>, shares: u64) -> Result<u64> {
    let assets = redeem_internal(ctx, shares)?;

    Ok(assets)
}

/// @notice same as redeem() but with an additional setting for minimum output amount.
/// reverts with `FTokenMinAmountOut()` if `minAmountOut` of assets is not reached.
pub fn redeem_with_min_amount_out(
    ctx: Context<Withdraw>,
    shares: u64,
    min_amount_out: u64,
) -> Result<()> {
    let assets = redeem_internal(ctx, shares)?;

    if assets < min_amount_out {
        return Err(ErrorCodes::FTokenMinAmountOut.into());
    }

    Ok(())
}
