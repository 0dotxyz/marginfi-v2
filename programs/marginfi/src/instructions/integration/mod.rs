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
        allows_order_execution, deposit_integration_slots, deposit_protocol_account_count,
        fetch_asset_price_for_bank_low_bias, finalize_integration_deposit,
        finalize_integration_withdraw, is_integration_asset_tag, record_withdrawal_outflow,
        validate_bank_state, validate_integration_deposit, withdraw_integration_slots,
        withdraw_protocol_account_count, InstructionKind,
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

/// Amounts computed while the marginfi balance is debited, before the venue CPI runs.
pub(crate) struct WithdrawAmounts {
    /// Units removed from the marginfi balance: collateral tokens (Kamino/Solend), scaled
    /// balance units (Drift), or fToken shares (JupLend).
    pub balance_units: u64,
    /// Native tokens expected to leave the venue. Kamino/Solend meter collateral units here
    /// because the redeemed liquidity is only known after the CPI.
    pub tokens: u64,
    /// Amount reported in the withdraw event, in the venue's established convention:
    /// collateral units for Kamino/Solend, native tokens for Drift/JupLend.
    pub event_amount: u64,
    pub shares: I80F48,
}

/// Weaves the named integration accounts into the venue's protocol account layout at `slots`,
/// filling every other slot from `remaining` in order. Consumes exactly
/// `total - slots.len()` accounts from `remaining`.
///
/// The assembled vec is leaked to get a true `'info` borrow (the bump allocator never frees, so
/// this costs nothing).
fn assemble_protocol_accounts<'info>(
    slots: &'static [(usize, u8)],
    total: usize,
    integration_accounts: [Option<AccountInfo<'info>>; 3],
    remaining: &'info [AccountInfo<'info>],
) -> MarginfiResult<&'info [AccountInfo<'info>]> {
    let mut assembled = Vec::with_capacity(total);
    let mut filler = remaining.iter();
    for slot in 0..total {
        match slots.iter().find(|(s, _)| *s == slot) {
            Some((_, acc_number)) => assembled.push(
                integration_accounts[*acc_number as usize - 1]
                    .clone()
                    .ok_or_else(|| error!(MarginfiError::IntegrationAccountKeyMismatch))?,
            ),
            None => assembled.push(
                filler
                    .next()
                    .ok_or_else(|| error!(MarginfiError::IntegrationAccountCountMismatch))?
                    .clone(),
            ),
        }
    }
    Ok(assembled.leak())
}

pub fn integration_deposit<'info>(
    ctx: Context<'info, IntegrationDeposit<'info>>,
    amount: u64,
    op_mode: IntegrationOpMode,
) -> MarginfiResult {
    let asset_tag = op_mode.to_asset_tag();
    // Checked before weaving so an op_mode/bank mismatch reports precisely rather than as a
    // layout error from the wrong venue's slot table.
    check!(
        ctx.accounts.bank.load()?.config.asset_tag == asset_tag,
        MarginfiError::IntegrationOpModeMismatch
    );
    let common = ctx.accounts.to_common();
    let protocol_accounts = assemble_protocol_accounts(
        deposit_integration_slots(asset_tag),
        deposit_protocol_account_count(asset_tag),
        ctx.accounts.integration_accounts(),
        ctx.remaining_accounts,
    )?;
    integration_deposit_impl(&common, protocol_accounts, amount, Some(asset_tag), false)
}

pub(crate) fn integration_deposit_impl<'info>(
    common: &CommonDeposit<'_, 'info>,
    protocol_accounts: &'info [AccountInfo<'info>],
    amount: u64,
    expected_asset_tag: Option<u8>,
    refresh_reserve: bool,
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
        ASSET_TAG_KAMINO => kamino_handler::deposit(
            protocol_accounts,
            common,
            amount,
            authority_bump,
            refresh_reserve,
        )?,
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
        balance_change,
        inflow_amount,
    )?;

    Ok(())
}

pub fn integration_withdraw<'info>(
    ctx: Context<'info, IntegrationWithdraw<'info>>,
    amount: u64,
    withdraw_all: Option<bool>,
    op_mode: IntegrationOpMode,
) -> MarginfiResult {
    let asset_tag = op_mode.to_asset_tag();
    // Checked before weaving so an op_mode/bank mismatch reports precisely rather than as a
    // layout error from the wrong venue's slot table.
    check!(
        ctx.accounts.bank.load()?.config.asset_tag == asset_tag,
        MarginfiError::IntegrationOpModeMismatch
    );
    let common = ctx.accounts.to_common();
    let slots = withdraw_integration_slots(asset_tag);
    let total = withdraw_protocol_account_count(asset_tag);
    let filler_count = total - slots.len();
    check!(
        ctx.remaining_accounts.len() >= filler_count,
        MarginfiError::IntegrationAccountCountMismatch
    );
    let protocol_accounts = assemble_protocol_accounts(
        slots,
        total,
        ctx.accounts.integration_accounts(),
        &ctx.remaining_accounts[..filler_count],
    )?;
    let oracle_accounts = &ctx.remaining_accounts[filler_count..];
    integration_withdraw_impl(
        &common,
        protocol_accounts,
        oracle_accounts,
        amount,
        withdraw_all,
        Some(asset_tag),
        false,
    )
}

