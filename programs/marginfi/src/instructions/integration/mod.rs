pub mod drift_handler;
pub mod juplend_handler;
pub mod kamino_handler;
pub mod solend_handler;

use crate::{
    bank_signer, check,
    events::DeleverageWithdrawFlowEvent,
    ix_utils::{get_discrim_hash, Hashable},
    state::{
        bank::{BankImpl, BankVaultType},
        marginfi_account::{
            account_not_frozen_for_authority, calc_value, is_signer_authorized, BankAccountWrapper,
            MarginfiAccountImpl,
        },
        marginfi_group::MarginfiGroupImpl,
        rate_limiter::GroupRateLimiterImpl,
    },
    utils::{
        fetch_asset_price_for_bank_low_bias, finalize_integration_deposit,
        finalize_integration_withdraw, is_integration_asset_tag, record_withdrawal_outflow,
        validate_bank_state, validate_integration_deposit, withdraw_protocol_account_count,
        InstructionKind,
    },
    MarginfiError, MarginfiResult,
};
use anchor_lang::prelude::*;
use anchor_lang::solana_program::clock::Clock;
use anchor_spl::token_interface::{
    transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked,
};
use fixed::types::I80F48;
use marginfi_type_crate::constants::LIQUIDITY_VAULT_AUTHORITY_SEED;
use marginfi_type_crate::constants::{
    ASSET_TAG_DRIFT, ASSET_TAG_JUPLEND, ASSET_TAG_KAMINO, ASSET_TAG_SOLEND,
};
use marginfi_type_crate::types::{
    Bank, MarginfiAccount, MarginfiGroup, ACCOUNT_DISABLED, ACCOUNT_IN_DELEVERAGE,
    ACCOUNT_IN_ORDER_EXECUTION, ACCOUNT_IN_RECEIVERSHIP,
};

pub use marginfi_type_crate::types::IntegrationOpMode;

pub(crate) fn account_info_slice<'info>(
    accounts: &[AccountInfo<'info>],
) -> &'info [AccountInfo<'info>] {
    // Anchor's AccountLoader APIs require the account slice borrow to be tied to `'info`.
    // Per-venue wrappers rebuild the ordered protocol account list from typed accounts, so we
    // contain that coercion here instead of letting it shape the public instruction accounts.
    unsafe { std::mem::transmute::<&[AccountInfo<'info>], &'info [AccountInfo<'info>]>(accounts) }
}

pub(crate) struct CommonDeposit<'a, 'info> {
    pub group: &'a AccountLoader<'info, MarginfiGroup>,
    pub marginfi_account: &'a AccountLoader<'info, MarginfiAccount>,
    pub authority: &'a Signer<'info>,
    pub bank: &'a AccountLoader<'info, Bank>,
    pub signer_token_account: AccountInfo<'info>,
    pub liquidity_vault_authority: AccountInfo<'info>,
    pub liquidity_vault: AccountInfo<'info>,
    pub mint: AccountInfo<'info>,
    pub mint_decimals: u8,
    pub token_program: AccountInfo<'info>,
}

pub(crate) struct CommonWithdraw<'a, 'info> {
    pub group: &'a AccountLoader<'info, MarginfiGroup>,
    pub marginfi_account: &'a AccountLoader<'info, MarginfiAccount>,
    pub authority: &'a Signer<'info>,
    pub bank: &'a AccountLoader<'info, Bank>,
    pub destination_token_account: AccountInfo<'info>,
    pub liquidity_vault_authority: AccountInfo<'info>,
    pub liquidity_vault: AccountInfo<'info>,
    pub mint: AccountInfo<'info>,
    pub mint_decimals: u8,
    pub token_program: AccountInfo<'info>,
}

pub fn integration_deposit<'info>(
    ctx: Context<'_, '_, 'info, 'info, IntegrationDeposit<'info>>,
    amount: u64,
    op_mode: IntegrationOpMode,
) -> MarginfiResult {
    let common = ctx.accounts.to_common();
    let protocol_accounts = ctx.remaining_accounts;
    integration_deposit_impl(
        &common,
        protocol_accounts,
        amount,
        Some(op_mode.to_asset_tag()),
    )
}

pub(crate) fn integration_deposit_impl<'info>(
    common: &CommonDeposit<'_, 'info>,
    protocol_accounts: &'info [AccountInfo<'info>],
    amount: u64,
    expected_asset_tag: Option<u8>,
) -> MarginfiResult {
    let authority_bump = validate_integration_deposit(common.marginfi_account, common.bank)?;

    let asset_tag = common.bank.load()?.config.asset_tag;

    if let Some(expected) = expected_asset_tag {
        check!(
            asset_tag == expected,
            MarginfiError::IntegrationOpModeMismatch
        );
    }

    cpi_transfer_signer_to_vault(common, amount)?;

    let (balance_change, inflow_amount) = match asset_tag {
        ASSET_TAG_KAMINO => {
            kamino_handler::deposit(protocol_accounts, common, amount, authority_bump)?
        }
        ASSET_TAG_DRIFT => {
            drift_handler::deposit(protocol_accounts, common, amount, authority_bump)?
        }
        ASSET_TAG_SOLEND => {
            solend_handler::deposit(protocol_accounts, common, amount, authority_bump)?
        }
        ASSET_TAG_JUPLEND => {
            juplend_handler::deposit(protocol_accounts, common, amount, authority_bump)?
        }
        _ => return err!(MarginfiError::UnsupportedIntegration),
    };

    finalize_integration_deposit(
        common.group,
        common.marginfi_account,
        common.bank,
        common.authority.key(),
        common.marginfi_account.key(),
        common.bank.key(),
        common.group.key(),
        balance_change,
        inflow_amount,
    )?;

    Ok(())
}

