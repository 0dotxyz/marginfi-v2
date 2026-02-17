use anchor_lang::prelude::*;
use anchor_lang::solana_program::pubkey::Pubkey;

mod constants;
mod errors;
mod events;
mod invokes;
mod state;
mod validate;

use crate::{constants::GOVERNANCE_MS, errors::ErrorCodes};
use events::*;
use state::*;

use invokes::liquidity_layer::{OperateInstructionParams, PreOperateInstructionParams};
use library::{structs::TokenTransferParams, token::transfer_spl_tokens};
use validate::{validate_flashloan, validate_flashloan_payback};

use library::math::{casting::*, safe_math::*};

#[cfg(feature = "staging")]
declare_id!("aaae6RAZNe4wLgKkjKnJxqQ15B58yPUdcygDbTUpXo3");

#[cfg(not(feature = "staging"))]
declare_id!("jupgfSgfuAXv4B6R2Uxu85Z1qdzgju79s6MfZekN6XS");

#[program]
pub mod flashloan {

    use super::*;

    /***********************************|
    |           Admin Module             |
    |__________________________________*/

    pub fn init_flashloan_admin(
        ctx: Context<InitFlashloanAdmin>,
        authority: Pubkey,
        flashloan_fee: u16,
        liquidity_program: Pubkey,
    ) -> Result<()> {
        let flashloan_admin = &mut ctx.accounts.flashloan_admin;
        flashloan_admin.init(
            authority,
            flashloan_fee,
            liquidity_program,
            ctx.bumps.flashloan_admin,
        )?;

        Ok(())
    }

    pub fn update_authority(
        context: Context<UpdateAuthority>,
        new_authority: Pubkey,
    ) -> Result<()> {
        if context.accounts.authority.key() != context.accounts.flashloan_admin.authority {
            // second check on top of context.rs to be extra sure
            return Err(ErrorCodes::FlashloanInvalidAuthority.into());
        }

        if new_authority != GOVERNANCE_MS {
            return Err(ErrorCodes::FlashloanInvalidParams.into());
        }

        context.accounts.flashloan_admin.authority = new_authority;

        emit!(LogUpdateAuthority {
            new_authority: new_authority,
        });

        Ok(())
    }

    pub fn pause_protocol(ctx: Context<FlashloanProtocol>) -> Result<()> {
        let flashloan_admin = &mut ctx.accounts.flashloan_admin;
        flashloan_admin.pause_protocol()?;

        emit!(PauseProtocol {});

        Ok(())
    }

    pub fn activate_protocol(ctx: Context<FlashloanProtocol>) -> Result<()> {
        let flashloan_admin = &mut ctx.accounts.flashloan_admin;
        flashloan_admin.activate_protocol()?;

        emit!(ActivateProtocol {});

        Ok(())
    }

    pub fn set_flashloan_fee(ctx: Context<FlashloanProtocol>, flashloan_fee: u16) -> Result<()> {
        let flashloan_admin = &mut ctx.accounts.flashloan_admin;
        flashloan_admin.set_flashloan_fee(flashloan_fee)?;

        emit!(SetFlashloanFee { flashloan_fee });

        Ok(())
    }

    /***********************************|
    |           Flashloan Module        |
    |__________________________________*/

    pub fn flashloan_borrow(ctx: Context<Flashloan>, amount: u64) -> Result<()> {
        validate_flashloan(&ctx, amount)?;

        if ctx.accounts.flashloan_admin.is_paused() {
            return Err(ErrorCodes::FlashloanPaused.into());
        }

        let bump = {
            let flashloan_admin = &mut ctx.accounts.flashloan_admin;
            flashloan_admin.set_flashloan_as_active(amount)?;
            flashloan_admin.bump
        };

        let accounts = ctx.accounts.get_borrow_accounts();
        let signer_seeds: &[&[&[u8]]] = &[&[FLASHLOAN_ADMIN_SEED, &[bump]]];

        accounts.operate_with_signer(
            OperateInstructionParams {
                supply_amount: 0,
                borrow_amount: amount.cast::<i128>()?, // borrow amount is positive
                withdraw_to: ctx.accounts.liquidity.key(), // withdraw to liquidity address itself, as this is borrow instruction
                borrow_to: ctx.accounts.signer.key(),      // borrow to happens at signer address
            },
            signer_seeds,
        )?;

        Ok(())
    }

    pub fn flashloan_payback(ctx: Context<Flashloan>, amount: u64) -> Result<()> {
        let active_flashloan_amount = ctx.accounts.flashloan_admin.active_flashloan_amount;

        validate_flashloan_payback(active_flashloan_amount, amount)?;

        if ctx.accounts.flashloan_admin.is_paused() {
            return Err(ErrorCodes::FlashloanPaused.into());
        }

        let amount_with_fee: u64 = ctx
            .accounts
            .flashloan_admin
            .get_expected_payback_amount(amount)?;

        let bump = ctx.accounts.flashloan_admin.bump;
        let signer_seeds: &[&[&[u8]]] = &[&[FLASHLOAN_ADMIN_SEED, &[bump]]];
        let accounts = ctx.accounts.get_payback_accounts();

        // payback
        accounts.pre_operate_with_signer(
            PreOperateInstructionParams {
                mint: ctx.accounts.mint.key(),
            },
            signer_seeds,
        )?;

        // Transfer the amount including fee to the vault
        transfer_spl_tokens(TokenTransferParams {
            source: ctx.accounts.signer_borrow_token_account.to_account_info(),
            destination: ctx.accounts.vault.to_account_info(),
            authority: ctx.accounts.signer.to_account_info(),
            amount: amount_with_fee,
            token_program: ctx.accounts.token_program.to_account_info(),
            signer_seeds: None,
            mint: ctx.accounts.mint.clone(),
        })?;

        // Call operate for the payback in exact amounts
        accounts.operate_with_signer(
            OperateInstructionParams {
                supply_amount: 0,
                borrow_amount: active_flashloan_amount.cast::<i128>()?.safe_mul(-1)?, // payback amount is negative
                withdraw_to: ctx.accounts.liquidity.key(), // default withdraw_to will be liquidity PDA itself
                borrow_to: ctx.accounts.liquidity.key(), // default borrow_to will be liquidity PDA itself
            },
            signer_seeds,
        )?;

        {
            let flashloan_admin = &mut ctx.accounts.flashloan_admin;
            flashloan_admin.set_flashloan_as_inactive()?;
        }

        Ok(())
    }
}
