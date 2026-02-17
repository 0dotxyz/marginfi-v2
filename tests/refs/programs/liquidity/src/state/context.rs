use anchor_lang::prelude::*;

use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

use crate::constants::PROTOCOL_INIT_AUTH;
use crate::{errors::*, state::*};

#[derive(Accounts)]
#[instruction(mint: Pubkey)]
pub struct PreOperate<'info> {
    #[account()]
    pub protocol: Signer<'info>,

    #[account(constraint = !liquidity.is_locked() @ ErrorCodes::ProtocolLockdown)]
    pub liquidity: Box<Account<'info, Liquidity>>,

    #[account(has_one = protocol, has_one = mint)]
    pub user_supply_position: Option<AccountLoader<'info, UserSupplyPosition>>,
    #[account(has_one = protocol, has_one = mint)]
    pub user_borrow_position: Option<AccountLoader<'info, UserBorrowPosition>>,

    #[account(
        associated_token::mint = mint,
        associated_token::authority = liquidity,
        associated_token::token_program = token_program
    )]
    pub vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut, has_one = mint, has_one = vault)]
    pub token_reserve: AccountLoader<'info, TokenReserve>,

    pub associated_token_program: Program<'info, AssociatedToken>,
    pub token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
#[instruction(_supply_amount: i128, _borrow_amount: i128, withdraw_to: Pubkey, borrow_to: Pubkey)]
pub struct Operate<'info> {
    #[account()]
    pub protocol: Signer<'info>,

    #[account(constraint = !liquidity.is_locked() @ ErrorCodes::ProtocolLockdown)]
    pub liquidity: Box<Account<'info, Liquidity>>,

    #[account(mut, has_one = mint, has_one = vault)]
    pub token_reserve: AccountLoader<'info, TokenReserve>,

    #[account()]
    pub mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = liquidity,
        associated_token::token_program = token_program
    )]
    pub vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut, has_one = protocol)]
    pub user_supply_position: Option<AccountLoader<'info, UserSupplyPosition>>,
    #[account(mut, has_one = protocol)]
    pub user_borrow_position: Option<AccountLoader<'info, UserBorrowPosition>>,

    #[account(has_one = mint)]
    pub rate_model: AccountLoader<'info, RateModel>,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = withdraw_to,
        associated_token::token_program = token_program
    )]
    pub withdraw_to_account: Option<Box<InterfaceAccount<'info, TokenAccount>>>,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = borrow_to,
        associated_token::token_program = token_program
    )]
    pub borrow_to_account: Option<Box<InterfaceAccount<'info, TokenAccount>>>,

    #[account(mut, has_one = mint)]
    pub borrow_claim_account: Option<AccountLoader<'info, UserClaim>>,

    #[account(mut, has_one = mint)]
    pub withdraw_claim_account: Option<AccountLoader<'info, UserClaim>>,

    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

#[derive(Accounts)]
#[instruction(mint: Pubkey, user: Pubkey)]
pub struct InitClaimAccount<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    #[account(
        init,
        payer = signer,
        seeds = [USER_CLAIM_SEED, user.key().as_ref(), mint.key().as_ref()],
        space = 8 + UserClaim::INIT_SPACE,
        bump
    )]
    pub claim_account: AccountLoader<'info, UserClaim>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(mint: Pubkey)]
pub struct CloseClaimAccount<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut, 
        has_one = user, 
        has_one = mint, 
        constraint = claim_account.load()?.amount == 0 @ ErrorCodes::AmountNotZero,
        close = user
    )]
    pub claim_account: AccountLoader<'info, UserClaim>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(recipient: Pubkey)]
pub struct Claim<'info> {
    #[account()]
    pub user: Signer<'info>,

    #[account(constraint = !liquidity.is_locked() @ ErrorCodes::ProtocolLockdown)]
    pub liquidity: Box<Account<'info, Liquidity>>,

    #[account(mut, has_one = mint, has_one = vault)]
    pub token_reserve: AccountLoader<'info, TokenReserve>,

    #[account()]
    pub mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = recipient,
        associated_token::token_program = token_program
    )]
    pub recipient_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = liquidity,
        associated_token::token_program = token_program
    )]
    pub vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut, has_one = user, has_one = mint)]
    pub claim_account: AccountLoader<'info, UserClaim>,

    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

