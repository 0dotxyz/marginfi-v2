use crate::events::{GroupEventHeader, LendingPoolBankSetFixedOraclePriceEvent};
use crate::state::bank::BankImpl;
use crate::state::bank_config::BankConfigImpl;
use crate::{check, errors::MarginfiError, MarginfiResult};
use anchor_lang::prelude::*;
use fixed::types::I80F48;
use marginfi_type_crate::constants::ASSET_TAG_STAKED;
use marginfi_type_crate::{
    constants::FREEZE_SETTINGS,
    types::{Bank, MarginfiGroup, OracleSetup, WrappedI80F48},
};

pub fn lending_pool_set_fixed_oracle_price(
    ctx: Context<LendingPoolSetFixedOraclePrice>,
    price: WrappedI80F48,
) -> MarginfiResult {
    let mut bank = ctx.accounts.bank.load_mut()?;

    if bank.get_flag(FREEZE_SETTINGS) {
        panic!("cannot change oracle settings on frozen banks");
    }

    // Technically there is nothing wrong with allowing this on staked banks, but since they can
    // always inherit settings by propagation, this would be silly. There's also no reason we'd want
    // to do this anyways.
    if bank.config.asset_tag == ASSET_TAG_STAKED {
        msg!("Staked banks cannot set a fixed price");
        return err!(MarginfiError::Unauthorized);
    }

    bank.config.oracle_setup = OracleSetup::Fixed;
    // Note: We leave the other keys in place to make it easier to restore Kamino/Staked/etc banks
    // to their original state. This can leave fixed banks in a somewhat awkward-looking state where
    // oracles[0] is empty and other slots are not.
    bank.config.oracle_keys[0] = Pubkey::default();

    let price_i80: I80F48 = price.into();
    let price_f64 = price_i80.to_num::<f64>();
    msg!("price: {:?}", price_f64);

    check!(
        price_i80 >= I80F48::ZERO,
        MarginfiError::FixedOraclePriceNegative
    );

    bank.config.fixed_price = price;

    bank.config.validate_oracle_setup(&[], None, None, None)?;

    emit!(LendingPoolBankSetFixedOraclePriceEvent {
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
pub struct LendingPoolSetFixedOraclePrice<'info> {
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