pub fn integration_withdraw<'info>(
    ctx: Context<'_, '_, 'info, 'info, IntegrationWithdraw<'info>>,
    amount: u64,
    withdraw_all: Option<bool>,
    op_mode: IntegrationOpMode,
) -> MarginfiResult {
    let common = ctx.accounts.to_common();
    let asset_tag = op_mode.to_asset_tag();
    let pcount = withdraw_protocol_account_count(asset_tag);
    check!(
        ctx.remaining_accounts.len() >= pcount,
        MarginfiError::IntegrationAccountCountMismatch
    );
    let protocol_accounts = &ctx.remaining_accounts[..pcount];
    let oracle_accounts = &ctx.remaining_accounts[pcount..];
    integration_withdraw_impl(
        &common,
        protocol_accounts,
        oracle_accounts,
        amount,
        withdraw_all,
        Some(asset_tag),
    )
}

pub(crate) fn integration_withdraw_impl<'info>(
    common: &CommonWithdraw<'_, 'info>,
    protocol_accounts: &'info [AccountInfo<'info>],
    oracle_accounts: &'info [AccountInfo<'info>],
    amount: u64,
    withdraw_all: Option<bool>,
    expected_asset_tag: Option<u8>,
) -> MarginfiResult {
    let withdraw_all = withdraw_all.unwrap_or(false);
    let bank_key = common.bank.key();

    let (bank_mint, asset_tag) = {
        let bank = common.bank.load()?;
        (bank.mint, bank.config.asset_tag)
    };

    if let Some(expected) = expected_asset_tag {
        check!(
            asset_tag == expected,
            MarginfiError::IntegrationOpModeMismatch
        );
    }

    //Protocol-specific pre-refresh (before balance update)
    match asset_tag {
        ASSET_TAG_DRIFT => drift_handler::pre_refresh(protocol_accounts, common)?,
        ASSET_TAG_JUPLEND => juplend_handler::pre_refresh(protocol_accounts, common)?,
        _ => {}
    }

    //Balance update + rate limiting + deleverage tracking
    let authority_bump: u8;
    let collateral_amount: u64;
    // For Drift: (token_amount, expected_scaled_balance_change)
    // For JupLend: (token_amount, shares_to_burn)
    // For Kamino/Solend: (collateral_amount, collateral_amount) -- same value
    let (token_amount, balance_unit_amount) = {
        let mut marginfi_account = common.marginfi_account.load_mut()?;
        let mut bank = common.bank.load_mut()?;
        let group = common.group.load()?;
        let clock = Clock::get()?;
        authority_bump = bank.liquidity_vault_authority_bump;

        validate_bank_state(&bank, InstructionKind::FailsInPausedState)?;

        let in_receivership_or_order_execution =
            marginfi_account.get_flag(ACCOUNT_IN_RECEIVERSHIP | ACCOUNT_IN_ORDER_EXECUTION);
        let group_rate_limit_enabled = group.rate_limiter.is_enabled();
        let price = if in_receivership_or_order_execution || group_rate_limit_enabled {
            let price =
                fetch_asset_price_for_bank_low_bias(&bank_key, &bank, &clock, oracle_accounts)?;

            if in_receivership_or_order_execution {
                check!(price > I80F48::ZERO, MarginfiError::ZeroAssetPrice);
            }

            price
        } else {
            I80F48::ZERO
        };

        let (ca, token_amt, balance_unit) = match asset_tag {
            ASSET_TAG_KAMINO | ASSET_TAG_SOLEND => {
                let in_receivership = marginfi_account.get_flag(ACCOUNT_IN_RECEIVERSHIP);
                let mut ba = BankAccountWrapper::find(
                    &bank_key,
                    &mut bank,
                    &mut marginfi_account.lending_account,
                )?;
                let ca = if withdraw_all {
                    ba.withdraw_all(in_receivership)?
                } else {
                    ba.withdraw(I80F48::from_num(amount))?;
                    amount
                };
                (ca, ca, ca)
            }
            ASSET_TAG_DRIFT => {
                let (token_amount, expected_scaled_balance_change) = drift_handler::pre_withdraw(
                    protocol_accounts,
                    &mut bank,
                    &mut marginfi_account,
                    &bank_key,
                    amount,
                    withdraw_all,
                )?;
                (
                    expected_scaled_balance_change,
                    token_amount,
                    expected_scaled_balance_change,
                )
            }
            ASSET_TAG_JUPLEND => {
                let (token_amount, shares_to_burn) = juplend_handler::pre_withdraw(
                    protocol_accounts,
                    &mut bank,
                    &mut marginfi_account,
                    &bank_key,
                    amount,
                    withdraw_all,
                )?;
                (shares_to_burn, token_amount, shares_to_burn)
            }
            _ => return err!(MarginfiError::UnsupportedIntegration),
        };
        collateral_amount = ca;

        let rate_limit_amount = if withdraw_all { token_amt } else { amount };
        record_withdrawal_outflow(
            group_rate_limit_enabled,
            rate_limit_amount,
            price,
            &mut bank,
            &group,
            common.group.key(),
            bank_key,
            &marginfi_account,
            &clock,
        )?;

        if marginfi_account.get_flag(ACCOUNT_IN_DELEVERAGE) {
            let withdrawn_equity = calc_value(
                I80F48::from_num(collateral_amount),
                price,
                bank.get_balance_decimals(),
                None,
            )?;
            group.check_deleverage_withdraw_limit(withdrawn_equity, clock.unix_timestamp)?;
            emit!(DeleverageWithdrawFlowEvent {
                group: common.group.key(),
                bank: bank_key,
                mint: bank.mint,
                outflow_usd: withdrawn_equity.to_num(),
                current_timestamp: clock.unix_timestamp,
            });
        }

        bank.update_bank_cache(&group)?;
        marginfi_account.last_update = clock.unix_timestamp as u64;

        (token_amt, balance_unit)
    };

    //Protocol-specific CPI + verification + transfer
    let received = match asset_tag {
        ASSET_TAG_KAMINO => kamino_handler::withdraw_cpi(
            protocol_accounts,
            common,
            collateral_amount,
            authority_bump,
        )?,
        ASSET_TAG_DRIFT => drift_handler::withdraw_cpi(
            protocol_accounts,
            common,
            token_amount,
            balance_unit_amount,
            authority_bump,
        )?,
        ASSET_TAG_SOLEND => solend_handler::withdraw_cpi(
            protocol_accounts,
            common,
            collateral_amount,
            authority_bump,
        )?,
        ASSET_TAG_JUPLEND => juplend_handler::withdraw_cpi(
            protocol_accounts,
            common,
            token_amount,
            balance_unit_amount,
            authority_bump,
        )?,
        _ => return err!(MarginfiError::UnsupportedIntegration),
    };

    let clock = Clock::get()?;
    let event_amount = match asset_tag {
        ASSET_TAG_KAMINO | ASSET_TAG_SOLEND => collateral_amount,
        _ => received,
    };

    // Finalize: event emission, health check, price cache update
    finalize_integration_withdraw(
        common.marginfi_account,
        common.bank,
        bank_key,
        bank_mint,
        common.authority.key(),
        common.marginfi_account.key(),
        event_amount,
        withdraw_all,
        oracle_accounts,
        &clock,
    )?;

    Ok(())
}

