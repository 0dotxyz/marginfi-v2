use crate::{
    check,
    events::QueryMaxBorrowWithdrawEvent,
    state::marginfi_account::{
        check_account_init_health, BankAccountWrapper, LendingAccountImpl, MarginfiAccountImpl,
    },
    utils::{validate_asset_tags, validate_bank_state, InstructionKind},
    MarginfiError, MarginfiResult,
};
use anchor_lang::prelude::*;
use fixed::types::I80F48;
use marginfi_type_crate::types::{
    Bank, MarginfiAccount, MarginfiGroup, ACCOUNT_DISABLED, ACCOUNT_IN_ORDER_EXECUTION,
    ACCOUNT_IN_RECEIVERSHIP,
};

pub const QUERY_TYPE_BORROW: u8 = 0;
pub const QUERY_TYPE_WITHDRAW: u8 = 1;

pub fn query_max_borrow_withdraw<'info>(
    ctx: Context<'info, QueryMaxBorrowWithdraw<'info>>,
    query_type: u8,
) -> MarginfiResult {
    let marginfi_account = ctx.accounts.marginfi_account.load()?;
    let group = ctx.accounts.group.load()?;
    let bank = ctx.accounts.bank.load()?;

    check!(
        query_type == QUERY_TYPE_BORROW || query_type == QUERY_TYPE_WITHDRAW,
        MarginfiError::IllegalAction
    );

    let bank_pk = ctx.accounts.bank.key();
    let (max_amount, is_all) = match query_type {
        QUERY_TYPE_BORROW => {
            let amount = binary_search_max_borrow(
                &marginfi_account,
                &group,
                &bank,
                bank_pk,
                ctx.remaining_accounts,
            )?;
            (amount, false)
        }
        QUERY_TYPE_WITHDRAW => {
            let (amount, is_all) = binary_search_max_withdraw(
                &marginfi_account,
                &group,
                &bank,
                bank_pk,
                ctx.remaining_accounts,
            )?;
            (amount, is_all)
        }
        _ => return err!(MarginfiError::IllegalAction),
    };

    emit!(QueryMaxBorrowWithdrawEvent {
        marginfi_account: ctx.accounts.marginfi_account.key(),
        marginfi_group: marginfi_account.group,
        bank: bank_pk,
        query_type,
        max_amount,
        is_all,
    });

    Ok(())
}

fn binary_search_max_borrow<'info>(
    marginfi_account: &MarginfiAccount,
    group: &MarginfiGroup,
    bank: &Bank,
    bank_pk: Pubkey,
    remaining_ais: &'info [AccountInfo<'info>],
) -> MarginfiResult<u64> {
    check!(
        !marginfi_account.get_flag(ACCOUNT_DISABLED),
        MarginfiError::AccountDisabled
    );

    check!(
        !marginfi_account.get_flag(ACCOUNT_IN_RECEIVERSHIP),
        MarginfiError::AccountDisabled
    );

    validate_asset_tags(bank, marginfi_account)?;
    validate_bank_state(bank, InstructionKind::FailsIfPausedOrReduceState, false)?;

    let available_liquidity = I80F48::from(bank.total_asset_shares)
        .checked_mul(bank.asset_share_value.into())
        .unwrap_or(I80F48::ZERO);

    let available_liquidity_u64 = if available_liquidity > I80F48::ZERO {
        available_liquidity
            .checked_to_num::<u64>()
            .unwrap_or(u64::MAX)
    } else {
        0
    };

    if available_liquidity_u64 == 0 {
        return Ok(0);
    }

    let mut low: u64 = 0;
    let mut high: u64 = available_liquidity_u64;

    while low < high {
        let mid = low.saturating_add(high.saturating_sub(low) / 2);

        match try_borrow(marginfi_account, group, bank, bank_pk, mid, remaining_ais) {
            Ok(()) => {
                low = mid.saturating_add(1);
            }
            Err(_) => {
                high = mid;
            }
        }
    }

    Ok(if low > 0 { low.saturating_sub(1) } else { 0 })
}