pub(crate) fn integration_withdraw_impl<'info>(
    common: &CommonWithdraw<'_, 'info>,
    protocol_accounts: &'info [AccountInfo<'info>],
    oracle_accounts: &'info [AccountInfo<'info>],
    amount: u64,
    withdraw_all: Option<bool>,
    expected_asset_tag: Option<u8>,
    refresh_reserve: bool,
) -> MarginfiResult {
    let withdraw_all = withdraw_all.unwrap_or(false);
    let bank_key = common.bank.key();

    let asset_tag = common.bank.load()?.config.asset_tag;

    if let Some(expected) = expected_asset_tag {
        check!(
            asset_tag == expected,
            MarginfiError::IntegrationOpModeMismatch
        );
    }

    // Protocol-specific pre-refresh (before balance update)
    match asset_tag {
        ASSET_TAG_DRIFT => drift_handler::pre_refresh(protocol_accounts, common)?,
        ASSET_TAG_JUPLEND => juplend_handler::pre_refresh(protocol_accounts, common)?,
        _ => {}
    }

    // Balance update + rate limiting + deleverage tracking
    let authority_bump: u8;
    let amounts = {
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

        let amounts = match asset_tag {
            ASSET_TAG_KAMINO | ASSET_TAG_SOLEND => {
                let in_receivership = marginfi_account.get_flag(ACCOUNT_IN_RECEIVERSHIP);
                let mut ba = BankAccountWrapper::find(
                    &bank_key,
                    &mut bank,
                    &mut marginfi_account.lending_account,
                )?;
                let (collateral_amount, shares) = if withdraw_all {
                    ba.withdraw_all(in_receivership)?
                } else {
                    let shares = ba.withdraw(I80F48::from_num(amount))?;
                    (amount, shares)
                };
                WithdrawAmounts {
                    balance_units: collateral_amount,
                    tokens: collateral_amount,
                    event_amount: collateral_amount,
                    shares,
                }
            }
            ASSET_TAG_DRIFT => drift_handler::pre_withdraw(
                protocol_accounts,
                &mut bank,
                &mut marginfi_account,
                &bank_key,
                amount,
                withdraw_all,
            )?,
            ASSET_TAG_JUPLEND => juplend_handler::pre_withdraw(
                protocol_accounts,
                &mut bank,
                &mut marginfi_account,
                &bank_key,
                amount,
                withdraw_all,
            )?,
            _ => return err!(MarginfiError::UnsupportedIntegration),
        };

        // Metered in the venue's established convention: collateral units for Kamino/Solend,
        // native tokens for Drift/JupLend (for a Drift partial withdraw with an off-by-one
        // scaled-unit clamp this is strictly less than the requested `amount`).
        record_withdrawal_outflow(
            group_rate_limit_enabled,
            amounts.tokens,
            amounts.balance_units,
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
                I80F48::from_num(amounts.balance_units),
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

        amounts
    };

    // Protocol-specific CPI + verification + transfer
    match asset_tag {
        ASSET_TAG_KAMINO => kamino_handler::withdraw_cpi(
            protocol_accounts,
            common,
            amounts.balance_units,
            authority_bump,
            refresh_reserve,
        )?,
        ASSET_TAG_DRIFT => {
            drift_handler::withdraw_cpi(protocol_accounts, common, &amounts, authority_bump)?
        }
        ASSET_TAG_SOLEND => solend_handler::withdraw_cpi(
            protocol_accounts,
            common,
            amounts.balance_units,
            authority_bump,
        )?,
        ASSET_TAG_JUPLEND => {
            juplend_handler::withdraw_cpi(protocol_accounts, common, &amounts, authority_bump)?
        }
        _ => return err!(MarginfiError::UnsupportedIntegration),
    };

    // Finalize: event emission, health check, price cache update
    finalize_integration_withdraw(
        common.marginfi_account,
        common.bank,
        common.authority.key(),
        amounts.event_amount,
        amounts.shares,
        withdraw_all,
        oracle_accounts,
        &Clock::get()?,
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
    let cpi_ctx = CpiContext::new(common.token_program.key(), cpi_accounts);
    transfer_checked(cpi_ctx, amount, common.mint_decimals)?;
    Ok(())
}

/// Transfers `amount` of the bank mint from `from` (a token account owned by the bank's
/// liquidity vault authority) to the caller's destination token account.
pub(crate) fn cpi_transfer_to_destination<'info>(
    common: &CommonWithdraw<'_, 'info>,
    from: AccountInfo<'info>,
    authority_bump: u8,
    amount: u64,
) -> MarginfiResult {
    let bank_key = common.bank.key();
    let cpi_accounts = TransferChecked {
        from,
        to: common.destination_token_account.clone(),
        authority: common.liquidity_vault_authority.clone(),
        mint: common.mint.clone(),
    };
    let signer_seeds: &[&[&[u8]]] =
        bank_signer!(BankVaultType::Liquidity, bank_key, authority_bump);
    let cpi_ctx =
        CpiContext::new_with_signer(common.token_program.key(), cpi_accounts, signer_seeds);
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
        has_one = integration_acc_1 @ MarginfiError::IntegrationAccountKeyMismatch,
        has_one = integration_acc_2 @ MarginfiError::IntegrationAccountKeyMismatch,
        constraint = bank.load()?.integration_acc_3
            == integration_acc_3.as_ref().map(|a| a.key()).unwrap_or_default()
            @ MarginfiError::IntegrationAccountKeyMismatch,
        constraint = is_integration_asset_tag(bank.load()?.config.asset_tag)
            @ MarginfiError::UnsupportedIntegration
    )]
    pub bank: AccountLoader<'info, Bank>,

    /// The bank's first integration account (Kamino/Solend reserve, Drift spot market,
    /// JupLend lending).
    /// CHECK: validated against the bank's stored key
    #[account(mut)]
    pub integration_acc_1: UncheckedAccount<'info>,

    /// The bank's second integration account (Kamino/Solend obligation, Drift user,
    /// JupLend fToken vault).
    /// CHECK: validated against the bank's stored key
    #[account(mut)]
    pub integration_acc_2: UncheckedAccount<'info>,

    /// The bank's third integration account (Drift user stats, JupLend withdraw intermediary
    /// ATA). Omit for banks whose third slot is unset (Kamino, Solend).
    /// CHECK: validated against the bank's stored key
    #[account(mut)]
    pub integration_acc_3: Option<UncheckedAccount<'info>>,

    /// Owned by authority, the source account for the token deposit
    #[account(mut)]
    pub signer_token_account: InterfaceAccount<'info, TokenAccount>,

    /// The bank's liquidity vault authority, which owns the venue's position accounts
    #[account(
        mut,
        seeds = [
            LIQUIDITY_VAULT_AUTHORITY_SEED.as_bytes(),
            bank.key().as_ref()
        ],
        bump = bank.load()?.liquidity_vault_authority_bump
    )]
    pub liquidity_vault_authority: SystemAccount<'info>,

    /// Used as an intermediary to deposit tokens into the venue
    #[account(mut)]
    pub liquidity_vault: InterfaceAccount<'info, TokenAccount>,

    /// Bank's liquidity token mint
    pub mint: Box<InterfaceAccount<'info, Mint>>,

    pub token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct IntegrationWithdraw<'info> {
    #[account(
        constraint = (
            !group.load()?.is_protocol_paused()
                || marginfi_account.load()?.get_flag(ACCOUNT_IN_DELEVERAGE)
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
            let allow_order_execution = allows_order_execution(bank.load()?.config.asset_tag);
            is_signer_authorized(&a, g.admin, authority.key(), true, allow_order_execution)
        } @ MarginfiError::Unauthorized
    )]
    pub marginfi_account: AccountLoader<'info, MarginfiAccount>,

    pub authority: Signer<'info>,

    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
        has_one = liquidity_vault @ MarginfiError::InvalidLiquidityVault,
        has_one = mint @ MarginfiError::InvalidMint,
        has_one = integration_acc_1 @ MarginfiError::IntegrationAccountKeyMismatch,
        has_one = integration_acc_2 @ MarginfiError::IntegrationAccountKeyMismatch,
        constraint = bank.load()?.integration_acc_3
            == integration_acc_3.as_ref().map(|a| a.key()).unwrap_or_default()
            @ MarginfiError::IntegrationAccountKeyMismatch,
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

    /// The bank's first integration account (Kamino/Solend reserve, Drift spot market,
    /// JupLend lending).
    /// CHECK: validated against the bank's stored key
    #[account(mut)]
    pub integration_acc_1: UncheckedAccount<'info>,

    /// The bank's second integration account (Kamino/Solend obligation, Drift user,
    /// JupLend fToken vault).
    /// CHECK: validated against the bank's stored key
    #[account(mut)]
    pub integration_acc_2: UncheckedAccount<'info>,

    /// The bank's third integration account (Drift user stats, JupLend withdraw intermediary
    /// ATA). Omit for banks whose third slot is unset (Kamino, Solend).
    /// CHECK: validated against the bank's stored key
    #[account(mut)]
    pub integration_acc_3: Option<UncheckedAccount<'info>>,

    /// Token account that will receive the withdrawn tokens. Mint/owner are validated by the
    /// SPL transfer; the caller controls the destination.
    #[account(mut)]
    pub destination_token_account: InterfaceAccount<'info, TokenAccount>,

    /// The bank's liquidity vault authority, which owns the venue's position accounts
    #[account(
        mut,
        seeds = [
            LIQUIDITY_VAULT_AUTHORITY_SEED.as_bytes(),
            bank.key().as_ref()
        ],
        bump = bank.load()?.liquidity_vault_authority_bump
    )]
    pub liquidity_vault_authority: SystemAccount<'info>,

    /// Receives tokens from the venue withdrawal before they are forwarded to the destination
    /// (JupLend routes through the intermediary ATA instead)
    #[account(mut)]
    pub liquidity_vault: InterfaceAccount<'info, TokenAccount>,

    /// Bank's liquidity token mint
    pub mint: Box<InterfaceAccount<'info, Mint>>,

    pub token_program: Interface<'info, TokenInterface>,
}