fn cpi_transfer_signer_to_vault(common: &CommonDeposit, amount: u64) -> MarginfiResult {
    let cpi_accounts = TransferChecked {
        from: common.signer_token_account.clone(),
        to: common.liquidity_vault.clone(),
        authority: common.authority.to_account_info(),
        mint: common.mint.clone(),
    };
    let cpi_ctx = CpiContext::new(common.token_program.clone(), cpi_accounts);
    transfer_checked(cpi_ctx, amount, common.mint_decimals)?;
    Ok(())
}

pub(crate) fn cpi_transfer_vault_to_destination(
    common: &CommonWithdraw,
    bank_key: Pubkey,
    authority_bump: u8,
    amount: u64,
) -> MarginfiResult {
    let cpi_accounts = TransferChecked {
        from: common.liquidity_vault.clone(),
        to: common.destination_token_account.clone(),
        authority: common.liquidity_vault_authority.clone(),
        mint: common.mint.clone(),
    };
    let signer_seeds: &[&[&[u8]]] =
        bank_signer!(BankVaultType::Liquidity, bank_key, authority_bump);
    let cpi_ctx =
        CpiContext::new_with_signer(common.token_program.clone(), cpi_accounts, signer_seeds);
    transfer_checked(cpi_ctx, amount, common.mint_decimals)?;
    Ok(())
}

