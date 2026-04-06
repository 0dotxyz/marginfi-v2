use crate::{
    bank_signer, check,
    constants::PROGRAM_VERSION,
    debug,
    events::{AccountEventHeader, LendingPoolBankHandleBankruptcyEvent},
    math_error,
    prelude::MarginfiError,
    state::{
        bank::{BankImpl, BankVaultType},
        marginfi_account::{check_account_bankrupt, BankAccountWrapper, MarginfiAccountImpl},
        marginfi_group::MarginfiGroupImpl,
    },
    utils::{
        self, fetch_unbiased_price_for_bank, is_marginfi_asset_tag, validate_bank_state,
        InstructionKind,
    },
    MarginfiResult,
};
use anchor_lang::prelude::*;
use anchor_spl::token_interface::{TokenAccount, TokenInterface};
use bytemuck::Zeroable;
use fixed::types::I80F48;
use marginfi_type_crate::{
    constants::{
        INSURANCE_VAULT_AUTHORITY_SEED, INSURANCE_VAULT_SEED, LIQUIDITY_VAULT_SEED,
        PERMISSIONLESS_BAD_DEBT_SETTLEMENT_FLAG, ZERO_AMOUNT_THRESHOLD,
    },
    types::{
        Bank, BankOperationalState, HealthCache, MarginfiAccount, MarginfiGroup, ACCOUNT_DISABLED,
        ACCOUNT_IN_FLASHLOAN, ACCOUNT_IN_RECEIVERSHIP,
    },
};
use std::cmp::{max, min};

/// Handle a bankrupt marginfi account.
/// 1. Verify account is bankrupt, and lending account belonging to account contains bad debt.
/// 2. Determine the amount of bad debt covered by the insurance fund and the amount socialized between depositors.
/// 3. Cover the bad debt of the bankrupt account.
/// 4. Transfer the insured amount from the insurance fund.
/// 5. Socialize the loss between lenders if any.
pub fn lending_pool_handle_bankruptcy<'info>(
    mut ctx: Context<'_, '_, 'info, 'info, LendingPoolHandleBankruptcy<'info>>,
) -> MarginfiResult {
    let LendingPoolHandleBankruptcy {
        marginfi_account: marginfi_account_loader,
        insurance_vault,
        token_program,
        bank: bank_loader,
        group: marginfi_group_loader,
        ..
    } = ctx.accounts;
    let maybe_bank_mint = {
        let bank = bank_loader.load()?;
        let group = marginfi_group_loader.load()?;
        let signer = ctx.accounts.signer.key();
        let is_admin_or_risk_admin = signer == group.risk_admin || signer == group.admin;
        let permissionless_bad_debt_settlement =
            bank.get_flag(PERMISSIONLESS_BAD_DEBT_SETTLEMENT_FLAG);

        if permissionless_bad_debt_settlement {
            // if permissionless, users can bankrupt reduce-only or operational banks
            validate_bank_state(&bank, InstructionKind::FailsInPausedState)?;
        } else {
            // admin can bankrupt banks in any state
            validate_bank_state(&bank, InstructionKind::Unrestricted)?;
            check!(is_admin_or_risk_admin, MarginfiError::Unauthorized);
        }

        utils::maybe_take_bank_mint(&mut ctx.remaining_accounts, &bank, token_program.key)?
    };

    let clock = Clock::get()?;

    let mut marginfi_account = marginfi_account_loader.load_mut()?;

    let mut health_cache = HealthCache::zeroed();
    health_cache.timestamp = clock.unix_timestamp;
    health_cache.program_version = PROGRAM_VERSION;

    check_account_bankrupt(
        &marginfi_account,
        ctx.remaining_accounts,
        &mut Some(&mut health_cache),
    )?;

    let bank = bank_loader.load()?;
    let cached_price =
        fetch_unbiased_price_for_bank(&bank_loader.key(), &bank, &clock, ctx.remaining_accounts)
            .ok();
    drop(bank);

    health_cache.set_engine_ok(true);
    marginfi_account.health_cache = health_cache;

    let mut bank = bank_loader.load_mut()?;
    let group = &marginfi_group_loader.load()?;

    bank.accrue_interest(
        clock.unix_timestamp,
        group,
        #[cfg(not(feature = "client"))]
        bank_loader.key(),
    )?;

    let lending_account_balance = marginfi_account
        .lending_account
        .balances
        .iter_mut()
        .find(|balance| balance.is_active() && balance.bank_pk == bank_loader.key());

    check!(
        lending_account_balance.is_some(),
        MarginfiError::LendingAccountBalanceNotFound
    );

    let lending_account_balance = lending_account_balance.unwrap();

    let bad_debt: I80F48 =
        bank.get_liability_amount(lending_account_balance.liability_shares.into())?;

    check!(
        bad_debt > ZERO_AMOUNT_THRESHOLD,
        MarginfiError::BalanceNotBadDebt
    );

    let (covered_by_insurance, socialized_loss) = {
        let available_insurance_fund: I80F48 = maybe_bank_mint
            .as_ref()
            .map(|mint| {
                utils::calculate_post_fee_spl_deposit_amount(
                    mint.to_account_info(),
                    insurance_vault.amount,
                    clock.epoch,
                )
            })
            .transpose()?
            .unwrap_or(insurance_vault.amount)
            .into();

        let covered_by_insurance = min(bad_debt, available_insurance_fund);
        let socialized_loss = max(bad_debt - covered_by_insurance, I80F48::ZERO);

        (covered_by_insurance, socialized_loss)
    };

    // Cover bad debt with insurance funds.
    let covered_by_insurance_rounded_up: u64 = covered_by_insurance
        .checked_ceil()
        .ok_or_else(math_error!())?
        .checked_to_num()
        .ok_or_else(math_error!())?;
    debug!(
        "covered_by_insurance_rounded_up: {}; socialized loss {}",
        covered_by_insurance_rounded_up,
        socialized_loss.to_num::<f64>()
    );

    let insurance_coverage_deposit_pre_fee = maybe_bank_mint
        .as_ref()
        .map(|mint| {
            utils::calculate_pre_fee_spl_deposit_amount(
                mint.to_account_info(),
                covered_by_insurance_rounded_up,
                clock.epoch,
            )
        })
        .transpose()?
        .unwrap_or(covered_by_insurance_rounded_up);

    bank.withdraw_spl_transfer(
        insurance_coverage_deposit_pre_fee,
        ctx.accounts.insurance_vault.to_account_info(),
        ctx.accounts.liquidity_vault.to_account_info(),
        ctx.accounts.insurance_vault_authority.to_account_info(),
        maybe_bank_mint.as_ref(),
        token_program.to_account_info(),
        bank_signer!(
            BankVaultType::Insurance,
            bank_loader.key(),
            bank.insurance_vault_authority_bump
        ),
        ctx.remaining_accounts,
    )?;

    // Socialize bad debt among depositors.
    let kill_bank = bank.socialize_loss(socialized_loss)?;

    // Settle bad debt.
    // The liabilities of this account and global total liabilities are reduced by `bad_debt`
    BankAccountWrapper::find(
        &bank_loader.key(),
        &mut bank,
        &mut marginfi_account.lending_account,
    )?
    .repay(bad_debt)?;

    // Update bank cache after all manipulations (interest accrual, loss socialization, repay)
    bank.update_bank_cache(group)?;
    bank.update_cache_price(cached_price)?;

    marginfi_account.set_flag(ACCOUNT_DISABLED, true);
    marginfi_account.last_update = clock.unix_timestamp as u64;
    if kill_bank {
        msg!("bank had debt exceeding liabilities and has been killed");
        bank.config.operational_state = BankOperationalState::KilledByBankruptcy;
    }

    emit!(LendingPoolBankHandleBankruptcyEvent {
        header: AccountEventHeader {
            signer: Some(ctx.accounts.signer.key()),
            marginfi_account: marginfi_account_loader.key(),
            marginfi_account_authority: marginfi_account.authority,
            marginfi_group: marginfi_account.group,
        },
        bank: bank_loader.key(),
        mint: bank.mint,
        bad_debt: bad_debt.to_num::<f64>(),
        covered_amount: covered_by_insurance.to_num::<f64>(),
        socialized_amount: socialized_loss.to_num::<f64>(),
    });

    Ok(())
}

