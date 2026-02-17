use anchor_lang::prelude::*;

use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use mpl_token_metadata::{
    instructions::{BurnV1CpiBuilder, CreateV1CpiBuilder},
    types::{PrintSupply, TokenStandard},
    ID as TOKEN_METADATA_PROGRAM_ID,
};

use crate::constants::PROTOCOL_INIT_AUTH;
use crate::errors::ErrorCodes;
use crate::{invokes::liquidity_layer::*, state::*};

use liquidity::state::{TokenReserve, UserBorrowPosition, UserSupplyPosition};
use oracle::state::Oracle;

const JV_TOKEN_URI: &str = "https://cdn.instadapp.io/solana/vaults/metadata/";

/***********************************|
|           Admin Context           |
|__________________________________*/

#[derive(Accounts)]
pub struct InitVaultAdmin<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    #[account(
        init,
        payer = signer,
        space = 8 + VaultAdmin::INIT_SPACE,
        seeds = [VAULT_ADMIN_SEED],
        bump
    )]
    pub vault_admin: Account<'info, VaultAdmin>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(vault_id: u16)]
pub struct InitVaultConfig<'info> {
    #[account(mut, constraint = vault_admin.auths.contains(&authority.key()) || authority.key() == PROTOCOL_INIT_AUTH @ ErrorCodes::VaultAdminOnlyAuths)]
    pub authority: Signer<'info>,

    #[account(mut)]
    pub vault_admin: Account<'info, VaultAdmin>,

    #[account(
        init,
        payer = authority,
        space = 8 + VaultConfig::INIT_SPACE,
        seeds = [VAULT_CONFIG_SEED, vault_id.to_le_bytes().as_slice()],
        bump
    )]
    pub vault_config: AccountLoader<'info, VaultConfig>,

    #[account(
        init,
        payer = authority,
        space = 8 + VaultMetadata::INIT_SPACE,
        seeds = [VAULT_METADATA_SEED, vault_id.to_le_bytes().as_slice()],
        bump
    )]
    pub vault_metadata: Account<'info, VaultMetadata>,

    pub oracle: Account<'info, Oracle>,

    pub supply_token: Box<InterfaceAccount<'info, Mint>>,
    pub borrow_token: Box<InterfaceAccount<'info, Mint>>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(vault_id: u16)]
pub struct InitVaultState<'info> {
    #[account(mut, constraint = vault_admin.auths.contains(&authority.key()) || authority.key() == PROTOCOL_INIT_AUTH @ ErrorCodes::VaultAdminOnlyAuths)]
    pub authority: Signer<'info>,

    #[account()]
    pub vault_admin: Account<'info, VaultAdmin>,

    #[account()]
    /// @dev Verification inside instruction logic
    pub vault_config: AccountLoader<'info, VaultConfig>,

    #[account(
        init,
        payer = authority,
        space = 8 + VaultState::INIT_SPACE,
        seeds = [VAULT_STATE_SEED, vault_id.to_le_bytes().as_slice()],
        bump
    )]
    pub vault_state: AccountLoader<'info, VaultState>,

    #[account()]
    /// @dev Verification inside instruction logic
    pub supply_token_reserves_liquidity: AccountLoader<'info, TokenReserve>,
    #[account()]
    /// @dev Verification inside instruction logic
    pub borrow_token_reserves_liquidity: AccountLoader<'info, TokenReserve>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(vault_id: u16, branch_id: u32)]
pub struct InitBranch<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    /// @dev Verification inside instruction logic
    pub vault_config: AccountLoader<'info, VaultConfig>,

    #[account(
        init,
        payer = signer,
        space = 8 + Branch::INIT_SPACE,
        seeds = [BRANCH_SEED, vault_id.to_le_bytes().as_slice(), branch_id.to_le_bytes().as_slice()],
        bump
    )]
    pub branch: AccountLoader<'info, Branch>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(vault_id: u16, index: u8)]
pub struct InitTickHasDebtArray<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    /// @dev Verification inside instruction logic
    pub vault_config: AccountLoader<'info, VaultConfig>,

    #[account(
        init,
        payer = signer,
        space = 8 + TickHasDebtArray::INIT_SPACE,
        seeds = [TICK_HAS_DEBT_SEED, vault_id.to_le_bytes().as_slice(), index.to_le_bytes().as_slice()],
        bump
    )]
    pub tick_has_debt_array: AccountLoader<'info, TickHasDebtArray>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(vault_id: u16, tick: i32)]
pub struct InitTick<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    /// @dev Verification inside instruction logic
    pub vault_config: AccountLoader<'info, VaultConfig>,

    #[account(
        init,
        payer = signer,
        space = 8 + Tick::INIT_SPACE,
        seeds = [TICK_SEED, vault_id.to_le_bytes().as_slice(), (tick - MIN_TICK).to_le_bytes().as_slice()],
        // @dev MIN_TICK i.e., -16383 is the lowest tick value, so here we are subtracting -MIN_TICK to make it positive
        bump
    )]
    pub tick_data: AccountLoader<'info, Tick>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(vault_id: u16, tick: i32, total_ids: u32)]
