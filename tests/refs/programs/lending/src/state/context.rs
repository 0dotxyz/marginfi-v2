use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{Mint, TokenAccount, TokenInterface},
};
use mpl_token_metadata::{
    instructions::CreateV1CpiBuilder,
    types::{PrintSupply, TokenStandard},
    ID as TOKEN_METADATA_PROGRAM_ID,
};

use crate::{constant::PROTOCOL_INIT_AUTH, errors::ErrorCodes};
use lending_reward_rate_model::state::LendingRewardsRateModel;
use liquidity::state::{TokenReserve, UserSupplyPosition};

use crate::invokes::*;
use crate::state::*;
use crate::utils::helpers::{convert_to_assets, get_liquidity_exchange_price};

const JL_TOKEN_URI: &str = "https://cdn.instadapp.io/solana/tokens/metadata/";

#[derive(Accounts)]
pub struct InitLendingAdmin<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        init,
        payer = authority,
        space = 8 + LendingAdmin::INIT_SPACE,
        seeds = [LENDING_ADMIN_SEED],
        bump,
    )]
    pub lending_admin: Account<'info, LendingAdmin>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(symbol: String, liquidity_program: Pubkey)]
pub struct InitLending<'info> {
    // @dev Only the auths can initialize the lending
    #[account(mut, constraint = lending_admin.auths.contains(&signer.key()) || signer.key() == PROTOCOL_INIT_AUTH @ ErrorCodes::FTokenOnlyAuth)]
    pub signer: Signer<'info>,

    #[account(mut, has_one = liquidity_program)]
    pub lending_admin: Account<'info, LendingAdmin>,

    #[account(owner = token_program.key())]
    pub mint: InterfaceAccount<'info, Mint>,

    #[account(
        init,
        seeds = [F_TOKEN_MINT_SEED, mint.key().as_ref()],
        bump,
        payer = signer,
        mint::decimals = mint.decimals,
        mint::authority = lending_admin,
        mint::token_program = token_program,
    )]
    pub f_token_mint: Box<InterfaceAccount<'info, Mint>>,

    /// CHECK: Metadata account PDA
    #[account(
        mut,
        seeds = [METAPLEX_METADATA_SEED, TOKEN_METADATA_PROGRAM_ID.as_ref(), f_token_mint.key().as_ref()],
        bump,
        seeds::program = TOKEN_METADATA_PROGRAM_ID,
    )]
    pub metadata_account: UncheckedAccount<'info>,

    #[account(
        init,
        payer = signer,
        space = 8 + Lending::INIT_SPACE,
        seeds = [LENDING_SEED, mint.key().as_ref(), f_token_mint.key().as_ref()],
        bump,
    )]
    pub lending: Account<'info, Lending>,

    #[account(has_one = mint, owner = liquidity_program)]
    pub token_reserves_liquidity: AccountLoader<'info, TokenReserve>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,

    /// CHECK: Sysvar Instructions account
    #[account(address = anchor_lang::solana_program::sysvar::instructions::ID)]
    pub sysvar_instruction: UncheckedAccount<'info>,

    /// CHECK: Metaplex Token Metadata program
    #[account(address = TOKEN_METADATA_PROGRAM_ID)]
    pub metadata_program: UncheckedAccount<'info>,

    pub rent: Sysvar<'info, Rent>,
}

impl InitLending<'_> {
    pub fn initialize_token_metadata(&self, symbol: String) -> Result<()> {
        let metadata_name = format!("jupiter lend {}", symbol.to_uppercase());
        let metadata_symbol = format!("jl{}", symbol.to_uppercase());
        let metadata_uri = JL_TOKEN_URI.to_owned() + &format!("{}.json", symbol.to_lowercase());

        let signer_seeds: &[&[&[u8]]] = &[&[LENDING_ADMIN_SEED, &[self.lending_admin.bump]]];

        CreateV1CpiBuilder::new(&self.metadata_program)
            .metadata(&self.metadata_account)
            .mint(&self.f_token_mint.to_account_info(), false)
            .authority(&self.lending_admin.to_account_info())
            .update_authority(&self.lending_admin.to_account_info(), true)
            .payer(&self.signer.to_account_info())
            .system_program(&self.system_program)
            .sysvar_instructions(&self.sysvar_instruction.to_account_info())
            .spl_token_program(Some(&self.token_program.to_account_info()))
            .token_standard(TokenStandard::Fungible)
            .seller_fee_basis_points(0)
            .print_supply(PrintSupply::Unlimited)
            .name(metadata_name.clone())
            .symbol(metadata_symbol.clone())
            .uri(metadata_uri)
            .decimals(self.f_token_mint.decimals)
            .invoke_signed(signer_seeds)?;

        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(mint: Pubkey)]
