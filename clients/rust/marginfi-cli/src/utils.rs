use {
    crate::config::{Config, TxMode},
    anyhow::{bail, Context, Result},
    fixed::types::I80F48,
    fixed_macro::types::I80F48,
    marginfi::{bank_authority_seed, bank_seed, state::bank::BankVaultType},
    marginfi_type_crate::{
        constants::{
            EMISSIONS_TOKEN_ACCOUNT_SEED, EXECUTE_ORDER_SEED, FEE_STATE_SEED,
            LIQUIDATION_RECORD_SEED, ORDER_SEED,
        },
        types::{Bank, MarginfiAccount, OracleSetup},
    },
    solana_client::rpc_client::RpcClient,
    solana_sdk::{
        address_lookup_table::{state::AddressLookupTable, AddressLookupTableAccount},
        compute_budget::ComputeBudgetInstruction,
        hash::hashv,
        instruction::{AccountMeta, Instruction},
        message::{v0, VersionedMessage},
        pubkey::Pubkey,
        signature::{Keypair, Signature},
        transaction::{Transaction, VersionedTransaction},
    },
    std::collections::HashMap,
};

/// Simulate a transaction before sending. Logs program output.
/// Returns the estimated compute units consumed on success.
fn simulate_and_log(rpc_client: &RpcClient, tx: &Transaction) -> Result<u64> {
    let sim_result = rpc_client.simulate_transaction(tx)?;

    if let Some(logs) = &sim_result.value.logs {
        println!("------- program logs -------");
        for line in logs {
            println!("{line}");
        }
        println!("----------------------------");
    }

    if let Some(err) = sim_result.value.err {
        bail!("Simulation failed: {err}");
    }

    Ok(sim_result.value.units_consumed.unwrap_or(200_000))
}

/// Simulate a versioned transaction before sending. Logs program output.
#[allow(dead_code)]
fn simulate_versioned_and_log(rpc_client: &RpcClient, tx: &VersionedTransaction) -> Result<u64> {
    let sim_result = rpc_client.simulate_transaction(tx)?;

    if let Some(logs) = &sim_result.value.logs {
        println!("------- program logs -------");
        for line in logs {
            println!("{line}");
        }
        println!("----------------------------");
    }

    if let Some(err) = sim_result.value.err {
        bail!("Simulation failed: {err}");
    }

    Ok(sim_result.value.units_consumed.unwrap_or(200_000))
}

/// Output an unsigned transaction as base58 for Squads multisig.
fn output_multisig_tx(tx: &VersionedTransaction) -> Result<Signature> {
    let bytes = bincode::serialize(tx)?;
    let tx_size = bytes.len();
    let tx_serialized = bs58::encode(&bytes).into_string();

    println!("tx size: {} bytes", tx_size);
    println!("------- transaction (base58) -------");
    println!("{}", tx_serialized);
    println!("------------------------------------");

    Ok(Signature::default())
}

/// Output an unsigned legacy transaction as base58 for Squads multisig.
fn output_multisig_legacy_tx(tx: &Transaction) -> Result<Signature> {
    let bytes = bincode::serialize(tx)?;
    let tx_size = bytes.len();
    let tx_serialized = bs58::encode(&bytes).into_string();

    println!("tx size: {} bytes", tx_size);
    println!("------- transaction (base58) -------");
    println!("{}", tx_serialized);
    println!("------------------------------------");

    Ok(Signature::default())
}

fn load_lookup_tables(
    rpc_client: &RpcClient,
    lookup_tables: &[Pubkey],
) -> Result<Vec<AddressLookupTableAccount>> {
    let mut out = Vec::with_capacity(lookup_tables.len());

    for lut_pk in lookup_tables {
        let account = rpc_client
            .get_account(lut_pk)
            .with_context(|| format!("failed to fetch lookup table account {}", lut_pk))?;

        let lut = AddressLookupTable::deserialize(&account.data)
            .map_err(|e| anyhow::anyhow!("failed to deserialize lookup table {}: {}", lut_pk, e))?;

        out.push(AddressLookupTableAccount {
            key: *lut_pk,
            addresses: lut.addresses.to_vec(),
        });
    }

    Ok(out)
}

