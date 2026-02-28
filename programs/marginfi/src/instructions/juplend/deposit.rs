use crate::{
    bank_signer,
    events::{AccountEventHeader, LendingAccountDepositEvent},
    state::{
        bank::{BankImpl, BankVaultType},
        marginfi_account::{
            account_not_frozen_for_authority, is_signer_authorized, BankAccountWrapper,
            LendingAccountImpl, MarginfiAccountImpl,
        },
        marginfi_group::MarginfiGroupImpl,
    },
    utils::{
        is_juplend_asset_tag, record_deposit_inflow, validate_asset_tags, validate_bank_state,
        InstructionKind,
    },
    MarginfiError, MarginfiResult,
};
use anchor_lang::prelude::*;
use anchor_lang::solana_program::clock::Clock;
use anchor_spl::token::accessor;
use anchor_spl::token_interface::{
    transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked,
};
use fixed::types::I80F48;
use juplend_mocks::juplend_earn::cpi::accounts::{Deposit, UpdateRate};
use juplend_mocks::juplend_earn::cpi::{deposit, update_rate};
use juplend_mocks::state::{expected_shares_for_deposit_from_rates, Lending as JuplendLending};
use marginfi_type_crate::types::{Bank, MarginfiAccount, MarginfiGroup};
use marginfi_type_crate::{constants::LIQUIDITY_VAULT_AUTHORITY_SEED, types::ACCOUNT_DISABLED};

/// Deposit into a JupLend lending pool through a marginfi account.
///
/// Flow (program-first, exact-math):
/// 1. CPI `update_rate` to refresh `token_exchange_price`.
/// 2. Compute expected fTokens minted: `assets * 1e12 / token_exchange_price` (floor).
/// 3. Transfer underlying from user -> bank liquidity vault.
/// 4. CPI `deposit` (bank vault -> fToken vault).
/// 5. Verify minted fTokens == expected.
/// 6. Credit marginfi asset_shares by minted fTokens.
pub fn juplend_deposit(ctx: Context<JuplendDeposit>, amount: u64) -> MarginfiResult {
    let authority_bump: u8;
    {
        let marginfi_account = ctx.accounts.marginfi_account.load()?;
        let bank = ctx.accounts.bank.load()?;
        authority_bump = bank.liquidity_vault_authority_bump;

        validate_asset_tags(&bank, &marginfi_account)?;
        validate_bank_state(&bank, InstructionKind::FailsIfPausedOrReduceState)?;
    }

    // Refresh the exchange price (interest/rewards) for this slot.
    ctx.accounts.cpi_update_rate()?;

    let expected_shares = {
        let lending = ctx.accounts.integration_acc_1.load()?;
        // Compute expected shares minted (round-down) using the same math as JupLend.
        expected_shares_for_deposit_from_rates(
            amount,
            lending.liquidity_exchange_price,
            lending.token_exchange_price,
        )
        .ok_or_else(|| error!(MarginfiError::MathError))?
    };

    let pre_f_token_balance = accessor::amount(&ctx.accounts.integration_acc_2.to_account_info())?;

    // Move underlying into the vault and deposit into JupLend.
    ctx.accounts.cpi_transfer_user_to_liquidity_vault(amount)?;
    ctx.accounts.cpi_juplend_deposit(amount, authority_bump)?;

    let post_f_token_balance = accessor::amount(&ctx.accounts.integration_acc_2.to_account_info())?;
    let minted_shares = post_f_token_balance
        .checked_sub(pre_f_token_balance)
        .ok_or_else(|| error!(MarginfiError::MathError))?;

    // Exact match required.
    require_eq!(
        minted_shares,
        expected_shares,
        MarginfiError::JuplendDepositFailed
    );

    {
        let mut bank = ctx.accounts.bank.load_mut()?;
        let mut marginfi_account = ctx.accounts.marginfi_account.load_mut()?;
        let mut group = ctx.accounts.group.load_mut()?;
        let clock = Clock::get()?;

        let mut bank_account = BankAccountWrapper::find_or_create(
            &ctx.accounts.bank.key(),
            &mut bank,
            &mut marginfi_account.lending_account,
        )?;

        bank_account.deposit_no_repay(I80F48::from_num(minted_shares))?;

        record_deposit_inflow(
            &mut bank,
            &mut group,
            marginfi_account.account_flags,
            amount,
            &clock,
        )?;

        bank.update_bank_cache(&group)?;

        marginfi_account.last_update = clock.unix_timestamp as u64;
        marginfi_account.lending_account.sort_balances();

        emit!(LendingAccountDepositEvent {
            header: AccountEventHeader {
                signer: Some(ctx.accounts.authority.key()),
                marginfi_account: ctx.accounts.marginfi_account.key(),
                marginfi_account_authority: marginfi_account.authority,
                marginfi_group: marginfi_account.group,
            },
            bank: ctx.accounts.bank.key(),
            mint: bank.mint,
            amount,
        });
    }

    Ok(())
}