pub struct SetRewardsRateModel<'info> {
    // @dev Only the auths can update the rewards rate model
    #[account(constraint = lending_admin.auths.contains(&signer.key()) @ ErrorCodes::FTokenOnlyAuth)]
    pub signer: Signer<'info>,

    #[account()]
    pub lending_admin: Account<'info, LendingAdmin>,

    #[account(mut, has_one = mint, has_one = f_token_mint)]
    pub lending: Account<'info, Lending>,

    #[account()]
    pub f_token_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(has_one = mint)]
    pub new_rewards_rate_model: Box<Account<'info, LendingRewardsRateModel>>,

    #[account(has_one = mint, address = lending.token_reserves_liquidity)]
    pub supply_token_reserves_liquidity: AccountLoader<'info, TokenReserve>,
}

#[derive(Accounts)]
pub struct UpdateAuthority<'info> {
    // @dev Only the authority can update the authority
    #[account(address = lending_admin.authority @ ErrorCodes::FTokenOnlyAuthority)]
    pub signer: Signer<'info>,

    #[account(mut)]
    pub lending_admin: Account<'info, LendingAdmin>,
}

#[derive(Accounts)]
pub struct UpdateAuths<'info> {
    // @dev Only the authority can update the auths
    #[account(address = lending_admin.authority @ ErrorCodes::FTokenOnlyAuthority)]
    pub signer: Signer<'info>,

    #[account(mut)]
    pub lending_admin: Account<'info, LendingAdmin>,
}

#[derive(Accounts)]
pub struct UpdateRebalancer<'info> {
    #[account(constraint = lending_admin.auths.contains(&signer.key()) @ ErrorCodes::FTokenOnlyAuth)]
    pub signer: Signer<'info>,

    #[account(mut)]
    pub lending_admin: Account<'info, LendingAdmin>,
}

#[derive(Accounts)]
#[instruction(_amount: u64)]
pub struct Deposit<'info> {
    // depositor
    #[account(mut)]
    pub signer: Signer<'info>,

    #[account(
        mut,
        token::mint = mint,
        token::authority = signer,
        token::token_program = token_program
    )]
    pub depositor_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        token::mint = f_token_mint,
        token::authority = signer,
        token::token_program = token_program
    )]
    pub recipient_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account()]
    pub mint: InterfaceAccount<'info, Mint>,

    #[account(has_one = liquidity_program)]
    pub lending_admin: Box<Account<'info, LendingAdmin>>,

    #[account(mut, has_one = mint, has_one = f_token_mint)]
    pub lending: Box<Account<'info, Lending>>,

    #[account(mut)]
    pub f_token_mint: Box<InterfaceAccount<'info, Mint>>,

    // Here we will list all liquidity accounts, needed for CPI call to liquidity program
    // @dev no verification of accounts here, as they are already handled in liquidity program,
    // Hence loading them as UncheckedAccount

    // No need to verify liquidity program key here, as it will be verified in liquidity program CPI call
    #[account(mut, address = lending.token_reserves_liquidity)]
    /// CHECK: Safe as this will be verified in liquidity program CPI call
    pub supply_token_reserves_liquidity: AccountLoader<'info, TokenReserve>,

    #[account(mut)]
    /// CHECK: Safe as this will be verified in liquidity program CPI call
    pub lending_supply_position_on_liquidity: AccountLoader<'info, UserSupplyPosition>,

    #[account()]
    /// CHECK: Safe as this will be verified in liquidity program CPI call
    pub rate_model: UncheckedAccount<'info>,

    #[account(mut)]
    /// CHECK: Safe as this will be verified in liquidity program CPI call
    pub vault: UncheckedAccount<'info>,

    // Liquidity PDA, which will be used for all the liquidity operations
    #[account(mut)]
    /// CHECK: Safe as this will be verified in liquidity program CPI call
    pub liquidity: UncheckedAccount<'info>,

    /// CHECK: Safe, we check the address in the lending_admin PDA
    pub liquidity_program: UncheckedAccount<'info>,

    // Now we will add rewards rate model account for rewards rate calculations
    #[account(address = lending.rewards_rate_model, has_one = mint)]
    pub rewards_rate_model: Box<Account<'info, LendingRewardsRateModel>>,

    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

