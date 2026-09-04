use crate::events::{GroupEventHeader, LendingPoolBankConfigureOracleEvent};
use crate::state::bank::BankImpl;
use crate::state::bank_config::BankConfigImpl;
use crate::{check, MarginfiError, MarginfiResult};
use anchor_lang::prelude::*;
use fixed::types::I80F48;
use marginfi_type_crate::constants::{BANK_SAME_ASSET_EMODE_ELIGIBLE, FREEZE_SETTINGS};
use marginfi_type_crate::types::{Bank, MarginfiGroup, OracleSetup};

pub fn lending_pool_configure_bank_oracle(
    ctx: Context<LendingPoolConfigureBankOracle>,
    setup: u8,
    oracle: Pubkey,
) -> MarginfiResult {
    let mut bank = ctx.accounts.bank.load_mut()?;

    // If settings are frozen, you can only update the deposit and borrow limits, so this ix will fail
    if bank.get_flag(FREEZE_SETTINGS) {
        panic!("cannot change oracle settings on frozen banks");
    } else {
        let setup_type =
            OracleSetup::from_u8(setup).unwrap_or_else(|| panic!("unsupported oracle type"));
        if matches!(
            setup_type,
            OracleSetup::Fixed
                | OracleSetup::FixedKamino
                | OracleSetup::FixedDrift
                | OracleSetup::FixedJuplend
                | OracleSetup::PTPyth
                | OracleSetup::PTFixed
        ) {
            return err!(MarginfiError::UseSetOraclePrice);
        }
        // Scope banks must go through `lending_pool_configure_bank_oracle_scope`, which takes
        // the entry index; this instruction has no way to provide it.
        if matches!(setup_type, OracleSetup::Scope) {
            return err!(MarginfiError::UseConfigureBankOracleScope);
        }
        check!(
            !bank.get_flag(BANK_SAME_ASSET_EMODE_ELIGIBLE)
                || (bank.config.oracle_keys[0] == oracle
                    && bank.config.oracle_setup.feed_family() == setup_type.feed_family()),
            MarginfiError::BadEmodeConfig,
            "disable same-asset e-mode eligibility before changing the oracle feed family or oracle_keys[0]"
        );

        bank.config.oracle_setup = setup_type;
        bank.config.oracle_keys[0] = oracle;
        bank.config.fixed_price = I80F48::ZERO.into();

        // mSOL / LST setups carry multiplier oracle key (Marinade State / SPL StakePool) beyond the primary feed, populated here from
        // remaining_accounts (validated immediately below):
        match setup_type {
            OracleSetup::PythMSOL | OracleSetup::PythLST => {
                require!(
                    ctx.remaining_accounts.len() == 2,
                    MarginfiError::WrongNumberOfOracleAccounts
                );
                bank.config.oracle_keys[1] = ctx.remaining_accounts[1].key();
            }
            OracleSetup::KaminoMSOL
            | OracleSetup::JuplendMSOL
            | OracleSetup::KaminoLST
            | OracleSetup::JuplendLST => {
                require!(
                    ctx.remaining_accounts.len() == 3,
                    MarginfiError::WrongNumberOfOracleAccounts
                );
                // Note: integration oracle is not set here to ensure it's only assigned on the bank creation.
                bank.config.oracle_keys[2] = ctx.remaining_accounts[2].key();
            }
            _ => {}
        }

        msg!(
            "setting oracle to type: {:?} key: {:?}",
            bank.config.oracle_setup,
            bank.config.oracle_keys[0]
        );

        bank.config
            .validate_oracle_setup(bank.mint, ctx.remaining_accounts, None, None, None)?;

        emit!(LendingPoolBankConfigureOracleEvent {
            header: GroupEventHeader {
                marginfi_group: ctx.accounts.group.key(),
                signer: Some(*ctx.accounts.admin.key)
            },
            bank: ctx.accounts.bank.key(),
            oracle_setup: setup,
            oracle
        });
    }

    Ok(())
}

/// Configure a bank to price from a Scope feed: sets `OracleSetup::Scope`, the feed's
/// `OraclePrices` account, and the entry index within it, then validates the feed can be read.
///
/// Separate from `lending_pool_configure_bank_oracle` because a Scope bank needs the entry index
/// alongside the account key - the pair `(oracle_keys[0], scope_entry_index)` is what identifies
/// the priced asset.
pub fn lending_pool_configure_bank_oracle_scope(
    ctx: Context<LendingPoolConfigureBankOracle>,
    oracle: Pubkey,
    entry_index: u16,
) -> MarginfiResult {
    let mut bank = ctx.accounts.bank.load_mut()?;

    if bank.get_flag(FREEZE_SETTINGS) {
        panic!("cannot change oracle settings on frozen banks");
    }

    // A Scope bank's priced asset is determined by the entry index as well as the account key, and
    // `feed_family` cannot express that, so same-asset e-mode must be disabled first rather than
    // relying on a family comparison.
    check!(
        !bank.get_flag(BANK_SAME_ASSET_EMODE_ELIGIBLE),
        MarginfiError::BadEmodeConfig,
        "disable same-asset e-mode eligibility before moving a bank to a Scope oracle"
    );

    bank.config.oracle_setup = OracleSetup::Scope;
    bank.config.oracle_keys[0] = oracle;
    bank.config.scope_entry_index = entry_index;
    bank.config.fixed_price = I80F48::ZERO.into();

    msg!(
        "setting scope oracle key: {:?} entry: {:?}",
        oracle,
        entry_index
    );

    bank.config
        .validate_oracle_setup(bank.mint, ctx.remaining_accounts, None, None, None)?;

    emit!(LendingPoolBankConfigureOracleEvent {
        header: GroupEventHeader {
            marginfi_group: ctx.accounts.group.key(),
            signer: Some(*ctx.accounts.admin.key)
        },
        bank: ctx.accounts.bank.key(),
        oracle_setup: OracleSetup::Scope as u8,
        oracle
    });

    Ok(())
}

#[derive(Accounts)]
pub struct LendingPoolConfigureBankOracle<'info> {
    #[account(
        has_one = admin @ MarginfiError::Unauthorized
    )]
    pub group: AccountLoader<'info, MarginfiGroup>,

    pub admin: Signer<'info>,

    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
    )]
    pub bank: AccountLoader<'info, Bank>,
}