/// Build, simulate, and either output unsigned base58 (default) or sign and send (--send-tx).
///
/// Flow:
/// 1. Always simulate first — on failure, log program output and abort
/// 2. If `--send-tx`: sign and broadcast
/// 3. Otherwise (default): serialize unsigned tx as base58 for Squads multisig
///
/// Compute budget: if `config.compute_unit_limit` is set, uses that. Otherwise,
/// uses the simulation result (1.4x consumed, minimum 50_000).
pub fn send_tx(config: &Config, ixs: Vec<Instruction>, signers: &[&Keypair]) -> Result<Signature> {
    let rpc_client = config.mfi_program.rpc();
    let payer = config.explicit_fee_payer();
    let blockhash = rpc_client.get_latest_blockhash()?;

    // Step 1: Simulate to estimate CU and validate the transaction
    let sim_tx = Transaction::new_signed_with_payer(&ixs, Some(&payer), signers, blockhash);
    let consumed_cu = simulate_and_log(&rpc_client, &sim_tx)?;

    let cu_limit = config
        .compute_unit_limit
        .unwrap_or_else(|| ((consumed_cu as f64 * 1.4) as u32).max(50_000));
    let cu_price = config.compute_unit_price.unwrap_or(1);

    let mut final_ixs = vec![
        ComputeBudgetInstruction::set_compute_unit_price(cu_price),
        ComputeBudgetInstruction::set_compute_unit_limit(cu_limit),
    ];
    final_ixs.extend(ixs);

    // Re-fetch blockhash for the actual transaction
    let blockhash = rpc_client.get_latest_blockhash()?;
    let tx_mode = config.get_tx_mode();

    if config.legacy_tx {
        match tx_mode {
            TxMode::SendTx => {
                let tx =
                    Transaction::new_signed_with_payer(&final_ixs, Some(&payer), signers, blockhash);
                let sig = rpc_client.send_and_confirm_transaction_with_spinner(&tx)?;
                println!("Transaction confirmed: {sig}");
                return Ok(sig);
            }
            TxMode::MultisigOutput => {
                let tx =
                    Transaction::new_signed_with_payer(&final_ixs, Some(&payer), signers, blockhash);
                return output_multisig_legacy_tx(&tx);
            }
        }
    }

    let lookup_tables = load_lookup_tables(&rpc_client, &config.lookup_tables)?;
    let v0_message = v0::Message::try_compile(&payer, &final_ixs, &lookup_tables, blockhash)?;

    match tx_mode {
        TxMode::MultisigOutput => {
            let num_required_signatures = v0_message.header.num_required_signatures as usize;
            let vtx = VersionedTransaction {
                signatures: vec![Signature::default(); num_required_signatures],
                message: VersionedMessage::V0(v0_message),
            };
            output_multisig_tx(&vtx)
        }
        TxMode::SendTx => {
            let vtx =
                VersionedTransaction::try_new(VersionedMessage::V0(v0_message), signers)?;
            let sig = rpc_client.send_and_confirm_transaction_with_spinner(&vtx)?;
            println!("Transaction confirmed: {sig}");
            Ok(sig)
        }
    }
}

pub fn find_bank_vault_pda(
    bank_pk: &Pubkey,
    vault_type: BankVaultType,
    program_id: &Pubkey,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(bank_seed!(vault_type, bank_pk), program_id)
}

pub fn find_bank_vault_authority_pda(
    bank_pk: &Pubkey,
    vault_type: BankVaultType,
    program_id: &Pubkey,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(bank_authority_seed!(vault_type, bank_pk), program_id)
}

pub fn find_bank_emssions_token_account_pda(
    bank: Pubkey,
    emissions_mint: Pubkey,
    program_id: Pubkey,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            EMISSIONS_TOKEN_ACCOUNT_SEED.as_bytes(),
            bank.as_ref(),
            emissions_mint.as_ref(),
        ],
        &program_id,
    )
}

pub fn find_fee_state_pda(program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[FEE_STATE_SEED.as_bytes()], program_id)
}