pub struct InitTickIdLiquidation<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    /// @dev Verification inside instruction logic
    pub tick_data: AccountLoader<'info, Tick>,

    #[account(
        init,
        payer = signer,
        space = 8 + TickIdLiquidation::INIT_SPACE,
        seeds = [TICK_ID_LIQUIDATION_SEED, vault_id.to_le_bytes().as_slice(), (tick - MIN_TICK).to_le_bytes().as_slice(), ((total_ids + 2) / 3).to_le_bytes().as_slice()],
        bump
    )]
    pub tick_id_liquidation: AccountLoader<'info, TickIdLiquidation>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Admin<'info> {
    #[account(constraint = vault_admin.auths.contains(&authority.key()) @ ErrorCodes::VaultAdminOnlyAuths)]
    pub authority: Signer<'info>,

    #[account()]
    pub vault_admin: Account<'info, VaultAdmin>,

    #[account(mut)]
    /// @dev Verification inside instruction logic
    pub vault_state: AccountLoader<'info, VaultState>,

    #[account(mut)]
    /// @dev Verification inside instruction logic
    pub vault_config: AccountLoader<'info, VaultConfig>,

    /// @dev Verification inside instruction logic
    pub supply_token_reserves_liquidity: AccountLoader<'info, TokenReserve>,
    /// @dev Verification inside instruction logic
    pub borrow_token_reserves_liquidity: AccountLoader<'info, TokenReserve>,
}

#[derive(Accounts)]
pub struct UpdateAuths<'info> {
    // @dev Only the authority can update the auths
    #[account(address = vault_admin.authority @ ErrorCodes::VaultAdminOnlyAuthority)]
    pub signer: Signer<'info>,

    #[account(mut)]
    pub vault_admin: Account<'info, VaultAdmin>,
}

#[derive(Accounts)]
pub struct UpdateAuthority<'info> {
    // @dev Only the authority can update the authority
    #[account(address = vault_admin.authority @ ErrorCodes::VaultAdminOnlyAuthority)]
    pub signer: Signer<'info>,

    #[account(mut)]
    pub vault_admin: Account<'info, VaultAdmin>,
}

#[derive(Accounts)]
pub struct UpdateLookupTable<'info> {
    #[account(constraint = vault_admin.auths.contains(&authority.key()) || authority.key() == PROTOCOL_INIT_AUTH @ ErrorCodes::VaultAdminOnlyAuths)]
    pub authority: Signer<'info>,

    #[account()]
    pub vault_admin: Account<'info, VaultAdmin>,

    #[account(mut)]
    /// @dev Verification inside instruction logic
    pub vault_metadata: Account<'info, VaultMetadata>,
}

#[derive(Accounts)]
pub struct UpdateOracle<'info> {
    #[account(constraint = vault_admin.auths.contains(&authority.key()) @ ErrorCodes::VaultAdminOnlyAuths)]
    pub authority: Signer<'info>,

    #[account()]
    pub vault_admin: Account<'info, VaultAdmin>,

    #[account(mut)]
    /// @dev Verification inside instruction logic
    pub vault_state: AccountLoader<'info, VaultState>,

    #[account(mut)]
    /// @dev Verification inside instruction logic
    pub vault_config: AccountLoader<'info, VaultConfig>,

    #[account()]
    pub new_oracle: Account<'info, Oracle>,

    /// @dev Verification inside instruction logic
    pub supply_token_reserves_liquidity: AccountLoader<'info, TokenReserve>,
    /// @dev Verification inside instruction logic
    pub borrow_token_reserves_liquidity: AccountLoader<'info, TokenReserve>,
}

#[derive(Accounts)]
pub struct UpdateExchangePrices<'info> {
    #[account(mut)]
    /// @dev Verification inside instruction logic
    pub vault_state: AccountLoader<'info, VaultState>,

    /// @dev Verification inside instruction logic
    pub vault_config: AccountLoader<'info, VaultConfig>,

    /// @dev Verification inside instruction logic
    pub supply_token_reserves_liquidity: AccountLoader<'info, TokenReserve>,
    /// @dev Verification inside instruction logic
    pub borrow_token_reserves_liquidity: AccountLoader<'info, TokenReserve>,
}

/***********************************|
|           User Context            |
|__________________________________*/

