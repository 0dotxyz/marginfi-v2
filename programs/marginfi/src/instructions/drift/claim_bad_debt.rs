use crate::{
    bank_signer,
    events::{DriftClaimBadDebtEvent, GroupEventHeader},
    ix_utils::get_discrim_hash,
    state::bank::BankVaultType,
    utils::is_drift_asset_tag,
    MarginfiError, MarginfiResult,
};
use anchor_lang::{
    prelude::*,
    solana_program::{
        instruction::{AccountMeta, Instruction},
        program::invoke_signed,
    },
    system_program,
};
use anchor_spl::{
    associated_token::{
        create_idempotent, get_associated_token_address_with_program_id, AssociatedToken, Create,
    },
    token::{self, accessor, Mint, Token, TokenAccount, Transfer},
};
use marginfi_type_crate::{
    constants::{FEE_STATE_SEED, LIQUIDITY_VAULT_AUTHORITY_SEED},
    types::{Bank, FeeState},
};

pub const MERKLE_DISTRIBUTOR_PROGRAM_ID: Pubkey =
    pubkey!("AtXLVASdFhmdq2KZxzhVFonmNXL76dTTsEABXySEHgLh");
const CLAIM_STATUS_LEN: usize = 112;

#[derive(AnchorSerialize)]
struct NewClaimIxArgs {
    amount_unlocked: u64,
    amount_locked: u64,
    proof: Vec<[u8; 32]>,
}

/// Claim a Drift bad-debt portal allocation for a Drift bank.
///
/// The bad-debt portal's merkle leaf must use this bank's `liquidity_vault_authority` as the
/// claimant. The distributor requires claimed tokens to land in a token account owned by that
/// claimant, so this instruction claims into the authority's ATA, then transfers the full balance
/// to the global fee wallet's ATA.
pub fn drift_claim_bad_debt<'info>(
    ctx: Context<'info, DriftClaimBadDebt<'info>>,
    amount: u64,
    proof: Vec<[u8; 32]>,
) -> MarginfiResult {
    ctx.accounts.create_claimant_token_account()?;
    ctx.accounts.create_destination_token_account()?;
    ctx.accounts.prefund_claim_status()?;
    let pre_claim_balance = ctx.accounts.claimant_token_balance()?;
    ctx.accounts.cpi_new_claim(amount, proof)?;
    let post_claim_balance = ctx.accounts.claimant_token_balance()?;
    let received_amount = post_claim_balance
        .checked_sub(pre_claim_balance)
        .ok_or_else(|| error!(MarginfiError::InternalLogicError))?;
    let swept_amount = ctx.accounts.cpi_transfer_to_destination()?;
    ctx.accounts
        .emit_claim_event(amount, received_amount, swept_amount)?;
    Ok(())
}

#[derive(Accounts)]
pub struct DriftClaimBadDebt<'info> {
    /// Pays transaction fees, ATA creation, and ClaimStatus rent.
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        has_one = integration_acc_2 @ MarginfiError::InvalidDriftUser,
        has_one = integration_acc_3 @ MarginfiError::InvalidDriftUserStats,
        constraint = is_drift_asset_tag(bank.load()?.config.asset_tag)
            @ MarginfiError::WrongBankAssetTagForDriftOperation
    )]
    pub bank: AccountLoader<'info, Bank>,

    /// Global fee state containing the global_fee_wallet destination owner.
    #[account(
        seeds = [FEE_STATE_SEED.as_bytes()],
        bump
    )]
    pub fee_state: AccountLoader<'info, FeeState>,

    /// The bank's liquidity vault authority. This PDA is the claimant in Drift's merkle tree.
    #[account(
        mut,
        seeds = [
            LIQUIDITY_VAULT_AUTHORITY_SEED.as_bytes(),
            bank.key().as_ref()
        ],
        bump = bank.load()?.liquidity_vault_authority_bump
    )]
    pub liquidity_vault_authority: SystemAccount<'info>,

    /// Drift user account owned by liquidity_vault_authority.
    /// CHECK: Address is locked by the bank's integration account field.
    pub integration_acc_2: UncheckedAccount<'info>,

    /// Drift user stats account owned by liquidity_vault_authority.
    /// CHECK: Address is locked by the bank's integration account field.
    pub integration_acc_3: UncheckedAccount<'info>,

    /// CHECK: MerkleDistributor account. The distributor program validates its contents during CPI.
    #[account(mut, owner = MERKLE_DISTRIBUTOR_PROGRAM_ID)]
    pub distributor: UncheckedAccount<'info>,

    /// CHECK: PDA of ["ClaimStatus", liquidity_vault_authority, distributor] under the distributor
    /// program. The distributor initializes and validates this account.
    #[account(
        mut,
        seeds = [
            b"ClaimStatus",
            liquidity_vault_authority.key().as_ref(),
            distributor.key().as_ref()
        ],
        bump,
        seeds::program = MERKLE_DISTRIBUTOR_PROGRAM_ID
    )]
    pub claim_status: UncheckedAccount<'info>,

    /// Distributor token vault.
    #[account(mut, token::mint = claim_mint)]
    pub from: Box<Account<'info, TokenAccount>>,

    pub claim_mint: Box<Account<'info, Mint>>,

    /// CHECK: Must match FeeState.global_fee_wallet. Used as the owner for the destination ATA.
    #[account(address = fee_state.load()?.global_fee_wallet @ MarginfiError::InvalidFeeAta)]
    pub global_fee_wallet: UncheckedAccount<'info>,

    /// Canonical ATA for the claim mint owned by liquidity_vault_authority.
    /// CHECK: Created idempotently and validated by address.
    #[account(
        mut,
        address = get_associated_token_address_with_program_id(
            &liquidity_vault_authority.key(),
            &claim_mint.key(),
            &token_program.key()
        ) @ MarginfiError::InvalidDriftAccount
    )]
    pub claimant_token_account: UncheckedAccount<'info>,

    /// Canonical ATA for the claim mint owned by FeeState.global_fee_wallet.
    /// CHECK: Created idempotently and validated by address.
    #[account(
        mut,
        address = get_associated_token_address_with_program_id(
            &fee_state.load()?.global_fee_wallet,
            &claim_mint.key(),
            &token_program.key()
        ) @ MarginfiError::InvalidFeeAta
    )]
    pub destination_token_account: UncheckedAccount<'info>,

    /// CHECK: validated against the Drift merkle distributor program id.
    #[account(address = MERKLE_DISTRIBUTOR_PROGRAM_ID)]
    pub merkle_distributor_program: UncheckedAccount<'info>,

    pub associated_token_program: Program<'info, AssociatedToken>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

