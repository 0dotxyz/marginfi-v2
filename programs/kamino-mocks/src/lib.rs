#![allow(
    clippy::diverging_sub_expression,
    clippy::too_many_arguments,
    unexpected_cfgs
)]

pub mod macros;
pub mod state;

use anchor_lang::{
    prelude::*,
    solana_program::{
        instruction::{AccountMeta, Instruction},
        program::invoke,
    },
};
use solana_sha256_hasher::hash;

declare_id!(marginfi_type_crate::pdas::KAMINO_PROGRAM_ID);

declare_program!(kamino_lending);
#[cfg(not(target_os = "solana"))]
declare_program!(kamino_lending_complete);
declare_program!(kamino_farms);

#[program]
pub mod kamino_mocks {
    use super::*;

    pub fn refresh_reserve(_ctx: Context<Noop>) -> Result<()> {
        Ok(())
    }

    pub fn refresh_reserves_batch(_ctx: Context<Noop>, _skip_price_updates: bool) -> Result<()> {
        Ok(())
    }

    pub fn refresh_obligation(_ctx: Context<Noop>) -> Result<()> {
        Ok(())
    }

    pub fn close_balance_via_cpi(ctx: Context<CloseBalanceViaCpi>) -> Result<()> {
        let CloseBalanceViaCpi {
            group,
            marginfi_account,
            authority,
            bank,
            marginfi_program,
        } = ctx.accounts;

        let ix = Instruction {
            program_id: *marginfi_program.key,
            accounts: vec![
                AccountMeta::new_readonly(*group.key, false),
                AccountMeta::new(*marginfi_account.key, false),
                AccountMeta::new_readonly(*authority.key, true),
                AccountMeta::new(*bank.key, false),
            ],
            data: lending_account_close_balance_discriminator().to_vec(),
        };

        invoke(
            &ix,
            &[
                group.to_account_info(),
                marginfi_account.to_account_info(),
                authority.to_account_info(),
                bank.to_account_info(),
                marginfi_program.to_account_info(),
            ],
        )?;

        Ok(())
    }
}

#[derive(Accounts)]
pub struct Noop {}

#[derive(Accounts)]
pub struct CloseBalanceViaCpi<'info> {
    /// CHECK: Validated by marginfi.
    pub group: UncheckedAccount<'info>,

    /// CHECK: Validated by marginfi.
    #[account(mut)]
    pub marginfi_account: UncheckedAccount<'info>,

    pub authority: Signer<'info>,

    /// CHECK: Validated by marginfi.
    #[account(mut)]
    pub bank: UncheckedAccount<'info>,

    /// CHECK: The marginfi program that receives the CPI.
    pub marginfi_program: UncheckedAccount<'info>,
}

#[error_code]
pub enum KaminoMocksError {
    #[msg("Math error")]
    MathError,
}

fn lending_account_close_balance_discriminator() -> [u8; 8] {
    let mut sighash = [0u8; 8];
    sighash
        .copy_from_slice(&hash("global:lending_account_close_balance".as_bytes()).to_bytes()[..8]);
    sighash
}
