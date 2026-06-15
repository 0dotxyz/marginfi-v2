use anchor_lang::prelude::*;
use anchor_lang::solana_program::instruction::{AccountMeta, Instruction};
use anchor_lang::InstructionData;

#[derive(Accounts)]
pub struct StartLiquidationViaCpi<'info> {
    /// CHECK: Marginfi account under liquidation (validated by marginfi)
    #[account(mut)]
    pub marginfi_account: UncheckedAccount<'info>,

    /// CHECK: Liquidation record PDA associated with the marginfi_account (validated by marginfi)
    #[account(mut)]
    pub liquidation_record: UncheckedAccount<'info>,

    /// CHECK: Arbitrary liquidation receiver chosen by the liquidator
    pub liquidation_receiver: UncheckedAccount<'info>,

    /// CHECK: Must be the sysvar instructions account; marginfi enforces the address
    pub instructions_sysvar: UncheckedAccount<'info>,

    /// CHECK: The marginfi program we’re calling via CPI
    pub marginfi_program: UncheckedAccount<'info>,
}

impl StartLiquidationViaCpi<'_> {
    pub fn start_liquidation_via_cpi<'info>(
        ctx: Context<'info, StartLiquidationViaCpi<'info>>,
    ) -> Result<()> {
        let mut account_metas = vec![
            AccountMeta::new(ctx.accounts.marginfi_account.key(), false),
            AccountMeta::new(ctx.accounts.liquidation_record.key(), false),
            AccountMeta::new_readonly(ctx.accounts.liquidation_receiver.key(), false),
            AccountMeta::new_readonly(ctx.accounts.instructions_sysvar.key(), false),
        ];

        account_metas.extend(ctx.remaining_accounts.iter().map(|ai| {
            if ai.is_writable {
                AccountMeta::new(ai.key(), false)
            } else {
                AccountMeta::new_readonly(ai.key(), false)
            }
        }));

        let ix = Instruction {
            program_id: ctx.accounts.marginfi_program.key(),
            accounts: account_metas,
            data: marginfi::instruction::StartLiquidation {}.data(),
        };

        let mut infos = vec![
            ctx.accounts.marginfi_account.to_account_info(),
            ctx.accounts.liquidation_record.to_account_info(),
            ctx.accounts.liquidation_receiver.to_account_info(),
            ctx.accounts.instructions_sysvar.to_account_info(),
        ];

        // Append remaining AccountInfos (same order as above)
        infos.extend_from_slice(ctx.remaining_accounts);

        anchor_lang::solana_program::program::invoke(&ix, &infos)?;

        Ok(())
    }
}
