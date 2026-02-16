use {
    crate::config::TxMode,
    anyhow::{bail, Result},
    fixed::types::I80F48,
    fixed_macro::types::I80F48,
    log::error,
    marginfi::{bank_authority_seed, bank_seed, state::bank::BankVaultType},
    marginfi_type_crate::{
        constants::{
            EMISSIONS_AUTH_SEED, EMISSIONS_TOKEN_ACCOUNT_SEED, FEE_STATE_SEED, ORDER_SEED,
        },
        types::{Bank, BankConfig, MarginfiAccount},
    },
    solana_client::rpc_client::RpcClient,
    solana_sdk::{
        hash::hashv, instruction::AccountMeta, pubkey::Pubkey, signature::Signature,
        transaction::Transaction,
    },
    std::collections::HashMap,
};

pub fn process_transaction(
    tx: &Transaction,
    rpc_client: &RpcClient,
    tx_mode: TxMode,
) -> Result<Signature> {
    match tx_mode {
        TxMode::DryRun => match rpc_client.simulate_transaction(tx) {
            Ok(response) => {
                println!("------- program logs -------");
                response
                    .value
                    .logs
                    .unwrap()
                    .into_iter()
                    .for_each(|line| println!("{line}"));
                println!("----------------------------");
                Ok(Signature::default())
            }
            Err(err) => bail!(err),
        },
        TxMode::Multisig => {
            let bytes = bincode::serialize(tx)?;
            let tx_size = bytes.len();
            let tx_serialized = bs58::encode(bytes).into_string();

            println!("tx size: {} bytes", tx_size);
            println!("------- transaction -------");
            println!("{}", tx_serialized);
            println!("---------------------------");

            Ok(Signature::default())
        }
        TxMode::Normal => match rpc_client.send_and_confirm_transaction_with_spinner(tx) {
            Ok(sig) => Ok(sig),
            Err(err) => {
                error!("transaction failed: {:?}", err);
                bail!(err);
            }
        },
    }
}

pub fn bank_to_oracle_key(bank_config: &BankConfig) -> Pubkey {
    *bank_config.oracle_keys.first().unwrap()
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

pub fn find_bank_emssions_auth_pda(
    bank: Pubkey,
    emissions_mint: Pubkey,
    program_id: Pubkey,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            EMISSIONS_AUTH_SEED.as_bytes(),
            bank.as_ref(),
            emissions_mint.as_ref(),
        ],
        &program_id,
    )
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

pub fn load_observation_account_metas(
    marginfi_account: &MarginfiAccount,
    banks_map: &HashMap<Pubkey, Bank>,
    include_banks: Vec<Pubkey>,
    exclude_banks: Vec<Pubkey>,
) -> Vec<AccountMeta> {
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

    // Sort all bank_pks in descending order
    bank_pks.sort_by(|a, b| b.cmp(a));

    let mut banks = vec![];
    for bank_pk in bank_pks.clone() {
        let bank = banks_map.get(&bank_pk).unwrap();
        banks.push(bank);
    }

    let account_metas = banks
        .iter()
        .zip(bank_pks.iter())
        .flat_map(|(bank, bank_pk)| {
            let oracle_key = bank_to_oracle_key(&bank.config);

            vec![
                AccountMeta {
                    pubkey: *bank_pk,
                    is_signer: false,
                    is_writable: false,
                },
                AccountMeta {
                    pubkey: oracle_key,
                    is_signer: false,
                    is_writable: false,
                },
            ]
        })
        .collect::<Vec<_>>();
    account_metas
}

pub fn calc_emissions_rate(ui_rate: f64, emissions_mint_decimals: u8) -> u64 {
    (ui_rate * 10u64.pow(emissions_mint_decimals as u32) as f64) as u64
}

pub fn ui_to_native(ui_amount: f64, decimals: u8) -> u64 {
    (ui_amount * (10u64.pow(decimals as u32) as f64)) as u64
}

#[allow(dead_code)]
pub fn native_to_ui(native_amount: u64, decimals: u8) -> f64 {
    native_amount as f64 / 10u64.pow(decimals as u32) as f64
}