#[derive(Accounts)]
pub struct JuplendDeposit<'info> {
    #[account(
        mut,
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
        has_one = integration_acc_1 @ MarginfiError::InvalidJuplendLending,
        has_one = integration_acc_2 @ MarginfiError::InvalidJuplendFTokenVault,
        has_one = mint @ MarginfiError::InvalidMint,
        constraint = is_juplend_asset_tag(bank.load()?.config.asset_tag)
            @ MarginfiError::WrongBankAssetTagForJuplendOperation
    )]
    pub bank: AccountLoader<'info, Bank>,

    /// Owned by authority, the source account for the token deposit.
    #[account(mut)]
    pub signer_token_account: InterfaceAccount<'info, TokenAccount>,

    /// The bank's liquidity vault authority PDA (acts as signer for JupLend CPIs).
    /// NOTE: JupLend marks the signer as writable in their deposit instruction.
    #[account(
        mut,
        seeds = [
            LIQUIDITY_VAULT_AUTHORITY_SEED.as_bytes(),
            bank.key().as_ref(),
        ],
        bump = bank.load()?.liquidity_vault_authority_bump
    )]
    pub liquidity_vault_authority: SystemAccount<'info>,

    /// Bank liquidity vault (holds underlying mint and is used as depositor_token_account).
    #[account(mut)]
    pub liquidity_vault: InterfaceAccount<'info, TokenAccount>,

    /// Underlying mint.
    pub mint: Box<InterfaceAccount<'info, Mint>>,

    /// JupLend lending state account.
    #[account(mut, has_one = f_token_mint @ MarginfiError::InvalidJuplendLending)]
    pub integration_acc_1: AccountLoader<'info, JuplendLending>,

    /// JupLend fToken mint.
    #[account(mut)]
    pub f_token_mint: Box<InterfaceAccount<'info, Mint>>,

    /// Bank's fToken vault (validated via has_one on bank).
    #[account(mut)]
    pub integration_acc_2: InterfaceAccount<'info, TokenAccount>,

    // ---- JupLend CPI accounts ----
    /// CHECK: validated by the JupLend program
    pub lending_admin: UncheckedAccount<'info>,
    /// CHECK: validated by the JupLend program
    #[account(
        mut,
        constraint = supply_token_reserves_liquidity.key() == integration_acc_1.load()?.token_reserves_liquidity
            @ MarginfiError::InvalidJuplendLending,
    )]
    pub supply_token_reserves_liquidity: UncheckedAccount<'info>,
    /// CHECK: validated by the JupLend program
    #[account(
        mut,
        constraint = lending_supply_position_on_liquidity.key() == integration_acc_1.load()?.supply_position_on_liquidity
            @ MarginfiError::InvalidJuplendLending,
    )]
    pub lending_supply_position_on_liquidity: UncheckedAccount<'info>,
    /// CHECK: validated by the JupLend program
    pub rate_model: UncheckedAccount<'info>,
    /// CHECK: validated by the JupLend program
    #[account(mut)]
    pub vault: UncheckedAccount<'info>,
    /// CHECK: validated by the JupLend program
    #[account(mut)]
    pub liquidity: UncheckedAccount<'info>,
    /// CHECK: validated by the JupLend program
    pub liquidity_program: UncheckedAccount<'info>,
    /// CHECK: validated by the JupLend program
    pub rewards_rate_model: UncheckedAccount<'info>,

    /// CHECK: validated against hardcoded program id
    #[account(address = juplend_mocks::ID)]
    pub juplend_program: UncheckedAccount<'info>,

    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, anchor_spl::associated_token::AssociatedToken>,
    pub system_program: Program<'info, System>,
}

