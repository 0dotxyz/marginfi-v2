use crate::constants::{FARMS_PROGRAM_ID, KAMINO_PROGRAM_ID};
use crate::state::bank::BankVaultType;
use crate::utils::is_kamino_asset_tag;
use crate::{bank_signer, optional_account, MarginfiError, MarginfiResult};
use anchor_lang::solana_program::sysvar;
use anchor_lang::{prelude::*, system_program};
use anchor_spl::token::Token;
use anchor_spl::token_interface::{
    transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked,
};
use kamino_mocks::kamino_lending::cpi::accounts::{
    DepositReserveLiquidityAndObligationCollateral,
    DepositReserveLiquidityAndObligationCollateralV2, InitObligation,
    InitObligationFarmsForReserve, InitUserMetadata, RefreshObligation, RefreshReserve,
    SocializeLossV2FarmsAccounts,
};
use kamino_mocks::kamino_lending::cpi::{
    deposit_reserve_liquidity_and_obligation_collateral_v2, init_obligation,
    init_obligation_farms_for_reserve, init_user_metadata, refresh_obligation, refresh_reserve,
};
use kamino_mocks::kamino_lending::types::InitObligationArgs;
use kamino_mocks::state::MinimalReserve;
use marginfi_type_crate::constants::LIQUIDITY_VAULT_AUTHORITY_SEED;
use marginfi_type_crate::types::Bank;

/// Initialize a Kamino obligation for a marginfi account
pub fn kamino_init_obligation(ctx: Context<KaminoInitObligation>, amount: u64) -> MarginfiResult {
    // Arbitrarily setting minimum deposit to 10 absolute units to always keep the obligation alive.
    // Obligations auto close when empty, but Kamino does not have any threshold that rounds down to
    // zero: even a single lamport suffices to keep a balance open.
    require_gte!(amount, 10, MarginfiError::ObligationInitDepositInsufficient);

    ctx.accounts.cpi_refresh_reserve()?;
    ctx.accounts.cpi_init_user_metadata()?;
    ctx.accounts.cpi_init_obligation()?;
    if ctx.accounts.reserve_farm_state.is_some() {
        ctx.accounts.cpi_init_farms()?
    }
    // Refresh obligation is needed before a deposit can be made
    ctx.accounts.cpi_refresh_obligation()?;
    // Transfer tokens from user (signer_token_account) -> obligation owner (liquidity vault)
    ctx.accounts.cpi_transfer_user_to_obligation_owner(amount)?;
    // Deposit into Kamino (liquidity vault) -> (reserve_liquidity_supply)
    ctx.accounts.cpi_kamino_deposit(amount)?;

    msg!("Kamino obligation initialized with amount: {}", amount);
    // Note: This ix is close to redline, any more msgs may exceed the standard CU limit.

    Ok(())
}

