#![allow(clippy::diverging_sub_expression)]

use anchor_lang::prelude::*;

pub mod errors;
pub mod instructions;
pub mod macros;
pub mod state;
// pub mod utils;

use crate::instructions::*;
// use crate::state::*;
// use errors::*;

declare_id!("rec5EKMGg6MxZYaMdyBfgwp4d5rB9T1VQH5pJv5LtFJ");

#[program]
pub mod mocks {
    use super::*;
    use std::io::Write as IoWrite;

    /// Do nothing
    pub fn do_nothing(ctx: Context<DoNothing>) -> Result<()> {
        instructions::do_nothing::do_nothing(ctx)
    }

    /// Benchmark the emode reconcile functions' compute cost (logs CU via sol_log_compute_units).
    /// Not used in normal flows — invoked only by the opt-in `anchor run bench-emode`.
    pub fn bench_reconcile_emode(
        ctx: Context<BenchReconcileEmode>,
        num_configs: u8,
    ) -> Result<()> {
        instructions::bench_emode::bench_reconcile_emode(ctx, num_configs)
    }

    /// Init authority for fake jupiter-like swap pools
    pub fn init_pool_auth(ctx: Context<InitPoolAuth>, nonce: u16) -> Result<()> {
        instructions::init_pool_auth::init_pool_auth(ctx, nonce)
    }

    /// Execute an exchange of a:b like-jupiter. You set the amount a sent and b received.
    pub fn swap_like_jupiter<'info>(
        ctx: Context<'info, SwapLikeJupiter<'info>>,
        amt_a: u64,
        amt_b: u64,
    ) -> Result<()> {
        instructions::swap_like_jupiter::SwapLikeJupiter::swap_like_jup(ctx, amt_a, amt_b)
    }

    #[derive(Accounts)]
    pub struct Write<'info> {
        #[account(mut)]
        target: Signer<'info>,
    }

    /// Write arbitrary bytes to an arbitrary account. YOLO.
    pub fn write(ctx: Context<Write>, offset: u64, data: Vec<u8>) -> Result<()> {
        let account_data = ctx.accounts.target.to_account_info().data;
        let borrow_data = &mut *account_data.borrow_mut();
        let offset = offset as usize;

        Ok((&mut borrow_data[offset..]).write_all(&data[..])?)
    }

    /// Create a marginfi account PDA via CPI
    pub fn create_marginfi_account_pda_via_cpi(
        ctx: Context<CreateMarginfiAccountPdaViaCpi>,
        account_index: u16,
        third_party_id: Option<u16>,
    ) -> Result<()> {
        instructions::pda_account_creation::CreateMarginfiAccountPdaViaCpi::create_marginfi_account_pda_via_cpi(
            ctx,
            account_index,
            third_party_id,
        )
    }

    /// Start a liquidation via CPI
    pub fn start_liquidation_via_cpi<'info>(
        ctx: Context<'info, StartLiquidationViaCpi<'info>>,
    ) -> Result<()> {
        instructions::start_liquidate::StartLiquidationViaCpi::start_liquidation_via_cpi(ctx)
    }

    /// Handle bankruptcy via CPI
    pub fn handle_bankruptcy<'info>(
        ctx: Context<'info, HandleBankruptcyViaCpi<'info>>,
    ) -> Result<()> {
        instructions::handle_bankruptcy::HandleBankruptcyViaCpi::handle_bankruptcy_via_cpi(ctx)
    }
}