/// Implements `to_common` mapping an accounts struct onto [`CommonDeposit`]. The optional second
/// argument names the token-program field when it is not `token_program`.
macro_rules! impl_common_deposit {
    ($ty:ident) => {
        impl_common_deposit!($ty, token_program);
    };
    ($ty:ident, $token_program:ident) => {
        impl<'info> $ty<'info> {
            pub(crate) fn to_common(
                &self,
            ) -> $crate::instructions::integration::CommonDeposit<'_, 'info> {
                $crate::instructions::integration::CommonDeposit {
                    group: &self.group,
                    marginfi_account: &self.marginfi_account,
                    authority: &self.authority,
                    bank: &self.bank,
                    signer_token_account: self.signer_token_account.to_account_info(),
                    liquidity_vault_authority: self.liquidity_vault_authority.to_account_info(),
                    liquidity_vault: self.liquidity_vault.to_account_info(),
                    mint: self.mint.to_account_info(),
                    mint_decimals: self.mint.decimals,
                    token_program: self.$token_program.to_account_info(),
                }
            }
        }
    };
}
pub(crate) use impl_common_deposit;

/// Implements `to_common` mapping an accounts struct onto [`CommonWithdraw`]. The optional
/// second and third arguments name the liquidity-vault and token-program fields when they are
/// not `liquidity_vault` / `token_program`.
macro_rules! impl_common_withdraw {
    ($ty:ident) => {
        impl_common_withdraw!($ty, liquidity_vault, token_program);
    };
    ($ty:ident, $liquidity_vault:ident, $token_program:ident) => {
        impl<'info> $ty<'info> {
            pub(crate) fn to_common(
                &self,
            ) -> $crate::instructions::integration::CommonWithdraw<'_, 'info> {
                $crate::instructions::integration::CommonWithdraw {
                    group: &self.group,
                    marginfi_account: &self.marginfi_account,
                    authority: &self.authority,
                    bank: &self.bank,
                    destination_token_account: self.destination_token_account.to_account_info(),
                    liquidity_vault_authority: self.liquidity_vault_authority.to_account_info(),
                    liquidity_vault: self.$liquidity_vault.to_account_info(),
                    mint: self.mint.to_account_info(),
                    mint_decimals: self.mint.decimals,
                    token_program: self.$token_program.to_account_info(),
                }
            }
        }
    };
}
pub(crate) use impl_common_withdraw;

impl_common_deposit!(IntegrationDeposit);
impl_common_withdraw!(IntegrationWithdraw);

macro_rules! impl_integration_accounts {
    ($ty:ident) => {
        impl<'info> $ty<'info> {
            /// The named integration accounts, indexed by integration account number minus one.
            fn integration_accounts(&self) -> [Option<AccountInfo<'info>>; 3] {
                [
                    Some(self.integration_acc_1.to_account_info()),
                    Some(self.integration_acc_2.to_account_info()),
                    self.integration_acc_3.as_ref().map(|a| a.to_account_info()),
                ]
            }
        }
    };
}
impl_integration_accounts!(IntegrationDeposit);
impl_integration_accounts!(IntegrationWithdraw);

impl Hashable for IntegrationWithdraw<'_> {
    fn get_hash() -> [u8; 8] {
        get_discrim_hash("global", "integration_withdraw")
    }
}
