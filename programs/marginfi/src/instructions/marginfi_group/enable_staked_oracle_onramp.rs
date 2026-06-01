use super::staked_pool_utils::derive_single_pool_keys_from_vote_and_validate_owner;
use crate::{
    check, check_eq,
    constants::NATIVE_STAKE_ID,
    events::{GroupEventHeader, LendingPoolStakedOracleOnrampEnabledEvent},
    MarginfiError, MarginfiResult,
};
use anchor_lang::prelude::*;
use marginfi_type_crate::{
    constants::{ASSET_TAG_STAKED, STAKED_ORACLE_PRICE_USES_ONRAMP},
    types::{Bank, MarginfiGroup, OracleSetup},
};

pub fn lending_pool_enable_staked_oracle_onramp(
    ctx: Context<LendingPoolEnableStakedOracleOnramp>,
) -> MarginfiResult {
    let mut bank = ctx.accounts.bank.load_mut()?;

    check!(
        bank.config.asset_tag == ASSET_TAG_STAKED,
        MarginfiError::AssetTagMismatch
    );
    check!(
        bank.config.oracle_setup == OracleSetup::StakedWithPythPush,
        MarginfiError::StakePoolValidationFailed
    );

    let validator_vote = ctx.accounts.validator_vote_account.key();
    let (_stake_pool, exp_mint, exp_sol_pool, exp_onramp) =
        derive_single_pool_keys_from_vote_and_validate_owner(
            &ctx.accounts.validator_vote_account.to_account_info(),
        )?;

    check_eq!(
        exp_mint,
        bank.mint,
        MarginfiError::StakePoolValidationFailed
    );
    check_eq!(
        exp_mint,
        bank.config.oracle_keys[1],
        MarginfiError::StakePoolValidationFailed
    );
    check_eq!(
        exp_sol_pool,
        bank.config.oracle_keys[2],
        MarginfiError::StakePoolValidationFailed
    );
    check_eq!(
        exp_onramp,
        ctx.accounts.pool_onramp.key(),
        MarginfiError::StakePoolValidationFailed
    );
    check!(
        ctx.accounts.pool_onramp.owner == &NATIVE_STAKE_ID,
        MarginfiError::StakePoolValidationFailed
    );

    if bank.integration_acc_1 != Pubkey::default() {
        check_eq!(
            bank.integration_acc_1,
            validator_vote,
            MarginfiError::StakePoolValidationFailed
        );
    } else {
        bank.integration_acc_1 = validator_vote;
    }

    if bank.config.oracle_keys[3] != Pubkey::default() {
        check_eq!(
            bank.config.oracle_keys[3],
            exp_onramp,
            MarginfiError::StakePoolValidationFailed
        );
    } else {
        bank.config.oracle_keys[3] = exp_onramp;
    }

    // To be removed once SVSP update is rolled out (likely in 1.10)
    bank.config.config_flags |= STAKED_ORACLE_PRICE_USES_ONRAMP;

    emit!(LendingPoolStakedOracleOnrampEnabledEvent {
        header: GroupEventHeader {
            marginfi_group: ctx.accounts.group.key(),
            signer: Some(*ctx.accounts.admin.key)
        },
        bank: ctx.accounts.bank.key(),
        validator_vote_account: validator_vote,
        pool_onramp: exp_onramp,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct LendingPoolEnableStakedOracleOnramp<'info> {
    #[account(
        has_one = admin @ MarginfiError::Unauthorized,
    )]
    pub group: AccountLoader<'info, MarginfiGroup>,

    pub admin: Signer<'info>,

    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
    )]
    pub bank: AccountLoader<'info, Bank>,

    /// CHECK: validated by vote-program owner check + PDA derivation.
    pub validator_vote_account: UncheckedAccount<'info>,

    /// CHECK: validated against the SPL single-pool on-ramp PDA and native stake owner.
    pub pool_onramp: UncheckedAccount<'info>,
}