pub fn find_order_pda(
    marginfi_account: &Pubkey,
    bank_keys: &[Pubkey],
    program_id: &Pubkey,
) -> (Pubkey, u8) {
    let mut slices: Vec<&[u8]> = bank_keys.iter().map(|pk| pk.as_ref()).collect();
    slices.sort_unstable();
    let bank_keys_hash = hashv(&slices).to_bytes();

    Pubkey::find_program_address(
        &[
            ORDER_SEED.as_bytes(),
            marginfi_account.as_ref(),
            &bank_keys_hash,
        ],
        program_id,
    )
}

pub fn find_execute_order_pda(order: &Pubkey, program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[EXECUTE_ORDER_SEED.as_bytes(), order.as_ref()], program_id)
}

pub fn find_liquidation_record_pda(marginfi_account: &Pubkey, program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            LIQUIDATION_RECORD_SEED.as_bytes(),
            marginfi_account.as_ref(),
        ],
        program_id,
    )
}

// ---------------------------------------------------------------------------
// JupLend PDA derivation
// ---------------------------------------------------------------------------

pub const JUPLEND_LENDING_PROGRAM_ID: Pubkey =
    solana_sdk::pubkey!("jup3YeL8QhtSx1e253b2FDvsMNC87fDrgQZivbrndc9");
pub const JUPLEND_LIQUIDITY_PROGRAM_ID: Pubkey =
    solana_sdk::pubkey!("jupeiUmn818Jg1ekPURTpr4mFo29p46vygyykFJ3wZC");
pub const JUPLEND_REWARDS_PROGRAM_ID: Pubkey =
    solana_sdk::pubkey!("jup7TthsMgcR9Y3L277b8Eo9uboVSmu1utkuXHNUKar");

/// `["lending_admin"]` on JupLend Lending program
pub fn find_juplend_lending_admin() -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"lending_admin"], &JUPLEND_LENDING_PROGRAM_ID)
}

/// `["rate_model", mint]` on JupLend Liquidity program
pub fn find_juplend_rate_model(mint: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"rate_model", mint.as_ref()], &JUPLEND_LIQUIDITY_PROGRAM_ID)
}

/// `["liquidity"]` on JupLend Liquidity program
pub fn find_juplend_liquidity() -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"liquidity"], &JUPLEND_LIQUIDITY_PROGRAM_ID)
}

/// ATA of the liquidity PDA for the given mint
pub fn find_juplend_vault(mint: &Pubkey, token_program: &Pubkey) -> Pubkey {
    let (liquidity, _) = find_juplend_liquidity();
    anchor_spl::associated_token::get_associated_token_address_with_program_id(
        &liquidity,
        mint,
        token_program,
    )
}

/// `["user_claim", user, mint]` on JupLend Liquidity program
pub fn find_juplend_claim_account(user: &Pubkey, mint: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[b"user_claim", user.as_ref(), mint.as_ref()],
        &JUPLEND_LIQUIDITY_PROGRAM_ID,
    )
}

/// `["lending_rewards_rate_model", mint]` on JupLend Rewards program
pub fn find_juplend_rewards_rate_model(mint: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[b"lending_rewards_rate_model", mint.as_ref()],
        &JUPLEND_REWARDS_PROGRAM_ID,
    )
}

/// Load the JupLend Lending account from `integration_acc_1` on a bank,
/// and extract the accounts needed for CPI calls.
pub struct JuplendCpiAccounts {
    pub f_token_mint: Pubkey,
    pub lending_admin: Pubkey,
    pub supply_token_reserves_liquidity: Pubkey,
    pub lending_supply_position_on_liquidity: Pubkey,
    pub rate_model: Pubkey,
    pub vault: Pubkey,
    pub liquidity: Pubkey,
    pub liquidity_program: Pubkey,
    pub rewards_rate_model: Pubkey,
    /// Only needed for withdraw
    pub claim_account: Pubkey,
}

