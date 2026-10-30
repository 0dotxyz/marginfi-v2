//! The Kamino / JupLend part of `lending_pool_close_bank`: withdraws the bank's seed position into
//! the liquidity vault and closes the venue accounts.
use crate::bank_signer;
use crate::check;
use crate::state::bank::BankImpl;
use crate::utils::{is_juplend_asset_tag, is_kamino_asset_tag};
use crate::{MarginfiError, MarginfiResult};
use anchor_lang::prelude::*;
use anchor_lang::system_program::{transfer, Transfer};
use anchor_spl::associated_token::{create_idempotent, Create};
use anchor_spl::token::accessor;
use anchor_spl::token_interface::{close_account, CloseAccount, Mint};
use juplend_mocks::juplend_earn::cpi::accounts::{UpdateRate, Withdraw as JuplendWithdrawCpi};
use juplend_mocks::juplend_earn::cpi::{update_rate, withdraw as juplend_withdraw};
use kamino_mocks::kamino_lending::cpi::accounts::{
    DepositFarmsAccounts, RefreshObligation, RefreshReservesBatch,
    WithdrawObligationCollateralAndRedeemReserveCollateral,
    WithdrawObligationCollateralAndRedeemReserveCollateralV2,
};
use kamino_mocks::kamino_lending::cpi::{
    refresh_obligation, refresh_reserves_batch,
    withdraw_obligation_collateral_and_redeem_reserve_collateral_v2,
};
use marginfi_type_crate::{
    pdas::KAMINO_PROGRAM_ID,
    types::{Bank, BankVaultType},
};

/// `[obligation, reserve, lending_market, lending_market_authority, reserve_liquidity_supply,
/// reserve_collateral_mint, reserve_source_collateral, obligation_farm_user_state,
/// reserve_farm_state, kamino_program, farms_program, collateral_token_program, system_program]`
/// (farm accounts: marginfi program id if the reserve has no farm)
const KAMINO_CLOSE_ACCOUNTS: usize = 13;

/// `[lending, f_token_mint, f_token_vault, withdraw_intermediary_ata, lending_admin,
/// supply_token_reserves_liquidity, lending_supply_position_on_liquidity, rate_model, vault,
/// claim_account, liquidity, liquidity_program, rewards_rate_model, juplend_program,
/// associated_token_program, system_program]`
const JUPLEND_CLOSE_ACCOUNTS: usize = 16;

/// Number of venue accounts `lending_pool_close_bank` expects for a bank with this asset tag.
pub(super) fn venue_accounts_len(asset_tag: u8) -> usize {
    if is_kamino_asset_tag(asset_tag) {
        KAMINO_CLOSE_ACCOUNTS
    } else if is_juplend_asset_tag(asset_tag) {
        JUPLEND_CLOSE_ACCOUNTS
    } else {
        0
    }
}