#[derive(Accounts)]
pub struct InitLiquidity<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    #[account(
        init,
        seeds = [LIQUIDITY_SEED],
        payer = signer,
        space = 8 + Liquidity::INIT_SPACE,
        bump
    )]
    pub liquidity: Account<'info, Liquidity>,

    #[account(
        init,
        seeds = [AUTH_LIST_SEED],
        payer = signer,
        space = 8 + AuthorizationList::INIT_SPACE,
        bump
    )]
    pub auth_list: Account<'info, AuthorizationList>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct InitTokenReserve<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account()]
    pub liquidity: Account<'info, Liquidity>,

    #[account(
        constraint = auth_list.auth_users.contains(&authority.key()) || authority.key() == PROTOCOL_INIT_AUTH @ ErrorCodes::OnlyAuth
    )]
    pub auth_list: Account<'info, AuthorizationList>,

    // @dev needed as an account for associated token program verification of TokenAccount
    pub mint: InterfaceAccount<'info, Mint>,

    #[account(
        init_if_needed,
        payer = authority,
        associated_token::mint = mint,
        associated_token::authority = liquidity,
        associated_token::token_program = token_program
    )]
    pub vault: InterfaceAccount<'info, TokenAccount>,

    #[account(
        init,
        payer = authority,
        seeds = [RATE_MODEL_SEED, mint.key().as_ref()],
        space = 8 + RateModel::INIT_SPACE,
        bump,
    )]
    pub rate_model: AccountLoader<'info, RateModel>,

    #[account(
        init,
        payer = authority,
        seeds = [RESERVE_SEED, mint.key().as_ref()],
        space = 8 + TokenReserve::INIT_SPACE,
        bump,
    )]
    pub token_reserve: AccountLoader<'info, TokenReserve>,

    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(supply_mint: Pubkey, borrow_mint: Pubkey, protocol: Pubkey)]
pub struct InitNewProtocol<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        constraint = auth_list.auth_users.contains(&authority.key()) || authority.key() == PROTOCOL_INIT_AUTH @ ErrorCodes::OnlyAuth
    )]
    pub auth_list: Account<'info, AuthorizationList>,

    #[account(
        init,
        payer = authority,
        seeds = [USER_SUPPLY_POSITION_SEED, supply_mint.key().as_ref(), protocol.key().as_ref()], // token specific user supply position
        space = 8 + UserSupplyPosition::INIT_SPACE,
        bump,
    )]
    pub user_supply_position: AccountLoader<'info, UserSupplyPosition>,

    #[account(
        init,
        payer = authority,
        seeds = [USER_BORROW_POSITION_SEED, borrow_mint.key().as_ref(), protocol.key().as_ref()], // token specific user borrow position
        space = 8 + UserBorrowPosition::INIT_SPACE,
        bump,
    )]
    pub user_borrow_position: AccountLoader<'info, UserBorrowPosition>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateAuthority<'info> {
    #[account(address = liquidity.authority @ ErrorCodes::OnlyLiquidityAuthority)]
    pub authority: Signer<'info>,

    #[account(mut)]
    pub liquidity: Account<'info, Liquidity>,

    #[account(mut)]
    pub auth_list: Account<'info, AuthorizationList>,
}

#[derive(Accounts)]
pub struct UpdateAuths<'info> {
    #[account(address = liquidity.authority @ ErrorCodes::OnlyLiquidityAuthority)]
    pub authority: Signer<'info>,

    #[account()]
    pub liquidity: Account<'info, Liquidity>,

    #[account(mut)]
    pub auth_list: Account<'info, AuthorizationList>,
}

#[derive(Accounts)]
pub struct UpdateRevenueCollector<'info> {
    #[account(address = liquidity.authority @ ErrorCodes::OnlyLiquidityAuthority)]
    pub authority: Signer<'info>,

    #[account(mut)]
    pub liquidity: Account<'info, Liquidity>,
}

#[derive(Accounts)]
pub struct CollectRevenue<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account()]
    pub liquidity: Account<'info, Liquidity>,

    #[account(constraint = auth_list.auth_users.contains(&authority.key()) @ ErrorCodes::OnlyAuth)]
    pub auth_list: Account<'info, AuthorizationList>,

    // @dev needed as an account for associated token program verification of TokenAccount
    pub mint: InterfaceAccount<'info, Mint>,

    #[account(
        init_if_needed,
        payer = authority,
        associated_token::mint = mint,
        associated_token::authority = revenue_collector,
        associated_token::token_program = token_program
    )]
    pub revenue_collector_account: InterfaceAccount<'info, TokenAccount>,

    #[account(address = liquidity.revenue_collector)]
    /// CHECK: This account is used a read only, and is revenue collector
    pub revenue_collector: UncheckedAccount<'info>,

    #[account(mut, has_one = mint, has_one = vault)]
    pub token_reserve: AccountLoader<'info, TokenReserve>,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = liquidity,
        associated_token::token_program = token_program
    )]
    pub vault: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ChangeStatus<'info> {
    #[account()]
    pub authority: Signer<'info>,

    #[account(mut)]
    pub liquidity: Account<'info, Liquidity>,

    #[account(constraint = auth_list.auth_users.contains(&authority.key()) @ ErrorCodes::OnlyAuth)]
    pub auth_list: Account<'info, AuthorizationList>,
}

