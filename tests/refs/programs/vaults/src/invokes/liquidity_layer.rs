use anchor_lang::prelude::*;

use crate::errors::*;
use liquidity::state::TransferType;

pub struct PreOperateInstructionParams {
    pub mint: Pubkey,
}

pub struct OperateInstructionParams {
    pub supply_amount: i128,
    pub borrow_amount: i128,
    pub borrow_to: Pubkey,
    pub withdraw_to: Pubkey,
    pub mint: Pubkey,
    pub transfer_type: Option<TransferType>,
}

pub struct OperateCpiAccounts<'info> {
    pub protocol: AccountInfo<'info>,
    pub liquidity_program: AccountInfo<'info>,
    pub liquidity: AccountInfo<'info>,
    pub token_reserve: AccountInfo<'info>,
    pub vault: AccountInfo<'info>,
    pub user_supply_position: Option<AccountInfo<'info>>,
    pub user_borrow_position: Option<AccountInfo<'info>>,
    pub rate_model: AccountInfo<'info>,
    pub withdraw_to_account: Option<AccountInfo<'info>>,
    pub borrow_to_account: Option<AccountInfo<'info>>,
    pub token_program: AccountInfo<'info>,
    pub borrow_claim_account: Option<AccountInfo<'info>>,
    pub withdraw_claim_account: Option<AccountInfo<'info>>,
    pub mint: AccountInfo<'info>,
    pub associated_token_program: AccountInfo<'info>,
}

impl<'info> OperateCpiAccounts<'info> {
    pub fn pre_operate_with_signer(
        &self,
        params: PreOperateInstructionParams,
        signer_seeds: &[&[&[u8]]],
        is_supply_mint: bool,
    ) -> Result<()> {
        let cpi_program: AccountInfo<'info> = self.liquidity_program.to_account_info();
        let mut cpi_accounts = liquidity::cpi::accounts::PreOperate {
            protocol: self.protocol.clone(),
            user_supply_position: self.user_supply_position.clone(),
            user_borrow_position: self.user_borrow_position.clone(),
            token_reserve: self.token_reserve.clone(),
            vault: self.vault.clone(),
            token_program: self.token_program.clone(),
            liquidity: self.liquidity.clone(),
            associated_token_program: self.associated_token_program.clone(),
        };

        if is_supply_mint {
            cpi_accounts.user_borrow_position = None;
        } else {
            cpi_accounts.user_supply_position = None;
        }

        let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer_seeds);

        let res: std::result::Result<(), Error> = liquidity::cpi::pre_operate(cpi_ctx, params.mint);

        match res {
            Ok(_) => Ok(()),
            Err(_) => err!(ErrorCodes::VaultCpiToLiquidityFailed),
        }
    }

    pub fn operate_with_signer(
        &self,
        params: OperateInstructionParams,
        signer_seeds: &[&[&[u8]]],
    ) -> Result<(u64, u64)> {
        let cpi_program: AccountInfo<'info> = self.liquidity_program.to_account_info();
        let cpi_accounts = liquidity::cpi::accounts::Operate {
            protocol: self.protocol.clone(),
            liquidity: self.liquidity.clone(),
            token_reserve: self.token_reserve.clone(),
            vault: self.vault.clone(),
            user_supply_position: self.user_supply_position.clone(),
            user_borrow_position: self.user_borrow_position.clone(),
            rate_model: self.rate_model.clone(),
            withdraw_to_account: self.withdraw_to_account.clone(),
            borrow_to_account: self.borrow_to_account.clone(),
            borrow_claim_account: self.borrow_claim_account.clone(),
            withdraw_claim_account: self.withdraw_claim_account.clone(),
            token_program: self.token_program.clone(),
            associated_token_program: self.associated_token_program.clone(),
            mint: self.mint.clone(),
        };

        // Create the CPI context with signer seeds
        let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer_seeds);

        let res: std::result::Result<liquidity::cpi::Return<(u64, u64)>, Error> =
            liquidity::cpi::operate(
                cpi_ctx,
                params.supply_amount,
                params.borrow_amount,
                params.withdraw_to,
                params.borrow_to,
                params.transfer_type.unwrap_or(TransferType::DIRECT),
            );

        match res {
            Ok(result) => Ok(result.get()),
            Err(e) => {
                msg!("VaultCpiToLiquidityFailed with error: {:?}", e);
                err!(ErrorCodes::VaultCpiToLiquidityFailed)
            }
        }
    }
}