/// Withdraws the seed position into the liquidity vault (swept to the admin with the vaults) and
/// closes the venue accounts. `vault_accounts` must already be validated. Programs are invoked by
/// their fixed ids and the venues validate their own accounts, so only the bank's links to them are
/// checked here.
pub(super) fn close_venue_position<'info>(
    bank: &Bank,
    bank_key: Pubkey,
    vault_accounts: &'info [AccountInfo<'info>],
    venue: &'info [AccountInfo<'info>],
    admin: &AccountInfo<'info>,
    hook_accounts: &[AccountInfo<'info>],
    instruction_sysvar: &AccountInfo<'info>,
) -> MarginfiResult {
    let [token_program, mint, _admin_token_account, liquidity_vault, authority, ..] =
        vault_accounts
    else {
        return err!(MarginfiError::BankCannotClose);
    };
    let signer: &[&[&[u8]]] = bank_signer!(
        BankVaultType::Liquidity,
        bank_key,
        bank.liquidity_vault_authority_bump
    );

    if is_kamino_asset_tag(bank.config.asset_tag) {
        let [obligation, reserve, lending_market, lending_market_authority, reserve_liquidity_supply, reserve_collateral_mint, reserve_source_collateral, obligation_farm_user_state, reserve_farm_state, _kamino_program, farms_program, collateral_token_program, _system_program] =
            venue
        else {
            return err!(MarginfiError::BankCannotClose);
        };
        check!(
            obligation.key() == bank.integration_acc_2,
            MarginfiError::InvalidKaminoObligation
        );
        check!(
            reserve.key() == bank.integration_acc_1,
            MarginfiError::InvalidKaminoReserve
        );
        // Never initialized (bank still `Uninitialized`) or already closed.
        if *obligation.owner != KAMINO_PROGRAM_ID {
            return Ok(());
        }

        refresh_reserves_batch(
            CpiContext::new(KAMINO_PROGRAM_ID, RefreshReservesBatch {})
                .with_remaining_accounts(vec![reserve.clone(), lending_market.clone()]),
            true,
        )?;
        refresh_obligation(
            CpiContext::new(
                KAMINO_PROGRAM_ID,
                RefreshObligation {
                    lending_market: lending_market.clone(),
                    obligation: obligation.clone(),
                },
            )
            .with_remaining_accounts(vec![reserve.clone()]),
        )?;
        let farm_account =
            |account: &AccountInfo<'info>| (account.key() != crate::ID).then(|| account.clone());
        // Kamino closes the emptied obligation, rent to the authority.
        withdraw_obligation_collateral_and_redeem_reserve_collateral_v2(
            CpiContext::new_with_signer(
                KAMINO_PROGRAM_ID,
                WithdrawObligationCollateralAndRedeemReserveCollateralV2 {
                    withdraw_accounts: WithdrawObligationCollateralAndRedeemReserveCollateral {
                        owner: authority.clone(),
                        obligation: obligation.clone(),
                        lending_market: lending_market.clone(),
                        lending_market_authority: lending_market_authority.clone(),
                        withdraw_reserve: reserve.clone(),
                        reserve_liquidity_mint: mint.clone(),
                        reserve_source_collateral: reserve_source_collateral.clone(),
                        reserve_collateral_mint: reserve_collateral_mint.clone(),
                        reserve_liquidity_supply: reserve_liquidity_supply.clone(),
                        user_destination_liquidity: liquidity_vault.clone(),
                        placeholder_user_destination_collateral: None,
                        collateral_token_program: collateral_token_program.clone(),
                        liquidity_token_program: token_program.clone(),
                        instruction_sysvar_account: instruction_sysvar.clone(),
                    },
                    withdraw_farms_accounts: DepositFarmsAccounts {
                        obligation_farm_user_state: farm_account(obligation_farm_user_state),
                        reserve_farm_state: farm_account(reserve_farm_state),
                    },
                    farms_program: farms_program.clone(),
                },
                signer,
            ),
            u64::MAX,
        )?;
        check!(
            obligation.data_is_empty(),
            MarginfiError::BankCannotClose,
            "Kamino obligation still open after withdrawing everything"
        );

        // The obligation's rent.
        transfer(
            CpiContext::new_with_signer(
                System::id(),
                Transfer {
                    from: authority.clone(),
                    to: admin.clone(),
                },
                signer,
            ),
            authority.lamports(),
        )?;
    } else {
        let [lending, f_token_mint, f_token_vault, intermediary_ata, lending_admin, supply_token_reserves_liquidity, lending_supply_position_on_liquidity, rate_model, vault, claim_account, liquidity, liquidity_program, rewards_rate_model, _juplend_program, associated_token_program, system_program] =
            venue
        else {
            return err!(MarginfiError::BankCannotClose);
        };
        check!(
            lending.key() == bank.integration_acc_1,
            MarginfiError::InvalidJuplendLending
        );
        check!(
            f_token_vault.key() == bank.integration_acc_2,
            MarginfiError::InvalidJuplendFTokenVault
        );
        check!(
            intermediary_ata.key() == bank.integration_acc_3,
            MarginfiError::InvalidJuplendWithdrawIntermediaryAta
        );

        // Only withdraws use this ATA, so it may not exist yet.
        create_idempotent(CpiContext::new(
            anchor_spl::associated_token::ID,
            Create {
                payer: admin.clone(),
                associated_token: intermediary_ata.clone(),
                authority: authority.clone(),
                mint: mint.clone(),
                system_program: system_program.clone(),
                token_program: token_program.clone(),
            },
        ))?;
        if accessor::amount(f_token_vault)? > 0 {
            update_rate(CpiContext::new(
                juplend_mocks::ID,
                UpdateRate {
                    lending: lending.clone(),
                    mint: mint.clone(),
                    f_token_mint: f_token_mint.clone(),
                    supply_token_reserves_liquidity: supply_token_reserves_liquidity.clone(),
                    rewards_rate_model: rewards_rate_model.clone(),
                },
            ))?;
            // `u64::MAX` redeems the whole fToken balance.
            juplend_withdraw(
                CpiContext::new_with_signer(
                    juplend_mocks::ID,
                    JuplendWithdrawCpi {
                        signer: authority.clone(),
                        owner_token_account: f_token_vault.clone(),
                        recipient_token_account: intermediary_ata.clone(),
                        lending_admin: lending_admin.clone(),
                        lending: lending.clone(),
                        mint: mint.clone(),
                        f_token_mint: f_token_mint.clone(),
                        supply_token_reserves_liquidity: supply_token_reserves_liquidity.clone(),
                        lending_supply_position_on_liquidity: lending_supply_position_on_liquidity
                            .clone(),
                        rate_model: rate_model.clone(),
                        vault: vault.clone(),
                        claim_account: Some(claim_account.clone()),
                        liquidity: liquidity.clone(),
                        liquidity_program: liquidity_program.clone(),
                        rewards_rate_model: rewards_rate_model.clone(),
                        token_program: token_program.clone(),
                        associated_token_program: associated_token_program.clone(),
                        system_program: system_program.clone(),
                    },
                    signer,
                ),
                u64::MAX,
            )?;
        }

        let redeemed = accessor::amount(intermediary_ata)?;
        if redeemed > 0 {
            let maybe_mint: Option<InterfaceAccount<Mint>> =
                if token_program.key() == anchor_spl::token_2022::ID {
                    Some(InterfaceAccount::try_from(mint)?)
                } else {
                    None
                };
            bank.withdraw_spl_transfer(
                redeemed,
                intermediary_ata.clone(),
                liquidity_vault.clone(),
                authority.clone(),
                maybe_mint.as_ref(),
                token_program.clone(),
                signer,
                hook_accounts,
            )?;
        }
        for account in [f_token_vault, intermediary_ata] {
            close_account(CpiContext::new_with_signer(
                token_program.key(),
                CloseAccount {
                    account: account.clone(),
                    destination: admin.clone(),
                    authority: authority.clone(),
                },
                signer,
            ))?;
        }
    }

    Ok(())
}