impl<'info> DriftClaimBadDebt<'info> {
    fn create_claimant_token_account(&self) -> MarginfiResult {
        let accounts = Create {
            payer: self.payer.to_account_info(),
            associated_token: self.claimant_token_account.to_account_info(),
            authority: self.liquidity_vault_authority.to_account_info(),
            mint: self.claim_mint.to_account_info(),
            system_program: self.system_program.to_account_info(),
            token_program: self.token_program.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(self.associated_token_program.key(), accounts);
        create_idempotent(cpi_ctx)?;
        Ok(())
    }

    fn create_destination_token_account(&self) -> MarginfiResult {
        let accounts = Create {
            payer: self.payer.to_account_info(),
            associated_token: self.destination_token_account.to_account_info(),
            authority: self.global_fee_wallet.to_account_info(),
            mint: self.claim_mint.to_account_info(),
            system_program: self.system_program.to_account_info(),
            token_program: self.token_program.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(self.associated_token_program.key(), accounts);
        create_idempotent(cpi_ctx)?;
        Ok(())
    }

    fn prefund_claim_status(&self) -> MarginfiResult {
        let required = Rent::get()?
            .minimum_balance(CLAIM_STATUS_LEN)
            .saturating_sub(self.claim_status.to_account_info().lamports());

        if required > 0 {
            let accounts = system_program::Transfer {
                from: self.payer.to_account_info(),
                to: self.claim_status.to_account_info(),
            };
            let cpi_ctx = CpiContext::new(self.system_program.key(), accounts);
            system_program::transfer(cpi_ctx, required)?;
        }

        Ok(())
    }

    fn cpi_new_claim(&self, amount: u64, proof: Vec<[u8; 32]>) -> MarginfiResult {
        let mut data = get_discrim_hash("global", "new_claim").to_vec();
        NewClaimIxArgs {
            amount_unlocked: amount,
            amount_locked: 0,
            proof,
        }
        .serialize(&mut data)?;

        let ix = Instruction {
            program_id: self.merkle_distributor_program.key(),
            accounts: vec![
                AccountMeta::new(self.distributor.key(), false),
                AccountMeta::new(self.claim_status.key(), false),
                AccountMeta::new(self.from.key(), false),
                AccountMeta::new(self.claimant_token_account.key(), false),
                AccountMeta::new(self.liquidity_vault_authority.key(), true),
                AccountMeta::new_readonly(self.token_program.key(), false),
                AccountMeta::new_readonly(self.system_program.key(), false),
            ],
            data,
        };

        let account_infos = [
            self.distributor.to_account_info(),
            self.claim_status.to_account_info(),
            self.from.to_account_info(),
            self.claimant_token_account.to_account_info(),
            self.liquidity_vault_authority.to_account_info(),
            self.token_program.to_account_info(),
            self.system_program.to_account_info(),
        ];

        let bump = self.bank.load()?.liquidity_vault_authority_bump;
        let signer_seeds: &[&[&[u8]]] =
            bank_signer!(BankVaultType::Liquidity, self.bank.key(), bump);

        invoke_signed(&ix, &account_infos, signer_seeds)?;
        Ok(())
    }

    fn cpi_transfer_to_destination(&self) -> MarginfiResult<u64> {
        let amount = accessor::amount(&self.claimant_token_account.to_account_info())?;
        if amount == 0 {
            return Ok(0);
        }

        let accounts = Transfer {
            from: self.claimant_token_account.to_account_info(),
            to: self.destination_token_account.to_account_info(),
            authority: self.liquidity_vault_authority.to_account_info(),
        };
        let bump = self.bank.load()?.liquidity_vault_authority_bump;
        let signer_seeds: &[&[&[u8]]] =
            bank_signer!(BankVaultType::Liquidity, self.bank.key(), bump);
        let cpi_ctx = CpiContext::new_with_signer(self.token_program.key(), accounts, signer_seeds);

        token::transfer(cpi_ctx, amount)?;
        Ok(amount)
    }

    fn claimant_token_balance(&self) -> MarginfiResult<u64> {
        accessor::amount(&self.claimant_token_account.to_account_info())
    }

    fn emit_claim_event(
        &self,
        requested_amount: u64,
        received_amount: u64,
        swept_amount: u64,
    ) -> MarginfiResult {
        let bank = self.bank.load()?;

        emit!(DriftClaimBadDebtEvent {
            header: GroupEventHeader {
                signer: Some(self.payer.key()),
                marginfi_group: bank.group,
            },
            bank: self.bank.key(),
            claim_mint: self.claim_mint.key(),
            distributor: self.distributor.key(),
            claim_status: self.claim_status.key(),
            liquidity_vault_authority: self.liquidity_vault_authority.key(),
            global_fee_wallet: self.global_fee_wallet.key(),
            requested_amount,
            received_amount,
            swept_amount,
        });

        Ok(())
    }
}
