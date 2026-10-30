use super::close_bank_venue::{close_venue_position, venue_accounts_len};
use crate::bank_signer;
use crate::check;
use crate::ix_utils;
use crate::state::bank::BankImpl;
use crate::utils::NumTraitsWithTolerance;
use crate::{MarginfiError, MarginfiResult};
use anchor_lang::prelude::*;
use anchor_spl::token::accessor;
use anchor_spl::token_interface::{close_account, CloseAccount, Mint};
use fixed::types::I80F48;
use marginfi_type_crate::{
    constants::{CLOSE_ENABLED_FLAG, ZERO_AMOUNT_THRESHOLD},
    types::{Bank, BankVaultType, MarginfiGroup},
};

/// `[token_program, mint, admin_token_account, liquidity_vault, liquidity_vault_authority,
/// insurance_vault, insurance_vault_authority, fee_vault, fee_vault_authority]`
const CLOSE_VAULT_ACCOUNTS: usize = 9;

/// * force_close - (admin escape hatch) when `Some(true)`, skips the `CLOSE_ENABLED_FLAG` and
///   open-position checks. Intended for legacy pre-0.1.4 banks whose position count is
///   non-authoritative. The zero-shares/zero-emissions checks are always enforced, so a bank still
///   holding value can never be force-closed.
///
/// Optional remaining accounts: `CLOSE_VAULT_ACCOUNTS`, then (Kamino / JupLend) the venue accounts
/// (see `close_bank_venue`), then transfer hook accounts. Venue banks first withdraw their seed
/// position into the liquidity vault and close the venue accounts. Then every vault is swept to the
/// admin's token account and closed. All rent goes to the admin.
pub fn lending_pool_close_bank<'info>(
    ctx: Context<'info, LendingPoolCloseBank<'info>>,
    force_close: Option<bool>,
) -> MarginfiResult {
    ix_utils::check_no_durable_nonce(&ctx.accounts.instruction_sysvar)?;

    let mut group = ctx.accounts.group.load_mut()?;
    // Note: Groups created prior to 0.1.2 have a non-authoritative count here, so subtraction
    // without saturation could reduce the count below zero.
    group.banks = group.banks.saturating_sub(1);

    let bank = ctx.accounts.bank.load()?;

    if !force_close.unwrap_or(false) {
        // banks created prior to 0.1.4 can never be closed because we cannot guarantee an accurate
        // position count for those banks.
        check!(
            bank.get_flag(CLOSE_ENABLED_FLAG),
            MarginfiError::BankCannotClose,
            "Only banks created in 0.1.4 and later can close"
        );
        check!(
            bank.lending_position_count == 0 && bank.borrowing_position_count == 0,
            MarginfiError::BankCannotClose,
            "Only banks with no open positions can close"
        );
    }
    check!(
        I80F48::from(bank.total_asset_shares).is_zero_with_tolerance(ZERO_AMOUNT_THRESHOLD)
            && I80F48::from(bank.total_liability_shares)
                .is_zero_with_tolerance(ZERO_AMOUNT_THRESHOLD),
        MarginfiError::BankCannotClose
    );
    check!(
        I80F48::from(bank.emissions_remaining).is_zero_with_tolerance(ZERO_AMOUNT_THRESHOLD),
        MarginfiError::BankCannotClose
    );

    let remaining = ctx.remaining_accounts;
    if !remaining.is_empty() {
        // Owed to the protocol, not the group admin (sub-unit dust is uncollectable).
        check!(
            I80F48::from(bank.collected_program_fees_outstanding) < I80F48::ONE
                && I80F48::from(bank.collected_premium_outstanding) < I80F48::ONE,
            MarginfiError::BankCannotClose,
            "collect the program fees and premium before closing the vaults"
        );
        let venue_accounts = venue_accounts_len(bank.config.asset_tag);
        check!(
            remaining.len() >= CLOSE_VAULT_ACCOUNTS + venue_accounts,
            MarginfiError::BankCannotClose,
            "expected at least {} close accounts, got {}",
            CLOSE_VAULT_ACCOUNTS + venue_accounts,
            remaining.len()
        );
        let (vault_accounts, rest) = remaining.split_at(CLOSE_VAULT_ACCOUNTS);
        let (venue, hook_accounts) = rest.split_at(venue_accounts);
        let admin = ctx.accounts.admin.to_account_info();

        check_vault_accounts(&bank, ctx.accounts.bank.key(), &admin, vault_accounts)?;
        if venue_accounts > 0 {
            close_venue_position(
                &bank,
                ctx.accounts.bank.key(),
                vault_accounts,
                venue,
                &admin,
                hook_accounts,
                &ctx.accounts.instruction_sysvar.to_account_info(),
            )?;
        }
        close_vaults(
            &bank,
            ctx.accounts.bank.key(),
            vault_accounts,
            &admin,
            hook_accounts,
        )?;
    }

    drop(bank);
    Ok(())
}