#[derive(Accounts)]
pub struct IntegrationDeposit<'info> {
    #[account(
        constraint = (
            !group.load()?.is_protocol_paused()
        ) @ MarginfiError::ProtocolPaused
    )]
    pub group: AccountLoader<'info, MarginfiGroup>,

    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
        constraint = {
            let acc = marginfi_account.load()?;
            !acc.get_flag(ACCOUNT_DISABLED)
        } @MarginfiError::AccountDisabled,
        constraint = {
            let a = marginfi_account.load()?;
            account_not_frozen_for_authority(&a, authority.key())
        } @ MarginfiError::AccountFrozen,
        constraint = {
            let a = marginfi_account.load()?;
            let g = group.load()?;
            is_signer_authorized(&a, g.admin, authority.key(), false, false)
        } @ MarginfiError::Unauthorized
    )]
    pub marginfi_account: AccountLoader<'info, MarginfiAccount>,

    pub authority: Signer<'info>,

    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
        has_one = liquidity_vault @ MarginfiError::InvalidLiquidityVault,
        has_one = mint @ MarginfiError::InvalidMint,
        constraint = is_integration_asset_tag(bank.load()?.config.asset_tag)
            @ MarginfiError::UnsupportedIntegration
    )]
    pub bank: AccountLoader<'info, Bank>,

    #[account(mut)]
    pub signer_token_account: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [
            LIQUIDITY_VAULT_AUTHORITY_SEED.as_bytes(),
            bank.key().as_ref()
        ],
        bump = bank.load()?.liquidity_vault_authority_bump
    )]
    pub liquidity_vault_authority: SystemAccount<'info>,

    #[account(mut)]
    pub liquidity_vault: InterfaceAccount<'info, TokenAccount>,

    pub mint: Box<InterfaceAccount<'info, Mint>>,

    pub token_program: Interface<'info, TokenInterface>,
}

