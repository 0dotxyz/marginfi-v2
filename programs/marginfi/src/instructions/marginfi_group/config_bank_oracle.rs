use super::timelocked_utils::*;
use crate::events::{GroupEventHeader, LendingPoolBankConfigureOracleEvent};
use crate::state::bank::BankImpl;
use crate::state::bank_config::BankConfigImpl;
use crate::{MarginfiError, MarginfiResult};
use anchor_lang::prelude::*;
use marginfi_type_crate::constants::FREEZE_SETTINGS;
use marginfi_type_crate::types::{Bank, MarginfiGroup, OracleSetup};

pub fn lending_pool_configure_bank_oracle(
    ctx: Context<LendingPoolConfigureBankOracle>,
    setup: u8,
    oracle: Pubkey,
) -> MarginfiResult {
    {
        let marginfi_group = ctx.accounts.group.load()?;
        assert_timelocked_admin_not_set(&marginfi_group)?;
    }

    let mut bank = ctx.accounts.bank.load_mut()?;

    if bank.get_flag(FREEZE_SETTINGS) {
        panic!("cannot change oracle settings on frozen banks");
    } else {
        let setup_type =
            OracleSetup::from_u8(setup).unwrap_or_else(|| panic!("unsupported oracle type"));
        if setup_type == OracleSetup::Fixed {
            panic!("Use set_fixed_oracle_price instead");
        }

        bank.config.oracle_setup = setup_type;
        bank.config.oracle_keys[0] = oracle;

        msg!(
            "setting oracle to type: {:?} key: {:?}",
            bank.config.oracle_setup,
            bank.config.oracle_keys[0]
        );

        bank.config
            .validate_oracle_setup(ctx.remaining_accounts, None, None, None)?;

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
