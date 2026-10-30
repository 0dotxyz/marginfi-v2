//! Instruction builders generated from the marginfi IDL.
#![allow(clippy::doc_lazy_continuation)]

use super::ToAccountMetas;
use crate::types::{
    BankConfigCompact, BankConfigFast, BankConfigGov, EmodeEntry, InterestRateConfigOpt,
    WrappedI80F48,
};
use borsh::BorshSerialize;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

/// Fixed accounts for [`init_bank_metadata`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InitBankMetadata {
    /// keypair-based bank. When initialized with a known seed, the PDA is verified in the handler.
    pub bank: Pubkey,
    /// Pays the init fee
    pub fee_payer: Pubkey,
    /// Note: unique per-bank.
    pub metadata: Pubkey,
    pub system_program: Pubkey,
}
impl InitBankMetadata {
    pub const DISCRIMINATOR: [u8; 8] = [94, 239, 50, 136, 137, 204, 254, 213];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.bank, false),
            AccountMeta::new(self.fee_payer, true),
            AccountMeta::new(self.metadata, false),
            AccountMeta::new_readonly(self.system_program, false),
        ]
    }
}
impl ToAccountMetas for InitBankMetadata {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (permissionless) pay the rent to open metadata for a bank. The bank account does not have
/// to exist yet — callers can pre-create metadata for an upcoming bank pubkey at their own
/// rent expense. When the bank is initialized and its seed is on-chain, the PDA is verified.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn init_bank_metadata(accounts: &InitBankMetadata) -> Instruction {
    init_bank_metadata_with_program_id(crate::ID, accounts)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn init_bank_metadata_with_program_id(
    program_id: Pubkey,
    accounts: &InitBankMetadata,
) -> Instruction {
    let data = InitBankMetadata::DISCRIMINATOR.to_vec();
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`lending_pool_accrue_bank_interest`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingPoolAccrueBankInterest {
    pub group: Pubkey,
    pub bank: Pubkey,
}
impl LendingPoolAccrueBankInterest {
    pub const DISCRIMINATOR: [u8; 8] = [108, 201, 30, 87, 47, 65, 97, 188];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new(self.bank, false),
        ]
    }
}
impl ToAccountMetas for LendingPoolAccrueBankInterest {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (permissionless) Accrue interest on a bank, updating share values and collecting fees.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_accrue_bank_interest(accounts: &LendingPoolAccrueBankInterest) -> Instruction {
    lending_pool_accrue_bank_interest_with_program_id(crate::ID, accounts)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_accrue_bank_interest_with_program_id(
    program_id: Pubkey,
    accounts: &LendingPoolAccrueBankInterest,
) -> Instruction {
    let data = LendingPoolAccrueBankInterest::DISCRIMINATOR.to_vec();
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`lending_pool_add_bank`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingPoolAddBank {
    pub marginfi_group: Pubkey,
    pub governance_admin: Pubkey,
    /// Pays to init accounts and pays `fee_state.bank_init_flat_sol_fee` lamports to the protocol
    pub fee_payer: Pubkey,
    pub fee_state: Pubkey,
    pub global_fee_wallet: Pubkey,
    pub bank_mint: Pubkey,
    pub bank: Pubkey,
    pub liquidity_vault_authority: Pubkey,
    pub liquidity_vault: Pubkey,
    pub insurance_vault_authority: Pubkey,
    pub insurance_vault: Pubkey,
    pub fee_vault_authority: Pubkey,
    pub fee_vault: Pubkey,
    pub token_program: Pubkey,
    pub system_program: Pubkey,
    pub instruction_sysvar: Pubkey,
}
impl LendingPoolAddBank {
    pub const DISCRIMINATOR: [u8; 8] = [215, 68, 72, 78, 208, 218, 103, 182];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.marginfi_group, false),
            AccountMeta::new_readonly(self.governance_admin, true),
            AccountMeta::new(self.fee_payer, true),
            AccountMeta::new_readonly(self.fee_state, false),
            AccountMeta::new(self.global_fee_wallet, false),
            AccountMeta::new_readonly(self.bank_mint, false),
            AccountMeta::new(self.bank, true),
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
impl ToAccountMetas for LendingPoolAddBank {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (governance_admin only) Add a new bank to the lending pool
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_add_bank(
    accounts: &LendingPoolAddBank,
    bank_config: BankConfigCompact,
) -> Instruction {
    lending_pool_add_bank_with_program_id(crate::ID, accounts, bank_config)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_add_bank_with_program_id(
    program_id: Pubkey,
    accounts: &LendingPoolAddBank,
    bank_config: BankConfigCompact,
) -> Instruction {
    let mut data = LendingPoolAddBank::DISCRIMINATOR.to_vec();
    bank_config
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`lending_pool_add_bank_permissionless`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingPoolAddBankPermissionless {
    pub marginfi_group: Pubkey,
    pub staked_settings: Pubkey,
    pub fee_payer: Pubkey,
    /// Mint of the spl-single-pool LST (a PDA derived from `stake_pool`)
    ///
    /// because the sol_pool and stake_pool will not derive to a valid PDA which is also owned by
    /// the staking program and spl-single-pool program.
    pub bank_mint: Pubkey,
    pub sol_pool: Pubkey,
    pub pool_onramp: Pubkey,
    /// this key.
    ///
    /// If derives the same `bank_mint`, then this must be the correct stake pool for that mint, and
    /// we can subsequently use it to validate the `sol_pool`
    pub stake_pool: Pubkey,
    /// Validator vote account for this staked bank.
    ///
    /// vote -> stake_pool -> mint/stake/on-ramp.
    pub validator_vote_account: Pubkey,
    pub bank: Pubkey,
    pub liquidity_vault_authority: Pubkey,
    pub liquidity_vault: Pubkey,
    pub insurance_vault_authority: Pubkey,
    pub insurance_vault: Pubkey,
    pub fee_vault_authority: Pubkey,
    pub fee_vault: Pubkey,
    pub token_program: Pubkey,
    pub system_program: Pubkey,
}
impl LendingPoolAddBankPermissionless {
    pub const DISCRIMINATOR: [u8; 8] = [127, 187, 121, 34, 187, 167, 238, 102];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.marginfi_group, false),
            AccountMeta::new_readonly(self.staked_settings, false),
            AccountMeta::new(self.fee_payer, true),
            AccountMeta::new_readonly(self.bank_mint, false),
            AccountMeta::new_readonly(self.sol_pool, false),
            AccountMeta::new_readonly(self.pool_onramp, false),
            AccountMeta::new_readonly(self.stake_pool, false),
            AccountMeta::new_readonly(self.validator_vote_account, false),
            AccountMeta::new(self.bank, false),
            AccountMeta::new_readonly(self.liquidity_vault_authority, false),
            AccountMeta::new(self.liquidity_vault, false),
            AccountMeta::new_readonly(self.insurance_vault_authority, false),
            AccountMeta::new(self.insurance_vault, false),
            AccountMeta::new_readonly(self.fee_vault_authority, false),
            AccountMeta::new(self.fee_vault, false),
            AccountMeta::new_readonly(self.token_program, false),
            AccountMeta::new_readonly(self.system_program, false),
        ]
    }
}
impl ToAccountMetas for LendingPoolAddBankPermissionless {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (permissionless) Add a staked collateral bank. Requires a valid SPL single-pool LST mint.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_add_bank_permissionless(
    accounts: &LendingPoolAddBankPermissionless,
    bank_seed: u64,
) -> Instruction {
    lending_pool_add_bank_permissionless_with_program_id(crate::ID, accounts, bank_seed)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_add_bank_permissionless_with_program_id(
    program_id: Pubkey,
    accounts: &LendingPoolAddBankPermissionless,
    bank_seed: u64,
) -> Instruction {
    let mut data = LendingPoolAddBankPermissionless::DISCRIMINATOR.to_vec();
    bank_seed
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`lending_pool_add_bank_with_seed`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingPoolAddBankWithSeed {
    pub marginfi_group: Pubkey,
    pub governance_admin: Pubkey,
    /// Pays to init accounts and pays `fee_state.bank_init_flat_sol_fee` lamports to the protocol
    pub fee_payer: Pubkey,
    pub fee_state: Pubkey,
    pub global_fee_wallet: Pubkey,
    pub bank_mint: Pubkey,
    pub bank: Pubkey,
    pub liquidity_vault_authority: Pubkey,
    pub liquidity_vault: Pubkey,
    pub insurance_vault_authority: Pubkey,
    pub insurance_vault: Pubkey,
    pub fee_vault_authority: Pubkey,
    pub fee_vault: Pubkey,
    pub token_program: Pubkey,
    pub system_program: Pubkey,
    pub instruction_sysvar: Pubkey,
}
impl LendingPoolAddBankWithSeed {
    pub const DISCRIMINATOR: [u8; 8] = [76, 211, 213, 171, 117, 78, 158, 76];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.marginfi_group, false),
            AccountMeta::new_readonly(self.governance_admin, true),
            AccountMeta::new(self.fee_payer, true),
            AccountMeta::new_readonly(self.fee_state, false),
            AccountMeta::new(self.global_fee_wallet, false),
            AccountMeta::new_readonly(self.bank_mint, false),
            AccountMeta::new(self.bank, false),
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
impl ToAccountMetas for LendingPoolAddBankWithSeed {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (governance_admin only) A copy of lending_pool_add_bank with an additional bank seed.
/// This seed is used to create a PDA for the bank's signature.
/// lending_pool_add_bank is preserved for backwards compatibility.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_add_bank_with_seed(
    accounts: &LendingPoolAddBankWithSeed,
    bank_config: BankConfigCompact,
    bank_seed: u64,
) -> Instruction {
    lending_pool_add_bank_with_seed_with_program_id(crate::ID, accounts, bank_config, bank_seed)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_add_bank_with_seed_with_program_id(
    program_id: Pubkey,
    accounts: &LendingPoolAddBankWithSeed,
    bank_config: BankConfigCompact,
    bank_seed: u64,
) -> Instruction {
    let mut data = LendingPoolAddBankWithSeed::DISCRIMINATOR.to_vec();
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

/// Fixed accounts for [`lending_pool_backfill_bank_is_t22_flag`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingPoolBackfillBankIsT22Flag {
    pub bank: Pubkey,
    pub group: Pubkey,
    pub mint: Pubkey,
}
impl LendingPoolBackfillBankIsT22Flag {
    pub const DISCRIMINATOR: [u8; 8] = [189, 14, 205, 160, 172, 46, 157, 52];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.bank, false),
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new_readonly(self.mint, false),
        ]
    }
}
impl ToAccountMetas for LendingPoolBackfillBankIsT22Flag {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (permissionless) Backfill `IS_T22` on existing banks created before this flag existed.
/// Also optionally backfills `bank_seed` in the same call.
/// Pass `None` to skip seed backfill, `Some(seed)` to backfill (including `Some(0)`).
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_backfill_bank_is_t22_flag(
    accounts: &LendingPoolBackfillBankIsT22Flag,
    bank_seed: Option<u64>,
) -> Instruction {
    lending_pool_backfill_bank_is_t22_flag_with_program_id(crate::ID, accounts, bank_seed)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_backfill_bank_is_t22_flag_with_program_id(
    program_id: Pubkey,
    accounts: &LendingPoolBackfillBankIsT22Flag,
    bank_seed: Option<u64>,
) -> Instruction {
    let mut data = LendingPoolBackfillBankIsT22Flag::DISCRIMINATOR.to_vec();
    bank_seed
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`lending_pool_backfill_staked_bank_validator_vote_account`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingPoolBackfillStakedBankValidatorVoteAccount {
    pub bank: Pubkey,
    pub validator_vote_account: Pubkey,
}
impl LendingPoolBackfillStakedBankValidatorVoteAccount {
    pub const DISCRIMINATOR: [u8; 8] = [141, 6, 23, 125, 72, 34, 199, 24];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.bank, false),
            AccountMeta::new_readonly(self.validator_vote_account, false),
        ]
    }
}
impl ToAccountMetas for LendingPoolBackfillStakedBankValidatorVoteAccount {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (permissionless) Backfill validator vote account on existing staked-collateral banks.
/// No-op if already set to the same validator vote account.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_backfill_staked_bank_validator_vote_account(
    accounts: &LendingPoolBackfillStakedBankValidatorVoteAccount,
) -> Instruction {
    lending_pool_backfill_staked_bank_validator_vote_account_with_program_id(crate::ID, accounts)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_backfill_staked_bank_validator_vote_account_with_program_id(
    program_id: Pubkey,
    accounts: &LendingPoolBackfillStakedBankValidatorVoteAccount,
) -> Instruction {
    let data = LendingPoolBackfillStakedBankValidatorVoteAccount::DISCRIMINATOR.to_vec();
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`lending_pool_clear_circuit_breaker`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingPoolClearCircuitBreaker {
    pub group: Pubkey,
    /// Either `group.admin` or `group.risk_admin`. Validated in the handler.
    pub authority: Pubkey,
    pub bank: Pubkey,
    pub instruction_sysvar: Pubkey,
}
impl LendingPoolClearCircuitBreaker {
    pub const DISCRIMINATOR: [u8; 8] = [64, 73, 106, 46, 213, 86, 31, 48];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new_readonly(self.authority, true),
            AccountMeta::new(self.bank, false),
            AccountMeta::new_readonly(self.instruction_sysvar, false),
        ]
    }
}
impl ToAccountMetas for LendingPoolClearCircuitBreaker {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (admin or risk_admin) Clear an active circuit-breaker halt on a bank.
/// * `reseed_reference` - If true, also zero the EMA reference so the next pulse reseeds it
/// from live oracle data (use when clearing because the new price level is valid and the
/// pre-halt reference would cause an immediate re-halt).
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_clear_circuit_breaker(
    accounts: &LendingPoolClearCircuitBreaker,
    reseed_reference: bool,
) -> Instruction {
    lending_pool_clear_circuit_breaker_with_program_id(crate::ID, accounts, reseed_reference)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_clear_circuit_breaker_with_program_id(
    program_id: Pubkey,
    accounts: &LendingPoolClearCircuitBreaker,
    reseed_reference: bool,
) -> Instruction {
    let mut data = LendingPoolClearCircuitBreaker::DISCRIMINATOR.to_vec();
    reseed_reference
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`lending_pool_clone_bank`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingPoolCloneBank {
    pub marginfi_group: Pubkey,
    pub governance_admin: Pubkey,
    pub fee_payer: Pubkey,
    pub bank_mint: Pubkey,
    /// Source bank to clone from mainnet program
    ///
    pub source_bank: Pubkey,
    pub bank: Pubkey,
    pub liquidity_vault_authority: Pubkey,
    pub liquidity_vault: Pubkey,
    pub insurance_vault_authority: Pubkey,
    pub insurance_vault: Pubkey,
    pub fee_vault_authority: Pubkey,
    pub fee_vault: Pubkey,
    pub token_program: Pubkey,
    pub system_program: Pubkey,
    pub instruction_sysvar: Pubkey,
}
impl LendingPoolCloneBank {
    pub const DISCRIMINATOR: [u8; 8] = [214, 93, 17, 236, 177, 228, 78, 17];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.marginfi_group, false),
            AccountMeta::new(self.governance_admin, true),
            AccountMeta::new(self.fee_payer, true),
            AccountMeta::new_readonly(self.bank_mint, false),
            AccountMeta::new_readonly(self.source_bank, false),
            AccountMeta::new(self.bank, false),
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
impl ToAccountMetas for LendingPoolCloneBank {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (governance_admin only) Staging or localnet only, panics on mainnet
/// This instruction is used to clone a bank to a new PDA.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_clone_bank(accounts: &LendingPoolCloneBank, bank_seed: u64) -> Instruction {
    lending_pool_clone_bank_with_program_id(crate::ID, accounts, bank_seed)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_clone_bank_with_program_id(
    program_id: Pubkey,
    accounts: &LendingPoolCloneBank,
    bank_seed: u64,
) -> Instruction {
    let mut data = LendingPoolCloneBank::DISCRIMINATOR.to_vec();
    bank_seed
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`lending_pool_clone_emode`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingPoolCloneEmode {
    pub group: Pubkey,
    pub governance_admin: Pubkey,
    pub copy_from_bank: Pubkey,
    pub copy_to_bank: Pubkey,
    pub instruction_sysvar: Pubkey,
}
impl LendingPoolCloneEmode {
    pub const DISCRIMINATOR: [u8; 8] = [146, 167, 94, 106, 184, 202, 15, 10];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new_readonly(self.governance_admin, true),
            AccountMeta::new_readonly(self.copy_from_bank, false),
            AccountMeta::new(self.copy_to_bank, false),
            AccountMeta::new_readonly(self.instruction_sysvar, false),
        ]
    }
}
impl ToAccountMetas for LendingPoolCloneEmode {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (governance_admin only) Copies emode settings from one bank to another. Useful when applying
/// emode settings from e.g. one LST to another.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_clone_emode(accounts: &LendingPoolCloneEmode) -> Instruction {
    lending_pool_clone_emode_with_program_id(crate::ID, accounts)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_clone_emode_with_program_id(
    program_id: Pubkey,
    accounts: &LendingPoolCloneEmode,
) -> Instruction {
    let data = LendingPoolCloneEmode::DISCRIMINATOR.to_vec();
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`lending_pool_close_bank`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingPoolCloseBank {
    pub group: Pubkey,
    pub bank: Pubkey,
    pub admin: Pubkey,
    pub instruction_sysvar: Pubkey,
}
impl LendingPoolCloseBank {
    pub const DISCRIMINATOR: [u8; 8] = [22, 115, 7, 130, 227, 85, 0, 47];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.group, false),
            AccountMeta::new(self.bank, false),
            AccountMeta::new(self.admin, true),
            AccountMeta::new_readonly(self.instruction_sysvar, false),
        ]
    }
}
impl ToAccountMetas for LendingPoolCloseBank {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (admin only) Close a bank. Requires CLOSE_ENABLED_FLAG and zero positions/shares.
///
/// Pass `force_close = Some(true)` to bypass the CLOSE_ENABLED_FLAG and open-position checks
/// (zero-shares/emissions are still required). Forcing a bank closed is **VERY DANGEROUS**.
/// Only do it if a Bank was fundamentally broken in some way. The admin **MUST ENSURE** that:
///
/// * **NO USER** has a Balance in this bank (zero-shares on the bank  is not sufficient to
/// guarantee this, a user can have a zero-share Balance, this could brick their account.)
/// * fee and insurance vault balances are withdrawn (unless you don't care if they are lost
/// **FOREVER**).
/// * all three vault token-account balances are zero (or you don't care if anything remaining
/// is lost **FOREVER**), including the liquidity vault
/// * all three outstanding-fee fields are zero (or you don't care if anything remaining is lost
/// **FOREVER**)
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_close_bank(
    accounts: &LendingPoolCloseBank,
    force_close: Option<bool>,
) -> Instruction {
    lending_pool_close_bank_with_program_id(crate::ID, accounts, force_close)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_close_bank_with_program_id(
    program_id: Pubkey,
    accounts: &LendingPoolCloseBank,
    force_close: Option<bool>,
) -> Instruction {
    let mut data = LendingPoolCloseBank::DISCRIMINATOR.to_vec();
    force_close
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`lending_pool_collect_bank_fees`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingPoolCollectBankFees {
    pub group: Pubkey,
    pub bank: Pubkey,
    pub liquidity_vault_authority: Pubkey,
    pub liquidity_vault: Pubkey,
    pub insurance_vault: Pubkey,
    pub fee_vault: Pubkey,
    pub fee_state: Pubkey,
    /// (validated in handler). Must already exist, may require initializing the ATA if it does not
    /// already exist prior to this ix.
    pub fee_ata: Pubkey,
    pub token_program: Pubkey,
}
impl LendingPoolCollectBankFees {
    pub const DISCRIMINATOR: [u8; 8] = [201, 5, 215, 116, 230, 92, 75, 150];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new(self.bank, false),
            AccountMeta::new_readonly(self.liquidity_vault_authority, false),
            AccountMeta::new(self.liquidity_vault, false),
            AccountMeta::new(self.insurance_vault, false),
            AccountMeta::new(self.fee_vault, false),
            AccountMeta::new_readonly(self.fee_state, false),
            AccountMeta::new(self.fee_ata, false),
            AccountMeta::new_readonly(self.token_program, false),
        ]
    }
}
impl ToAccountMetas for LendingPoolCollectBankFees {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (permissionless) Transfer accrued fees from the liquidity vault to insurance/fee/program
/// vaults.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_collect_bank_fees(accounts: &LendingPoolCollectBankFees) -> Instruction {
    lending_pool_collect_bank_fees_with_program_id(crate::ID, accounts)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_collect_bank_fees_with_program_id(
    program_id: Pubkey,
    accounts: &LendingPoolCollectBankFees,
) -> Instruction {
    let data = LendingPoolCollectBankFees::DISCRIMINATOR.to_vec();
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`lending_pool_collect_bank_premium_fees`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingPoolCollectBankPremiumFees {
    pub group: Pubkey,
    pub bank: Pubkey,
    pub liquidity_vault_authority: Pubkey,
    pub liquidity_vault: Pubkey,
    pub fee_state: Pubkey,
    /// Canonical ATA of the `FeeState.premium_wallet` for the mint used by this bank
    /// (validated in handler). Must already exist.
    pub premium_ata: Pubkey,
    pub token_program: Pubkey,
}
impl LendingPoolCollectBankPremiumFees {
    pub const DISCRIMINATOR: [u8; 8] = [193, 168, 164, 176, 80, 191, 33, 180];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new(self.bank, false),
            AccountMeta::new_readonly(self.liquidity_vault_authority, false),
            AccountMeta::new(self.liquidity_vault, false),
            AccountMeta::new_readonly(self.fee_state, false),
            AccountMeta::new(self.premium_ata, false),
            AccountMeta::new_readonly(self.token_program, false),
        ]
    }
}
impl ToAccountMetas for LendingPoolCollectBankPremiumFees {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (permissionless) Sweep realized variable-borrow premium from the liquidity vault to the
/// canonical ATA of `FeeState.premium_wallet` for the bank's mint.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_collect_bank_premium_fees(
    accounts: &LendingPoolCollectBankPremiumFees,
) -> Instruction {
    lending_pool_collect_bank_premium_fees_with_program_id(crate::ID, accounts)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_collect_bank_premium_fees_with_program_id(
    program_id: Pubkey,
    accounts: &LendingPoolCollectBankPremiumFees,
) -> Instruction {
    let data = LendingPoolCollectBankPremiumFees::DISCRIMINATOR.to_vec();
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`lending_pool_configure_bank`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingPoolConfigureBank {
    pub group: Pubkey,
    pub admin: Pubkey,
    pub bank: Pubkey,
    pub instruction_sysvar: Pubkey,
}
impl LendingPoolConfigureBank {
    pub const DISCRIMINATOR: [u8; 8] = [121, 173, 156, 40, 93, 148, 56, 237];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new_readonly(self.admin, true),
            AccountMeta::new(self.bank, false),
            AccountMeta::new_readonly(self.instruction_sysvar, false),
        ]
    }
}
impl ToAccountMetas for LendingPoolConfigureBank {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// Configure fast-admin bank parameters. Operational banks may be paused, set to
/// ReduceOnly, or set to ReduceOnlyWithBorrowingPower; the latter may subsequently be set to
/// ReduceOnly. The fast admin may pause a bank from any state. Restoring a bank to
/// Operational requires `lending_pool_configure_bank_gov`.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_configure_bank(
    accounts: &LendingPoolConfigureBank,
    bank_config_opt: BankConfigFast,
) -> Instruction {
    lending_pool_configure_bank_with_program_id(crate::ID, accounts, bank_config_opt)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_configure_bank_with_program_id(
    program_id: Pubkey,
    accounts: &LendingPoolConfigureBank,
    bank_config_opt: BankConfigFast,
) -> Instruction {
    let mut data = LendingPoolConfigureBank::DISCRIMINATOR.to_vec();
    bank_config_opt
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`lending_pool_configure_bank_emode`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingPoolConfigureBankEmode {
    pub group: Pubkey,
    pub governance_admin: Pubkey,
    pub bank: Pubkey,
    pub instruction_sysvar: Pubkey,
}
impl LendingPoolConfigureBankEmode {
    pub const DISCRIMINATOR: [u8; 8] = [17, 175, 91, 57, 239, 86, 49, 71];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new_readonly(self.governance_admin, true),
            AccountMeta::new(self.bank, false),
            AccountMeta::new_readonly(self.instruction_sysvar, false),
        ]
    }
}
impl ToAccountMetas for LendingPoolConfigureBankEmode {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (governance_admin only)
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_configure_bank_emode(
    accounts: &LendingPoolConfigureBankEmode,
    emode_tag: u16,
    entries: [EmodeEntry; 10],
) -> Instruction {
    lending_pool_configure_bank_emode_with_program_id(crate::ID, accounts, emode_tag, entries)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_configure_bank_emode_with_program_id(
    program_id: Pubkey,
    accounts: &LendingPoolConfigureBankEmode,
    emode_tag: u16,
    entries: [EmodeEntry; 10],
) -> Instruction {
    let mut data = LendingPoolConfigureBankEmode::DISCRIMINATOR.to_vec();
    emode_tag
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    entries
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`lending_pool_configure_bank_gov`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingPoolConfigureBankGov {
    pub group: Pubkey,
    pub governance_admin: Pubkey,
    pub bank: Pubkey,
    pub instruction_sysvar: Pubkey,
}
impl LendingPoolConfigureBankGov {
    pub const DISCRIMINATOR: [u8; 8] = [122, 87, 141, 86, 182, 34, 146, 25];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new_readonly(self.governance_admin, true),
            AccountMeta::new(self.bank, false),
            AccountMeta::new_readonly(self.instruction_sysvar, false),
        ]
    }
}
impl ToAccountMetas for LendingPoolConfigureBankGov {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// Configure slow, timelocked governance bank parameters, including risk settings,
/// freezing bank settings, and restoring a bank to Operational.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_configure_bank_gov(
    accounts: &LendingPoolConfigureBankGov,
    bank_config_opt: BankConfigGov,
) -> Instruction {
    lending_pool_configure_bank_gov_with_program_id(crate::ID, accounts, bank_config_opt)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_configure_bank_gov_with_program_id(
    program_id: Pubkey,
    accounts: &LendingPoolConfigureBankGov,
    bank_config_opt: BankConfigGov,
) -> Instruction {
    let mut data = LendingPoolConfigureBankGov::DISCRIMINATOR.to_vec();
    bank_config_opt
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`lending_pool_configure_bank_interest_only`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingPoolConfigureBankInterestOnly {
    pub group: Pubkey,
    pub delegate_curve_admin: Pubkey,
    pub bank: Pubkey,
    pub instruction_sysvar: Pubkey,
}
impl LendingPoolConfigureBankInterestOnly {
    pub const DISCRIMINATOR: [u8; 8] = [245, 107, 83, 38, 103, 219, 163, 241];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new_readonly(self.delegate_curve_admin, true),
            AccountMeta::new(self.bank, false),
            AccountMeta::new_readonly(self.instruction_sysvar, false),
        ]
    }
}
impl ToAccountMetas for LendingPoolConfigureBankInterestOnly {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (delegate_curve_admin only) Update interest rate config. Does nothing if bank has
/// `FREEZE_SETTINGS`.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_configure_bank_interest_only(
    accounts: &LendingPoolConfigureBankInterestOnly,
    interest_rate_config: InterestRateConfigOpt,
) -> Instruction {
    lending_pool_configure_bank_interest_only_with_program_id(
        crate::ID,
        accounts,
        interest_rate_config,
    )
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_configure_bank_interest_only_with_program_id(
    program_id: Pubkey,
    accounts: &LendingPoolConfigureBankInterestOnly,
    interest_rate_config: InterestRateConfigOpt,
) -> Instruction {
    let mut data = LendingPoolConfigureBankInterestOnly::DISCRIMINATOR.to_vec();
    interest_rate_config
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`lending_pool_configure_bank_limits_only`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingPoolConfigureBankLimitsOnly {
    pub group: Pubkey,
    pub delegate_limit_admin: Pubkey,
    pub bank: Pubkey,
    pub instruction_sysvar: Pubkey,
}
impl LendingPoolConfigureBankLimitsOnly {
    pub const DISCRIMINATOR: [u8; 8] = [157, 196, 221, 200, 202, 62, 84, 21];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new_readonly(self.delegate_limit_admin, true),
            AccountMeta::new(self.bank, false),
            AccountMeta::new_readonly(self.instruction_sysvar, false),
        ]
    }
}
impl ToAccountMetas for LendingPoolConfigureBankLimitsOnly {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (delegate_limit_admin only) Update deposit/borrow/init limits only.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_configure_bank_limits_only(
    accounts: &LendingPoolConfigureBankLimitsOnly,
    deposit_limit: Option<u64>,
    borrow_limit: Option<u64>,
    total_asset_value_init_limit: Option<u64>,
) -> Instruction {
    lending_pool_configure_bank_limits_only_with_program_id(
        crate::ID,
        accounts,
        deposit_limit,
        borrow_limit,
        total_asset_value_init_limit,
    )
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_configure_bank_limits_only_with_program_id(
    program_id: Pubkey,
    accounts: &LendingPoolConfigureBankLimitsOnly,
    deposit_limit: Option<u64>,
    borrow_limit: Option<u64>,
    total_asset_value_init_limit: Option<u64>,
) -> Instruction {
    let mut data = LendingPoolConfigureBankLimitsOnly::DISCRIMINATOR.to_vec();
    deposit_limit
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    borrow_limit
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    total_asset_value_init_limit
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`lending_pool_configure_bank_oracle`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingPoolConfigureBankOracle {
    pub group: Pubkey,
    pub governance_admin: Pubkey,
    pub bank: Pubkey,
    pub instruction_sysvar: Pubkey,
}
impl LendingPoolConfigureBankOracle {
    pub const DISCRIMINATOR: [u8; 8] = [209, 82, 255, 171, 124, 21, 71, 81];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new_readonly(self.governance_admin, true),
            AccountMeta::new(self.bank, false),
            AccountMeta::new_readonly(self.instruction_sysvar, false),
        ]
    }
}
impl ToAccountMetas for LendingPoolConfigureBankOracle {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (governance_admin only)
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_configure_bank_oracle(
    accounts: &LendingPoolConfigureBankOracle,
    setup: u8,
    oracle: Pubkey,
) -> Instruction {
    lending_pool_configure_bank_oracle_with_program_id(crate::ID, accounts, setup, oracle)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_configure_bank_oracle_with_program_id(
    program_id: Pubkey,
    accounts: &LendingPoolConfigureBankOracle,
    setup: u8,
    oracle: Pubkey,
) -> Instruction {
    let mut data = LendingPoolConfigureBankOracle::DISCRIMINATOR.to_vec();
    setup
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    oracle
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`lending_pool_configure_bank_oracle_scope`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingPoolConfigureBankOracleScope {
    pub group: Pubkey,
    pub governance_admin: Pubkey,
    pub bank: Pubkey,
    pub instruction_sysvar: Pubkey,
}
impl LendingPoolConfigureBankOracleScope {
    pub const DISCRIMINATOR: [u8; 8] = [134, 228, 127, 3, 117, 132, 85, 146];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new_readonly(self.governance_admin, true),
            AccountMeta::new(self.bank, false),
            AccountMeta::new_readonly(self.instruction_sysvar, false),
        ]
    }
}
impl ToAccountMetas for LendingPoolConfigureBankOracleScope {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (governance_admin only) Point a bank at a Scope feed entry. Picks `Scope`, `ScopeKamino`, or
/// `ScopeJuplend` from the bank's asset tag; Kamino/JupLend banks pass their reserve/lending
/// account after the feed in remaining accounts.
/// * oracle - the feed's `OraclePrices` account
/// * entry_index - which of the 512 entries in that account prices this bank
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_configure_bank_oracle_scope(
    accounts: &LendingPoolConfigureBankOracleScope,
    oracle: Pubkey,
    entry_index: u16,
) -> Instruction {
    lending_pool_configure_bank_oracle_scope_with_program_id(
        crate::ID,
        accounts,
        oracle,
        entry_index,
    )
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_configure_bank_oracle_scope_with_program_id(
    program_id: Pubkey,
    accounts: &LendingPoolConfigureBankOracleScope,
    oracle: Pubkey,
    entry_index: u16,
) -> Instruction {
    let mut data = LendingPoolConfigureBankOracleScope::DISCRIMINATOR.to_vec();
    oracle
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    entry_index
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`lending_pool_configure_bank_premium`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingPoolConfigureBankPremium {
    pub group: Pubkey,
    pub admin: Pubkey,
    pub bank: Pubkey,
    pub instruction_sysvar: Pubkey,
}
impl LendingPoolConfigureBankPremium {
    pub const DISCRIMINATOR: [u8; 8] = [216, 82, 134, 217, 11, 240, 78, 42];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new_readonly(self.admin, true),
            AccountMeta::new(self.bank, false),
            AccountMeta::new_readonly(self.instruction_sysvar, false),
        ]
    }
}
impl ToAccountMetas for LendingPoolConfigureBankPremium {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (fast group admin only) Set a bank's premium tag and toggle premium accrual for its borrowers.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_configure_bank_premium(
    accounts: &LendingPoolConfigureBankPremium,
    premium_tag: u16,
    active: bool,
) -> Instruction {
    lending_pool_configure_bank_premium_with_program_id(crate::ID, accounts, premium_tag, active)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_configure_bank_premium_with_program_id(
    program_id: Pubkey,
    accounts: &LendingPoolConfigureBankPremium,
    premium_tag: u16,
    active: bool,
) -> Instruction {
    let mut data = LendingPoolConfigureBankPremium::DISCRIMINATOR.to_vec();
    premium_tag
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    active
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`lending_pool_configure_group_premium`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingPoolConfigureGroupPremium {
    pub group: Pubkey,
    pub admin: Pubkey,
    pub instruction_sysvar: Pubkey,
}
impl LendingPoolConfigureGroupPremium {
    pub const DISCRIMINATOR: [u8; 8] = [184, 196, 245, 41, 203, 77, 75, 170];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.group, false),
            AccountMeta::new_readonly(self.admin, true),
            AccountMeta::new_readonly(self.instruction_sysvar, false),
        ]
    }
}
impl ToAccountMetas for LendingPoolConfigureGroupPremium {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (fast group admin only) Set one pair of the group's variable-borrow premium matrix:
/// `rate > 0` inserts or updates the pair, `rate == 0` removes it.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_configure_group_premium(
    accounts: &LendingPoolConfigureGroupPremium,
    collateral_tag: u16,
    liability_tag: u16,
    rate: u32,
) -> Instruction {
    lending_pool_configure_group_premium_with_program_id(
        crate::ID,
        accounts,
        collateral_tag,
        liability_tag,
        rate,
    )
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_configure_group_premium_with_program_id(
    program_id: Pubkey,
    accounts: &LendingPoolConfigureGroupPremium,
    collateral_tag: u16,
    liability_tag: u16,
    rate: u32,
) -> Instruction {
    let mut data = LendingPoolConfigureGroupPremium::DISCRIMINATOR.to_vec();
    collateral_tag
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    liability_tag
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    rate.serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`lending_pool_emissions_deposit`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingPoolEmissionsDeposit {
    pub group: Pubkey,
    pub bank: Pubkey,
    pub mint: Pubkey,
    /// NOTE: This is a TokenAccount, spl transfer will validate it.
    ///
    pub emissions_funding_account: Pubkey,
    pub depositor: Pubkey,
    pub liquidity_vault: Pubkey,
    pub token_program: Pubkey,
}
impl LendingPoolEmissionsDeposit {
    pub const DISCRIMINATOR: [u8; 8] = [121, 118, 123, 58, 59, 192, 74, 138];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new(self.bank, false),
            AccountMeta::new_readonly(self.mint, false),
            AccountMeta::new(self.emissions_funding_account, false),
            AccountMeta::new(self.depositor, true),
            AccountMeta::new(self.liquidity_vault, false),
            AccountMeta::new_readonly(self.token_program, false),
        ]
    }
}
impl ToAccountMetas for LendingPoolEmissionsDeposit {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (permissionless) Deposit same-bank emissions directly into liquidity vault and increase
/// depositors' value via `asset_share_value`.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_emissions_deposit(
    accounts: &LendingPoolEmissionsDeposit,
    amount: u64,
) -> Instruction {
    lending_pool_emissions_deposit_with_program_id(crate::ID, accounts, amount)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_emissions_deposit_with_program_id(
    program_id: Pubkey,
    accounts: &LendingPoolEmissionsDeposit,
    amount: u64,
) -> Instruction {
    let mut data = LendingPoolEmissionsDeposit::DISCRIMINATOR.to_vec();
    amount
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`lending_pool_force_tokenless_repay_complete`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingPoolForceTokenlessRepayComplete {
    pub group: Pubkey,
    pub risk_admin: Pubkey,
    pub bank: Pubkey,
    pub instruction_sysvar: Pubkey,
}
impl LendingPoolForceTokenlessRepayComplete {
    pub const DISCRIMINATOR: [u8; 8] = [15, 203, 147, 232, 199, 14, 231, 37];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new_readonly(self.risk_admin, true),
            AccountMeta::new(self.bank, false),
            AccountMeta::new_readonly(self.instruction_sysvar, false),
        ]
    }
}
impl ToAccountMetas for LendingPoolForceTokenlessRepayComplete {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (risk_admin only) - Signals all of a bank's liability have been deleveraged. Used if a bank
/// still has liability dust after the risk admin has completed deleveraging all debts. The
/// risk admin is trusted not to execute this until all non-dust debts have been deleveraged.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_force_tokenless_repay_complete(
    accounts: &LendingPoolForceTokenlessRepayComplete,
) -> Instruction {
    lending_pool_force_tokenless_repay_complete_with_program_id(crate::ID, accounts)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_force_tokenless_repay_complete_with_program_id(
    program_id: Pubkey,
    accounts: &LendingPoolForceTokenlessRepayComplete,
) -> Instruction {
    let data = LendingPoolForceTokenlessRepayComplete::DISCRIMINATOR.to_vec();
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`lending_pool_handle_bankruptcy`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingPoolHandleBankruptcy {
    pub group: Pubkey,
    /// Must be risk_admin or admin, unless the bank has PERMISSIONLESS_BAD_DEBT_SETTLEMENT_FLAG
    /// set, in which case any signer is accepted.
    pub signer: Pubkey,
    pub bank: Pubkey,
    pub marginfi_account: Pubkey,
    pub liquidity_vault: Pubkey,
    pub insurance_vault: Pubkey,
    pub insurance_vault_authority: Pubkey,
    pub token_program: Pubkey,
    pub instruction_sysvar: Pubkey,
}
impl LendingPoolHandleBankruptcy {
    pub const DISCRIMINATOR: [u8; 8] = [162, 11, 56, 139, 90, 128, 70, 173];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new_readonly(self.signer, true),
            AccountMeta::new(self.bank, false),
            AccountMeta::new(self.marginfi_account, false),
            AccountMeta::new(self.liquidity_vault, false),
            AccountMeta::new(self.insurance_vault, false),
            AccountMeta::new_readonly(self.insurance_vault_authority, false),
            AccountMeta::new_readonly(self.token_program, false),
            AccountMeta::new_readonly(self.instruction_sysvar, false),
        ]
    }
}
impl ToAccountMetas for LendingPoolHandleBankruptcy {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (risk_admin or admin, unless `PERMISSIONLESS_BAD_DEBT_SETTLEMENT_FLAG` is set on the bank)
/// Handle bad debt of a bankrupt marginfi account for a given bank. Covers bad debt from the
/// insurance fund and socializes any remainder among depositors.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_handle_bankruptcy(accounts: &LendingPoolHandleBankruptcy) -> Instruction {
    lending_pool_handle_bankruptcy_with_program_id(crate::ID, accounts)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_handle_bankruptcy_with_program_id(
    program_id: Pubkey,
    accounts: &LendingPoolHandleBankruptcy,
) -> Instruction {
    let data = LendingPoolHandleBankruptcy::DISCRIMINATOR.to_vec();
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`lending_pool_init_same_asset_emode_registry`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingPoolInitSameAssetEmodeRegistry {
    pub group: Pubkey,
    pub governance_admin: Pubkey,
    pub same_asset_emode_registry: Pubkey,
    pub system_program: Pubkey,
    pub instruction_sysvar: Pubkey,
}
impl LendingPoolInitSameAssetEmodeRegistry {
    pub const DISCRIMINATOR: [u8; 8] = [217, 78, 227, 223, 147, 231, 213, 108];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new(self.governance_admin, true),
            AccountMeta::new(self.same_asset_emode_registry, false),
            AccountMeta::new_readonly(self.system_program, false),
            AccountMeta::new_readonly(self.instruction_sysvar, false),
        ]
    }
}
impl ToAccountMetas for LendingPoolInitSameAssetEmodeRegistry {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (governance_admin only) Initialize the per-group same-asset e-mode registry.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_init_same_asset_emode_registry(
    accounts: &LendingPoolInitSameAssetEmodeRegistry,
) -> Instruction {
    lending_pool_init_same_asset_emode_registry_with_program_id(crate::ID, accounts)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_init_same_asset_emode_registry_with_program_id(
    program_id: Pubkey,
    accounts: &LendingPoolInitSameAssetEmodeRegistry,
) -> Instruction {
    let data = LendingPoolInitSameAssetEmodeRegistry::DISCRIMINATOR.to_vec();
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`lending_pool_pulse_bank_price_cache`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingPoolPulseBankPriceCache {
    pub group: Pubkey,
    pub bank: Pubkey,
}
impl LendingPoolPulseBankPriceCache {
    pub const DISCRIMINATOR: [u8; 8] = [192, 19, 201, 135, 105, 203, 32, 222];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new(self.bank, false),
        ]
    }
}
impl ToAccountMetas for LendingPoolPulseBankPriceCache {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (Permissionless) Refresh the cached oracle price for a bank.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_pulse_bank_price_cache(
    accounts: &LendingPoolPulseBankPriceCache,
) -> Instruction {
    lending_pool_pulse_bank_price_cache_with_program_id(crate::ID, accounts)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_pulse_bank_price_cache_with_program_id(
    program_id: Pubkey,
    accounts: &LendingPoolPulseBankPriceCache,
) -> Instruction {
    let data = LendingPoolPulseBankPriceCache::DISCRIMINATOR.to_vec();
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`lending_pool_resize_bank_account`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingPoolResizeBankAccount {
    /// undersized bank can still be resized under the future (larger-struct) program.
    pub bank: Pubkey,
    /// Funds the rent for the added account space.
    pub payer: Pubkey,
    pub system_program: Pubkey,
}
impl LendingPoolResizeBankAccount {
    pub const DISCRIMINATOR: [u8; 8] = [44, 109, 34, 113, 110, 33, 139, 160];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.bank, false),
            AccountMeta::new(self.payer, true),
            AccountMeta::new_readonly(self.system_program, false),
        ]
    }
}
impl ToAccountMetas for LendingPoolResizeBankAccount {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (permissionless) Resize a v1-sized bank account to the current struct size; `payer`
/// funds the added rent.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_resize_bank_account(accounts: &LendingPoolResizeBankAccount) -> Instruction {
    lending_pool_resize_bank_account_with_program_id(crate::ID, accounts)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_resize_bank_account_with_program_id(
    program_id: Pubkey,
    accounts: &LendingPoolResizeBankAccount,
) -> Instruction {
    let data = LendingPoolResizeBankAccount::DISCRIMINATOR.to_vec();
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`lending_pool_resize_group_account`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingPoolResizeGroupAccount {
    /// undersized group can still be resized under the future (larger-struct) program.
    pub group: Pubkey,
    /// Funds the rent for the added account space.
    pub payer: Pubkey,
    pub system_program: Pubkey,
}
impl LendingPoolResizeGroupAccount {
    pub const DISCRIMINATOR: [u8; 8] = [97, 221, 69, 96, 204, 162, 174, 250];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.group, false),
            AccountMeta::new(self.payer, true),
            AccountMeta::new_readonly(self.system_program, false),
        ]
    }
}
impl ToAccountMetas for LendingPoolResizeGroupAccount {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (permissionless) Resize the group account to the v2 layout size; `payer` funds the
/// added rent.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_resize_group_account(accounts: &LendingPoolResizeGroupAccount) -> Instruction {
    lending_pool_resize_group_account_with_program_id(crate::ID, accounts)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_resize_group_account_with_program_id(
    program_id: Pubkey,
    accounts: &LendingPoolResizeGroupAccount,
) -> Instruction {
    let data = LendingPoolResizeGroupAccount::DISCRIMINATOR.to_vec();
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`lending_pool_set_bank_same_asset_emode_eligibility`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingPoolSetBankSameAssetEmodeEligibility {
    pub group: Pubkey,
    pub governance_admin: Pubkey,
    pub bank: Pubkey,
    pub same_asset_emode_registry: Pubkey,
    pub instruction_sysvar: Pubkey,
}
impl LendingPoolSetBankSameAssetEmodeEligibility {
    pub const DISCRIMINATOR: [u8; 8] = [149, 50, 162, 236, 150, 119, 9, 47];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new_readonly(self.governance_admin, true),
            AccountMeta::new(self.bank, false),
            AccountMeta::new(self.same_asset_emode_registry, false),
            AccountMeta::new_readonly(self.instruction_sysvar, false),
        ]
    }
}
impl ToAccountMetas for LendingPoolSetBankSameAssetEmodeEligibility {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (governance_admin only) Opt a bank in/out of same-asset e-mode participation.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_set_bank_same_asset_emode_eligibility(
    accounts: &LendingPoolSetBankSameAssetEmodeEligibility,
    enabled: bool,
) -> Instruction {
    lending_pool_set_bank_same_asset_emode_eligibility_with_program_id(crate::ID, accounts, enabled)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_set_bank_same_asset_emode_eligibility_with_program_id(
    program_id: Pubkey,
    accounts: &LendingPoolSetBankSameAssetEmodeEligibility,
    enabled: bool,
) -> Instruction {
    let mut data = LendingPoolSetBankSameAssetEmodeEligibility::DISCRIMINATOR.to_vec();
    enabled
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`lending_pool_set_oracle_price`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingPoolSetOraclePrice {
    pub group: Pubkey,
    pub governance_admin: Pubkey,
    pub bank: Pubkey,
    pub instruction_sysvar: Pubkey,
}
impl LendingPoolSetOraclePrice {
    pub const DISCRIMINATOR: [u8; 8] = [234, 244, 37, 65, 101, 255, 217, 160];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new_readonly(self.governance_admin, true),
            AccountMeta::new(self.bank, false),
            AccountMeta::new_readonly(self.instruction_sysvar, false),
        ]
    }
}
impl ToAccountMetas for LendingPoolSetOraclePrice {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (governance_admin only)
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_set_oracle_price(
    accounts: &LendingPoolSetOraclePrice,
    price: WrappedI80F48,
    setup: u8,
) -> Instruction {
    lending_pool_set_oracle_price_with_program_id(crate::ID, accounts, price, setup)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_set_oracle_price_with_program_id(
    program_id: Pubkey,
    accounts: &LendingPoolSetOraclePrice,
    price: WrappedI80F48,
    setup: u8,
) -> Instruction {
    let mut data = LendingPoolSetOraclePrice::DISCRIMINATOR.to_vec();
    price
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    setup
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`lending_pool_update_fees_destination_account`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingPoolUpdateFeesDestinationAccount {
    pub group: Pubkey,
    pub bank: Pubkey,
    pub admin: Pubkey,
    /// Bank fees will be sent to this account which must be an ATA of the bank's mint.
    pub destination_account: Pubkey,
    pub instruction_sysvar: Pubkey,
}
impl LendingPoolUpdateFeesDestinationAccount {
    pub const DISCRIMINATOR: [u8; 8] = [102, 4, 121, 243, 237, 110, 95, 13];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new(self.bank, false),
            AccountMeta::new_readonly(self.admin, true),
            AccountMeta::new_readonly(self.destination_account, false),
            AccountMeta::new_readonly(self.instruction_sysvar, false),
        ]
    }
}
impl ToAccountMetas for LendingPoolUpdateFeesDestinationAccount {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (admin only) Set the destination wallet for permissionless fee withdrawals.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_update_fees_destination_account(
    accounts: &LendingPoolUpdateFeesDestinationAccount,
) -> Instruction {
    lending_pool_update_fees_destination_account_with_program_id(crate::ID, accounts)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_update_fees_destination_account_with_program_id(
    program_id: Pubkey,
    accounts: &LendingPoolUpdateFeesDestinationAccount,
) -> Instruction {
    let data = LendingPoolUpdateFeesDestinationAccount::DISCRIMINATOR.to_vec();
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`lending_pool_withdraw_fees`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingPoolWithdrawFees {
    pub group: Pubkey,
    pub bank: Pubkey,
    pub admin: Pubkey,
    pub fee_vault: Pubkey,
    pub fee_vault_authority: Pubkey,
    pub dst_token_account: Pubkey,
    pub token_program: Pubkey,
    pub instruction_sysvar: Pubkey,
}
impl LendingPoolWithdrawFees {
    pub const DISCRIMINATOR: [u8; 8] = [92, 140, 215, 254, 170, 0, 83, 174];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new_readonly(self.bank, false),
            AccountMeta::new_readonly(self.admin, true),
            AccountMeta::new(self.fee_vault, false),
            AccountMeta::new_readonly(self.fee_vault_authority, false),
            AccountMeta::new(self.dst_token_account, false),
            AccountMeta::new_readonly(self.token_program, false),
            AccountMeta::new_readonly(self.instruction_sysvar, false),
        ]
    }
}
impl ToAccountMetas for LendingPoolWithdrawFees {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (admin only) Withdraw collected group fees from the fee vault.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_withdraw_fees(accounts: &LendingPoolWithdrawFees, amount: u64) -> Instruction {
    lending_pool_withdraw_fees_with_program_id(crate::ID, accounts, amount)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_withdraw_fees_with_program_id(
    program_id: Pubkey,
    accounts: &LendingPoolWithdrawFees,
    amount: u64,
) -> Instruction {
    let mut data = LendingPoolWithdrawFees::DISCRIMINATOR.to_vec();
    amount
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`lending_pool_withdraw_fees_permissionless`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingPoolWithdrawFeesPermissionless {
    pub group: Pubkey,
    pub bank: Pubkey,
    pub fee_vault: Pubkey,
    pub fee_vault_authority: Pubkey,
    pub fees_destination_account: Pubkey,
    pub token_program: Pubkey,
}
impl LendingPoolWithdrawFeesPermissionless {
    pub const DISCRIMINATOR: [u8; 8] = [57, 245, 1, 208, 130, 18, 145, 113];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new_readonly(self.bank, false),
            AccountMeta::new(self.fee_vault, false),
            AccountMeta::new_readonly(self.fee_vault_authority, false),
            AccountMeta::new(self.fees_destination_account, false),
            AccountMeta::new_readonly(self.token_program, false),
        ]
    }
}
impl ToAccountMetas for LendingPoolWithdrawFeesPermissionless {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (permissionless) Withdraw group fees to the pre-configured `fees_destination_account`.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_withdraw_fees_permissionless(
    accounts: &LendingPoolWithdrawFeesPermissionless,
    amount: u64,
) -> Instruction {
    lending_pool_withdraw_fees_permissionless_with_program_id(crate::ID, accounts, amount)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_withdraw_fees_permissionless_with_program_id(
    program_id: Pubkey,
    accounts: &LendingPoolWithdrawFeesPermissionless,
    amount: u64,
) -> Instruction {
    let mut data = LendingPoolWithdrawFeesPermissionless::DISCRIMINATOR.to_vec();
    amount
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`lending_pool_withdraw_insurance`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingPoolWithdrawInsurance {
    pub group: Pubkey,
    pub bank: Pubkey,
    pub admin: Pubkey,
    pub insurance_vault: Pubkey,
    pub insurance_vault_authority: Pubkey,
    pub dst_token_account: Pubkey,
    pub token_program: Pubkey,
    pub instruction_sysvar: Pubkey,
}
impl LendingPoolWithdrawInsurance {
    pub const DISCRIMINATOR: [u8; 8] = [108, 60, 60, 246, 104, 79, 159, 243];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new_readonly(self.bank, false),
            AccountMeta::new_readonly(self.admin, true),
            AccountMeta::new(self.insurance_vault, false),
            AccountMeta::new_readonly(self.insurance_vault_authority, false),
            AccountMeta::new(self.dst_token_account, false),
            AccountMeta::new_readonly(self.token_program, false),
            AccountMeta::new_readonly(self.instruction_sysvar, false),
        ]
    }
}
impl ToAccountMetas for LendingPoolWithdrawInsurance {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (admin only) Withdraw from the insurance vault.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_withdraw_insurance(
    accounts: &LendingPoolWithdrawInsurance,
    amount: u64,
) -> Instruction {
    lending_pool_withdraw_insurance_with_program_id(crate::ID, accounts, amount)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_withdraw_insurance_with_program_id(
    program_id: Pubkey,
    accounts: &LendingPoolWithdrawInsurance,
    amount: u64,
) -> Instruction {
    let mut data = LendingPoolWithdrawInsurance::DISCRIMINATOR.to_vec();
    amount
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`marginfi_group_configure`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarginfiGroupConfigure {
    pub marginfi_group: Pubkey,
    pub admin: Pubkey,
    pub instruction_sysvar: Pubkey,
}
impl MarginfiGroupConfigure {
    pub const DISCRIMINATOR: [u8; 8] = [62, 199, 81, 78, 33, 13, 236, 61];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.marginfi_group, false),
            AccountMeta::new_readonly(self.admin, true),
            AccountMeta::new_readonly(self.instruction_sysvar, false),
        ]
    }
}
impl ToAccountMetas for MarginfiGroupConfigure {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// Configure fast-admin group roles. `None` leaves a field unchanged.
///
/// Note: `new_emissions_admin` is deprecated and currently has no on-chain effect.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn marginfi_group_configure(
    accounts: &MarginfiGroupConfigure,
    new_admin: Option<Pubkey>,
    new_curve_admin: Option<Pubkey>,
    new_limit_admin: Option<Pubkey>,
    new_flow_admin: Option<Pubkey>,
    new_emissions_admin: Option<Pubkey>,
    new_metadata_admin: Option<Pubkey>,
) -> Instruction {
    marginfi_group_configure_with_program_id(
        crate::ID,
        accounts,
        new_admin,
        new_curve_admin,
        new_limit_admin,
        new_flow_admin,
        new_emissions_admin,
        new_metadata_admin,
    )
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn marginfi_group_configure_with_program_id(
    program_id: Pubkey,
    accounts: &MarginfiGroupConfigure,
    new_admin: Option<Pubkey>,
    new_curve_admin: Option<Pubkey>,
    new_limit_admin: Option<Pubkey>,
    new_flow_admin: Option<Pubkey>,
    new_emissions_admin: Option<Pubkey>,
    new_metadata_admin: Option<Pubkey>,
) -> Instruction {
    let mut data = MarginfiGroupConfigure::DISCRIMINATOR.to_vec();
    new_admin
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    new_curve_admin
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    new_limit_admin
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    new_flow_admin
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    new_emissions_admin
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    new_metadata_admin
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`marginfi_group_configure_gov`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarginfiGroupConfigureGov {
    pub marginfi_group: Pubkey,
    pub governance_admin: Pubkey,
    pub instruction_sysvar: Pubkey,
}
impl MarginfiGroupConfigureGov {
    pub const DISCRIMINATOR: [u8; 8] = [3, 33, 122, 31, 239, 33, 35, 5];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.marginfi_group, false),
            AccountMeta::new_readonly(self.governance_admin, true),
            AccountMeta::new_readonly(self.instruction_sysvar, false),
        ]
    }
}
impl ToAccountMetas for MarginfiGroupConfigureGov {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// Configure slow, timelocked governance group roles, the fast admin, and e-mode leverage
/// caps. `None` leaves a field unchanged.
/// Same-asset emode leverage is disabled by configuring both init and maint leverage to `1`;
/// values below `1`, including `0`, are invalid.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn marginfi_group_configure_gov(
    accounts: &MarginfiGroupConfigureGov,
    new_admin: Option<Pubkey>,
    new_emode_admin: Option<Pubkey>,
    new_risk_admin: Option<Pubkey>,
    emode_max_init_leverage: Option<WrappedI80F48>,
    emode_max_maint_leverage: Option<WrappedI80F48>,
    same_asset_emode_init_leverage: Option<WrappedI80F48>,
    same_asset_emode_maint_leverage: Option<WrappedI80F48>,
) -> Instruction {
    marginfi_group_configure_gov_with_program_id(
        crate::ID,
        accounts,
        new_admin,
        new_emode_admin,
        new_risk_admin,
        emode_max_init_leverage,
        emode_max_maint_leverage,
        same_asset_emode_init_leverage,
        same_asset_emode_maint_leverage,
    )
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn marginfi_group_configure_gov_with_program_id(
    program_id: Pubkey,
    accounts: &MarginfiGroupConfigureGov,
    new_admin: Option<Pubkey>,
    new_emode_admin: Option<Pubkey>,
    new_risk_admin: Option<Pubkey>,
    emode_max_init_leverage: Option<WrappedI80F48>,
    emode_max_maint_leverage: Option<WrappedI80F48>,
    same_asset_emode_init_leverage: Option<WrappedI80F48>,
    same_asset_emode_maint_leverage: Option<WrappedI80F48>,
) -> Instruction {
    let mut data = MarginfiGroupConfigureGov::DISCRIMINATOR.to_vec();
    new_admin
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    new_emode_admin
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    new_risk_admin
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    emode_max_init_leverage
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    emode_max_maint_leverage
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    same_asset_emode_init_leverage
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    same_asset_emode_maint_leverage
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`marginfi_group_initialize`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarginfiGroupInitialize {
    pub marginfi_group: Pubkey,
    pub admin: Pubkey,
    pub fee_state: Pubkey,
    pub system_program: Pubkey,
    pub instruction_sysvar: Pubkey,
}
impl MarginfiGroupInitialize {
    pub const DISCRIMINATOR: [u8; 8] = [255, 67, 67, 26, 94, 31, 34, 20];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.marginfi_group, true),
            AccountMeta::new(self.admin, true),
            AccountMeta::new_readonly(self.fee_state, false),
            AccountMeta::new_readonly(self.system_program, false),
            AccountMeta::new_readonly(self.instruction_sysvar, false),
        ]
    }
}
impl ToAccountMetas for MarginfiGroupInitialize {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (admin only) Initialize a new marginfi group. The signer becomes the group admin.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn marginfi_group_initialize(accounts: &MarginfiGroupInitialize) -> Instruction {
    marginfi_group_initialize_with_program_id(crate::ID, accounts)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn marginfi_group_initialize_with_program_id(
    program_id: Pubkey,
    accounts: &MarginfiGroupInitialize,
) -> Instruction {
    let data = MarginfiGroupInitialize::DISCRIMINATOR.to_vec();
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`marginfi_group_set_governance_admin`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarginfiGroupSetGovernanceAdmin {
    pub marginfi_group: Pubkey,
    pub signer: Pubkey,
    pub instruction_sysvar: Pubkey,
}
impl MarginfiGroupSetGovernanceAdmin {
    pub const DISCRIMINATOR: [u8; 8] = [87, 57, 22, 85, 152, 57, 23, 75];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.marginfi_group, false),
            AccountMeta::new_readonly(self.signer, true),
            AccountMeta::new_readonly(self.instruction_sysvar, false),
        ]
    }
}
impl ToAccountMetas for MarginfiGroupSetGovernanceAdmin {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// Bootstrap or rotate the slow governance admin. When the stored value is zero on a resized
/// legacy group, the fast admin may bootstrap it once; thereafter only the current governance
/// admin may rotate it.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn marginfi_group_set_governance_admin(
    accounts: &MarginfiGroupSetGovernanceAdmin,
    new_governance_admin: Pubkey,
) -> Instruction {
    marginfi_group_set_governance_admin_with_program_id(crate::ID, accounts, new_governance_admin)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn marginfi_group_set_governance_admin_with_program_id(
    program_id: Pubkey,
    accounts: &MarginfiGroupSetGovernanceAdmin,
    new_governance_admin: Pubkey,
) -> Instruction {
    let mut data = MarginfiGroupSetGovernanceAdmin::DISCRIMINATOR.to_vec();
    new_governance_admin
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`sync_indexer_flags`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyncIndexerFlags {
    pub payer: Pubkey,
}
impl SyncIndexerFlags {
    pub const DISCRIMINATOR: [u8; 8] = [171, 146, 145, 43, 190, 175, 9, 32];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![AccountMeta::new(self.payer, true)]
    }
}
impl ToAccountMetas for SyncIndexerFlags {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (Permissionless) Batch-sync balance-derived indexer flags for existing accounts.
/// Pass MarginfiAccounts as writable remaining_accounts.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn sync_indexer_flags(accounts: &SyncIndexerFlags) -> Instruction {
    sync_indexer_flags_with_program_id(crate::ID, accounts)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn sync_indexer_flags_with_program_id(
    program_id: Pubkey,
    accounts: &SyncIndexerFlags,
) -> Instruction {
    let data = SyncIndexerFlags::DISCRIMINATOR.to_vec();
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`write_bank_metadata`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WriteBankMetadata {
    pub group: Pubkey,
    /// Must be initialized. The metadata-to-bank binding is enforced by `metadata.has_one = bank`,
    /// and `bank.has_one = group` ties this bank to the admin's group.
    pub bank: Pubkey,
    pub metadata_admin: Pubkey,
    pub metadata: Pubkey,
    pub instruction_sysvar: Pubkey,
}
impl WriteBankMetadata {
    pub const DISCRIMINATOR: [u8; 8] = [147, 78, 81, 133, 129, 138, 233, 59];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new_readonly(self.bank, false),
            AccountMeta::new(self.metadata_admin, true),
            AccountMeta::new(self.metadata, false),
            AccountMeta::new_readonly(self.instruction_sysvar, false),
        ]
    }
}
impl ToAccountMetas for WriteBankMetadata {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (metadata admin only) Write ticker/description for an initialized bank. The bank account
/// must exist; when its seed is on-chain, the canonical PDA is verified.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn write_bank_metadata(
    accounts: &WriteBankMetadata,
    ticker: Option<Vec<u8>>,
    description: Option<Vec<u8>>,
) -> Instruction {
    write_bank_metadata_with_program_id(crate::ID, accounts, ticker, description)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn write_bank_metadata_with_program_id(
    program_id: Pubkey,
    accounts: &WriteBankMetadata,
    ticker: Option<Vec<u8>>,
    description: Option<Vec<u8>>,
) -> Instruction {
    let mut data = WriteBankMetadata::DISCRIMINATOR.to_vec();
    ticker
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    description
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`write_bank_metadata_pre_init`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WriteBankMetadataPreInit {
    pub group: Pubkey,
    pub bank_mint: Pubkey,
    pub bank: Pubkey,
    pub metadata_admin: Pubkey,
    pub metadata: Pubkey,
    pub instruction_sysvar: Pubkey,
}
impl WriteBankMetadataPreInit {
    pub const DISCRIMINATOR: [u8; 8] = [224, 124, 22, 73, 60, 209, 80, 170];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new_readonly(self.bank_mint, false),
            AccountMeta::new_readonly(self.bank, false),
            AccountMeta::new(self.metadata_admin, true),
            AccountMeta::new(self.metadata, false),
            AccountMeta::new_readonly(self.instruction_sysvar, false),
        ]
    }
}
impl ToAccountMetas for WriteBankMetadataPreInit {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (metadata admin only) Write ticker/description before bank initialization, for canonical
/// seeded banks only.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn write_bank_metadata_pre_init(
    accounts: &WriteBankMetadataPreInit,
    bank_seed: u64,
    ticker: Option<Vec<u8>>,
    description: Option<Vec<u8>>,
) -> Instruction {
    write_bank_metadata_pre_init_with_program_id(
        crate::ID,
        accounts,
        bank_seed,
        ticker,
        description,
    )
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn write_bank_metadata_pre_init_with_program_id(
    program_id: Pubkey,
    accounts: &WriteBankMetadataPreInit,
    bank_seed: u64,
    ticker: Option<Vec<u8>>,
    description: Option<Vec<u8>>,
) -> Instruction {
    let mut data = WriteBankMetadataPreInit::DISCRIMINATOR.to_vec();
    bank_seed
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    ticker
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    description
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}