impl<'info> IntegrationDeposit<'info> {
    pub(crate) fn to_common(&self) -> CommonDeposit<'_, 'info> {
        CommonDeposit {
            group: &self.group,
            marginfi_account: &self.marginfi_account,
            authority: &self.authority,
            bank: &self.bank,
            signer_token_account: self.signer_token_account.to_account_info(),
            liquidity_vault_authority: self.liquidity_vault_authority.to_account_info(),
            liquidity_vault: self.liquidity_vault.to_account_info(),
            mint: self.mint.to_account_info(),
            mint_decimals: self.mint.decimals,
            token_program: self.token_program.to_account_info(),
        }
    }
}

#[derive(Accounts)]
pub struct IntegrationWithdraw<'info> {
    #[account(
        constraint = (
            !group.load()?.is_protocol_paused()
        ) @ MarginfiError::ProtocolPaused
    )]
    pub group: AccountLoader<'info, MarginfiGroup>,

    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
        constraint = {
            let acc = marginfi_account.load()?;
            !acc.get_flag(ACCOUNT_DISABLED)
        } @MarginfiError::AccountDisabled,
        constraint = {
            let a = marginfi_account.load()?;
            account_not_frozen_for_authority(&a, authority.key())
        } @ MarginfiError::AccountFrozen,
        constraint = {
            let a = marginfi_account.load()?;
            let g = group.load()?;
            is_signer_authorized(&a, g.admin, authority.key(), true, true)
        } @ MarginfiError::Unauthorized
    )]
    pub marginfi_account: AccountLoader<'info, MarginfiAccount>,

    pub authority: Signer<'info>,

    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
        has_one = liquidity_vault @ MarginfiError::InvalidLiquidityVault,
        has_one = mint @ MarginfiError::InvalidMint,
        constraint = is_integration_asset_tag(bank.load()?.config.asset_tag)
            @ MarginfiError::UnsupportedIntegration,
        constraint = {
            let a = marginfi_account.load()?;
            let b = bank.load()?;
            let weight: I80F48 = b.config.asset_weight_init.into();
            !(a.get_flag(ACCOUNT_IN_RECEIVERSHIP) && weight == I80F48::ZERO)
        } @ MarginfiError::LiquidationPremiumTooHigh
    )]
    pub bank: AccountLoader<'info, Bank>,

    #[account(mut)]
    pub destination_token_account: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [
            LIQUIDITY_VAULT_AUTHORITY_SEED.as_bytes(),
            bank.key().as_ref()
        ],
        bump = bank.load()?.liquidity_vault_authority_bump
    )]
    pub liquidity_vault_authority: SystemAccount<'info>,

    #[account(mut)]
    pub liquidity_vault: InterfaceAccount<'info, TokenAccount>,

    pub mint: Box<InterfaceAccount<'info, Mint>>,

    pub token_program: Interface<'info, TokenInterface>,
}

impl<'info> IntegrationWithdraw<'info> {
    pub(crate) fn to_common(&self) -> CommonWithdraw<'_, 'info> {
        CommonWithdraw {
            group: &self.group,
            marginfi_account: &self.marginfi_account,
            authority: &self.authority,
            bank: &self.bank,
            destination_token_account: self.destination_token_account.to_account_info(),
            liquidity_vault_authority: self.liquidity_vault_authority.to_account_info(),
            liquidity_vault: self.liquidity_vault.to_account_info(),
            mint: self.mint.to_account_info(),
            mint_decimals: self.mint.decimals,
            token_program: self.token_program.to_account_info(),
        }
    }
}

impl Hashable for IntegrationWithdraw<'_> {
    fn get_hash() -> [u8; 8] {
        get_discrim_hash("global", "integration_withdraw")
    }
}
