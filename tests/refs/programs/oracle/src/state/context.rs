use anchor_lang::prelude::*;

use crate::errors::ErrorCodes;
use crate::state::structs::Sources;
use crate::state::{Oracle, OracleAdmin, ORACLE_ADMIN_SEED, ORACLE_SEED};

#[derive(Accounts)]
#[instruction(_sources: Vec<Sources>, nonce: u16)]
pub struct InitOracleConfig<'info> {
    #[account(mut, constraint = oracle_admin.auths.contains(&signer.key()) @ ErrorCodes::OracleAdminOnlyAuth)]
    pub signer: Signer<'info>,

    pub oracle_admin: Account<'info, OracleAdmin>,

    #[account(
        init,
        payer = signer,
        space = 8 + Oracle::INIT_SPACE,
        seeds = [ORACLE_SEED, nonce.to_le_bytes().as_slice()],
        bump
    )]
    pub oracle: Account<'info, Oracle>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(nonce: u16)]
pub struct GetExchangeRate<'info> {
    #[account(constraint = oracle.nonce == nonce @ ErrorCodes::OracleNonceMismatch)]
    pub oracle: Account<'info, Oracle>,
}

#[derive(Accounts)]
pub struct InitAdmin<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    #[account(
        init,
        payer = signer,
        space = 8 + OracleAdmin::INIT_SPACE,
        seeds = [ORACLE_ADMIN_SEED],
        bump
    )]
    pub oracle_admin: Account<'info, OracleAdmin>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateAuths<'info> {
    #[account(address = oracle_admin.authority @ ErrorCodes::OracleAdminOnlyAuthority)]
    pub authority: Signer<'info>,

    #[account(mut)]
    pub oracle_admin: Account<'info, OracleAdmin>,
}

#[derive(Accounts)]
pub struct UpdateAuthority<'info> {
    #[account(address = oracle_admin.authority @ ErrorCodes::OracleAdminOnlyAuthority)]
    pub authority: Signer<'info>,

    #[account(mut)]
    pub oracle_admin: Account<'info, OracleAdmin>,
}
