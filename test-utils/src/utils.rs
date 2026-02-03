use anchor_lang::prelude::*;
use anchor_lang::solana_program::hash::hashv;
use anchor_lang::solana_program::instruction::Instruction;
use anchor_lang::Discriminator;
use anchor_spl::token_2022::spl_token_2022::extension::transfer_fee::MAX_FEE_BASIS_POINTS;
use marginfi::constants::SWITCHBOARD_PULL_ID;
use marginfi_type_crate::constants::EMISSIONS_AUTH_SEED;
use marginfi_type_crate::constants::EMISSIONS_TOKEN_ACCOUNT_SEED;
use marginfi_type_crate::constants::{EXECUTE_ORDER_SEED, ORDER_SEED};
use pyth_solana_receiver_sdk::price_update::FeedId;
use pyth_solana_receiver_sdk::price_update::PriceUpdateV2;
use pyth_solana_receiver_sdk::price_update::VerificationLevel;
use solana_cli_output::CliAccount;
use solana_program_test::*;
use solana_sdk::account::ReadableAccount;
use solana_sdk::account::WritableAccount;
use solana_sdk::{
    account::Account, account::AccountSharedData, pubkey::Pubkey, rent::Rent, signature::Keypair,
};
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use std::str::FromStr;
use std::{cell::RefCell, rc::Rc};

pub const MS_PER_SLOT: u64 = 400;
pub const RUST_LOG_DEFAULT: &str = "solana_rbpf::vm=info,\
             solana_program_runtime::stable_log=debug,\
             solana_runtime::message_processor=debug,\
             solana_runtime::system_instruction_processor=info,\
             solana_program_test=info,\
             solana_bpf_loader_program=debug";

pub async fn load_and_deserialize<T: AccountDeserialize>(
    ctx: Rc<RefCell<ProgramTestContext>>,
    address: &Pubkey,
) -> T {
    // Clone the client so we don't hold a RefCell borrow across await.
    let banks_client = ctx.borrow().banks_client.clone();
    let ai = banks_client.get_account(*address).await.unwrap().unwrap();

    T::try_deserialize(&mut ai.data.as_slice()).unwrap()
}

pub fn make_ix<T>(accounts: T, ix_data: Vec<u8>) -> Instruction
where
    T: ToAccountMetas,
{
    Instruction {
        program_id: marginfi::ID,
        accounts: accounts.to_account_metas(Some(true)),
        data: ix_data,
    }
}

pub fn create_pyth_push_oracle_account_from_bytes(data: Vec<u8>) -> Account {
    Account {
        lamports: 1_000_000,
        data,
        owner: pyth_solana_receiver_sdk::ID,
        executable: false,
        rent_epoch: 361,
    }
}

pub fn create_pyth_push_oracle_account(
    feed_id: FeedId,
    ui_price: f64,
    mint_decimals: i32,
    timestamp: Option<i64>,
    verification_level: VerificationLevel,
) -> Account {
    let native_price = (ui_price * 10_f64.powf(mint_decimals as f64)) as i64;

    let price_update = PriceUpdateV2 {
        write_authority: Pubkey::default(),
        verification_level,
        price_message: pyth_solana_receiver_sdk::price_update::PriceFeedMessage {
            feed_id,
            price: native_price,
            conf: 0,
            exponent: -mint_decimals,
            publish_time: timestamp.unwrap_or_default(),
            prev_publish_time: timestamp.unwrap_or_default(),
            ema_price: native_price,
            ema_conf: 0,
        },
        posted_slot: 1,
    };

    let mut data = vec![];
    let mut account_data = vec![];

    data.extend_from_slice(PriceUpdateV2::DISCRIMINATOR);

    price_update.serialize(&mut account_data).unwrap();

    data.extend_from_slice(&account_data);

    create_pyth_push_oracle_account_from_bytes(data)
}

pub fn create_switch_pull_oracle_account_from_bytes(data: Vec<u8>) -> Account {
    Account {
        lamports: 1_000_000,
        data,
        owner: SWITCHBOARD_PULL_ID,
        executable: false,
        rent_epoch: 361,
    }
}