impl<'info> JuplendDeposit<'info> {
    pub fn cpi_transfer_user_to_liquidity_vault(&self, amount: u64) -> MarginfiResult {
        let program = self.token_program.to_account_info();
        let accounts = TransferChecked {
            from: self.signer_token_account.to_account_info(),
            to: self.liquidity_vault.to_account_info(),
            authority: self.authority.to_account_info(),
            mint: self.mint.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(program, accounts);
        transfer_checked(cpi_ctx, amount, self.mint.decimals)?;
        Ok(())
    }

    pub fn cpi_update_rate(&self) -> MarginfiResult {
        let accounts = UpdateRate {
            lending: self.integration_acc_1.to_account_info(),
            mint: self.mint.to_account_info(),
            f_token_mint: self.f_token_mint.to_account_info(),
            supply_token_reserves_liquidity: self.supply_token_reserves_liquidity.to_account_info(),
            rewards_rate_model: self.rewards_rate_model.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(self.juplend_program.to_account_info(), accounts);
        update_rate(cpi_ctx)?;
        Ok(())
    }

    pub fn cpi_juplend_deposit(&self, amount: u64, authority_bump: u8) -> MarginfiResult {
        let accounts = Deposit {
            signer: self.liquidity_vault_authority.to_account_info(),
            depositor_token_account: self.liquidity_vault.to_account_info(),
            recipient_token_account: self.integration_acc_2.to_account_info(),
            mint: self.mint.to_account_info(),
            lending_admin: self.lending_admin.to_account_info(),
            lending: self.integration_acc_1.to_account_info(),
            f_token_mint: self.f_token_mint.to_account_info(),
            supply_token_reserves_liquidity: self.supply_token_reserves_liquidity.to_account_info(),
            lending_supply_position_on_liquidity: self
                .lending_supply_position_on_liquidity
                .to_account_info(),
            rate_model: self.rate_model.to_account_info(),
            vault: self.vault.to_account_info(),
            liquidity: self.liquidity.to_account_info(),
            liquidity_program: self.liquidity_program.to_account_info(),
            rewards_rate_model: self.rewards_rate_model.to_account_info(),
            token_program: self.token_program.to_account_info(),
            associated_token_program: self.associated_token_program.to_account_info(),
            system_program: self.system_program.to_account_info(),
        };

        let signer_seeds: &[&[&[u8]]] =
            bank_signer!(BankVaultType::Liquidity, self.bank.key(), authority_bump);

        let cpi_ctx = CpiContext::new_with_signer(
            self.juplend_program.to_account_info(),
            accounts,
            signer_seeds,
        );
        deposit(cpi_ctx, amount)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shares_for_deposit_price_eq_1e12() {
        let shares = expected_shares_for_deposit_from_rates(
            50_000_000,
            1_000_000_000_000,
            1_000_000_000_000,
        )
        .unwrap();
        assert_eq!(shares, 50_000_000);
    }

    #[test]
    fn shares_for_deposit_token_price_above_1e12_mints_less() {
        // floor(100 * 1e12 / 1.1e12) = floor(90.909...) = 90
        let shares =
            expected_shares_for_deposit_from_rates(100, 1_000_000_000_000, 1_100_000_000_000)
                .unwrap();
        assert_eq!(shares, 90);
        assert!(shares < 100);
    }

    #[test]
    fn shares_for_deposit_token_price_below_1e12_mints_more() {
        // floor(100 * 1e12 / 0.9e12) = floor(111.111...) = 111
        let shares =
            expected_shares_for_deposit_from_rates(100, 1_000_000_000_000, 900_000_000_000)
                .unwrap();
        assert_eq!(shares, 111);
        assert!(shares > 100);
    }

    #[test]
    fn shares_for_deposit_zero_price_errors() {
        assert!(expected_shares_for_deposit_from_rates(1, 1_000_000_000_000, 0).is_none());
        assert!(expected_shares_for_deposit_from_rates(1, 0, 1_000_000_000_000).is_none());
    }

    #[test]
    fn shares_for_deposit_non_divisible_rounds_down() {
        // floor(7 * 1e12 / 3e12) = floor(2.333...) = 2
        let shares =
            expected_shares_for_deposit_from_rates(7, 1_000_000_000_000, 3_000_000_000_000)
                .unwrap();
        assert_eq!(shares, 2);
    }

    #[test]
    fn shares_for_deposit_tiny_amount_can_floor_to_zero() {
        // With liquidity_price > 1e12, raw floor can hit zero.
        let shares =
            expected_shares_for_deposit_from_rates(1, 1_100_000_000_000, 1_000_000_000_000)
                .unwrap();
        assert_eq!(shares, 0);
    }
}
