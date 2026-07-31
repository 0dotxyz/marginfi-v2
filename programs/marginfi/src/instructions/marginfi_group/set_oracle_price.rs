use crate::events::{GroupEventHeader, LendingPoolBankSetOraclePriceEvent};
use crate::state::bank::BankImpl;
use crate::state::bank_config::BankConfigImpl;
use crate::{check, errors::MarginfiError, MarginfiResult};
use anchor_lang::prelude::*;
use fixed::types::I80F48;
use marginfi_type_crate::constants::{
    ASSET_TAG_DEFAULT, ASSET_TAG_DRIFT, ASSET_TAG_JUPLEND, ASSET_TAG_KAMINO, ASSET_TAG_STAKED,
};
use marginfi_type_crate::{
    constants::{BANK_SAME_ASSET_EMODE_ELIGIBLE, FREEZE_SETTINGS},
    types::{Bank, MarginfiGroup, OracleSetup, WrappedI80F48},
};

/// Installs a fixed-price or PT (Exponent linear-pricing) oracle on a bank. The caller declares the
/// intended mode via `setup`:
///
/// * `Fixed` - a flat fixed price (>= 0), specialized to `FixedKamino` / `FixedDrift` /
///   `FixedJuplend` by the bank's `asset_tag`. Each venue variant expects its single
///   reserve/lending account in `remaining_accounts`; a plain (`ASSET_TAG_DEFAULT`) bank expects
///   none. `ASSET_TAG_SOL` / `ASSET_TAG_STAKED` banks have no fixed-price mode and are rejected.
/// * `PTPyth` - a PT token priced as `base_pyth_feed * linear_rate`. Pass `[Pyth base feed,
///   Exponent vault]`; `price` is the linear-pricing start price, in (0, 1].
/// * `PTFixed` - a PT token priced as `linear_rate` against an implicit $1 base (no external feed,
///   e.g. a PT of a ~$1 stable). Pass `[Exponent vault]`; `price` is the start price, in (0, 1].
///
/// Any other setup must be configured through `configure_bank_oracle` and is rejected here.
pub fn lending_pool_set_oracle_price(
    ctx: Context<LendingPoolSetOraclePrice>,
    price: WrappedI80F48,
    setup: u8,
) -> MarginfiResult {
    let mut bank = ctx.accounts.bank.load_mut()?;

    if bank.get_flag(FREEZE_SETTINGS) {
        panic!("cannot change oracle settings on frozen banks");
    }

    check!(
        !bank.get_flag(BANK_SAME_ASSET_EMODE_ELIGIBLE),
        MarginfiError::BadEmodeConfig,
        "disable same-asset e-mode eligibility before setting a fixed price"
    );

    // Staked banks must always price against their SVSP/stake-pool accounts and must never switch
    // oracle types; they inherit settings by propagation instead.
    if bank.config.asset_tag == ASSET_TAG_STAKED {
        msg!("Staked banks cannot set a fixed price");
        return err!(MarginfiError::Unauthorized);
    }

    let price_i80: I80F48 = price.into();
    let requested = OracleSetup::from_u8(setup).unwrap_or_else(|| panic!("unsupported oracle type"));
    let remaining = ctx.remaining_accounts;

    match requested {
        OracleSetup::PTPyth => {
            check!(
                remaining.len() == 2,
                MarginfiError::WrongNumberOfOracleAccounts
            );
            check!(
                price_i80 > I80F48::ZERO && price_i80 <= I80F48::ONE,
                MarginfiError::InvalidPtStartPrice
            );
            bank.config.oracle_setup = OracleSetup::PTPyth;
            // [0] Pyth base feed, [1] Exponent vault
            bank.config.oracle_keys[0] = remaining[0].key();
            bank.config.oracle_keys[1] = remaining[1].key();
        }
        OracleSetup::PTFixed => {
            check!(
                remaining.len() == 1,
                MarginfiError::WrongNumberOfOracleAccounts
            );
            check!(
                price_i80 > I80F48::ZERO && price_i80 <= I80F48::ONE,
                MarginfiError::InvalidPtStartPrice
            );
            bank.config.oracle_setup = OracleSetup::PTFixed;
            // [0] Exponent vault (no base feed)
            bank.config.oracle_keys[0] = remaining[0].key();
        }
        OracleSetup::Fixed => {
            check!(
                price_i80 >= I80F48::ZERO,
                MarginfiError::FixedOraclePriceNegative
            );
            bank.config.oracle_setup = match bank.config.asset_tag {
                ASSET_TAG_DEFAULT => OracleSetup::Fixed,
                ASSET_TAG_KAMINO => OracleSetup::FixedKamino,
                ASSET_TAG_DRIFT => OracleSetup::FixedDrift,
                ASSET_TAG_JUPLEND => OracleSetup::FixedJuplend,
                _ => return err!(MarginfiError::InvalidOracleSetup),
            };
            // Note: We leave the other keys in place to make it easier to restore Kamino/JupLend/etc
            // banks to their original state (the venue reserve/lending account stays in keys[1] and
            // is re-checked by validation). This can leave fixed banks in a somewhat awkward-looking
            // state where oracles[0] is empty and other slots are not.
            bank.config.oracle_keys[0] = Pubkey::default();
        }
        _ => {
            return err!(MarginfiError::InvalidOracleSetup);
        }
    }

    bank.config.fixed_price = price;

    bank.config
        .validate_oracle_setup(bank.mint, remaining, None, None, None)?;

    emit!(LendingPoolBankSetOraclePriceEvent {
        header: GroupEventHeader {
            marginfi_group: ctx.accounts.group.key(),
            signer: Some(*ctx.accounts.admin.key),
        },
        bank: ctx.accounts.bank.key(),
        price,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct LendingPoolSetOraclePrice<'info> {
    #[account(
        has_one = admin
    )]
    pub group: AccountLoader<'info, MarginfiGroup>,

    pub admin: Signer<'info>,

    #[account(
        mut,
        has_one = group
    )]
    pub bank: AccountLoader<'info, Bank>,
}