#[macro_export]
macro_rules! assert_custom_error {
    ($error:expr, $matcher:expr) => {
        match $error {
            // direct transaction error
            solana_program_test::BanksClientError::TransactionError(
                solana_sdk::transaction::TransactionError::InstructionError(
                    _,
                    anchor_lang::solana_program::instruction::InstructionError::Custom(n),
                ),
            )
            // simulation (preflight) error
            | solana_program_test::BanksClientError::SimulationError {
                err: solana_sdk::transaction::TransactionError::InstructionError(
                    _,
                    anchor_lang::solana_program::instruction::InstructionError::Custom(n),
                ),
                ..
            } => {
                let expected = anchor_lang::error::ERROR_CODE_OFFSET + $matcher as u32;
                assert_eq!(n, expected);
            }
            other => panic!("expected custom error, got {:?}", other),
        }
    };
}

#[macro_export]
macro_rules! assert_anchor_error {
    ($error:expr, $matcher:expr) => {
        match $error {
            // direct transaction error
            solana_program_test::BanksClientError::TransactionError(
                solana_sdk::transaction::TransactionError::InstructionError(
                    _,
                    anchor_lang::solana_program::instruction::InstructionError::Custom(n),
                ),
            )
            // simulation (preflight) failure
            | solana_program_test::BanksClientError::SimulationError {
                err: solana_sdk::transaction::TransactionError::InstructionError(
                    _,
                    anchor_lang::solana_program::instruction::InstructionError::Custom(n),
                ),
                ..
            } => {
                assert_eq!(n, $matcher as u32);
            }
            other => panic!("expected anchor error {:?}, got {:?}", $matcher, other),
        }
    };
}

#[macro_export]
macro_rules! assert_program_error {
    ($error:expr, $matcher:expr) => {
        match $error {
            solana_sdk::transport::TransportError::TransactionError(
                solana_sdk::transaction::InstructionError(_, x),
            ) => {
                assert_eq!(x, $matcher)
            }
            _ => assert!(false),
        };
    };
}

#[macro_export]
macro_rules! assert_eq_noise {
    ($a:expr, $b:expr, $tolerance:expr) => {
        let diff = ($a - $b).abs();
        assert!(
            diff <= $tolerance,
            "Difference between {} and {} larger than {} tolerated",
            $a,
            $b,
            $tolerance
        )
    };

    ($a:expr, $b:expr) => {
        let tolerance = fixed_macro::types::I80F48!(0.00001);
        let diff = ($a - $b).abs();
        assert!(
            diff < tolerance,
            "Difference between {} and {} larger than {} tolerated",
            $a,
            $b,
            tolerance
        )
    };
}

#[macro_export]
macro_rules! ui_to_native {
    ($val: expr, $mint_decimals: expr) => {
        ($val * 10_u64.pow($mint_decimals as u32) as f64) as u64
    };
}

#[macro_export]
macro_rules! native {
    ($val: expr, "USDC") => {
        $val * 10_u64.pow(6)
    };

    ($val: expr, "USDC", f64) => {
        (($val) * 10_u64.pow(6) as f64) as u64
    };

    ($val: expr, "PYUSD") => {
        $val * 10_u64.pow(6)
    };

    ($val: expr, "PYUSD", f64) => {
        (($val) * 10_u64.pow(6) as f64) as u64
    };
    ($val: expr, "T22_WITH_FEE") => {
        $val * 10_u64.pow(6)
    };

    ($val: expr, "T22_WITH_FEE", f64) => {
        (($val) * 10_u64.pow(6) as f64) as u64
    };

    ($val: expr, "SOL") => {
        $val * 10_u64.pow(9)
    };

    ($val: expr, "SOL", f64) => {
        (($val) * 10_u64.pow(9) as f64) as u64
    };

    ($val: expr, "SOL_EQ") => {
        $val * 10_u64.pow(9)
    };

    ($val: expr, "SOL_EQ", f64) => {
        (($val) * 10_u64.pow(9) as f64) as u64
    };

    ($val: expr, "MNDE") => {
        $val * 10_u64.pow(9)
    };

    ($val: expr, "MNDE", f64) => {
        (($val) * 10_u64.pow(9) as f64) as u64
    };

    ($val: expr, "SOL_EQ_ISO") => {
        $val * 10_u64.pow(9)
    };

    ($val: expr, "SOL_EQ_ISO", f64) => {
        (($val) * 10_u64.pow(9) as f64) as u64
    };

    ($val: expr, "FIXED") => {
        $val * 10_u64.pow(6)
    };

    ($val: expr, "FIXED", f64) => {
        (($val) * 10_u64.pow(6) as f64) as u64
    };

    ($val: expr, "FIXED_LOW") => {
        $val * 10_u64.pow(9)
    };

    ($val: expr, "FIXED_LOW", f64) => {
        (($val) * 10_u64.pow(9) as f64) as u64
    };

    ($val: expr, $decimals: expr) => {
        $val * 10_u64.pow($decimals as u32)
    };

    ($val: expr, $decimals: expr, f64) => {
        (($val) * 10_u64.pow($decimals as u32) as f64) as u64
    };
}