/// Derive all JupLend CPI accounts from a bank's integration_acc_1.
/// Fetches the Lending account from RPC and derives all PDAs.
pub fn derive_juplend_cpi_accounts(
    rpc_client: &solana_client::rpc_client::RpcClient,
    bank: &Bank,
    liquidity_vault_authority: &Pubkey,
) -> Result<JuplendCpiAccounts> {
    use juplend_mocks::state::Lending;

    let lending_data = rpc_client.get_account_data(&bank.integration_acc_1)?;

    // Skip 8-byte discriminator for zero-copy deserialization
    if lending_data.len() < 8 + std::mem::size_of::<Lending>() {
        anyhow::bail!(
            "JupLend Lending account {} data too small ({} bytes)",
            bank.integration_acc_1,
            lending_data.len()
        );
    }
    // Safety: Lending is repr(C, packed), so we can cast from bytes
    let lending: &Lending =
        bytemuck::from_bytes(&lending_data[8..8 + std::mem::size_of::<Lending>()]);

    let bank_mint_account = rpc_client.get_account(&bank.mint)?;
    let token_program = bank_mint_account.owner;

    let (lending_admin, _) = find_juplend_lending_admin();
    let (rate_model, _) = find_juplend_rate_model(&bank.mint);
    let (liquidity, _) = find_juplend_liquidity();
    let vault = find_juplend_vault(&bank.mint, &token_program);
    let (claim_account, _) = find_juplend_claim_account(liquidity_vault_authority, &bank.mint);
    let (rewards_rate_model, _) = find_juplend_rewards_rate_model(&bank.mint);

    Ok(JuplendCpiAccounts {
        f_token_mint: lending.f_token_mint,
        lending_admin,
        supply_token_reserves_liquidity: lending.token_reserves_liquidity,
        lending_supply_position_on_liquidity: lending.supply_position_on_liquidity,
        rate_model,
        vault,
        liquidity,
        liquidity_program: JUPLEND_LIQUIDITY_PROGRAM_ID,
        rewards_rate_model,
        claim_account,
    })
}

pub const EXP_10_I80F48: [I80F48; 15] = [
    I80F48!(1),
    I80F48!(10),
    I80F48!(100),
    I80F48!(1_000),
    I80F48!(10_000),
    I80F48!(100_000),
    I80F48!(1_000_000),
    I80F48!(10_000_000),
    I80F48!(100_000_000),
    I80F48!(1_000_000_000),
    I80F48!(10_000_000_000),
    I80F48!(100_000_000_000),
    I80F48!(1_000_000_000_000),
    I80F48!(10_000_000_000_000),
    I80F48!(100_000_000_000_000),
];

pub fn bank_observation_keys(bank: &Bank) -> Vec<Pubkey> {
    let keys = &bank.config.oracle_keys;

    let mut out = match bank.config.oracle_setup {
        OracleSetup::None | OracleSetup::Fixed => vec![],
        OracleSetup::FixedKamino | OracleSetup::FixedDrift | OracleSetup::FixedJuplend => {
            vec![keys[1]]
        }
        OracleSetup::StakedWithPythPush => vec![keys[0], keys[1], keys[2]],
        OracleSetup::PythLegacy
        | OracleSetup::SwitchboardV2
        | OracleSetup::PythPushOracle
        | OracleSetup::SwitchboardPull => vec![keys[0]],
        OracleSetup::KaminoPythPush
        | OracleSetup::KaminoSwitchboardPull
        | OracleSetup::DriftPythPull
        | OracleSetup::DriftSwitchboardPull
        | OracleSetup::SolendPythPull
        | OracleSetup::SolendSwitchboardPull
        | OracleSetup::JuplendPythPull
        | OracleSetup::JuplendSwitchboardPull => vec![keys[0], keys[1]],
    };

    out.retain(|pk| *pk != Pubkey::default());
    out
}

