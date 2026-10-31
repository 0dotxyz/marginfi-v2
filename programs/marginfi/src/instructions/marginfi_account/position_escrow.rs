use super::transfer_position::{
    charge_position_transfer, check_can_receive, check_health_and_refresh_premium,
    check_position_flags, takes_unsolicited_deposit,
};
use crate::{
    check,
    constants::POSITION_ESCROW_EXPIRY_SECONDS,
    events::{AccountEventHeader, LendingAccountEscrowPositionEvent, PositionEscrowReleasedEvent},
    math_error,
    prelude::*,
    state::{
        bank::BankImpl,
        marginfi_account::{
            account_not_frozen_for_authority, deposit_is_halt_safe, is_signer_authorized,
            BankAccountWrapper, LendingAccountImpl, MarginfiAccountImpl,
        },
        marginfi_group::MarginfiGroupImpl,
    },
    utils::{
        fetch_unbiased_price_for_bank_cache, validate_asset_tags, validate_bank_state,
        InstructionKind,
    },
};
use anchor_lang::prelude::*;
use fixed::types::I80F48;
use marginfi_type_crate::{
    constants::{FEE_STATE_SEED, POSITION_ESCROW_SEED, TOKENLESS_REPAYMENTS_ALLOWED},
    types::{
        is_marginfi_asset_tag, BalanceSide, Bank, FeeState, MarginfiAccount, MarginfiGroup,
        PositionEscrow, ACCOUNT_DISABLED, ACCOUNT_FROZEN,
    },
};

/// Moves `amount` of the source's deposit in `bank` into the escrow, where the receiver may accept
/// it. The first send opens the escrow. The fee payer pays its rent and gets it back when the
/// escrow closes. Each send restarts the expiry. Remaining accounts: the source's observation set.
pub fn lending_account_escrow_position<'info>(
    ctx: Context<'info, LendingAccountEscrowPosition<'info>>,
    amount: u64,
) -> MarginfiResult {
    check!(
        ctx.accounts.source_marginfi_account.key() != ctx.accounts.receiver_marginfi_account.key(),
        MarginfiError::PositionTransferIdenticalAccounts
    );
    check!(amount > 0, MarginfiError::InvalidPositionTransferAmount);

    let clock = Clock::get()?;
    let bank_key = ctx.accounts.bank.key();
    let mut source_account = ctx.accounts.source_marginfi_account.load_mut()?;
    let receiver_account = ctx.accounts.receiver_marginfi_account.load()?;
    // `load_init` succeeds only on the first send, before Anchor writes the discriminator.
    let mut escrow = match ctx.accounts.position_escrow.load_init() {
        Ok(mut escrow) => {
            escrow.bank = bank_key;
            escrow.source_account = ctx.accounts.source_marginfi_account.key();
            escrow.receiver_account = ctx.accounts.receiver_marginfi_account.key();
            escrow.rent_payer = ctx.accounts.fee_payer.key();
            escrow.bump = ctx.bumps.position_escrow;
            escrow
        }
        Err(_) => ctx.accounts.position_escrow.load_mut()?,
    };
    let mut bank = ctx.accounts.bank.load_mut()?;
    let group = ctx.accounts.group.load()?;

    check_position_flags(&source_account)?;
    check!(
        !receiver_account.get_flag(ACCOUNT_DISABLED),
        MarginfiError::AccountDisabled
    );
    check_can_receive(&receiver_account)?;

    let source_shares: I80F48 = match source_account.lending_account.get_balance(&bank_key) {
        Some(balance) if matches!(balance.get_side(), Some(BalanceSide::Assets)) => {
            balance.asset_shares.into()
        }
        _ => return err!(MarginfiError::NoAssetFound),
    };
    validate_bank_state(
        &bank,
        InstructionKind::FailsIfPausedOrReduceState,
        !source_account.lending_account.has_liabilities(),
    )?;

    let amount = I80F48::from_num(amount);
    let position_transfer_fee = charge_position_transfer(
        &*ctx.accounts.fee_state.load()?,
        &group,
        &bank_key,
        &bank,
        &clock,
        ctx.remaining_accounts,
        amount,
        ctx.accounts.fee_payer.to_account_info(),
        ctx.accounts.global_fee_wallet.to_account_info(),
        &ctx.accounts.system_program,
    )?;

    bank.accrue_interest(
        clock.unix_timestamp,
        &group,
        #[cfg(not(feature = "client"))]
        bank_key,
    )?;
    check!(
        bank.get_asset_amount(source_shares)? >= amount,
        MarginfiError::PositionTransferInsufficientFunds
    );
    let share_amount =
        BankAccountWrapper::find(&bank_key, &mut bank, &mut source_account.lending_account)?
            .transfer_out(amount)?;
    bank.change_escrowed_asset_shares(share_amount)?;
    let escrow_shares = I80F48::from(escrow.asset_shares)
        .checked_add(share_amount)
        .ok_or_else(math_error!())?;
    escrow.asset_shares = escrow_shares.into();
    escrow.expires_at = clock
        .unix_timestamp
        .checked_add(POSITION_ESCROW_EXPIRY_SECONDS)
        .ok_or_else(math_error!())?;
    escrow.source_authority = source_account.authority;
    escrow.receiver_authority = receiver_account.authority;

    source_account.last_update = clock.unix_timestamp as u64;
    source_account.lending_account.sort_balances();
    source_account.sync_indexer_flags();
    bank.update_bank_cache(&group)?;
    let bank_mint = bank.mint;
    drop(bank);

    let source_carries_risk = source_account.lending_account.has_liabilities();
    check_health_and_refresh_premium(
        &mut source_account,
        &group,
        ctx.remaining_accounts,
        &clock,
        source_carries_risk,
        false,
        true,
    )?;

    let maybe_price = fetch_unbiased_price_for_bank_cache(
        &bank_key,
        &*ctx.accounts.bank.load()?,
        &clock,
        ctx.remaining_accounts,
    )
    .ok();
    ctx.accounts
        .bank
        .load_mut()?
        .update_cache_price(maybe_price)?;

    emit!(LendingAccountEscrowPositionEvent {
        header: AccountEventHeader {
            signer: Some(ctx.accounts.authority.key()),
            marginfi_account: ctx.accounts.source_marginfi_account.key(),
            marginfi_account_authority: source_account.authority,
            marginfi_group: source_account.group,
        },
        position_escrow: ctx.accounts.position_escrow.key(),
        receiver_account: ctx.accounts.receiver_marginfi_account.key(),
        bank: bank_key,
        mint: bank_mint,
        amount: amount.checked_to_num().ok_or_else(math_error!())?,
        share_amount: share_amount.into(),
        escrow_shares: escrow_shares.into(),
        expires_at: escrow.expires_at,
        protocol_fee_lamports: position_transfer_fee,
    });

    Ok(())
}

