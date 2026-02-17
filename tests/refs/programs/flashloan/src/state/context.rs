use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar::instructions::ID as SYSVAR_INSTRUCTIONS_ID;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{Mint, TokenAccount, TokenInterface},
};

use crate::errors::ErrorCodes;
use crate::invokes::liquidity_layer::OperateCpiAccounts;
use crate::state::*;

#[derive(Accounts)]
pub struct InitFlashloanAdmin<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    #[account(
        init,
        payer = signer,
        space = 8 + FlashloanAdmin::INIT_SPACE,
        seeds = [FLASHLOAN_ADMIN_SEED],
        bump,
    )]
    pub flashloan_admin: Account<'info, FlashloanAdmin>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateAuthority<'info> {
    #[account(address = flashloan_admin.authority @ ErrorCodes::FlashloanInvalidAuthority)]
    pub authority: Signer<'info>,

    #[account(mut)]
    pub flashloan_admin: Account<'info, FlashloanAdmin>,
}

#[derive(Accounts)]
pub struct FlashloanProtocol<'info> {
    #[account(address = flashloan_admin.authority @ ErrorCodes::FlashloanInvalidAuthority)]
    pub authority: Signer<'info>,

    #[account(mut)]
    pub flashloan_admin: Account<'info, FlashloanAdmin>,
}

#[derive(Accounts)]
pub struct Flashloan<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    #[account(mut, has_one = liquidity_program)]
    pub flashloan_admin: Account<'info, FlashloanAdmin>,

    #[account(
        init_if_needed,
        payer = signer,
        associated_token::mint = mint,
        associated_token::authority = signer, // @dev borrow happens at signer account
        associated_token::token_program = token_program
    )]
    pub signer_borrow_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    pub mint: InterfaceAccount<'info, Mint>,

    // Here we will list all liquidity accounts, needed for CPI call to liquidity program
    // @dev no verification of accounts here, as they are already handled in liquidity program,
    // Hence loading them as UncheckedAccount

    // No need to verify liquidity program key here, as it will be verified in liquidity program CPI call
    #[account(mut)]
    /// CHECK: Safe as this will be verified in liquidity program CPI call
    pub flashloan_token_reserves_liquidity: UncheckedAccount<'info>,

    #[account(mut)]
    /// CHECK: Safe as this will be verified in liquidity program CPI call
    pub flashloan_borrow_position_on_liquidity: UncheckedAccount<'info>,

    /// CHECK: Safe as this will be verified in liquidity program CPI call
    pub rate_model: UncheckedAccount<'info>,

    #[account(mut)]
    /// CHECK: Safe as this will be verified in liquidity program CPI call
    pub vault: UncheckedAccount<'info>,

    // Liquidity PDA, which will be used for all the liquidity operations
    /// CHECK: Safe as this will be verified in liquidity program CPI call
    pub liquidity: UncheckedAccount<'info>,

    /// CHECK: Safe, we check the address in the lending_admin PDA
    pub liquidity_program: UncheckedAccount<'info>,

    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,

    #[account(address = SYSVAR_INSTRUCTIONS_ID)]
    /// CHECK: Safe
    pub instruction_sysvar: AccountInfo<'info>,
}

impl<'info> Flashloan<'info> {
    // default base accounts are for borrow operations
    fn get_base_accounts(&self) -> OperateCpiAccounts<'info> {
        OperateCpiAccounts {
            liquidity_program: self.liquidity_program.to_account_info(),
            protocol: self.flashloan_admin.to_account_info(),
            liquidity: self.liquidity.to_account_info(),
            token_reserve: self.flashloan_token_reserves_liquidity.to_account_info(),

            vault: self.vault.to_account_info(),

            user_supply_position: None,
            user_borrow_position: Some(
                self.flashloan_borrow_position_on_liquidity
                    .to_account_info(),
            ),

            rate_model: self.rate_model.to_account_info(),

            withdraw_to_account: None, // None as we are borrowing and paying back to LL
            borrow_to_account: Some(self.signer_borrow_token_account.to_account_info()),

            borrow_claim_account: None,
            withdraw_claim_account: None,

            mint: self.mint.to_account_info(),
            token_program: self.token_program.to_account_info(),
            associated_token_program: self.associated_token_program.to_account_info(),
        }
    }

    pub fn get_borrow_accounts(&self) -> OperateCpiAccounts<'info> {
        self.get_base_accounts()
    }

    pub fn get_payback_accounts(&self) -> OperateCpiAccounts<'info> {
        let mut base_accounts = self.get_base_accounts();

        base_accounts.borrow_to_account = None;

        base_accounts
    }
}