/// `[(vault type, vault, authority bump)]` in `CLOSE_VAULT_ACCOUNTS` order.
fn bank_vaults(bank: &Bank) -> [(BankVaultType, Pubkey, u8); 3] {
    [
        (
            BankVaultType::Liquidity,
            bank.liquidity_vault,
            bank.liquidity_vault_authority_bump,
        ),
        (
            BankVaultType::Insurance,
            bank.insurance_vault,
            bank.insurance_vault_authority_bump,
        ),
        (
            BankVaultType::Fee,
            bank.fee_vault,
            bank.fee_vault_authority_bump,
        ),
    ]
}

fn check_vault_accounts(
    bank: &Bank,
    bank_key: Pubkey,
    admin: &AccountInfo,
    vault_accounts: &[AccountInfo],
) -> MarginfiResult {
    let [token_program, _mint, admin_token_account, vaults @ ..] = vault_accounts else {
        return err!(MarginfiError::BankCannotClose);
    };
    let token_program = token_program.key();
    check!(
        token_program == anchor_spl::token::ID || token_program == anchor_spl::token_2022::ID,
        MarginfiError::BankCannotClose,
        "first vault account must be the token program"
    );
    check!(
        accessor::authority(admin_token_account)? == admin.key(),
        MarginfiError::BankCannotClose,
        "admin_token_account must be owned by the admin"
    );

    for (i, (vault_type, vault_key, authority_bump)) in bank_vaults(bank).into_iter().enumerate() {
        let vault = &vaults[2 * i];
        let authority = &vaults[2 * i + 1];

        check!(
            vault.key() == vault_key,
            MarginfiError::BankCannotClose,
            "wrong {:?} vault",
            vault_type
        );
        let expected_authority = Pubkey::create_program_address(
            &[
                vault_type.get_authority_seed(),
                bank_key.as_ref(),
                &[authority_bump],
            ],
            &crate::ID,
        )
        .map_err(|_| MarginfiError::BankCannotClose)?;
        check!(
            authority.key() == expected_authority,
            MarginfiError::BankCannotClose,
            "wrong {:?} vault authority",
            vault_type
        );
        check!(
            *vault.owner == token_program,
            MarginfiError::BankCannotClose,
            "{:?} vault is not owned by the passed token program",
            vault_type
        );
    }
    Ok(())
}

/// Sweeps the (checked) vaults to the admin's token account and closes them into `admin`. With no
/// shares left, whatever they hold is surplus, so donations can't block the close.
fn close_vaults<'info>(
    bank: &Bank,
    bank_key: Pubkey,
    vault_accounts: &'info [AccountInfo<'info>],
    admin: &AccountInfo<'info>,
    hook_accounts: &[AccountInfo<'info>],
) -> MarginfiResult {
    let [token_program, mint, admin_token_account, vaults @ ..] = vault_accounts else {
        return err!(MarginfiError::BankCannotClose);
    };
    let maybe_mint: Option<InterfaceAccount<Mint>> =
        if token_program.key() == anchor_spl::token_2022::ID {
            Some(InterfaceAccount::try_from(mint)?)
        } else {
            None
        };

    for (i, (vault_type, _, authority_bump)) in bank_vaults(bank).into_iter().enumerate() {
        let vault = &vaults[2 * i];
        let authority = &vaults[2 * i + 1];
        let signer: &[&[&[u8]]] = bank_signer!(vault_type, bank_key, authority_bump);

        let amount = accessor::amount(vault)?;
        if amount > 0 {
            bank.withdraw_spl_transfer(
                amount,
                vault.clone(),
                admin_token_account.clone(),
                authority.clone(),
                maybe_mint.as_ref(),
                token_program.clone(),
                signer,
                hook_accounts,
            )?;
        }
        close_account(CpiContext::new_with_signer(
            token_program.key(),
            CloseAccount {
                account: vault.clone(),
                destination: admin.clone(),
                authority: authority.clone(),
            },
            signer,
        ))?;
    }

    Ok(())
}

#[derive(Accounts)]
pub struct LendingPoolCloseBank<'info> {
    #[account(
        mut,
        has_one = admin @ MarginfiError::Unauthorized,
    )]
    pub group: AccountLoader<'info, MarginfiGroup>,

    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
        close = admin,
    )]
    pub bank: AccountLoader<'info, Bank>,

    #[account(mut)]
    pub admin: Signer<'info>,

    /// CHECK: instruction sysvar
    #[account(address = solana_instructions_sysvar::id())]
    pub instruction_sysvar: UncheckedAccount<'info>,
}
