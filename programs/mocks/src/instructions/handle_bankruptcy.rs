use anchor_lang::prelude::*;
use anchor_lang::solana_program::instruction::{AccountMeta, Instruction};
use anchor_lang::InstructionData;

#[derive(Accounts)]
pub struct HandleBankruptcyViaCpi<'info> {
    /// CHECK: don't care
    pub group: UncheckedAccount<'info>,

    /// CHECK: don't care
    pub signer: Signer<'info>,

    /// CHECK: don't care
    #[account(mut)]
    pub bank: UncheckedAccount<'info>,

    /// CHECK: don't care
    #[account(mut)]
    pub marginfi_account: UncheckedAccount<'info>,

    /// CHECK: don't care
    #[account(mut)]
    pub liquidity_vault: UncheckedAccount<'info>,

    /// CHECK: don't care
    #[account(mut)]
    pub insurance_vault: UncheckedAccount<'info>,

    /// CHECK: don't care
    pub insurance_vault_authority: UncheckedAccount<'info>,

    /// CHECK: don't care
    pub token_program: UncheckedAccount<'info>,

    /// CHECK: don't care
    pub marginfi_program: UncheckedAccount<'info>,
}

impl HandleBankruptcyViaCpi<'_> {
    pub fn handle_bankruptcy_via_cpi<'info>(
        ctx: Context<'info, HandleBankruptcyViaCpi<'info>>,
    ) -> Result<()> {
        let mut account_metas = vec![
            AccountMeta::new_readonly(ctx.accounts.group.key(), false),
            AccountMeta::new_readonly(ctx.accounts.signer.key(), true),
            AccountMeta::new(ctx.accounts.bank.key(), false),
            AccountMeta::new(ctx.accounts.marginfi_account.key(), false),
            AccountMeta::new(ctx.accounts.liquidity_vault.key(), false),
            AccountMeta::new(ctx.accounts.insurance_vault.key(), false),
            AccountMeta::new_readonly(ctx.accounts.insurance_vault_authority.key(), false),
            AccountMeta::new_readonly(ctx.accounts.token_program.key(), false),
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
            data: marginfi::instruction::LendingPoolHandleBankruptcy {}.data(),
        };

        let mut infos = vec![
            ctx.accounts.group.to_account_info(),
            ctx.accounts.signer.to_account_info(),
            ctx.accounts.bank.to_account_info(),
            ctx.accounts.marginfi_account.to_account_info(),
            ctx.accounts.liquidity_vault.to_account_info(),
            ctx.accounts.insurance_vault.to_account_info(),
            ctx.accounts.insurance_vault_authority.to_account_info(),
            ctx.accounts.token_program.to_account_info(),
        ];

        infos.extend_from_slice(ctx.remaining_accounts);

        anchor_lang::solana_program::program::invoke(&ix, &infos)?;

        Ok(())
    }
}