#[derive(Accounts)]
#[instruction(vault_id: u16, next_position_id: u32)]
pub struct InitPosition<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    pub vault_admin: Box<Account<'info, VaultAdmin>>,

    #[account(mut)]
    /// @dev Verification inside instruction logic
    pub vault_state: AccountLoader<'info, VaultState>,

    #[account(
        init,
        payer = signer,
        space = 8 + Position::INIT_SPACE,
        seeds = [POSITION_SEED, vault_id.to_le_bytes().as_slice(), next_position_id.to_le_bytes().as_slice()],
        bump,
    )]
    pub position: AccountLoader<'info, Position>,

    #[account(
        init,
        payer = signer,
        mint::authority = vault_admin.key(),
        mint::decimals = 0,
        seeds = [POSITION_MINT_SEED, vault_id.to_le_bytes().as_slice(), next_position_id.to_le_bytes().as_slice()],
        bump,
    )]
    pub position_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        init,
        payer = signer,
        associated_token::mint = position_mint,
        associated_token::authority = signer.to_account_info(),
    )]
    pub position_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    /// CHECK: Metadata account PDA
    #[account(
        mut,
        seeds = [METAPLEX_METADATA_SEED, TOKEN_METADATA_PROGRAM_ID.as_ref(), position_mint.key().as_ref()],
        bump,
        seeds::program = TOKEN_METADATA_PROGRAM_ID,
    )]
    pub metadata_account: UncheckedAccount<'info>,

    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,

    /// CHECK: Sysvar Instructions account
    #[account(address = anchor_lang::solana_program::sysvar::instructions::ID)]
    pub sysvar_instruction: UncheckedAccount<'info>,

    /// CHECK: Metaplex Token Metadata program
    #[account(address = TOKEN_METADATA_PROGRAM_ID)]
    pub metadata_program: UncheckedAccount<'info>,

    pub rent: Sysvar<'info, Rent>,
}

impl<'info> InitPosition<'info> {
    pub fn initialize_token_metadata(&self, vault_id: u16) -> Result<()> {
        let metadata_name = format!("jupiter vault {}", vault_id);
        let metadata_symbol = format!("jv{}", vault_id);
        let metadata_uri = JV_TOKEN_URI.to_owned() + &format!("{}.json", vault_id);

        let signer_seeds: &[&[&[u8]]] = &[&[VAULT_ADMIN_SEED, &[self.vault_admin.bump]]];

        CreateV1CpiBuilder::new(&self.metadata_program)
            .metadata(&self.metadata_account)
            .mint(&self.position_mint.to_account_info(), false)
            .authority(&self.vault_admin.to_account_info())
            .update_authority(&self.vault_admin.to_account_info(), true)
            .payer(&self.signer.to_account_info())
            .system_program(&self.system_program)
            .sysvar_instructions(&self.sysvar_instruction.to_account_info())
            .spl_token_program(Some(&self.token_program.to_account_info()))
            .token_standard(TokenStandard::Fungible)
            .seller_fee_basis_points(0)
            .print_supply(PrintSupply::Zero)
            .name(metadata_name.clone())
            .symbol(metadata_symbol.clone())
            .uri(metadata_uri)
            .decimals(0)
            .invoke_signed(signer_seeds)?;

        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(vault_id: u16, position_id: u32)]
pub struct ClosePosition<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    pub vault_admin: Box<Account<'info, VaultAdmin>>,

    #[account(mut)]
    /// @dev Verification inside instruction logic
    pub vault_state: AccountLoader<'info, VaultState>,

    #[account(mut)]
    /// @dev Verification inside instruction logic
    pub vault_config: AccountLoader<'info, VaultConfig>,

    #[account(mut, close = signer)]
    /// @dev Verification inside instruction logic
    pub position: AccountLoader<'info, Position>,

    #[account(mut)]
    /// @dev Verification inside instruction logic
    pub position_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut)]
    /// @dev Verification inside instruction logic
    pub position_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    /// CHECK: Metadata account PDA
    #[account(
        mut,
        seeds = [METAPLEX_METADATA_SEED, TOKEN_METADATA_PROGRAM_ID.as_ref(), position_mint.key().as_ref()],
        bump,
        seeds::program = TOKEN_METADATA_PROGRAM_ID,
    )]
    pub metadata_account: UncheckedAccount<'info>,

    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,

    /// CHECK: Sysvar Instructions account
    #[account(address = anchor_lang::solana_program::sysvar::instructions::ID)]
    pub sysvar_instruction: UncheckedAccount<'info>,

    /// CHECK: Metaplex Token Metadata program
    #[account(address = TOKEN_METADATA_PROGRAM_ID)]
    pub metadata_program: UncheckedAccount<'info>,
}

impl<'info> ClosePosition<'info> {
    // @dev burns token, closes token account, and closes metadata account
    pub fn burn_token_metadata(&self) -> Result<()> {
        let signer_seeds: &[&[&[u8]]] = &[&[VAULT_ADMIN_SEED, &[self.vault_admin.bump]]];

        BurnV1CpiBuilder::new(&self.metadata_program)
            .authority(&self.signer.to_account_info())
            .metadata(&self.metadata_account.to_account_info())
            .mint(&self.position_mint.to_account_info())
            .system_program(&self.system_program.to_account_info())
            .sysvar_instructions(&self.sysvar_instruction.to_account_info())
            .spl_token_program(&self.token_program.to_account_info())
            .token(&self.position_token_account.to_account_info())
            .amount(1)
            .invoke_signed(signer_seeds)?;

        Ok(())
    }
}

