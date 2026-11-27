use anchor_lang::prelude::*;
use fixed::types::I80F48;
use marginfi_type_crate::{
    constants::{EMPTY_BALANCE_THRESHOLD, REBALANCE_FEE_POOL_SEED, ZERO_AMOUNT_THRESHOLD},
    types::{
        Bank, BankOperationalState, MarginfiAccount, MarginfiGroup, RiskTier, ACCOUNT_DISABLED,
        ACCOUNT_FROZEN, ACCOUNT_IN_DELEVERAGE, ACCOUNT_IN_FLASHLOAN, ACCOUNT_IN_ORDER_EXECUTION,
        ACCOUNT_IN_REBALANCE, ACCOUNT_IN_RECEIVERSHIP, ACCOUNT_IN_RISK_ADMIN_LIQUIDATION,
    },
};

use crate::{
    check,
    events::{AccountEventHeader, AdminCloseBankruptAccountEvent, PurgedBalance},
    instructions::marginfi_account::pay_from_fee_pool,
    ix_utils,
    state::{
        bank::BankImpl,
        marginfi_account::{BalanceImpl, BankAccountWrapper},
    },
    utils::NumTraitsWithTolerance,
    MarginfiError, MarginfiResult,
};

/// (risk admin only) Close an account disabled by `lending_pool_handle_bankruptcy`, which blocks
/// every other close path. Each active balance is purged against its bank:
/// * Asset shares are removed without moving tokens, so integration banks work too.
/// * Liability shares are removed only if they are dust or the bank is `KilledByBankruptcy`.
///
/// Isolated-tier collateral has zero weight in the bankruptcy check and may be worth real tokens,
/// so such accounts are rejected.
///
/// Rent and the rebalance fee pool go to the global fee wallet. Open orders don't block the close;
/// close them afterwards with `marginfi_account_keeper_close_order` so an account re-created at
/// the same address doesn't inherit them.
///
/// Remaining accounts: one writable bank per active balance, in balance order. No oracles.
pub fn admin_close_bankrupt_account<'info>(
    ctx: Context<'info, AdminCloseBankruptAccount<'info>>,
) -> MarginfiResult {
    ix_utils::check_no_durable_nonce(&ctx.accounts.instruction_sysvar)?;

    let group_key = ctx.accounts.group.key();
    let group = ctx.accounts.group.load()?;
    let mut marginfi_account = ctx.accounts.marginfi_account.load_mut()?;

    // Bankruptcy and `transfer_to_new_account` both set ACCOUNT_DISABLED; only the latter sets
    // `migrated_to`. `has_been_bankrupted` isn't used: older bankruptcies never set it.
    check!(
        marginfi_account.get_flag(ACCOUNT_DISABLED)
            && marginfi_account.migrated_to == Pubkey::default(),
        MarginfiError::IllegalAction,
        "Only bankrupted accounts can be closed with this instruction"
    );
    check!(
        !marginfi_account.get_flag(
            ACCOUNT_FROZEN
                | ACCOUNT_IN_FLASHLOAN
                | ACCOUNT_IN_RECEIVERSHIP
                | ACCOUNT_IN_RISK_ADMIN_LIQUIDATION
                | ACCOUNT_IN_DELEVERAGE
                | ACCOUNT_IN_ORDER_EXECUTION
                | ACCOUNT_IN_REBALANCE
        ),
        MarginfiError::IllegalAction,
        "Account is frozen or in a transient state"
    );
    check!(
        marginfi_account.liquidation_record == Pubkey::default(),
        MarginfiError::IllegalAction,
        "Account has a liquidation record"
    );

    let active_balance_count = marginfi_account
        .lending_account
        .get_active_balances_iter()
        .count();
    check!(
        ctx.remaining_accounts.len() == active_balance_count,
        MarginfiError::InvalidBankAccount,
        "Expected one bank per active balance"
    );

    let clock = Clock::get()?;
    let mut purged = Vec::with_capacity(active_balance_count);

    for (balance, bank_ai) in marginfi_account
        .lending_account
        .balances
        .iter_mut()
        .filter(|balance| balance.is_active())
        .zip(ctx.remaining_accounts)
    {
        check!(
            bank_ai.key == &balance.bank_pk,
            MarginfiError::InvalidBankAccount,
            "Banks must be passed in balance order"
        );
        let asset_shares: I80F48 = balance.asset_shares.into();
        let liability_shares: I80F48 = balance.liability_shares.into();

        // `lending_pool_close_bank` can close a bank while bankruptcy residue (uncounted dust) still
        // references it. Its totals were dust, so only the balance needs clearing.
        let premium_written_off = if bank_ai.data_is_empty() {
            check!(
                asset_shares.is_zero_with_tolerance(ZERO_AMOUNT_THRESHOLD)
                    && liability_shares.is_zero_with_tolerance(ZERO_AMOUNT_THRESHOLD),
                MarginfiError::InvalidBankAccount
            );
            balance.close()?;
            I80F48::ZERO
        } else {
            let bank_loader = AccountLoader::<Bank>::try_from(bank_ai)?;
            let mut bank = bank_loader.load_mut()?;
            check!(bank.group == group_key, MarginfiError::InvalidGroup);
            bank.accrue_interest(
                clock.unix_timestamp,
                &group,
                #[cfg(not(feature = "client"))]
                bank_ai.key(),
            )?;

            check!(
                !(bank.config.risk_tier == RiskTier::Isolated
                    && bank.get_asset_amount(asset_shares)? >= EMPTY_BALANCE_THRESHOLD),
                MarginfiError::BankruptAccountHasIsolatedCollateral
            );

            // Unsettled debt must go through `lending_pool_handle_bankruptcy`, which draws on
            // insurance first. A killed bank has no depositor value left, so its residue can go.
            check!(
                bank.get_liability_amount(liability_shares)? <= ZERO_AMOUNT_THRESHOLD
                    || bank.cb_effective_operational_state()
                        == BankOperationalState::KilledByBankruptcy,
                MarginfiError::BankruptAccountHasUnsettledDebt
            );

            let mut bank_account = BankAccountWrapper {
                balance,
                bank: &mut bank,
            };
            let premium = bank_account.write_off_premium()?;
            bank_account.close_balance_internal(false, I80F48::ZERO)?;
            bank.update_bank_cache(&group)?;
            premium
        };

        purged.push(PurgedBalance {
            bank: bank_ai.key(),
            asset_shares: asset_shares.to_num::<f64>(),
            liability_shares: liability_shares.to_num::<f64>(),
            premium_written_off: premium_written_off.to_num::<f64>(),
        });
    }

    let fee_pool_swept = ctx.accounts.rebalance_fee_pool.lamports();
    pay_from_fee_pool(
        &ctx.accounts.rebalance_fee_pool,
        &ctx.accounts.global_fee_wallet.to_account_info(),
        &ctx.accounts.system_program,
        &ctx.accounts.marginfi_account.key(),
        ctx.bumps.rebalance_fee_pool,
        fee_pool_swept,
    )?;

    emit!(AdminCloseBankruptAccountEvent {
        header: AccountEventHeader {
            signer: Some(ctx.accounts.risk_admin.key()),
            marginfi_account: ctx.accounts.marginfi_account.key(),
            marginfi_account_authority: marginfi_account.authority,
            marginfi_group: group_key,
        },
        global_fee_wallet: ctx.accounts.global_fee_wallet.key(),
        fee_pool_swept,
        balances: purged,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct AdminCloseBankruptAccount<'info> {
    #[account(has_one = risk_admin @ MarginfiError::Unauthorized)]
    pub group: AccountLoader<'info, MarginfiGroup>,

    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
        close = global_fee_wallet
    )]
    pub marginfi_account: AccountLoader<'info, MarginfiAccount>,

    pub risk_admin: Signer<'info>,

    /// CHECK: Validated against group fee state cache
    #[account(
        mut,
        constraint = global_fee_wallet.key() == group.load()?.fee_state_cache.global_fee_wallet
            @ MarginfiError::InvalidGlobalFeeWallet
    )]
    pub global_fee_wallet: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [REBALANCE_FEE_POOL_SEED.as_bytes(), marginfi_account.key().as_ref()],
        bump,
    )]
    pub rebalance_fee_pool: SystemAccount<'info>,

    pub system_program: Program<'info, System>,

    /// CHECK: instruction sysvar
    #[account(address = solana_instructions_sysvar::id())]
    pub instruction_sysvar: UncheckedAccount<'info>,
}
