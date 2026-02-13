use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    account_info::AccountInfo,
    instruction::{AccountMeta, Instruction},
    program::invoke,
};

use crate::errors::*;

fn get_update_rate_discriminator() -> Vec<u8> {
    // discriminator = sha256("global:update_rate")[0..8]
    vec![24, 225, 53, 189, 72, 212, 225, 178]
}

pub struct UpdateRateAccounts<'info> {
    pub lending_account: AccountInfo<'info>,
    pub mint: AccountInfo<'info>,
    pub f_token_mint: AccountInfo<'info>,
    pub supply_token_reserves_liquidity: AccountInfo<'info>,
    pub rewards_rate_model: AccountInfo<'info>,
    pub lending_program: UncheckedAccount<'info>,
}

impl<'info> UpdateRateAccounts<'info> {
    pub fn update_rate(&self) -> Result<()> {
        let instruction_data = get_update_rate_discriminator();

        let account_metas = vec![
            // lending account (mutable)
            AccountMeta::new(*self.lending_account.key, false),
            // mint account (readonly)
            AccountMeta::new_readonly(*self.mint.key, false),
            // f_token_mint account (readonly)
            AccountMeta::new_readonly(*self.f_token_mint.key, false),
            // supply_token_reserves_liquidity account (readonly)
            AccountMeta::new_readonly(*self.supply_token_reserves_liquidity.key, false),
            // rewards_rate_model account (readonly)
            AccountMeta::new_readonly(*self.rewards_rate_model.key, false),
        ];

        let instruction = Instruction {
            program_id: *self.lending_program.key,
            accounts: account_metas,
            data: instruction_data,
        };

        invoke(
            &instruction,
            &[
                self.lending_account.clone(),
                self.mint.clone(),
                self.f_token_mint.clone(),
                self.supply_token_reserves_liquidity.clone(),
                self.rewards_rate_model.clone(),
            ],
        )
        .map_err(|_| ErrorCodes::CpiToLendingProgramFailed.into())
    }
}