#[derive(Accounts)]
pub struct Operate<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    // This is the token account, which will deposit tokens to the vault
    #[account(
        init_if_needed,
        payer = signer,
        associated_token::mint = supply_token,
        associated_token::authority = signer, // @dev supply happens at signer account
        associated_token::token_program = supply_token_program
    )]
    pub signer_supply_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        init_if_needed,
        payer = signer,
        associated_token::mint = borrow_token,
        associated_token::authority = signer, // @dev payback happens at signer account
        associated_token::token_program = borrow_token_program
    )]
    pub signer_borrow_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account()]
    /// CHECK: This account is used as a destination for funds
    pub recipient: Option<AccountInfo<'info>>,

    // This is the token account, which will receive the tokens in case of withdraw or borrow, its key should match "to" parameter
    #[account(
        init_if_needed,
        payer = signer,
        associated_token::mint = borrow_token,
        associated_token::authority = recipient, // @dev borrow happens at recipient account
        associated_token::token_program = borrow_token_program
    )]
    pub recipient_borrow_token_account: Option<Box<InterfaceAccount<'info, TokenAccount>>>,

    #[account(
        init_if_needed,
        payer = signer,
        associated_token::mint = supply_token,
        associated_token::authority = recipient, // @dev withdraw happens at recipient account
        associated_token::token_program = supply_token_program
    )]
    pub recipient_supply_token_account: Option<Box<InterfaceAccount<'info, TokenAccount>>>,

    /// @dev mut because this PDA signs the CPI to liquidity program
    /// @dev verification inside instruction logic
    pub vault_config: AccountLoader<'info, VaultConfig>,
    #[account(mut)]
    /// @dev verification inside instruction logic
    pub vault_state: AccountLoader<'info, VaultState>,

    // This is the mint account for spl tokens
    pub supply_token: Box<InterfaceAccount<'info, Mint>>,
    pub borrow_token: Box<InterfaceAccount<'info, Mint>>,

    // This is oracle PDA account for Fluid Oracle, for hop 0 like WETH/USD
    /// CHECK: This account is used as a first oracle account and is verified in the instruction logic
    pub oracle: Box<Account<'info, Oracle>>,
    // @dev sources have been moved to remaining_accounts

    // @dev position init will be a separate instruction
    #[account(mut)]
    pub position: AccountLoader<'info, Position>,
    /// @dev verification inside instruction logic
    pub position_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    // @dev The tick associated with current position of user.
    pub current_position_tick: AccountLoader<'info, Tick>,

    #[account(mut)]
    pub final_position_tick: AccountLoader<'info, Tick>,

    // @dev This account should be associated with the existing position tick
    pub current_position_tick_id: AccountLoader<'info, TickIdLiquidation>,

    #[account(mut)]
    // @dev This account should be associated with the new tick after operate
    pub final_position_tick_id: AccountLoader<'info, TickIdLiquidation>,

    // @dev this is the branch for the new tick, it should be created if it doesn't exits before operate call
    #[account(mut)]
    pub new_branch: AccountLoader<'info, Branch>,

    // Here we will list all liquidity accounts, needed for CPI call to liquidity program
    // @dev no verification of accounts here, as they are already handled in liquidity program,
    // Hence loading them as UncheckedAccount
    // No need to verify liquidity program key here, as it will be verified in liquidity program CPI call
    #[account(mut)]
    pub supply_token_reserves_liquidity: AccountLoader<'info, TokenReserve>,
    #[account(mut)]
    pub borrow_token_reserves_liquidity: AccountLoader<'info, TokenReserve>,

    // @dev This is current vault UserSupplyPosition and UserBorrowPosition PDA on liquidity program
    #[account(mut)]
    /// CHECK: Safe as this will be verified in liquidity program CPI call
    pub vault_supply_position_on_liquidity: AccountLoader<'info, UserSupplyPosition>,
    #[account(mut)]
    /// CHECK: Safe as this will be verified in liquidity program CPI call
    pub vault_borrow_position_on_liquidity: AccountLoader<'info, UserBorrowPosition>,

    /// CHECK: Safe as this will be verified in liquidity program CPI call
    pub supply_rate_model: UncheckedAccount<'info>,
    /// CHECK: Safe as this will be verified in liquidity program CPI call
    pub borrow_rate_model: UncheckedAccount<'info>,

    #[account(mut)]
    /// CHECK: Safe as this will be verified in liquidity program CPI call
    pub vault_supply_token_account: UncheckedAccount<'info>,
    #[account(mut)]
    /// CHECK: Safe as this will be verified in liquidity program CPI call
    pub vault_borrow_token_account: UncheckedAccount<'info>,

    /// CHECK: Safe as this will be verified in liquidity program CPI call
    #[account(mut)]
    pub supply_token_claim_account: Option<UncheckedAccount<'info>>,
    /// CHECK: Safe as this will be verified in liquidity program CPI call
    #[account(mut)]
    pub borrow_token_claim_account: Option<UncheckedAccount<'info>>,

    /// CHECK: Safe as this will be verified in liquidity program CPI call
    pub liquidity: UncheckedAccount<'info>,

    /// CHECK: Safe as we are checking liquidity program key in instruction logic
    pub liquidity_program: UncheckedAccount<'info>,

    /// CHECK: Safe as we are checking oracle program key in instruction logic
    pub oracle_program: AccountInfo<'info>,

    // These are system program and can not be mutated
    pub supply_token_program: Interface<'info, TokenInterface>,
    pub borrow_token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
    // remaining_accounts[0..remaining_accounts_indices[0]] are oracle sources
    // remaining_accounts[remaining_accounts_indices[0]..remaining_accounts_indices[1]] are branch accounts
    // remaining_accounts[(remaining_accounts_indices[0] + remaining_accounts_indices[1])..remaining_accounts_indices[2]] are tick_has_debt_array accounts
}