fn binary_search_max_withdraw<'info>(
    marginfi_account: &MarginfiAccount,
    group: &MarginfiGroup,
    bank: &Bank,
    bank_pk: Pubkey,
    remaining_ais: &'info [AccountInfo<'info>],
) -> MarginfiResult<(u64, bool)> {
    check!(
        !marginfi_account.get_flag(ACCOUNT_DISABLED),
        MarginfiError::AccountDisabled
    );

    check!(
        !marginfi_account.get_flag(ACCOUNT_IN_ORDER_EXECUTION),
        MarginfiError::AccountDisabled
    );

    validate_asset_tags(bank, marginfi_account)?;

    let withdraw_is_halt_safe = !marginfi_account.lending_account.has_liabilities();
    validate_bank_state(
        bank,
        InstructionKind::FailsInPausedState,
        withdraw_is_halt_safe,
    )?;

    let user_balance_shares: I80F48 = marginfi_account
        .lending_account
        .balances
        .iter()
        .find(|b| b.bank_pk == bank_pk)
        .map(|b| b.asset_shares.into())
        .unwrap_or(I80F48::ZERO);

    if user_balance_shares <= I80F48::ZERO {
        return Ok((0, false));
    }

    let bank_shares_to_token: I80F48 = bank.asset_share_value.into();
    let user_balance_tokens = user_balance_shares
        .checked_mul(bank_shares_to_token)
        .unwrap_or(I80F48::ZERO);
    let user_balance_u64 = if user_balance_tokens > I80F48::ZERO {
        user_balance_tokens
            .checked_to_num::<u64>()
            .unwrap_or(u64::MAX)
    } else {
        0
    };

    if user_balance_u64 == 0 {
        return Ok((0, false));
    }

    if let Ok(()) = try_withdraw(
        marginfi_account,
        group,
        bank,
        bank_pk,
        user_balance_u64,
        remaining_ais,
    ) {
        return Ok((user_balance_u64, true));
    }

    let mut low: u64 = 0;
    let mut high: u64 = user_balance_u64;

    while low < high {
        let mid = low.saturating_add(high.saturating_sub(low) / 2);

        match try_withdraw(marginfi_account, group, bank, bank_pk, mid, remaining_ais) {
            Ok(()) => {
                low = mid.saturating_add(1);
            }
            Err(_) => {
                high = mid;
            }
        }
    }

    let max_amount = if low > 0 { low.saturating_sub(1) } else { 0 };
    let is_all = false;

    Ok((max_amount, is_all))
}

fn try_borrow<'info>(
    marginfi_account: &MarginfiAccount,
    group: &MarginfiGroup,
    bank: &Bank,
    bank_pk: Pubkey,
    amount: u64,
    remaining_ais: &'info [AccountInfo<'info>],
) -> MarginfiResult<()> {
    if amount == 0 {
        return Ok(());
    }

    let mut test_account = Box::new(*marginfi_account);
    let mut test_bank = Box::new(*bank);

    let amount_i80f48 = I80F48::from_num(amount);

    let lending_account = &mut test_account.lending_account;
    let mut bank_account =
        BankAccountWrapper::find_or_create(&bank_pk, &mut test_bank, lending_account)?;

    bank_account.borrow(amount_i80f48)?;

    check_account_init_health(&test_account, group, remaining_ais, &mut None)?;

    Ok(())
}

fn try_withdraw<'info>(
    marginfi_account: &MarginfiAccount,
    group: &MarginfiGroup,
    bank: &Bank,
    bank_pk: Pubkey,
    amount: u64,
    remaining_ais: &'info [AccountInfo<'info>],
) -> MarginfiResult<()> {
    if amount == 0 {
        return Ok(());
    }

    let mut test_account = Box::new(*marginfi_account);
    let mut test_bank = Box::new(*bank);

    let amount_i80f48 = I80F48::from_num(amount);

    let lending_account = &mut test_account.lending_account;
    let mut bank_account = BankAccountWrapper::find(&bank_pk, &mut test_bank, lending_account)?;

    bank_account.withdraw(amount_i80f48)?;

    check_account_init_health(&test_account, group, remaining_ais, &mut None)?;

    Ok(())
}

#[derive(Accounts)]
pub struct QueryMaxBorrowWithdraw<'info> {
    #[account(
        has_one = group @ MarginfiError::InvalidGroup
    )]
    pub marginfi_account: AccountLoader<'info, MarginfiAccount>,

    pub group: AccountLoader<'info, MarginfiGroup>,

    #[account(
        has_one = group @ MarginfiError::InvalidGroup
    )]
    pub bank: AccountLoader<'info, Bank>,
}