impl<'info> Deposit<'info> {
    pub fn get_deposit_accounts(&self) -> OperateCpiAccounts<'info> {
        OperateCpiAccounts {
            liquidity_program: self.liquidity_program.to_account_info(),
            protocol: self.lending.to_account_info(), // lending PDA supplying
            liquidity: self.liquidity.to_account_info(),
            token_reserve: self.supply_token_reserves_liquidity.to_account_info(), // as we are supplying, so pass the supply token reserve

            vault: self.vault.to_account_info(),

            user_supply_position: Some(self.lending_supply_position_on_liquidity.to_account_info()),
            user_borrow_position: None, // No borrow position for lending

            rate_model: self.rate_model.to_account_info(),

            // @dev As this is deposit instruction, we are not withdrawing or borrowing or doing any claim
            // passing these as None
            withdraw_to_account: None,
            borrow_to_account: None,

            borrow_claim_account: None,
            withdraw_claim_account: None,
            mint: self.mint.to_account_info(),

            token_program: self.token_program.to_account_info(),
            associated_token_program: self.associated_token_program.to_account_info(),
        }
    }

    pub fn preview_mint(&self, shares: u64) -> Result<u64> {
        let liquidity_exchange_price =
            get_liquidity_exchange_price(&self.supply_token_reserves_liquidity)?;

        convert_to_assets(
            &self.lending,
            &self.f_token_mint,
            &self.rewards_rate_model,
            liquidity_exchange_price,
            shares,
            true,
        )
    }

    pub fn get_supply_position_amount(&self) -> Result<u128> {
        let amount = {
            let position = self.lending_supply_position_on_liquidity.load()?;
            position.get_amount()?
        }; // Ref<UserSupplyPosition> is dropped here, releasing the borrow

        Ok(amount)
    }
}

#[derive(Accounts)]
pub struct Withdraw<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    #[account(
        mut,
        token::mint = f_token_mint,
        token::authority = signer,
        token::token_program = token_program
    )]
    pub owner_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        token::mint = mint,
        token::authority = signer,
        token::token_program = token_program
    )]
    pub recipient_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(has_one = liquidity_program)]
    pub lending_admin: Account<'info, LendingAdmin>,

    #[account(mut, has_one = mint, has_one = f_token_mint)]
    pub lending: Account<'info, Lending>,

    #[account()]
    pub mint: InterfaceAccount<'info, Mint>,

    #[account(mut)]
    pub f_token_mint: Box<InterfaceAccount<'info, Mint>>,

    // Here we will list all liquidity accounts, needed for CPI call to liquidity program
    // @dev no verification of accounts here, as they are already handled in liquidity program,
    // Hence loading them as UncheckedAccount
    #[account(mut, address = lending.token_reserves_liquidity)]
    /// CHECK: Safe as this will be verified in liquidity program CPI call
    pub supply_token_reserves_liquidity: AccountLoader<'info, TokenReserve>,

    #[account(mut)]
    /// CHECK: Safe as this will be verified in liquidity program CPI call
    pub lending_supply_position_on_liquidity: UncheckedAccount<'info>,

    #[account()]
    /// CHECK: Safe as this will be verified in liquidity program CPI call
    pub rate_model: UncheckedAccount<'info>,

    #[account(mut)]
    /// CHECK: Safe as this will be verified in liquidity program CPI call
    pub vault: UncheckedAccount<'info>,

    #[account(mut)]
    /// CHECK: Safe as this will be verified in liquidity program CPI call
    pub claim_account: Option<UncheckedAccount<'info>>,

    // Liquidity PDA, which will be used for all the liquidity operations
    #[account(mut)]
    /// CHECK: Safe as this will be verified in liquidity program CPI call
    pub liquidity: UncheckedAccount<'info>,

    /// CHECK: Safe, we check the address in the lending_admin PDA
    pub liquidity_program: UncheckedAccount<'info>,

    // Rewards rate model accounts
    #[account(address = lending.rewards_rate_model, has_one = mint)]
    pub rewards_rate_model: Box<Account<'info, LendingRewardsRateModel>>,

    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

impl<'info> Withdraw<'info> {
    pub fn get_withdraw_accounts(&self) -> OperateCpiAccounts<'info> {
        OperateCpiAccounts {
            liquidity_program: self.liquidity_program.to_account_info(),
            protocol: self.lending.to_account_info(),
            liquidity: self.liquidity.to_account_info(),
            token_reserve: self.supply_token_reserves_liquidity.to_account_info(),

            vault: self.vault.to_account_info(),

            user_supply_position: Some(self.lending_supply_position_on_liquidity.to_account_info()),
            user_borrow_position: None, // No borrow position for lending

            rate_model: self.rate_model.to_account_info(),

            withdraw_to_account: Some(self.recipient_token_account.to_account_info()), // Receiver's token account
            borrow_to_account: None, // @dev As this is withdraw instruction, we are not borrowing passing None

            borrow_claim_account: None,
            withdraw_claim_account: None, // Not required for withdraw, as the transferType is direct
            mint: self.mint.to_account_info(),

            token_program: self.token_program.to_account_info(),
            associated_token_program: self.associated_token_program.to_account_info(),
        }
    }

    pub fn preview_redeem(&self, shares: u64) -> Result<u64> {
        let liquidity_exchange_price =
            get_liquidity_exchange_price(&self.supply_token_reserves_liquidity)?;

        convert_to_assets(
            &self.lending,
            &self.f_token_mint,
            &self.rewards_rate_model,
            liquidity_exchange_price,
            shares,
            false,
        )
    }
}