impl<'info> Operate<'info> {
    // default base accounts are for deposit operations
    fn get_base_accounts(&self) -> OperateCpiAccounts<'info> {
        OperateCpiAccounts {
            liquidity_program: self.liquidity_program.to_account_info(),
            protocol: self.vault_config.to_account_info(),
            liquidity: self.liquidity.to_account_info(),
            token_reserve: self.supply_token_reserves_liquidity.to_account_info(),

            vault: self.vault_supply_token_account.to_account_info(),

            user_supply_position: Some(self.vault_supply_position_on_liquidity.to_account_info()),
            user_borrow_position: Some(self.vault_borrow_position_on_liquidity.to_account_info()),

            rate_model: self.supply_rate_model.to_account_info(),

            // @dev As this is deposit instruction, we are not withdrawing or borrowing or doing any claim
            // passing these as None
            withdraw_to_account: None,
            borrow_to_account: None,

            borrow_claim_account: None,
            withdraw_claim_account: None,
            mint: self.supply_token.to_account_info(),

            token_program: self.supply_token_program.to_account_info(),
            associated_token_program: self.associated_token_program.to_account_info(),
        }
    }

    // @dev In deposit, the token in context is supply token, so we need to pass supply token accounts
    // Since in deposit there is no withdraw or borrow, we pass default withdraw and borrow to accounts
    pub fn get_deposit_accounts(&self) -> OperateCpiAccounts<'info> {
        self.get_base_accounts()
    }

    fn is_recipient_withdraw_account(&self) -> bool {
        self.recipient.is_some() && self.recipient_supply_token_account.is_some()
    }

    fn is_recipient_borrow_account(&self) -> bool {
        self.recipient.is_some() && self.recipient_borrow_token_account.is_some()
    }

    pub fn get_recipient(&self) -> Result<AccountInfo<'info>> {
        if self.recipient.is_some() {
            Ok(self.recipient.as_ref().unwrap().to_account_info())
        } else {
            Ok(self.signer.to_account_info())
        }
    }

    // @dev In withdraw, the token in context is supply token, so we need to pass supply token accounts
    // Since in withdraw there is no borrow, we pass default borrow to account
    // and respective withdraw at recipient account
    pub fn get_withdraw_accounts(&self, is_claim_type: bool) -> Result<OperateCpiAccounts<'info>> {
        let mut base_accounts = self.get_base_accounts();

        base_accounts.withdraw_to_account = if self.is_recipient_withdraw_account() {
            // Destination account for supply token, here recipient account
            Some(
                self.recipient_supply_token_account
                    .as_ref()
                    .ok_or(ErrorCodes::VaultRecipientWithdrawAccountRequired)?
                    .to_account_info(),
            )
        } else {
            // If recipient is not provided, we default to signer account as recipient
            Some(self.signer_supply_token_account.to_account_info())
        };

        if is_claim_type {
            let claim_account = self
                .supply_token_claim_account
                .as_ref()
                .ok_or(ErrorCodes::VaultClaimAccountRequired)?;

            base_accounts.withdraw_claim_account = Some(claim_account.to_account_info());
        }

        Ok(base_accounts)
    }

    // @dev In payback, the token in context is borrow token, so we need to pass borrow token accounts
    // Since in payback there is no withdraw or borrow, we pass default withdraw and borrow to accounts
    pub fn get_payback_accounts(&self) -> OperateCpiAccounts<'info> {
        let mut base_accounts = self.get_base_accounts();

        base_accounts.vault = self.vault_borrow_token_account.to_account_info();
        base_accounts.token_reserve = self.borrow_token_reserves_liquidity.to_account_info();
        base_accounts.rate_model = self.borrow_rate_model.to_account_info();
        base_accounts.mint = self.borrow_token.to_account_info();
        base_accounts.token_program = self.borrow_token_program.to_account_info();

        base_accounts
    }

    // @dev In borrow, the token in context is borrow token, so we need to pass borrow token accounts
    // Since in borrow there is no withdraw, we pass default withdraw to account
    // and respective borrow at vault account
    pub fn get_borrow_accounts(&self, is_claim_type: bool) -> Result<OperateCpiAccounts<'info>> {
        let mut base_accounts = self.get_payback_accounts();

        // Destination account for borrow token, here recipient account
        base_accounts.borrow_to_account = if self.is_recipient_borrow_account() {
            Some(
                self.recipient_borrow_token_account
                    .as_ref()
                    .ok_or(ErrorCodes::VaultRecipientBorrowAccountRequired)?
                    .to_account_info(),
            )
        } else {
            // If recipient is not provided, we default to signer account as recipient
            Some(self.signer_borrow_token_account.to_account_info())
        };

        if is_claim_type {
            let claim_account = self
                .borrow_token_claim_account
                .as_ref()
                .ok_or(ErrorCodes::VaultClaimAccountRequired)?;

            base_accounts.borrow_claim_account = Some(claim_account.to_account_info());
        }

        Ok(base_accounts)
    }
}

