use crate::{
    check, check_eq,
    constants::{
        DEFAULT_POSITION_TRANSFER_FEE_LAMPORTS, DEFAULT_POSITION_TRANSFER_MIN_VALUE_USD_CENTS,
    },
    events::{AccountEventHeader, LendingAccountTransferPositionEvent},
    prelude::*,
    require,
    state::{
        self,
        bank::BankImpl,
        marginfi_account,
        marginfi_account::{
            account_not_frozen_for_authority, calc_value, check_account_init_health,
            is_signer_authorized, BankAccountWrapper, LendingAccountImpl, MarginfiAccountImpl,
        },
        marginfi_group::MarginfiGroupImpl,
        price::PriceAdapter,
    },
    utils::{is_marginfi_asset_tag, validate_bank_state, InstructionKind},
};
use anchor_lang::prelude::*;
use anchor_lang::solana_program::clock::Clock;
use fixed::types::I80F48;
use marginfi_type_crate::types::{
    Bank, MarginfiAccount, MarginfiGroup, OraclePriceType, PriceBias, ACCOUNT_DISABLED,
    ACCOUNT_IN_DELEVERAGE, ACCOUNT_IN_ORDER_EXECUTION, ACCOUNT_IN_RECEIVERSHIP,
    ACCOUNT_POSITION_TRANSFER_RECEIVE_DISABLED, ACCOUNT_POSITION_TRANSFER_SEND_DISABLED,
    MAX_LENDING_ACCOUNT_BALANCES,
};