#[derive(Accounts)]
pub struct KaminoInitObligation<'info> {
    /// Pays to init the obligation and pays a nominal amount to ensure the obligation has a
    /// non-zero balance.
    #[account(mut)]
    pub fee_payer: Signer<'info>,

    #[account(
        has_one = liquidity_vault @ MarginfiError::InvalidLiquidityVault,
        has_one = integration_acc_1 @ MarginfiError::InvalidKaminoReserve,
        has_one = integration_acc_2 @ MarginfiError::InvalidKaminoObligation,
        has_one = mint @ MarginfiError::InvalidMint,
        constraint = is_kamino_asset_tag(bank.load()?.config.asset_tag)
            @ MarginfiError::WrongAssetTagForKaminoInstructions
    )]
    pub bank: AccountLoader<'info, Bank>,

    /// The fee payer must provide a nominal amount of bank tokens so the obligation is not empty.
    /// This amount is irrecoverable and and will prevent the obligation from ever being closed,
    /// even if the bank is otherwise empty (Kamino normally closes empty obligations automatically)
    #[account(
        mut,
        token::mint = mint,
        token::authority = fee_payer,
    )]
    pub signer_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    /// The liquidity vault authority (PDA that will own the Kamino obligation). Note: Kamino needs
    /// this to be mut because `deposit` might return the rent here
    #[account(
        mut,
        seeds = [
            LIQUIDITY_VAULT_AUTHORITY_SEED.as_bytes(),
            bank.key().as_ref()
        ],
        bump = bank.load()?.liquidity_vault_authority_bump
    )]
    pub liquidity_vault_authority: SystemAccount<'info>,

    /// Used as an intermediary to deposit a nominal amount of token into the obligation.
    #[account(mut)]
    pub liquidity_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// The obligation account to be created. Note that the key was already derived when
    /// initializing the bank, and this must match the obligation recorded at that time.
    #[account(
        mut,
        seeds = [
            &[0u8],
            &[0u8],
            liquidity_vault_authority.key().as_ref(),
            lending_market.key().as_ref(),
            system_program::ID.as_ref(),
            system_program::ID.as_ref()
        ],
        bump,
        seeds::program = KAMINO_PROGRAM_ID
    )]
    pub integration_acc_2: SystemAccount<'info>,

    /// CHECK: validated by the Kamino program
    #[account(mut)]
    pub user_metadata: SystemAccount<'info>,

    /// CHECK: validated by the Kamino program
    pub lending_market: UncheckedAccount<'info>,

    /// CHECK: validated by the Kamino program
    pub lending_market_authority: UncheckedAccount<'info>,

    /// CHECK: validated by the Kamino program
    #[account(mut)]
    pub integration_acc_1: AccountLoader<'info, MinimalReserve>,

    /// Bank's liquidity token mint (e.g., USDC). Kamino calls this the `reserve_liquidity_mint`
    #[account(mut)]
    pub mint: Box<InterfaceAccount<'info, Mint>>,

    /// CHECK: validated by the Kamino program
    #[account(mut)]
    pub reserve_liquidity_supply: Box<InterfaceAccount<'info, TokenAccount>>,

    /// The reserve's mint for tokenized representations of Kamino deposits.
    /// CHECK: validated by the Kamino program
    #[account(mut)]
    pub reserve_collateral_mint: UncheckedAccount<'info>,

    /// The reserve's destination for tokenized representations of deposits. Note: the
    /// `reserve_collateral_mint` will mint tokens directly to this account.
    /// CHECK: validated by the Kamino program
    #[account(mut)]
    pub reserve_destination_deposit_collateral: Box<InterfaceAccount<'info, TokenAccount>>,

    // Note: Only one of these four oracles accounts is required.
    /// CHECK: validated by the Kamino program
    pub pyth_oracle: Option<UncheckedAccount<'info>>,
    /// CHECK: validated by the Kamino program
    pub switchboard_price_oracle: Option<UncheckedAccount<'info>>,
    /// CHECK: validated by the Kamino program
    pub switchboard_twap_oracle: Option<UncheckedAccount<'info>>,
    /// CHECK: validated by the Kamino program
    pub scope_prices: Option<UncheckedAccount<'info>>,

    /// Required if the Kamino reserve has an active farm.
    /// CHECK: validated by the Kamino program
    #[account(mut)]
    pub obligation_farm_user_state: Option<UncheckedAccount<'info>>,

    /// Required if the Kamino reserve has an active farm.
    /// CHECK: validated by the Kamino program  
    #[account(mut)]
    pub reserve_farm_state: Option<UncheckedAccount<'info>>,

    /// CHECK: validated against hardcoded program id
    #[account(address = KAMINO_PROGRAM_ID)]
    pub kamino_program: UncheckedAccount<'info>,

    /// Farms program for Kamino staking functionality
    /// CHECK: validated against hardcoded program id
    #[account(address = FARMS_PROGRAM_ID)]
    pub farms_program: UncheckedAccount<'info>,

    /// Note: the collateral token always uses Token classic, never Token22.
    pub collateral_token_program: Program<'info, Token>,
    /// Note: Kamino does not have full Token22 support, certain Token22 features are disallowed.
    /// Expect this to update over time. Check with the Kamino source.
    pub liquidity_token_program: Interface<'info, TokenInterface>,

    /// CHECK: validated against hardcoded program id
    #[account(address = sysvar::instructions::ID)]
    pub instruction_sysvar_account: UncheckedAccount<'info>,

    pub rent: Sysvar<'info, Rent>,
    pub system_program: Program<'info, System>,
}