#[derive(Accounts)]
pub struct Liquidate<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    #[account(
        mut,
        associated_token::mint = borrow_token, // payback of borrow token
        associated_token::authority = signer, // Since payback of token is happening from signer account
        associated_token::token_program = borrow_token_program
    )]
    pub signer_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account()]
    /// CHECK: This account is used as a destination for funds and is verified in the instruction logic
    pub to: AccountInfo<'info>,

    #[account(
        init_if_needed,
        payer = signer,
        associated_token::mint = supply_token, // withdraw of supply token
        associated_token::authority = to, // Since withdraw of token is happening at to account
        associated_token::token_program = supply_token_program
    )]
    pub to_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    /// @dev mut because this PDA signs the CPI to liquidity program
    /// @dev verification inside instruction logic
    pub vault_config: AccountLoader<'info, VaultConfig>,
    #[account(mut)]
    pub vault_state: AccountLoader<'info, VaultState>,

    pub supply_token: Box<InterfaceAccount<'info, Mint>>,
    pub borrow_token: Box<InterfaceAccount<'info, Mint>>,

    // This is oracle PDA account for Fluid Oracle, for hop 0 like WETH/USD
    #[account()]
    /// CHECK: This account is used as a first oracle account and is verified in the instruction logic
    pub oracle: Box<Account<'info, Oracle>>,

    // @dev loading other branches in the remaining_accounts to save the stack usage
    // @notice this is the branch for the new tick, it should be created if it doesn't exits before liquidate call
    #[account(mut)]
    pub new_branch: AccountLoader<'info, Branch>,

    // Here we will list all liquidity accounts, needed for CPI call to liquidity program
    // @dev no verification of accounts here, as they are already handled in liquidity program,
    // Hence loading them as UncheckedAccount
    // No need to verify liquidity program key here, as it will be verified in liquidity program CPI call
    #[account(mut)]
    pub supply_token_reserves_liquidity: AccountLoader<'info, TokenReserve>,
    #[account(mut)]
    pub borrow_token_reserves_liquidity: AccountLoader<'info, TokenReserve>,

    // @dev This is current vault UserSupplyPosition and UserBorrowPosition PDA on liquidity program
    #[account(mut)]
    /// CHECK: Safe as this will be verified in liquidity program CPI call
    pub vault_supply_position_on_liquidity: AccountLoader<'info, UserSupplyPosition>,
    #[account(mut)]
    /// CHECK: Safe as this will be verified in liquidity program CPI call
    pub vault_borrow_position_on_liquidity: AccountLoader<'info, UserBorrowPosition>,

    /// CHECK: Safe as this will be verified in liquidity program CPI call
    pub supply_rate_model: UncheckedAccount<'info>,
    /// CHECK: Safe as this will be verified in liquidity program CPI call
    pub borrow_rate_model: UncheckedAccount<'info>,

    /// CHECK: Safe as this will be verified in liquidity program CPI call
    #[account(mut)]
    pub supply_token_claim_account: Option<UncheckedAccount<'info>>,

    /// CHECK: Safe as this will be verified in liquidity program CPI call
    pub liquidity: UncheckedAccount<'info>,

    /// CHECK: Safe as we are checking liquidity program key in instruction logic
    pub liquidity_program: UncheckedAccount<'info>,

    #[account(mut)]
    /// CHECK: Safe as this will be verified in liquidity program CPI call
    pub vault_supply_token_account: UncheckedAccount<'info>,
    #[account(mut)]
    /// CHECK: Safe as this will be verified in liquidity program CPI call
    pub vault_borrow_token_account: UncheckedAccount<'info>,

    // These are system program and can not be mutated
    pub supply_token_program: Interface<'info, TokenInterface>,
    pub borrow_token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
    pub associated_token_program: Program<'info, AssociatedToken>,

    /// CHECK: Safe as we are checking oracle program key in instruction logic
    pub oracle_program: AccountInfo<'info>,
    // remaining_accounts[0..remaining_accounts_indices[0]] are oracle sources
    // remaining_accounts[remaining_accounts_indices[0]..remaining_accounts_indices[1]] are branch accounts
    // remaining_accounts[(remaining_accounts_indices[0] + remaining_accounts_indices[1])..remaining_accounts_indices[2]] are tick data accounts
    // remaining_accounts[(remaining_accounts_indices[0] + remaining_accounts_indices[1] + remaining_accounts_indices[2])..remaining_accounts_indices[3]] are tick_has_debt_array accounts
}

