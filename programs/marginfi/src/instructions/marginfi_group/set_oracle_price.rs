use crate::events::{GroupEventHeader, LendingPoolBankSetOraclePriceEvent};
use crate::state::bank::BankImpl;
use crate::state::bank_config::BankConfigImpl;
use crate::{check, errors::MarginfiError, MarginfiResult};
use anchor_lang::prelude::*;
use fixed::types::I80F48;
use marginfi_type_crate::constants::{
    ASSET_TAG_DRIFT, ASSET_TAG_JUPLEND, ASSET_TAG_KAMINO, ASSET_TAG_STAKED,
};
use marginfi_type_crate::{
    constants::{BANK_SAME_ASSET_EMODE_ELIGIBLE, FREEZE_SETTINGS},
    types::{Bank, MarginfiGroup, OracleSetup, WrappedI80F48},
};

pub fn lending_pool_set_oracle_price(
    ctx: Context<LendingPoolSetOraclePrice>,
    price: WrappedI80F48,
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

    // Technically there is nothing wrong with allowing this on staked banks, but since they can
    // always inherit settings by propagation, this would be silly. There's also no reason we'd want
    // to do this anyways.
    if bank.config.asset_tag == ASSET_TAG_STAKED {
        msg!("Staked banks cannot set a fixed price");
        return err!(MarginfiError::Unauthorized);
    }

    let price_i80: I80F48 = price.into();

    // Two accounts ([Pyth SOL/USD, vault]) signal PTSOL; a single Exponent-owned vault signals
    // PT-hyUSD (hyUSD ~= $1, no base feed). A plain fixed price passes no accounts; the Fixed*
    // integrations pass a single non-vault (reserve/lending) account.
    let is_ptsol = ctx.remaining_accounts.len() == 2;
    let is_pthyusd = ctx.remaining_accounts.len() == 1
        && ctx.remaining_accounts[0].owner == &exponent_mocks::ID;

    if is_ptsol || is_pthyusd {
        // For PT the fixed price is the linear-pricing start price, which lives in (0, 1].
        check!(
            price_i80 > I80F48::ZERO && price_i80 <= I80F48::ONE,
            MarginfiError::InvalidPtStartPrice
        );
    } else {
        check!(
            price_i80 >= I80F48::ZERO,
            MarginfiError::FixedOraclePriceNegative
        );
    }

    bank.config.oracle_setup = if is_ptsol {
        OracleSetup::PTSOL
    } else if is_pthyusd {
        OracleSetup::PTHYUSD
    } else if bank.config.asset_tag == ASSET_TAG_KAMINO {
        OracleSetup::FixedKamino
    } else if bank.config.asset_tag == ASSET_TAG_DRIFT {
        OracleSetup::FixedDrift
    } else if bank.config.asset_tag == ASSET_TAG_JUPLEND {
        OracleSetup::FixedJuplend
    } else {
        OracleSetup::Fixed
    };

    if is_ptsol {
        // [0] Pyth SOL/USD feed, [1] Exponent vault
        bank.config.oracle_keys[0] = ctx.remaining_accounts[0].key();
        bank.config.oracle_keys[1] = ctx.remaining_accounts[1].key();
    } else if is_pthyusd {
        // [0] Exponent vault (no base feed)
        bank.config.oracle_keys[0] = ctx.remaining_accounts[0].key();
    } else {
        // Note: We leave the other keys in place to make it easier to restore Kamino/Staked/etc
        // banks to their original state. This can leave fixed banks in a somewhat awkward-looking
        // state where oracles[0] is empty and other slots are not.
        bank.config.oracle_keys[0] = Pubkey::default();
    }

    bank.config.fixed_price = price;

    bank.config
        .validate_oracle_setup(ctx.remaining_accounts, None, None, None)?;

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