#[derive(Accounts)]
pub struct LendingPoolHandleBankruptcy<'info> {
    #[account(
        constraint = {
            let g = group.load()?;
            !g.is_protocol_paused() || signer.key() == g.admin || signer.key() == g.risk_admin
        } @ MarginfiError::ProtocolPaused
    )]
    pub group: AccountLoader<'info, MarginfiGroup>,

    /// Must be risk_admin or admin, unless the bank has PERMISSIONLESS_BAD_DEBT_SETTLEMENT_FLAG
    /// set, in which case any signer is accepted.
    pub signer: Signer<'info>,

    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
        constraint = is_marginfi_asset_tag(bank.load()?.config.asset_tag)
            @ MarginfiError::WrongAssetTagForStandardInstructions
    )]
    pub bank: AccountLoader<'info, Bank>,

    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
        constraint = {
            !marginfi_account.load()?.get_flag(ACCOUNT_IN_RECEIVERSHIP)
        } @MarginfiError::UnexpectedLiquidationState,
        constraint = {
            !marginfi_account.load()?.get_flag(ACCOUNT_IN_FLASHLOAN)
        } @MarginfiError::AccountInFlashloan
    )]
    pub marginfi_account: AccountLoader<'info, MarginfiAccount>,

    /// CHECK: Seed constraint
    #[account(
        mut,
        seeds = [
            LIQUIDITY_VAULT_SEED.as_bytes(),
            bank.key().as_ref(),
        ],
        bump = bank.load()?.liquidity_vault_bump
    )]
    pub liquidity_vault: AccountInfo<'info>,

    #[account(
        mut,
        seeds = [
            INSURANCE_VAULT_SEED.as_bytes(),
            bank.key().as_ref(),
        ],
        bump = bank.load()?.insurance_vault_bump
    )]
    pub insurance_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// CHECK: Seed constraint
    #[account(
        seeds = [
            INSURANCE_VAULT_AUTHORITY_SEED.as_bytes(),
            bank.key().as_ref(),
        ],
        bump = bank.load()?.insurance_vault_authority_bump
    )]
    pub insurance_vault_authority: AccountInfo<'info>,

    pub token_program: Interface<'info, TokenInterface>,
}