/// Moves the escrowed shares into an account of the receiver's authority when it signs, or else
/// into the receiver's untagged deposit in the bank. Closes the escrow.
pub fn lending_account_claim_position_escrow(
    ctx: Context<ReleasePositionEscrow>,
) -> MarginfiResult {
    let escrow = ctx.accounts.position_escrow.load()?;
    let destination_key = ctx.accounts.destination_marginfi_account.key();
    let destination = ctx.accounts.destination_marginfi_account.load()?;

    let receiver_signed = ctx
        .accounts
        .authority
        .as_ref()
        .is_some_and(|signer| signer.key() == escrow.receiver_authority);
    let accepted = receiver_signed && destination.authority == escrow.receiver_authority;
    let swept = destination_key == escrow.receiver_account
        && takes_unsolicited_deposit(&destination, &escrow.bank);
    check!(
        accepted || swept,
        MarginfiError::PositionTransferConsentRequired
    );
    check_can_receive(&destination)?;
    validate_bank_state(
        &*ctx.accounts.bank.load()?,
        InstructionKind::FailsIfPausedOrReduceState,
        deposit_is_halt_safe(&destination, &escrow.bank),
    )?;
    drop(destination);
    drop(escrow);

    release_position_escrow(ctx, false)
}

/// Moves the escrowed shares back to the source account on the receiver's decline, or by anyone
/// after expiry. The sender's authority may sign to pick another of its accounts. Closes the escrow.
pub fn lending_account_return_position_escrow(
    ctx: Context<ReleasePositionEscrow>,
) -> MarginfiResult {
    let escrow = ctx.accounts.position_escrow.load()?;
    let destination_key = ctx.accounts.destination_marginfi_account.key();
    let destination = ctx.accounts.destination_marginfi_account.load()?;
    let signer = ctx.accounts.authority.as_ref().map(|s| s.key());

    check!(
        signer == Some(escrow.receiver_authority)
            || Clock::get()?.unix_timestamp >= escrow.expires_at,
        MarginfiError::PositionEscrowNotExpired
    );
    check!(
        destination_key == escrow.source_account
            || (signer == Some(escrow.source_authority)
                && destination.authority == escrow.source_authority),
        MarginfiError::Unauthorized
    );
    check!(
        !destination.get_flag(ACCOUNT_FROZEN),
        MarginfiError::AccountFrozen
    );
    validate_bank_state(
        &*ctx.accounts.bank.load()?,
        InstructionKind::FailsInPausedState,
        deposit_is_halt_safe(&destination, &escrow.bank),
    )?;
    drop(destination);
    drop(escrow);

    release_position_escrow(ctx, true)
}

