//! Instruction builders generated from the marginfi IDL.
#![allow(clippy::doc_lazy_continuation)]

use super::ToAccountMetas;
use crate::types::DriftConfigCompact;
use borsh::BorshSerialize;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

/// Fixed accounts for [`drift_claim_bad_debt`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DriftClaimBadDebt {
    /// Pays transaction fees, ATA creation, and ClaimStatus rent.
    pub payer: Pubkey,
    pub bank: Pubkey,
    /// Global fee state containing the global_fee_wallet destination owner.
    pub fee_state: Pubkey,
    /// The bank's liquidity vault authority. This PDA is the claimant in Drift's merkle tree.
    pub liquidity_vault_authority: Pubkey,
    /// Drift user account owned by liquidity_vault_authority.
    pub integration_acc_2: Pubkey,
    /// Drift user stats account owned by liquidity_vault_authority.
    pub integration_acc_3: Pubkey,
    pub distributor: Pubkey,
    /// program. The distributor initializes and validates this account.
    pub claim_status: Pubkey,
    /// Distributor token vault.
    pub from: Pubkey,
    pub claim_mint: Pubkey,
    pub global_fee_wallet: Pubkey,
    /// Canonical ATA for the claim mint owned by liquidity_vault_authority.
    pub claimant_token_account: Pubkey,
    /// Canonical ATA for the claim mint owned by FeeState.global_fee_wallet.
    pub destination_token_account: Pubkey,
    pub merkle_distributor_program: Pubkey,
    pub associated_token_program: Pubkey,
    pub token_program: Pubkey,
    pub system_program: Pubkey,
}
impl DriftClaimBadDebt {
    pub const DISCRIMINATOR: [u8; 8] = [163, 67, 144, 231, 119, 20, 220, 33];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.payer, true),
            AccountMeta::new_readonly(self.bank, false),
            AccountMeta::new_readonly(self.fee_state, false),
            AccountMeta::new(self.liquidity_vault_authority, false),
            AccountMeta::new_readonly(self.integration_acc_2, false),
            AccountMeta::new_readonly(self.integration_acc_3, false),
            AccountMeta::new(self.distributor, false),
            AccountMeta::new(self.claim_status, false),
            AccountMeta::new(self.from, false),
            AccountMeta::new_readonly(self.claim_mint, false),
            AccountMeta::new_readonly(self.global_fee_wallet, false),
            AccountMeta::new(self.claimant_token_account, false),
            AccountMeta::new(self.destination_token_account, false),
            AccountMeta::new_readonly(self.merkle_distributor_program, false),
            AccountMeta::new_readonly(self.associated_token_program, false),
            AccountMeta::new_readonly(self.token_program, false),
            AccountMeta::new_readonly(self.system_program, false),
        ]
    }
}
impl ToAccountMetas for DriftClaimBadDebt {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (permissionless) Claim a Drift bad-debt portal allocation for a Drift bank.
/// The merkle claimant is the bank's liquidity_vault_authority PDA, and claimed tokens are
/// swept to the global fee wallet's canonical ATA.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn drift_claim_bad_debt(
    accounts: &DriftClaimBadDebt,
    amount: u64,
    proof: Vec<[u8; 32]>,
) -> Instruction {
    drift_claim_bad_debt_with_program_id(crate::ID, accounts, amount, proof)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn drift_claim_bad_debt_with_program_id(
    program_id: Pubkey,
    accounts: &DriftClaimBadDebt,
    amount: u64,
    proof: Vec<[u8; 32]>,
) -> Instruction {
    let mut data = DriftClaimBadDebt::DISCRIMINATOR.to_vec();
    amount
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    proof
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`drift_deposit`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DriftDeposit {
    pub group: Pubkey,
    pub marginfi_account: Pubkey,
    pub authority: Pubkey,
    pub bank: Pubkey,
    /// The oracle account for the asset (not needed if using oracle type QuoteAsset)
    pub drift_oracle: Option<Pubkey>,
    /// The bank's liquidity vault authority, which owns the Drift user account
    pub liquidity_vault_authority: Pubkey,
    /// Used as an intermediary to deposit tokens into Drift
    pub liquidity_vault: Pubkey,
    /// Owned by authority, the source account for the token deposit
    pub signer_token_account: Pubkey,
    /// The Drift state account
    pub drift_state: Pubkey,
    /// The Drift user account owned by liquidity_vault_authority
    pub integration_acc_2: Pubkey,
    /// The Drift user stats account owned by liquidity_vault_authority
    pub integration_acc_3: Pubkey,
    /// The Drift spot market for this asset
    pub integration_acc_1: Pubkey,
    /// The Drift spot market vault that will receive tokens
    pub drift_spot_market_vault: Pubkey,
    /// Bank's liquidity token mint
    pub mint: Pubkey,
    pub drift_program: Pubkey,
    pub token_program: Pubkey,
    pub system_program: Pubkey,
}
impl DriftDeposit {
    pub const DISCRIMINATOR: [u8; 8] = [252, 63, 250, 201, 98, 55, 130, 12];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new(self.marginfi_account, false),
            AccountMeta::new_readonly(self.authority, true),
            AccountMeta::new(self.bank, false),
            match self.drift_oracle {
                Some(key) => AccountMeta::new_readonly(key, false),
                None => AccountMeta::new_readonly(program_id, false),
            },
            AccountMeta::new_readonly(self.liquidity_vault_authority, false),
            AccountMeta::new(self.liquidity_vault, false),
            AccountMeta::new(self.signer_token_account, false),
            AccountMeta::new_readonly(self.drift_state, false),
            AccountMeta::new(self.integration_acc_2, false),
            AccountMeta::new(self.integration_acc_3, false),
            AccountMeta::new(self.integration_acc_1, false),
            AccountMeta::new(self.drift_spot_market_vault, false),
            AccountMeta::new_readonly(self.mint, false),
            AccountMeta::new_readonly(self.drift_program, false),
            AccountMeta::new_readonly(self.token_program, false),
            AccountMeta::new_readonly(self.system_program, false),
        ]
    }
}
impl ToAccountMetas for DriftDeposit {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (user) Deposit into a Drift spot market through a marginfi account
/// * amount - in the underlying token (e.g., USDC), in native decimals
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn drift_deposit(accounts: &DriftDeposit, amount: u64) -> Instruction {
    drift_deposit_with_program_id(crate::ID, accounts, amount)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn drift_deposit_with_program_id(
    program_id: Pubkey,
    accounts: &DriftDeposit,
    amount: u64,
) -> Instruction {
    let mut data = DriftDeposit::DISCRIMINATOR.to_vec();
    amount
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`drift_harvest_reward`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DriftHarvestReward {
    pub bank: Pubkey,
    /// Global fee state that contains the global_fee_wallet
    pub fee_state: Pubkey,
    /// The bank's liquidity vault authority
    pub liquidity_vault_authority: Pubkey,
    /// To create this manually just send some of the reward token
    /// to the liquidity vault authority address before claiming
    pub intermediary_token_account: Pubkey,
    /// Destination token account must be owned by the global fee wallet
    pub destination_token_account: Pubkey,
    /// Drift accounts
    pub drift_state: Pubkey,
    pub integration_acc_2: Pubkey,
    pub integration_acc_3: Pubkey,
    /// The harvest spot market - MUST be different from bank's Drift spot market (integration_acc_1)
    /// This is the market that contains admin deposits to harvest
    pub harvest_drift_spot_market: Pubkey,
    /// The harvest spot market vault - derived from harvest_drift_spot_market
    pub harvest_drift_spot_market_vault: Pubkey,
    /// The Drift signer PDA
    pub drift_signer: Pubkey,
    pub reward_mint: Pubkey,
    pub drift_program: Pubkey,
    pub token_program: Pubkey,
}
impl DriftHarvestReward {
    pub const DISCRIMINATOR: [u8; 8] = [167, 161, 240, 194, 138, 54, 87, 189];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.bank, false),
            AccountMeta::new_readonly(self.fee_state, false),
            AccountMeta::new_readonly(self.liquidity_vault_authority, false),
            AccountMeta::new(self.intermediary_token_account, false),
            AccountMeta::new(self.destination_token_account, false),
            AccountMeta::new_readonly(self.drift_state, false),
            AccountMeta::new(self.integration_acc_2, false),
            AccountMeta::new(self.integration_acc_3, false),
            AccountMeta::new(self.harvest_drift_spot_market, false),
            AccountMeta::new(self.harvest_drift_spot_market_vault, false),
            AccountMeta::new_readonly(self.drift_signer, false),
            AccountMeta::new_readonly(self.reward_mint, false),
            AccountMeta::new_readonly(self.drift_program, false),
            AccountMeta::new_readonly(self.token_program, false),
        ]
    }
}
impl ToAccountMetas for DriftHarvestReward {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (permissionless) Harvest rewards from admin deposits in Drift spot markets.
/// Rewards are always sent to the global fee wallet's canonical ATA.
/// The harvest spot market must be different from the bank's main drift spot market.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn drift_harvest_reward(accounts: &DriftHarvestReward) -> Instruction {
    drift_harvest_reward_with_program_id(crate::ID, accounts)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn drift_harvest_reward_with_program_id(
    program_id: Pubkey,
    accounts: &DriftHarvestReward,
) -> Instruction {
    let data = DriftHarvestReward::DISCRIMINATOR.to_vec();
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`drift_init_user`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DriftInitUser {
    /// Pays to init the drift user and user stats accounts and provides initial deposit
    pub fee_payer: Pubkey,
    /// The fee payer must provide a nominal amount of bank tokens so the account is not empty.
    /// This amount is irrecoverable and will prevent the account from being closed.
    pub signer_token_account: Pubkey,
    pub bank: Pubkey,
    /// The liquidity vault authority (PDA that will own the Drift user)
    pub liquidity_vault_authority: Pubkey,
    /// Used as an intermediary to deposit a nominal amount of token into Drift.
    pub liquidity_vault: Pubkey,
    /// Bank's liquidity token mint (e.g., USDC)
    pub mint: Pubkey,
    /// The user stats account to be created
    pub integration_acc_3: Pubkey,
    /// The user account to be created (sub_account_id = 0)
    pub integration_acc_2: Pubkey,
    pub drift_state: Pubkey,
    pub integration_acc_1: Pubkey,
    /// The Drift spot market vault where tokens will be deposited
    pub drift_spot_market_vault: Pubkey,
    /// Oracle for the asset (can be null for USDC/market 0)
    pub drift_oracle: Option<Pubkey>,
    pub drift_program: Pubkey,
    pub token_program: Pubkey,
    pub rent: Pubkey,
    pub system_program: Pubkey,
}
impl DriftInitUser {
    pub const DISCRIMINATOR: [u8; 8] = [29, 18, 236, 190, 29, 254, 114, 169];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.fee_payer, true),
            AccountMeta::new(self.signer_token_account, false),
            AccountMeta::new_readonly(self.bank, false),
            AccountMeta::new_readonly(self.liquidity_vault_authority, false),
            AccountMeta::new(self.liquidity_vault, false),
            AccountMeta::new(self.mint, false),
            AccountMeta::new(self.integration_acc_3, false),
            AccountMeta::new(self.integration_acc_2, false),
            AccountMeta::new(self.drift_state, false),
            AccountMeta::new(self.integration_acc_1, false),
            AccountMeta::new(self.drift_spot_market_vault, false),
            match self.drift_oracle {
                Some(key) => AccountMeta::new_readonly(key, false),
                None => AccountMeta::new_readonly(program_id, false),
            },
            AccountMeta::new_readonly(self.drift_program, false),
            AccountMeta::new_readonly(self.token_program, false),
            AccountMeta::new_readonly(self.rent, false),
            AccountMeta::new_readonly(self.system_program, false),
        ]
    }
}
impl ToAccountMetas for DriftInitUser {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (permissionless) Initialize a Drift user and user stats for a marginfi bank
/// Creates user with sub_account_id = 0 and empty name
/// Requires a minimum deposit to ensure the account remains active
/// * amount - minimum deposit amount (at least 10 units) in native decimals
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn drift_init_user(accounts: &DriftInitUser, amount: u64) -> Instruction {
    drift_init_user_with_program_id(crate::ID, accounts, amount)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn drift_init_user_with_program_id(
    program_id: Pubkey,
    accounts: &DriftInitUser,
    amount: u64,
) -> Instruction {
    let mut data = DriftInitUser::DISCRIMINATOR.to_vec();
    amount
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`drift_withdraw`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DriftWithdraw {
    pub group: Pubkey,
    pub marginfi_account: Pubkey,
    pub authority: Pubkey,
    pub bank: Pubkey,
    /// The oracle account for the asset (not needed if using oracle type QuoteAsset)
    pub drift_oracle: Option<Pubkey>,
    /// The bank's liquidity vault authority, which owns the Drift user account
    pub liquidity_vault_authority: Pubkey,
    /// Receives tokens from Drift withdrawal
    pub liquidity_vault: Pubkey,
    /// Token account that will receive the withdrawn tokens
    pub destination_token_account: Pubkey,
    /// The Drift state account
    pub drift_state: Pubkey,
    /// The Drift user account owned by liquidity_vault_authority
    pub integration_acc_2: Pubkey,
    /// The Drift user stats account owned by liquidity_vault_authority
    pub integration_acc_3: Pubkey,
    /// The Drift spot market for this asset
    pub integration_acc_1: Pubkey,
    /// The Drift spot market vault that holds tokens
    pub drift_spot_market_vault: Pubkey,
    /// Optional: Oracle for first reward asset (only needed if rewards exist)
    pub drift_reward_oracle: Option<Pubkey>,
    /// Optional: Spot market for first reward asset (only needed if rewards exist)
    pub drift_reward_spot_market: Option<Pubkey>,
    /// Optional: Mint for first reward asset (only needed if rewards exist)
    pub drift_reward_mint: Option<Pubkey>,
    /// Optional: Oracle for second reward asset (backup in case multiple rewards)
    pub drift_reward_oracle_2: Option<Pubkey>,
    /// Optional: Spot market for second reward asset (backup in case multiple rewards)
    pub drift_reward_spot_market_2: Option<Pubkey>,
    /// Optional: Mint for second reward asset (backup in case multiple rewards)
    pub drift_reward_mint_2: Option<Pubkey>,
    /// The Drift signer PDA
    pub drift_signer: Pubkey,
    /// Bank's liquidity token mint
    pub mint: Pubkey,
    pub drift_program: Pubkey,
    pub token_program: Pubkey,
    pub system_program: Pubkey,
}
impl DriftWithdraw {
    pub const DISCRIMINATOR: [u8; 8] = [86, 59, 186, 123, 183, 181, 234, 137];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new(self.marginfi_account, false),
            AccountMeta::new_readonly(self.authority, true),
            AccountMeta::new(self.bank, false),
            match self.drift_oracle {
                Some(key) => AccountMeta::new_readonly(key, false),
                None => AccountMeta::new_readonly(program_id, false),
            },
            AccountMeta::new_readonly(self.liquidity_vault_authority, false),
            AccountMeta::new(self.liquidity_vault, false),
            AccountMeta::new(self.destination_token_account, false),
            AccountMeta::new_readonly(self.drift_state, false),
            AccountMeta::new(self.integration_acc_2, false),
            AccountMeta::new(self.integration_acc_3, false),
            AccountMeta::new(self.integration_acc_1, false),
            AccountMeta::new(self.drift_spot_market_vault, false),
            match self.drift_reward_oracle {
                Some(key) => AccountMeta::new_readonly(key, false),
                None => AccountMeta::new_readonly(program_id, false),
            },
            match self.drift_reward_spot_market {
                Some(key) => AccountMeta::new_readonly(key, false),
                None => AccountMeta::new_readonly(program_id, false),
            },
            match self.drift_reward_mint {
                Some(key) => AccountMeta::new_readonly(key, false),
                None => AccountMeta::new_readonly(program_id, false),
            },
            match self.drift_reward_oracle_2 {
                Some(key) => AccountMeta::new_readonly(key, false),
                None => AccountMeta::new_readonly(program_id, false),
            },
            match self.drift_reward_spot_market_2 {
                Some(key) => AccountMeta::new_readonly(key, false),
                None => AccountMeta::new_readonly(program_id, false),
            },
            match self.drift_reward_mint_2 {
                Some(key) => AccountMeta::new_readonly(key, false),
                None => AccountMeta::new_readonly(program_id, false),
            },
            AccountMeta::new_readonly(self.drift_signer, false),
            AccountMeta::new_readonly(self.mint, false),
            AccountMeta::new_readonly(self.drift_program, false),
            AccountMeta::new_readonly(self.token_program, false),
            AccountMeta::new_readonly(self.system_program, false),
        ]
    }
}
impl ToAccountMetas for DriftWithdraw {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (user) Withdraw from a Drift spot market through a marginfi account
/// * amount - in the underlying token (e.g., USDC), in native decimals
/// * if group rate limits are enabled, include the withdrawn bank's oracle group in
/// `remaining_accounts`
/// * withdraw_all - if true, withdraws entire position
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn drift_withdraw(
    accounts: &DriftWithdraw,
    amount: u64,
    withdraw_all: Option<bool>,
) -> Instruction {
    drift_withdraw_with_program_id(crate::ID, accounts, amount, withdraw_all)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn drift_withdraw_with_program_id(
    program_id: Pubkey,
    accounts: &DriftWithdraw,
    amount: u64,
    withdraw_all: Option<bool>,
) -> Instruction {
    let mut data = DriftWithdraw::DISCRIMINATOR.to_vec();
    amount
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    withdraw_all
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`lending_pool_add_bank_drift`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingPoolAddBankDrift {
    pub group: Pubkey,
    pub governance_admin: Pubkey,
    pub fee_payer: Pubkey,
    /// Must match the mint used by `integration_acc_1`
    pub bank_mint: Pubkey,
    pub bank: Pubkey,
    /// Drift spot market account that must match the bank mint
    pub integration_acc_1: Pubkey,
    /// Drift user account for the marginfi program (derived from liquidity_vault_authority)
    pub integration_acc_2: Pubkey,
    /// Drift user stats account for the marginfi program (derived from liquidity_vault_authority)
    pub integration_acc_3: Pubkey,
    /// Will be authority of the bank's liquidity vault. Used as intermediary for deposits/withdraws
    pub liquidity_vault_authority: Pubkey,
    /// For Drift banks, the `liquidity_vault` never holds assets, but is instead used as an
    /// intermediary when depositing/withdrawing, e.g., withdrawn funds move from Drift -> here ->
    /// the user's token account.
    pub liquidity_vault: Pubkey,
    /// Note: Currently does nothing.
    pub insurance_vault_authority: Pubkey,
    /// Note: Currently does nothing.
    pub insurance_vault: Pubkey,
    /// Note: Currently does nothing.
    pub fee_vault_authority: Pubkey,
    /// Note: Currently does nothing.
    pub fee_vault: Pubkey,
    pub token_program: Pubkey,
    pub system_program: Pubkey,
    pub instruction_sysvar: Pubkey,
}
impl LendingPoolAddBankDrift {
    pub const DISCRIMINATOR: [u8; 8] = [62, 63, 49, 48, 76, 55, 108, 155];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.group, false),
            AccountMeta::new_readonly(self.governance_admin, true),
            AccountMeta::new(self.fee_payer, true),
            AccountMeta::new_readonly(self.bank_mint, false),
            AccountMeta::new(self.bank, false),
            AccountMeta::new_readonly(self.integration_acc_1, false),
            AccountMeta::new_readonly(self.integration_acc_2, false),
            AccountMeta::new_readonly(self.integration_acc_3, false),
            AccountMeta::new_readonly(self.liquidity_vault_authority, false),
            AccountMeta::new(self.liquidity_vault, false),
            AccountMeta::new_readonly(self.insurance_vault_authority, false),
            AccountMeta::new(self.insurance_vault, false),
            AccountMeta::new_readonly(self.fee_vault_authority, false),
            AccountMeta::new(self.fee_vault, false),
            AccountMeta::new_readonly(self.token_program, false),
            AccountMeta::new_readonly(self.system_program, false),
            AccountMeta::new_readonly(self.instruction_sysvar, false),
        ]
    }
}
impl ToAccountMetas for LendingPoolAddBankDrift {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (governance_admin only) Add a Drift bank to the group.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_add_bank_drift(
    accounts: &LendingPoolAddBankDrift,
    bank_config: DriftConfigCompact,
    bank_seed: u64,
) -> Instruction {
    lending_pool_add_bank_drift_with_program_id(crate::ID, accounts, bank_config, bank_seed)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_add_bank_drift_with_program_id(
    program_id: Pubkey,
    accounts: &LendingPoolAddBankDrift,
    bank_config: DriftConfigCompact,
    bank_seed: u64,
) -> Instruction {
    let mut data = LendingPoolAddBankDrift::DISCRIMINATOR.to_vec();
    bank_config
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    bank_seed
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}