#[derive(Accounts)]
pub struct Rebalance<'info> {
    #[account(mut, address = lending_admin.rebalancer @ ErrorCodes::FTokenOnlyRebalancer)]
    pub signer: Signer<'info>,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = signer,
        associated_token::token_program = token_program
    )]
    pub depositor_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(has_one = liquidity_program)]
    pub lending_admin: Account<'info, LendingAdmin>,

    #[account(mut, has_one = mint, has_one = f_token_mint)]
    pub lending: Account<'info, Lending>,

    #[account()]
    pub mint: InterfaceAccount<'info, Mint>,

    #[account(mut)]
    pub f_token_mint: Box<InterfaceAccount<'info, Mint>>,

    // Here we will list all liquidity accounts, needed for CPI call to liquidity program
    // @dev no verification of accounts here, as they are already handled in liquidity program,
    // Hence loading them as UncheckedAccount
    #[account(mut)]
    /// CHECK: Safe as this will be verified in liquidity program CPI call
    pub supply_token_reserves_liquidity: AccountLoader<'info, TokenReserve>,

    #[account(mut)]
    /// CHECK: Safe as this will be verified in liquidity program CPI call
    pub lending_supply_position_on_liquidity: AccountLoader<'info, UserSupplyPosition>,

    #[account(mut)]
    /// CHECK: Safe as this will be verified in liquidity program CPI call
    pub rate_model: UncheckedAccount<'info>,

    #[account(mut)]
    /// CHECK: Safe as this will be verified in liquidity program CPI call
    pub vault: UncheckedAccount<'info>,

    // Liquidity PDA, which will be used for all the liquidity operations
    #[account(mut)]
    /// CHECK: Safe as this will be verified in liquidity program CPI call
    pub liquidity: UncheckedAccount<'info>,

    /// CHECK: Safe, we check the address in the lending_admin PDA
    pub liquidity_program: UncheckedAccount<'info>,

    // Rewards rate model accounts
    #[account(address = lending.rewards_rate_model, has_one = mint)]
    pub rewards_rate_model: Box<Account<'info, LendingRewardsRateModel>>,

    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

impl<'info> Rebalance<'info> {
    pub fn get_rebalance_accounts(&self) -> OperateCpiAccounts<'info> {
        OperateCpiAccounts {
            liquidity_program: self.liquidity_program.to_account_info(),
            protocol: self.lending.to_account_info().clone(), // Since this operation is not withdrawing or borrowing, we can pass the lending PDA and their associated ATA
            liquidity: self.liquidity.to_account_info(), // Since this operation is not withdrawing or borrowing, we can pass the lending PDA and their associated ATA

            token_reserve: self.supply_token_reserves_liquidity.to_account_info(), // as we are supplying, so pass the supply token reserve
            vault: self.vault.to_account_info(),

            user_supply_position: Some(self.lending_supply_position_on_liquidity.to_account_info()),
            user_borrow_position: None, // No borrow position for lending

            rate_model: self.rate_model.to_account_info(),

            // @dev As this is rebalance deposit instruction, we are not withdrawing or borrowing or doing any claim
            // passing these as None
            withdraw_to_account: None,
            borrow_to_account: None,

            borrow_claim_account: None,
            withdraw_claim_account: None,
            mint: self.mint.to_account_info(),

            token_program: self.token_program.to_account_info(),
            associated_token_program: self.associated_token_program.to_account_info(),
        }
    }
}

#[derive(Accounts)]
pub struct UpdateRate<'info> {
    #[account(mut, has_one = mint, has_one = f_token_mint)]
    pub lending: Account<'info, Lending>,

    #[account()]
    pub mint: InterfaceAccount<'info, Mint>,

    #[account()]
    pub f_token_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(has_one = mint, address = lending.token_reserves_liquidity)]
    pub supply_token_reserves_liquidity: AccountLoader<'info, TokenReserve>,

    // Rewards rate model account
    #[account(address = lending.rewards_rate_model)]
    pub rewards_rate_model: Box<Account<'info, LendingRewardsRateModel>>,
}