/// Credits the escrowed shares to the destination. The `close` constraint closes the escrow.
fn release_position_escrow(ctx: Context<ReleasePositionEscrow>, returned: bool) -> MarginfiResult {
    let clock = Clock::get()?;
    let bank_key = ctx.accounts.bank.key();
    let escrow_shares: I80F48 = ctx.accounts.position_escrow.load()?.asset_shares.into();
    let mut destination = ctx.accounts.destination_marginfi_account.load_mut()?;
    let mut bank = ctx.accounts.bank.load_mut()?;
    let group = ctx.accounts.group.load()?;

    check_position_flags(&destination)?;
    validate_asset_tags(&bank, &destination)?;
    bank.accrue_interest(
        clock.unix_timestamp,
        &group,
        #[cfg(not(feature = "client"))]
        bank_key,
    )?;

    let amount = bank.get_asset_amount(escrow_shares)?;
    let share_amount = if escrow_shares > I80F48::ZERO {
        bank.change_escrowed_asset_shares(-escrow_shares)?;
        let shares = BankAccountWrapper::find_or_create(
            &bank_key,
            &mut bank,
            &mut destination.lending_account,
        )?
        .transfer_in(amount)?;
        destination.last_update = clock.unix_timestamp as u64;
        destination.lending_account.sort_balances();
        destination.sync_indexer_flags();
        shares
    } else {
        I80F48::ZERO
    };
    bank.update_bank_cache(&group)?;

    emit!(PositionEscrowReleasedEvent {
        header: AccountEventHeader {
            signer: ctx.accounts.authority.as_ref().map(|s| s.key()),
            marginfi_account: ctx.accounts.destination_marginfi_account.key(),
            marginfi_account_authority: destination.authority,
            marginfi_group: destination.group,
        },
        position_escrow: ctx.accounts.position_escrow.key(),
        bank: bank_key,
        mint: bank.mint,
        returned,
        amount: amount.checked_to_num().ok_or_else(math_error!())?,
        share_amount: share_amount.into(),
    });

    Ok(())
}

#[derive(Accounts)]
pub struct LendingAccountEscrowPosition<'info> {
    #[account(
        constraint = !group.load()?.is_protocol_paused() @ MarginfiError::ProtocolPaused
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
            is_signer_authorized(&a, g.governance_admin, authority.key(), false, false, false)
        } @ MarginfiError::Unauthorized
    )]
    pub source_marginfi_account: AccountLoader<'info, MarginfiAccount>,

    #[account(has_one = group @ MarginfiError::InvalidGroup)]
    pub receiver_marginfi_account: AccountLoader<'info, MarginfiAccount>,

    #[account(
        init_if_needed,
        payer = fee_payer,
        seeds = [
            POSITION_ESCROW_SEED.as_bytes(),
            bank.key().as_ref(),
            receiver_marginfi_account.key().as_ref(),
            source_marginfi_account.key().as_ref(),
        ],
        bump,
        space = 8 + PositionEscrow::LEN
    )]
    pub position_escrow: AccountLoader<'info, PositionEscrow>,

    pub authority: Signer<'info>,

    #[account(mut)]
    pub fee_payer: Signer<'info>,

    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
        constraint = is_marginfi_asset_tag(bank.load()?.config.asset_tag)
            @ MarginfiError::WrongAssetTagForStandardInstructions,
        constraint = !bank.load()?.get_flag(TOKENLESS_REPAYMENTS_ALLOWED)
            @ MarginfiError::BankReduceOnly,
    )]
    pub bank: AccountLoader<'info, Bank>,

    /// CHECK: validated against `group.fee_state_cache.global_fee_wallet` in the handler.
    #[account(mut)]
    pub global_fee_wallet: UncheckedAccount<'info>,

    #[account(
        seeds = [FEE_STATE_SEED.as_bytes()],
        bump,
    )]
    pub fee_state: AccountLoader<'info, FeeState>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ReleasePositionEscrow<'info> {
    #[account(
        constraint = !group.load()?.is_protocol_paused() @ MarginfiError::ProtocolPaused
    )]
    pub group: AccountLoader<'info, MarginfiGroup>,

    #[account(
        mut,
        close = rent_payer,
        has_one = bank @ MarginfiError::InvalidBankAccount,
        has_one = rent_payer @ MarginfiError::Unauthorized,
    )]
    pub position_escrow: AccountLoader<'info, PositionEscrow>,

    #[account(mut, has_one = group @ MarginfiError::InvalidGroup)]
    pub destination_marginfi_account: AccountLoader<'info, MarginfiAccount>,

    #[account(mut, has_one = group @ MarginfiError::InvalidGroup)]
    pub bank: AccountLoader<'info, Bank>,

    /// The receiver's authority to accept or decline, or the sender's authority to pick the
    /// account a return lands in. Omitted for a sweep or an expired return.
    pub authority: Option<Signer<'info>>,

    /// CHECK: matched against `position_escrow.rent_payer`.
    #[account(mut)]
    pub rent_payer: UncheckedAccount<'info>,
}