pub fn lending_account_transfer_position<'info>(
    ctx: Context<'info, LendingAccountTransferPosition<'info>>,
    transfer_amount: u64,
) -> MarginfiResult {
    check!(
        ctx.accounts.source_marginfi_account.key()
            != ctx.accounts.destination_marginfi_account.key(),
        MarginfiError::PositionTransferIdenticalAccounts
    );

    let clock = Clock::get()?;
    let mut source_account = ctx.accounts.source_marginfi_account.load_mut()?;
    let mut destination_account = ctx.accounts.destination_marginfi_account.load_mut()?;
    let mut bank = ctx.accounts.bank.load_mut()?;
    let group = ctx.accounts.group.load()?;

    check_eq!(
        ctx.accounts.global_fee_wallet.key(),
        group.fee_state_cache.global_fee_wallet,
        MarginfiError::InvalidGlobalFeeWallet
    );

    check!(
        !source_account.get_flag(ACCOUNT_DISABLED),
        MarginfiError::AccountDisabled
    );
    check!(
        !source_account.get_flag(ACCOUNT_IN_RECEIVERSHIP),
        MarginfiError::ForbiddenIx
    );
    check!(
        !source_account.get_flag(ACCOUNT_IN_ORDER_EXECUTION),
        MarginfiError::ForbiddenIx
    );
    check!(
        !source_account.get_flag(ACCOUNT_IN_DELEVERAGE),
        MarginfiError::ForbiddenIx
    );
    check!(
        !destination_account.get_flag(ACCOUNT_DISABLED),
        MarginfiError::AccountDisabled
    );
    check!(
        !destination_account.get_flag(ACCOUNT_IN_RECEIVERSHIP),
        MarginfiError::ForbiddenIx
    );
    check!(
        !destination_account.get_flag(ACCOUNT_IN_ORDER_EXECUTION),
        MarginfiError::ForbiddenIx
    );
    check!(
        !destination_account.get_flag(ACCOUNT_IN_DELEVERAGE),
        MarginfiError::ForbiddenIx
    );

    check!(
        !source_account.get_flag(ACCOUNT_POSITION_TRANSFER_SEND_DISABLED),
        MarginfiError::PositionTransferSendDisabled
    );
    check!(
        !destination_account.get_flag(ACCOUNT_POSITION_TRANSFER_RECEIVE_DISABLED),
        MarginfiError::PositionTransferDisabled
    );

    validate_bank_state(&bank, InstructionKind::FailsIfPausedOrReduceState, true)?;

    check!(
        transfer_amount > 0,
        MarginfiError::InvalidPositionTransferAmount
    );

    let source_balance = source_account
        .lending_account
        .get_balance(&ctx.accounts.bank.key())
        .ok_or(MarginfiError::LendingAccountBalanceNotFound)?;

    let available_shares = I80F48::from(source_balance.asset_shares);
    check!(available_shares > I80F48::ZERO, MarginfiError::NoAssetFound);

    let accounts_per_bank = marginfi_account::get_remaining_accounts_per_bank(&bank)?;

    let bank_index = ctx
        .remaining_accounts
        .iter()
        .position(|ai| ai.key() == ctx.accounts.bank.key())
        .ok_or(MarginfiError::InvalidBankAccount)?;

    require!(
        ctx.remaining_accounts.len() >= bank_index + accounts_per_bank,
        MarginfiError::WrongNumberOfOracleAccounts
    );

    let oracle_ais = &ctx.remaining_accounts[(bank_index + 1)..(bank_index + accounts_per_bank)];

    let pf = state::price::OraclePriceFeedAdapter::try_from_bank(&bank, oracle_ais, &clock)?;
    let price = pf.get_price_of_type(
        OraclePriceType::RealTime,
        Some(PriceBias::Low),
        bank.config.oracle_max_confidence,
    )?;

    let transfer_usd_value = calc_value(
        I80F48::from_num(transfer_amount),
        price,
        bank.get_balance_decimals(),
        None,
    )?;

    let min_value_usd_cents = if group
        .fee_state_cache
        .position_transfer_min_value_initialized
        != 0
    {
        group.fee_state_cache.position_transfer_min_value_usd_cents as u64
    } else {
        DEFAULT_POSITION_TRANSFER_MIN_VALUE_USD_CENTS as u64
    };
    let min_usd_value = I80F48::from_num(min_value_usd_cents) / I80F48::from_num(100u64);

    check!(
        transfer_usd_value >= min_usd_value,
        MarginfiError::InvalidPositionTransferAmount
    );

    bank.accrue_interest(
        clock.unix_timestamp,
        &group,
        #[cfg(not(feature = "client"))]
        ctx.accounts.bank.key(),
    )?;

    let source_balance_after_accrual = source_account
        .lending_account
        .get_balance(&ctx.accounts.bank.key())
        .ok_or(MarginfiError::LendingAccountBalanceNotFound)?;

    let available_amount =
        bank.get_asset_amount(I80F48::from(source_balance_after_accrual.asset_shares))?;
    check!(
        available_amount >= I80F48::from_num(transfer_amount),
        MarginfiError::PositionTransferInsufficientFunds
    );

    let position_transfer_fee = if group.fee_state_cache.position_transfer_fee_initialized != 0 {
        group.fee_state_cache.position_transfer_fee
    } else {
        DEFAULT_POSITION_TRANSFER_FEE_LAMPORTS
    };

    let share_amount = {
        let lending_account = &mut source_account.lending_account;
        let mut source_bank_account =
            BankAccountWrapper::find(&ctx.accounts.bank.key(), &mut bank, lending_account)?;

        source_bank_account.withdraw(I80F48::from_num(transfer_amount))?
    };

    let has_existing_balance = destination_account
        .lending_account
        .get_balance(&ctx.accounts.bank.key())
        .is_some();

    if !has_existing_balance {
        let active_balance_count = destination_account
            .lending_account
            .balances
            .iter()
            .filter(|b| b.is_active())
            .count();

        check!(
            active_balance_count < MAX_LENDING_ACCOUNT_BALANCES,
            MarginfiError::LendingAccountBalanceSlotsFull
        );
    }

    let asset_amount = bank.get_asset_amount(share_amount)?;
    {
        let lending_account = &mut destination_account.lending_account;
        let mut dest_bank_account = BankAccountWrapper::find_or_create(
            &ctx.accounts.bank.key(),
            &mut bank,
            lending_account,
        )?;
        dest_bank_account.deposit(asset_amount)?;
    }

    if position_transfer_fee > 0 {
        anchor_lang::system_program::transfer(
            ctx.accounts.transfer_fee(),
            position_transfer_fee as u64,
        )?;
    }

    source_account.last_update = clock.unix_timestamp as u64;
    destination_account.last_update = clock.unix_timestamp as u64;

    source_account.lending_account.sort_balances();
    source_account.sync_indexer_flags();
    destination_account.lending_account.sort_balances();
    destination_account.sync_indexer_flags();

    let source_authority = source_account.authority;
    let source_group = source_account.group;
    let dest_authority = destination_account.authority;
    let bank_mint = bank.mint;

    bank.update_bank_cache(&group)?;

    drop(bank);
    drop(source_account);
    drop(destination_account);

    let source_account_reloaded = ctx.accounts.source_marginfi_account.load()?;
    let destination_account_reloaded = ctx.accounts.destination_marginfi_account.load()?;

    match check_account_init_health(
        &source_account_reloaded,
        &group,
        ctx.remaining_accounts,
        &mut None,
    ) {
        Ok(_) => {}
        Err(_e) => {
            return err!(MarginfiError::PositionTransferHealthCheckFailed);
        }
    }

    match check_account_init_health(
        &destination_account_reloaded,
        &group,
        ctx.remaining_accounts,
        &mut None,
    ) {
        Ok(_) => {}
        Err(_e) => {
            return err!(MarginfiError::PositionTransferHealthCheckFailed);
        }
    }

    emit!(LendingAccountTransferPositionEvent {
        header: AccountEventHeader {
            signer: Some(ctx.accounts.authority.key()),
            marginfi_account: ctx.accounts.source_marginfi_account.key(),
            marginfi_account_authority: source_authority,
            marginfi_group: source_group,
        },
        source_account: ctx.accounts.source_marginfi_account.key(),
        source_account_authority: source_authority,
        destination_account: ctx.accounts.destination_marginfi_account.key(),
        destination_account_authority: dest_authority,
        bank: ctx.accounts.bank.key(),
        mint: bank_mint,
        transfer_amount,
        transfer_share_amount: share_amount.into(),
        protocol_fee_lamports: position_transfer_fee,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct LendingAccountTransferPosition<'info> {
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
            let a = source_marginfi_account.load()?;
            account_not_frozen_for_authority(&a, authority.key())
        } @ MarginfiError::AccountFrozen,
        constraint = {
            let a = source_marginfi_account.load()?;
            let g = group.load()?;
            is_signer_authorized(&a, g.admin, authority.key(), false, false)
        } @ MarginfiError::Unauthorized
    )]
    pub source_marginfi_account: AccountLoader<'info, MarginfiAccount>,

    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
        constraint = {
            let a = destination_marginfi_account.load()?;
            account_not_frozen_for_authority(&a, destination_authority.key())
        } @ MarginfiError::AccountFrozen,
        constraint = {
            let a = destination_marginfi_account.load()?;
            let g = group.load()?;
            is_signer_authorized(&a, g.admin, destination_authority.key(), false, false)
        } @ MarginfiError::Unauthorized
    )]
    pub destination_marginfi_account: AccountLoader<'info, MarginfiAccount>,

    pub authority: Signer<'info>,

    pub destination_authority: Signer<'info>,

    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
        constraint = is_marginfi_asset_tag(bank.load()?.config.asset_tag)
            @ MarginfiError::WrongAssetTagForStandardInstructions,
    )]
    pub bank: AccountLoader<'info, Bank>,

    /// CHECK: Validated against group.fee_state_cache.global_fee_wallet in the instruction handler
    #[account(mut)]
    pub global_fee_wallet: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

impl<'info> LendingAccountTransferPosition<'info> {
    fn transfer_fee(
        &self,
    ) -> CpiContext<'_, '_, '_, 'info, anchor_lang::system_program::Transfer<'info>> {
        CpiContext::new(
            self.system_program.key(),
            anchor_lang::system_program::Transfer {
                from: self.destination_authority.to_account_info(),
                to: self.global_fee_wallet.to_account_info(),
            },
        )
    }
}