fn collect_observation_bank_pks(
    marginfi_account: &MarginfiAccount,
    include_banks: Vec<Pubkey>,
    exclude_banks: Vec<Pubkey>,
    close_bank_last: Option<Pubkey>,
) -> Vec<Pubkey> {
    let mut bank_pks = marginfi_account
        .lending_account
        .balances
        .iter()
        .filter_map(|balance| balance.is_active().then_some(balance.bank_pk))
        .collect::<Vec<_>>();

    for bank_pk in include_banks {
        if !bank_pks.contains(&bank_pk) {
            bank_pks.push(bank_pk);
        }
    }

    bank_pks.retain(|bank_pk| !exclude_banks.contains(bank_pk));

    let close_last = close_bank_last.and_then(|close_bank| {
        bank_pks
            .iter()
            .position(|pk| *pk == close_bank)
            .map(|idx| bank_pks.remove(idx))
    });

    // Sort all bank_pks in descending order by raw pubkey bytes.
    bank_pks.sort_by(|a, b| b.cmp(a));

    if let Some(close_bank) = close_last {
        bank_pks.push(close_bank);
    }

    bank_pks
}

fn load_observation_account_metas_impl(
    marginfi_account: &MarginfiAccount,
    banks_map: &HashMap<Pubkey, Bank>,
    include_banks: Vec<Pubkey>,
    exclude_banks: Vec<Pubkey>,
    close_bank_last: Option<Pubkey>,
    banks_writable: bool,
    banks_only: bool,
) -> Vec<AccountMeta> {
    let bank_pks = collect_observation_bank_pks(
        marginfi_account,
        include_banks,
        exclude_banks,
        close_bank_last,
    );

    let mut account_metas = Vec::new();

    for bank_pk in bank_pks {
        let Some(bank) = banks_map.get(&bank_pk) else {
            continue;
        };

        account_metas.push(AccountMeta {
            pubkey: bank_pk,
            is_signer: false,
            is_writable: banks_writable,
        });

        if banks_only {
            continue;
        }

        for oracle_pk in bank_observation_keys(bank) {
            account_metas.push(AccountMeta {
                pubkey: oracle_pk,
                is_signer: false,
                is_writable: false,
            });
        }
    }

    account_metas
}

pub fn load_observation_account_metas(
    marginfi_account: &MarginfiAccount,
    banks_map: &HashMap<Pubkey, Bank>,
    include_banks: Vec<Pubkey>,
    exclude_banks: Vec<Pubkey>,
) -> Vec<AccountMeta> {
    load_observation_account_metas_impl(
        marginfi_account,
        banks_map,
        include_banks,
        exclude_banks,
        None,
        false,
        false,
    )
}

pub fn load_observation_account_metas_close_last(
    marginfi_account: &MarginfiAccount,
    banks_map: &HashMap<Pubkey, Bank>,
    include_banks: Vec<Pubkey>,
    exclude_banks: Vec<Pubkey>,
    close_bank: Pubkey,
) -> Vec<AccountMeta> {
    load_observation_account_metas_impl(
        marginfi_account,
        banks_map,
        include_banks,
        exclude_banks,
        Some(close_bank),
        false,
        false,
    )
}

pub fn load_observation_account_metas_with_bank_writable(
    marginfi_account: &MarginfiAccount,
    banks_map: &HashMap<Pubkey, Bank>,
    include_banks: Vec<Pubkey>,
    exclude_banks: Vec<Pubkey>,
    banks_writable: bool,
) -> Vec<AccountMeta> {
    load_observation_account_metas_impl(
        marginfi_account,
        banks_map,
        include_banks,
        exclude_banks,
        None,
        banks_writable,
        false,
    )
}

pub fn load_observation_bank_only_metas(
    marginfi_account: &MarginfiAccount,
    banks_map: &HashMap<Pubkey, Bank>,
    include_banks: Vec<Pubkey>,
    exclude_banks: Vec<Pubkey>,
    banks_writable: bool,
) -> Vec<AccountMeta> {
    load_observation_account_metas_impl(
        marginfi_account,
        banks_map,
        include_banks,
        exclude_banks,
        None,
        banks_writable,
        true,
    )
}

pub fn load_bank_oracle_account_metas(bank: &Bank) -> Vec<AccountMeta> {
    bank_observation_keys(bank)
        .into_iter()
        .map(|pubkey| AccountMeta::new_readonly(pubkey, false))
        .collect()
}


pub fn ui_to_native(ui_amount: f64, decimals: u8) -> u64 {
    (ui_amount * (10u64.pow(decimals as u32) as f64)) as u64
}