impl<'info> Liquidate<'info> {
    // Default base accounts are for withdraw operations
    fn get_base_accounts(&self) -> OperateCpiAccounts<'info> {
        OperateCpiAccounts {
            liquidity_program: self.liquidity_program.to_account_info(), // Liquidity program is calling contract in CPI
            protocol: self.vault_config.to_account_info(), // Since vault config is the PDA that signs the CPI
            liquidity: self.liquidity.to_account_info(), // Liquidity PDA account, which has auth to transfer funds from liquidity
            token_reserve: self.supply_token_reserves_liquidity.to_account_info(), // Supply token reserve account

            vault: self.vault_supply_token_account.to_account_info(), // Vault supply token account

            rate_model: self.supply_rate_model.to_account_info(), // Supply rate model

            user_supply_position: Some(self.vault_supply_position_on_liquidity.to_account_info()), // Vault supply position on liquidity
            user_borrow_position: Some(self.vault_borrow_position_on_liquidity.to_account_info()), // Vault borrow position on liquidity

            withdraw_to_account: Some(self.to_token_account.to_account_info()), // Destination account for supply token, here recipient account
            borrow_to_account: None, // No borrow, so no borrow to account

            borrow_claim_account: None,
            withdraw_claim_account: None,
            mint: self.supply_token.to_account_info(),

            token_program: self.supply_token_program.to_account_info(), // Token program
            associated_token_program: self.associated_token_program.to_account_info(), // Associated token program
        }
    }

    // @dev In withdraw, the token in context is supply token, so we need to pass supply token accounts
    // Since in withdraw there is no borrow, we pass default borrow to account
    // and respective withdraw at recipient account
    pub fn get_withdraw_accounts(&self, is_claim_type: bool) -> Result<OperateCpiAccounts<'info>> {
        let mut base_accounts = self.get_base_accounts();

        if is_claim_type {
            let claim_account = self
                .supply_token_claim_account
                .as_ref()
                .ok_or(ErrorCodes::VaultClaimAccountRequired)?;

            base_accounts.withdraw_claim_account = Some(claim_account.to_account_info());
        }

        Ok(base_accounts)
    }

    // @dev In payback, the token in context is borrow token, so we need to pass borrow token accounts
    // Since in payback there is no withdraw or borrow, we pass default withdraw and borrow to accounts
    pub fn get_payback_accounts(&self) -> OperateCpiAccounts<'info> {
        let mut base_accounts = self.get_base_accounts();

        base_accounts.vault = self.vault_borrow_token_account.to_account_info();

        // @dev As this is payback instruction, we are not withdrawing or borrowing or doing any claim
        // passing these as None
        base_accounts.withdraw_to_account = None;

        base_accounts.token_reserve = self.borrow_token_reserves_liquidity.to_account_info();
        base_accounts.rate_model = self.borrow_rate_model.to_account_info();
        base_accounts.mint = self.borrow_token.to_account_info();
        base_accounts.token_program = self.borrow_token_program.to_account_info();

        base_accounts
    }
}

#[derive(Accounts)]
pub struct Rebalance<'info> {
    #[account(mut)]
    pub rebalancer: Signer<'info>,

    // Rebalancer supply token account
    #[account(
        init_if_needed,
        payer = rebalancer,
        associated_token::mint = supply_token,
        associated_token::authority = rebalancer,
        associated_token::token_program = supply_token_program
    )]
    pub rebalancer_supply_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    // Rebalancer borrow token account
    #[account(
        init_if_needed,
        payer = rebalancer,
        associated_token::mint = borrow_token,
        associated_token::authority = rebalancer,
        associated_token::token_program = borrow_token_program
    )]
    pub rebalancer_borrow_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        has_one = supply_token @ ErrorCodes::VaultInvalidSupplyMint,
        has_one = borrow_token @ ErrorCodes::VaultInvalidBorrowMint,
        has_one = rebalancer @ ErrorCodes::VaultNotRebalancer
    )]
    /// @dev mut because this PDA signs the CPI to liquidity program
    /// @dev verification inside instruction logic
    pub vault_config: AccountLoader<'info, VaultConfig>,
    #[account(mut)]
    /// @dev verification inside instruction logic
    pub vault_state: AccountLoader<'info, VaultState>,

    // Vault supply and borrow token mint
    pub supply_token: Box<InterfaceAccount<'info, Mint>>,
    pub borrow_token: Box<InterfaceAccount<'info, Mint>>,

    // Here we will list all liquidity accounts, needed for CPI call to liquidity program
    // @dev no verification of accounts here, as they are already handled in liquidity program,
    // Hence loading them as UncheckedAccount
    // No need to verify liquidity program key here, as it will be verified in liquidity program CPI call
    #[account(mut)]
    pub supply_token_reserves_liquidity: AccountLoader<'info, TokenReserve>,
    #[account(mut)]
    pub borrow_token_reserves_liquidity: AccountLoader<'info, TokenReserve>,

    // @dev This is current vault UserSupplyPosition and UserBorrowPosition PDA on liquidity program
    #[account(mut)]
    /// CHECK: Safe as this will be verified in liquidity program CPI call
    pub vault_supply_position_on_liquidity: AccountLoader<'info, UserSupplyPosition>,
    #[account(mut)]
    /// CHECK: Safe as this will be verified in liquidity program CPI call
    pub vault_borrow_position_on_liquidity: AccountLoader<'info, UserBorrowPosition>,

    /// CHECK: Safe as this will be verified in liquidity program CPI call
    pub supply_rate_model: UncheckedAccount<'info>,
    /// CHECK: Safe as this will be verified in liquidity program CPI call
    pub borrow_rate_model: UncheckedAccount<'info>,

    /// CHECK: Safe as this will be verified in liquidity program CPI call
    pub liquidity: UncheckedAccount<'info>,

    /// CHECK: Safe as we are checking liquidity program key in instruction logic
    pub liquidity_program: UncheckedAccount<'info>,

    #[account(mut)]
    /// CHECK: Safe as this will be verified in liquidity program CPI call
    pub vault_supply_token_account: UncheckedAccount<'info>,
    #[account(mut)]
    /// CHECK: Safe as this will be verified in liquidity program CPI call
    pub vault_borrow_token_account: UncheckedAccount<'info>,

    // These are system program and can not be mutated
    pub system_program: Program<'info, System>,
    pub supply_token_program: Interface<'info, TokenInterface>,
    pub borrow_token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