impl<'info> KaminoInitObligation<'info> {
    pub fn cpi_refresh_reserve(&self) -> MarginfiResult {
        let accounts = RefreshReserve {
            reserve: self.integration_acc_1.to_account_info(),
            lending_market: self.lending_market.to_account_info(),
            pyth_oracle: optional_account!(self.pyth_oracle),
            switchboard_price_oracle: optional_account!(self.switchboard_price_oracle),
            switchboard_twap_oracle: optional_account!(self.switchboard_twap_oracle),
            scope_prices: optional_account!(self.scope_prices),
        };
        let program = self.kamino_program.to_account_info();
        let cpi_ctx = CpiContext::new(program, accounts);
        refresh_reserve(cpi_ctx)?;
        Ok(())
    }

    pub fn cpi_init_user_metadata(&self) -> MarginfiResult {
        let accounts = InitUserMetadata {
            owner: self.liquidity_vault_authority.to_account_info(),
            fee_payer: self.fee_payer.to_account_info(),
            user_metadata: self.user_metadata.to_account_info(),
            referrer_user_metadata: None,
            rent: self.rent.to_account_info(),
            system_program: self.system_program.to_account_info(),
        };
        let program = self.kamino_program.to_account_info();
        let bump = self.bank.load()?.liquidity_vault_authority_bump;
        let signer_seeds: &[&[&[u8]]] =
            bank_signer!(BankVaultType::Liquidity, self.bank.key(), bump);
        let cpi_ctx = CpiContext::new_with_signer(program, accounts, signer_seeds);
        // This is a non account, we handle our own LUTs separately
        let user_lookup_table = Pubkey::default();
        init_user_metadata(cpi_ctx, user_lookup_table)?;
        Ok(())
    }

    pub fn cpi_init_obligation(&self) -> MarginfiResult {
        let accounts = InitObligation {
            fee_payer: self.fee_payer.to_account_info(),
            lending_market: self.lending_market.to_account_info(),
            obligation: self.integration_acc_2.to_account_info(),
            obligation_owner: self.liquidity_vault_authority.to_account_info(),
            owner_user_metadata: self.user_metadata.to_account_info(),
            rent: self.rent.to_account_info(),
            seed1_account: self.system_program.to_account_info(),
            seed2_account: self.system_program.to_account_info(),
            system_program: self.system_program.to_account_info(),
        };
        let program = self.kamino_program.to_account_info();
        let bump = self.bank.load()?.liquidity_vault_authority_bump;
        let signer_seeds: &[&[&[u8]]] =
            bank_signer!(BankVaultType::Liquidity, self.bank.key(), bump);
        let cpi_ctx = CpiContext::new_with_signer(program, accounts, signer_seeds);
        let args = InitObligationArgs { id: 0, tag: 0 };
        init_obligation(cpi_ctx, args)?;
        Ok(())
    }

    pub fn cpi_refresh_obligation(&self) -> MarginfiResult {
        let accounts = RefreshObligation {
            lending_market: self.lending_market.to_account_info(),
            obligation: self.integration_acc_2.to_account_info(),
        };
        let program = self.kamino_program.to_account_info();
        let cpi_ctx = CpiContext::new(program, accounts);
        refresh_obligation(cpi_ctx)?;
        Ok(())
    }

