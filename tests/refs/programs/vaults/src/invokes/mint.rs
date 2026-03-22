use anchor_lang::prelude::*;
use anchor_spl::token::{self, MintTo, SetAuthority};
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

use crate::state::VaultAdmin;

pub fn mint_position_token_and_remove_authority<'info>(
    admin: &Account<'info, VaultAdmin>,
    position_mint: &InterfaceAccount<'info, Mint>,
    position_token_account: &InterfaceAccount<'info, TokenAccount>,
    token_program: &Interface<'info, TokenInterface>,
    seeds: &[&[&[u8]]],
) -> Result<()> {
    mint_position_token(
        admin,
        position_mint,
        position_token_account,
        token_program,
        seeds,
    )?;
    remove_position_token_mint_authority(admin, position_mint, token_program, seeds)
}

fn mint_position_token<'info>(
    admin: &Account<'info, VaultAdmin>,
    position_mint: &InterfaceAccount<'info, Mint>,
    position_token_account: &InterfaceAccount<'info, TokenAccount>,
    token_program: &Interface<'info, TokenInterface>,
    seeds: &[&[&[u8]]],
) -> Result<()> {
    token::mint_to(
        CpiContext::new_with_signer(
            token_program.to_account_info(),
            MintTo {
                mint: position_mint.to_account_info(),
                to: position_token_account.to_account_info(),
                authority: admin.to_account_info(),
            },
            seeds,
        ),
        1,
    )?;
    Ok(())
}

fn remove_position_token_mint_authority<'info>(
    admin: &Account<'info, VaultAdmin>,
    position_mint: &InterfaceAccount<'info, Mint>,
    token_program: &Interface<'info, TokenInterface>,
    seeds: &[&[&[u8]]],
) -> Result<()> {
    token::set_authority(
        CpiContext::new_with_signer(
            token_program.to_account_info(),
            SetAuthority {
                account_or_mint: position_mint.to_account_info(),
                current_authority: admin.to_account_info(),
            },
            seeds,
        ),
        spl_token::instruction::AuthorityType::MintTokens,
        None,
    )?;
    Ok(())
}