impl<'info> Rebalance<'info> {
    pub fn get_base_accounts(&self) -> OperateCpiAccounts<'info> {
        OperateCpiAccounts {
            liquidity_program: self.liquidity_program.to_account_info(), // Liquidity program is calling contract in CPI
            protocol: self.vault_config.to_account_info(), // Since vault config is the PDA that signs the CPI
            liquidity: self.liquidity.to_account_info(), // Liquidity PDA account, which has auth to transfer funds from liquidity
            token_reserve: self.supply_token_reserves_liquidity.to_account_info(), // Supply token reserve account

            vault: self.vault_supply_token_account.to_account_info(), // Vault supply token account

            rate_model: self.supply_rate_model.to_account_info(), // Supply rate model

            user_supply_position: Some(self.vault_supply_position_on_liquidity.to_account_info()), // Vault supply position on liquidity
            user_borrow_position: Some(self.vault_borrow_position_on_liquidity.to_account_info()), // Vault borrow position on liquidity

            // @dev As this is rebalance deposit instruction, we are not withdrawing or borrowing or doing any claim
            // passing these as None
            withdraw_to_account: None, // No withdraw, so no withdraw to account
            borrow_to_account: None,   // No borrow, so no borrow to account

            borrow_claim_account: None,
            withdraw_claim_account: None,
            mint: self.supply_token.to_account_info(),

            token_program: self.supply_token_program.to_account_info(), // Token program
            associated_token_program: self.associated_token_program.to_account_info(), // Associated token program
        }
    }

    // @dev In deposit, the token in context is supply token, so we need to pass supply token accounts
    // Since in deposit there is no withdraw or borrow, we pass default withdraw and borrow to accounts
    pub fn get_deposit_accounts(&self) -> OperateCpiAccounts<'info> {
        self.get_base_accounts()
    }

    // @dev In payback, the token in context is borrow token, so we need to pass borrow token accounts
    // Since in payback there is no withdraw or borrow, we pass default withdraw and borrow to accounts
    pub fn get_payback_accounts(&self) -> OperateCpiAccounts<'info> {
        let mut base_accounts = self.get_base_accounts();

        base_accounts.token_reserve = self.borrow_token_reserves_liquidity.to_account_info();
        base_accounts.vault = self.vault_borrow_token_account.to_account_info();
        base_accounts.rate_model = self.borrow_rate_model.to_account_info();
        base_accounts.mint = self.borrow_token.to_account_info();
        base_accounts.token_program = self.borrow_token_program.to_account_info();

        base_accounts
    }

    // @dev In withdraw, the token in context is supply token, so we need to pass supply token accounts
    // Since in withdraw there is no borrow, we pass default borrow to account
    // and respective withdraw at recipient account
    pub fn get_withdraw_accounts(&self) -> OperateCpiAccounts<'info> {
        let mut base_accounts = self.get_base_accounts();

        base_accounts.withdraw_to_account =
            Some(self.rebalancer_supply_token_account.to_account_info());

        base_accounts
    }

    // @dev In borrow, the token in context is borrow token, so we need to pass borrow token accounts
    // Since in borrow there is no withdraw, we pass default withdraw to account
    // and respective borrow at vault account
    pub fn get_borrow_accounts(&self) -> OperateCpiAccounts<'info> {
        let mut base_accounts = self.get_payback_accounts();

        base_accounts.borrow_to_account =
            Some(self.rebalancer_borrow_token_account.to_account_info());

        base_accounts
    }
}

/***********************************|
|      View Function Contexts     |
|__________________________________*/

// @dev All account should be loaded as readyOnly

#[derive(Accounts)]
pub struct GetExchangePrices<'info> {
    pub vault_state: AccountLoader<'info, VaultState>,
    pub vault_config: AccountLoader<'info, VaultConfig>,
    pub supply_token_reserves: AccountLoader<'info, TokenReserve>,
    pub borrow_token_reserves: AccountLoader<'info, TokenReserve>,
}