#[derive(Accounts)]
pub struct UpdateRateData<'info> {
    #[account()]
    pub authority: Signer<'info>,

    #[account(constraint = auth_list.auth_users.contains(&authority.key()) @ ErrorCodes::OnlyAuth)]
    pub auth_list: Account<'info, AuthorizationList>,

    #[account(mut, has_one = mint)]
    pub rate_model: AccountLoader<'info, RateModel>,

    pub mint: InterfaceAccount<'info, Mint>,

    #[account(mut, has_one = mint)]
    pub token_reserve: AccountLoader<'info, TokenReserve>,
}

#[derive(Accounts)]
pub struct UpdateTokenConfig<'info> {
    #[account()]
    pub authority: Signer<'info>,

    #[account(constraint = auth_list.auth_users.contains(&authority.key()) @ ErrorCodes::OnlyAuth)]
    pub auth_list: Account<'info, AuthorizationList>,

    #[account(mut, has_one = mint)]
    pub rate_model: AccountLoader<'info, RateModel>,

    pub mint: InterfaceAccount<'info, Mint>,

    #[account(mut, has_one = mint)]
    pub token_reserve: AccountLoader<'info, TokenReserve>,
}

#[derive(Accounts)]
pub struct UpdateUserClass<'info> {
    #[account()]
    pub authority: Signer<'info>,

    #[account(mut, constraint = auth_list.auth_users.contains(&authority.key()) @ ErrorCodes::OnlyAuth)]
    pub auth_list: Account<'info, AuthorizationList>,
}

#[derive(Accounts)]
#[instruction(new_limit: u128, protocol: Pubkey, mint: Pubkey)]
pub struct UpdateUserWithdrawalLimit<'info> {
    #[account()]
    pub authority: Signer<'info>,

    #[account(constraint = auth_list.auth_users.contains(&authority.key()) @ ErrorCodes::OnlyAuth)]
    pub auth_list: Account<'info, AuthorizationList>,

    #[account(mut, has_one = mint, has_one = protocol)]
    pub user_supply_position: AccountLoader<'info, UserSupplyPosition>,
}

#[derive(Accounts)]
pub struct UpdateUserSupplyConfig<'info> {
    #[account()]
    pub authority: Signer<'info>,

    #[account()]
    /// CHECK: This account is used a read only, and is the protocol account
    pub protocol: UncheckedAccount<'info>,

    #[account(constraint = auth_list.auth_users.contains(&authority.key()) @ ErrorCodes::OnlyAuth)]
    pub auth_list: Account<'info, AuthorizationList>,

    #[account(has_one = mint)]
    pub rate_model: AccountLoader<'info, RateModel>,

    pub mint: InterfaceAccount<'info, Mint>,

    #[account(mut, has_one = mint)]
    pub token_reserve: AccountLoader<'info, TokenReserve>,

    #[account(mut, has_one = mint, has_one = protocol)]
    pub user_supply_position: AccountLoader<'info, UserSupplyPosition>,
}

#[derive(Accounts)]
pub struct UpdateUserBorrowConfig<'info> {
    #[account()]
    pub authority: Signer<'info>,

    #[account()]
    /// CHECK: This account is used a read only, and is the protocol account
    pub protocol: UncheckedAccount<'info>,

    #[account(constraint = auth_list.auth_users.contains(&authority.key()) @ ErrorCodes::OnlyAuth)]
    pub auth_list: Account<'info, AuthorizationList>,

    #[account(has_one = mint)]
    pub rate_model: AccountLoader<'info, RateModel>,

    pub mint: InterfaceAccount<'info, Mint>,

    #[account(mut, has_one = mint)]
    pub token_reserve: AccountLoader<'info, TokenReserve>,

    #[account(mut, has_one = mint, has_one = protocol)]
    pub user_borrow_position: AccountLoader<'info, UserBorrowPosition>,
}

#[derive(Accounts)]
#[instruction(protocol: Pubkey)]
pub struct PauseUser<'info> {
    #[account()]
    pub authority: Signer<'info>,

    #[account(constraint = auth_list.guardians.contains(&authority.key()) @ ErrorCodes::OnlyGuardians)]
    pub auth_list: Account<'info, AuthorizationList>,

    #[account(mut, has_one = protocol)]
    pub user_supply_position: AccountLoader<'info, UserSupplyPosition>,
    #[account(mut, has_one = protocol)]
    pub user_borrow_position: AccountLoader<'info, UserBorrowPosition>,
}

#[derive(Accounts)]
#[instruction(mint: Pubkey)]
pub struct UpdateExchangePrice<'info> {
    #[account(mut, has_one = mint)]
    pub token_reserve: AccountLoader<'info, TokenReserve>,

    #[account(has_one = mint)]
    pub rate_model: AccountLoader<'info, RateModel>,
}