#[macro_export]
macro_rules! time {
    ($val: expr) => {
        $val
    };

    ($val: expr, "s") => {
        $val
    };

    ($val: expr, "m") => {
        $val * 60
    };

    ($val: expr, "h") => {
        $val * 60 * 60
    };

    ($val: expr, "d") => {
        $val * 60 * 60 * 24
    };

    ($val: expr, "w") => {
        $val * 60 * 60 * 24 * 7
    };

    ($val: expr, "y") => {
        $val * 60 * 60 * 24 * 365
    };

    ($val: expr, "M") => {
        $val * 60 * 60 * 24 * 30
    };
}

#[macro_export]
macro_rules! f_native {
    ($val: expr) => {
        I80F48::from_num($val * 10_u64.pow(6))
    };
}

pub fn clone_keypair(keypair: &Keypair) -> Keypair {
    Keypair::from_bytes(&keypair.to_bytes()).unwrap()
}

pub fn get_emissions_authority_address(bank_pk: Pubkey, emissions_mint: Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            EMISSIONS_AUTH_SEED.as_bytes(),
            bank_pk.as_ref(),
            emissions_mint.as_ref(),
        ],
        &marginfi::ID,
    )
}

pub fn get_emissions_token_account_address(
    bank_pk: Pubkey,
    emissions_mint: Pubkey,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            EMISSIONS_TOKEN_ACCOUNT_SEED.as_bytes(),
            bank_pk.as_ref(),
            emissions_mint.as_ref(),
        ],
        &marginfi::ID,
    )
}

pub fn get_max_deposit_amount_pre_fee(amount: f64) -> f64 {
    amount * (1f64 + MAX_FEE_BASIS_POINTS as f64 / 10_000f64)
}

pub fn get_sufficient_collateral_for_outflow(
    target_outflow: f64,
    collateral_mint_price: f64,
    outflow_mint_price: f64,
) -> f64 {
    target_outflow * outflow_mint_price / collateral_mint_price
}

pub fn load_account_from_file(relative_path: &str) -> (Pubkey, AccountSharedData) {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push(relative_path);

    let mut f = File::open(&path).expect("file not found");
    let mut raw = String::new();
    f.read_to_string(&mut raw).expect("file not readable");

    let cli: CliAccount = serde_json::from_str(&raw).expect("invalid account format");
    let address = Pubkey::from_str(&cli.keyed_account.pubkey).unwrap();
    let mut acc: AccountSharedData = cli.keyed_account.account.decode().unwrap();

    let need = Rent::default().minimum_balance(acc.data().len());
    if acc.lamports() < need {
        acc.set_lamports(need);
    }
    (address, acc)
}

pub fn keys_sha256_hash(keys: &[Pubkey]) -> [u8; 32] {
    let mut slices: Vec<&[u8]> = keys.iter().map(|pk| pk.as_ref()).collect();
    slices.sort_unstable();
    hashv(&slices).to_bytes()
}

pub fn find_order_pda(marginfi_account: &Pubkey, bank_keys: &[Pubkey]) -> (Pubkey, u8) {
    let hash = keys_sha256_hash(bank_keys);
    Pubkey::find_program_address(
        &[ORDER_SEED.as_bytes(), marginfi_account.as_ref(), &hash],
        &marginfi::ID,
    )
}

pub fn find_execute_order_pda(order: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[EXECUTE_ORDER_SEED.as_bytes(), order.as_ref()],
        &marginfi::ID,
    )
}