    pub fn cpi_transfer_user_to_obligation_owner(&self, amount: u64) -> MarginfiResult {
        let program = self.liquidity_token_program.to_account_info();
        let accounts = TransferChecked {
            from: self.signer_token_account.to_account_info(),
            to: self.liquidity_vault.to_account_info(),
            authority: self.fee_payer.to_account_info(),
            mint: self.mint.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(program, accounts);
        let decimals = self.mint.decimals;
        transfer_checked(cpi_ctx, amount, decimals)?;
        Ok(())
    }

    pub fn cpi_init_farms(&self) -> MarginfiResult {
        let accounts = InitObligationFarmsForReserve {
            payer: self.fee_payer.to_account_info(),
            owner: self.liquidity_vault_authority.to_account_info(),
            obligation: self.integration_acc_2.to_account_info(),
            lending_market_authority: self.lending_market_authority.to_account_info(),
            reserve: self.integration_acc_1.to_account_info(),
            reserve_farm_state: optional_account!(self.reserve_farm_state).unwrap(),
            obligation_farm: optional_account!(self.obligation_farm_user_state).unwrap(),
            lending_market: self.lending_market.to_account_info(),
            farms_program: self.farms_program.to_account_info(),
            rent: self.rent.to_account_info(),
            system_program: self.system_program.to_account_info(),
        };
        let program = self.kamino_program.to_account_info();
        let bump = self.bank.load()?.liquidity_vault_authority_bump;
        let signer_seeds: &[&[&[u8]]] =
            bank_signer!(BankVaultType::Liquidity, self.bank.key(), bump);
        let cpi_ctx = CpiContext::new_with_signer(program, accounts, signer_seeds);
        // mode 0 = collateral
        init_obligation_farms_for_reserve(cpi_ctx, 0)?;
        Ok(())
    }

    pub fn cpi_kamino_deposit(&self, amount: u64) -> MarginfiResult {
        let deposit_accounts = DepositReserveLiquidityAndObligationCollateral {
            owner: self.liquidity_vault_authority.to_account_info(),
            obligation: self.integration_acc_2.to_account_info(),
            lending_market: self.lending_market.to_account_info(),
            lending_market_authority: self.lending_market_authority.to_account_info(),
            reserve: self.integration_acc_1.to_account_info(),
            reserve_liquidity_mint: self.mint.to_account_info(),
            reserve_liquidity_supply: self.reserve_liquidity_supply.to_account_info(),
            reserve_collateral_mint: self.reserve_collateral_mint.to_account_info(),
            reserve_destination_deposit_collateral: self
                .reserve_destination_deposit_collateral
                .to_account_info(),
            user_source_liquidity: self.liquidity_vault.to_account_info(),
            placeholder_user_destination_collateral: None,
            collateral_token_program: self.collateral_token_program.to_account_info(),
            liquidity_token_program: self.liquidity_token_program.to_account_info(),
            instruction_sysvar_account: self.instruction_sysvar_account.to_account_info(),
        };

        // --- optional “farms_accounts” group ---
        let farms_accounts = SocializeLossV2FarmsAccounts {
            obligation_farm_user_state: optional_account!(self.obligation_farm_user_state),
            reserve_farm_state: optional_account!(self.reserve_farm_state),
        };

        // --- wrap both groups in the outer struct ---
        let accounts = DepositReserveLiquidityAndObligationCollateralV2 {
            deposit_reserve_liquidity_and_obligation_collateral_v2_deposit_accounts:
                deposit_accounts,
            deposit_reserve_liquidity_and_obligation_collateral_v2_farms_accounts: farms_accounts,
            farms_program: self.farms_program.to_account_info(),
        };
        let program = self.kamino_program.to_account_info();
        let bump = self.bank.load()?.liquidity_vault_authority_bump;
        let signer_seeds: &[&[&[u8]]] =
            bank_signer!(BankVaultType::Liquidity, self.bank.key(), bump);
        let cpi_ctx = CpiContext::new_with_signer(program, accounts, signer_seeds);
        deposit_reserve_liquidity_and_obligation_collateral_v2(cpi_ctx, amount)?;
        Ok(())
    }
}
