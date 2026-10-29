use anchor_lang::{
    solana_program::{program_option::COption, program_pack::Pack, system_instruction},
    system_program, AccountDeserialize, AccountSerialize, InstructionData, ToAccountMetas,
};
use anchor_spl::token::spl_token::{
    self,
    state::{Account as SplTokenAccount, Mint as SplTokenMint},
};
use fixed::types::I80F48;
use fixtures::{bank::BankFixture, prelude::*};
use marginfi::errors::MarginfiError;
use marginfi_type_crate::{
    constants::{LIQUIDITY_VAULT_AUTHORITY_SEED, UNWIND_CONFIG_SEED, UNWIND_TICKET_SEED},
    types::{Bank, MarginfiAccount, OracleSetup, UnwindConfig, UnwindTicket},
};
use pretty_assertions::assert_eq;
use solana_compute_budget_interface::ComputeBudgetInstruction;
use solana_program_test::BanksClientError;
use solana_sdk::{
    account::{Account, AccountSharedData},
    clock::Clock,
    instruction::{AccountMeta, Instruction},
    native_token::LAMPORTS_PER_SOL,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    sysvar::epoch_schedule::EpochSchedule,
    transaction::Transaction,
};
use solana_stake_interface::{
    program::ID as STAKE_PROGRAM_ID,
    stake_flags::StakeFlags,
    state::{Meta, Stake, StakeStateV2},
};
use solana_stake_interface_v2::stake_history::{StakeHistory, StakeHistoryEntry};
use spl_stake_pool::state::{
    AccountType as StakePoolAccountType, Fee as StakePoolFee, StakePool, StakeStatus,
    ValidatorList, ValidatorStakeInfo,
};

async fn process_ixs(
    test_f: &TestFixture,
    ixs: &[Instruction],
    additional_signers: &[&Keypair],
) -> Result<(), BanksClientError> {
    let ctx = test_f.context.borrow_mut();
    let payer = ctx.payer.insecure_clone();
    let mut signers: Vec<&Keypair> = vec![&payer];

    for signer in additional_signers {
        if signer.pubkey() != payer.pubkey() {
            signers.push(*signer);
        }
    }

    let blockhash = ctx.banks_client.get_latest_blockhash().await?;
    let tx = Transaction::new_signed_with_payer(ixs, Some(&payer.pubkey()), &signers, blockhash);

    let result = ctx
        .banks_client
        .process_transaction_with_metadata(tx)
        .await?;

    if result.result.is_err() {
        if let Some(metadata) = result.metadata.as_ref() {
            for log in &metadata.log_messages {
                eprintln!("{log}");
            }
        }
    }

    result.result.map_err(BanksClientError::TransactionError)
}

async fn process_ix(
    test_f: &TestFixture,
    ix: Instruction,
    additional_signers: &[&Keypair],
) -> Result<(), BanksClientError> {
    process_ixs(test_f, &[ix], additional_signers).await
}

fn unwind_config_pda(sol_bank: Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[UNWIND_CONFIG_SEED, sol_bank.as_ref()], &marginfi::ID)
}

fn unwind_ticket_pda(stake_account: Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[UNWIND_TICKET_SEED, stake_account.as_ref()], &marginfi::ID)
}

fn make_init_unwind_config_ix(
    test_f: &TestFixture,
    sol_bank: &BankFixture,
    fee_multiplier_bps: u32,
    max_pending_unwind_bps: u32,
    min_unwind_lamports: u64,
) -> Instruction {
    let payer = test_f.context.borrow().payer.pubkey();
    let (unwind_config, _) = unwind_config_pda(sol_bank.key);

    Instruction {
        program_id: marginfi::ID,
        accounts: marginfi::accounts::InitUnwindConfig {
            group: test_f.marginfi_group.key,
            admin: payer,
            sol_bank: sol_bank.key,
            unwind_config,
            system_program: system_program::ID,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::InitUnwindConfig {
            fee_multiplier_bps,
            max_pending_unwind_bps,
            min_unwind_lamports,
        }
        .data(),
    }
}

fn make_update_unwind_config_ix(
    test_f: &TestFixture,
    sol_bank: &BankFixture,
    fee_multiplier_bps: u32,
    max_pending_unwind_bps: u32,
    min_unwind_lamports: u64,
) -> Instruction {
    let payer = test_f.context.borrow().payer.pubkey();
    let (unwind_config, _) = unwind_config_pda(sol_bank.key);

    Instruction {
        program_id: marginfi::ID,
        accounts: marginfi::accounts::UpdateUnwindConfig {
            group: test_f.marginfi_group.key,
            admin: payer,
            sol_bank: sol_bank.key,
            unwind_config,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::UpdateUnwindConfig {
            fee_multiplier_bps,
            max_pending_unwind_bps,
            min_unwind_lamports,
        }
        .data(),
    }
}

async fn load_unwind_config(
    test_f: &TestFixture,
    config_key: Pubkey,
) -> anyhow::Result<UnwindConfig> {
    let account = test_f
        .try_load(&config_key)
        .await?
        .ok_or_else(|| anyhow::anyhow!("unwind config account does not exist"))?;

    let mut data: &[u8] = &account.data;
    Ok(UnwindConfig::try_deserialize(&mut data)?)
}

async fn load_token_account(test_f: &TestFixture, key: Pubkey) -> anyhow::Result<SplTokenAccount> {
    let account = test_f
        .try_load(&key)
        .await?
        .ok_or_else(|| anyhow::anyhow!("token account {key} does not exist"))?;
    Ok(SplTokenAccount::unpack(&account.data)?)
}

fn install_dummy_account(test_f: &TestFixture, key: Pubkey, owner: Pubkey) {
    test_f
        .context
        .borrow_mut()
        .set_account(&key, &AccountSharedData::new(1_000_000, 0, &owner));
}

#[derive(Clone, Copy)]
struct DummyPoolAccounts {
    stake_pool: Pubkey,
    validator_list: Pubkey,
    pool_withdraw_authority: Pubkey,
    pool_source_stake_account: Pubkey,
    manager_fee_account: Pubkey,
}

fn install_dummy_pool_accounts(test_f: &TestFixture) -> DummyPoolAccounts {
    let accounts = DummyPoolAccounts {
        stake_pool: Pubkey::new_unique(),
        validator_list: Pubkey::new_unique(),
        pool_withdraw_authority: Pubkey::new_unique(),
        pool_source_stake_account: Pubkey::new_unique(),
        manager_fee_account: Pubkey::new_unique(),
    };

    install_dummy_account(test_f, accounts.stake_pool, spl_stake_pool::id());
    install_dummy_account(test_f, accounts.validator_list, system_program::ID);
    install_dummy_account(test_f, accounts.pool_withdraw_authority, system_program::ID);
    install_dummy_account(
        test_f,
        accounts.pool_source_stake_account,
        system_program::ID,
    );
    install_dummy_account(test_f, accounts.manager_fee_account, system_program::ID);

    accounts
}

async fn native_sol_mint_fixture(test_f: &TestFixture) -> anyhow::Result<MintFixture> {
    let mint_key = spl_token::native_mint::ID;
    let rent = {
        let ctx = test_f.context.borrow_mut();
        ctx.banks_client.get_rent().await?
    };

    let native_mint = SplTokenMint {
        mint_authority: COption::Some(mint_key),
        supply: 0,
        decimals: 9,
        is_initialized: true,
        freeze_authority: COption::None,
    };
    let mut mint_data = vec![0_u8; SplTokenMint::LEN];
    SplTokenMint::pack(native_mint, &mut mint_data)?;

    let mint_account = Account {
        lamports: rent.minimum_balance(SplTokenMint::LEN),
        data: mint_data,
        owner: spl_token::ID,
        executable: false,
        rent_epoch: 0,
    };
    let mint_account: AccountSharedData = mint_account.into();
    test_f
        .context
        .borrow_mut()
        .set_account(&mint_key, &mint_account);

    let mint = MintFixture::from_existing(test_f.context.clone(), mint_key).await;
    anyhow::ensure!(
        mint.token_program == spl_token::ID,
        "native SOL mint must be owned by classic SPL Token"
    );
    anyhow::ensure!(
        mint.mint.decimals == 9,
        "native SOL mint must use 9 decimals"
    );

    Ok(mint)
}

async fn create_native_sol_bank_for_unwind(test_f: &TestFixture) -> anyhow::Result<BankFixture> {
    let native_mint = native_sol_mint_fixture(test_f).await?;

    let mut bank_config = test_f.get_bank(&BankMint::Sol).load().await.config;
    bank_config.oracle_setup = OracleSetup::Fixed;

    let bank = test_f
        .marginfi_group
        .try_lending_pool_add_bank(&native_mint, None, bank_config, Some(I80F48::from_num(10)))
        .await?;

    let bank_state = bank.load().await;
    anyhow::ensure!(
        bank_state.mint == spl_token::native_mint::ID,
        "created SOL bank must use the classic SPL Token native mint"
    );
    anyhow::ensure!(
        bank_state.config.oracle_setup == OracleSetup::Fixed,
        "test SOL bank must use a fixed oracle"
    );

    Ok(bank)
}

fn make_unwind_lst_ix_for_rejection(
    test_f: &TestFixture,
    marginfi_account: Pubkey,
    lst_bank: &BankFixture,
    sol_bank: &BankFixture,
    lst_liquidity_vault: Pubkey,
    sol_liquidity_vault: Pubkey,
    unwind_stake_account: Pubkey,
    pool_accounts: DummyPoolAccounts,
    max_lst_in: u64,
) -> (Instruction, Pubkey) {
    let payer = test_f.context.borrow().payer.pubkey();
    let (unwind_config, _) = unwind_config_pda(sol_bank.key);
    let (unwind_ticket, _) = unwind_ticket_pda(unwind_stake_account);
    let (lst_vault_authority, _) = Pubkey::find_program_address(
        &[
            LIQUIDITY_VAULT_AUTHORITY_SEED.as_bytes(),
            lst_bank.key.as_ref(),
        ],
        &marginfi::ID,
    );
    let (sol_vault_authority, _) = Pubkey::find_program_address(
        &[
            LIQUIDITY_VAULT_AUTHORITY_SEED.as_bytes(),
            sol_bank.key.as_ref(),
        ],
        &marginfi::ID,
    );

    let ix = Instruction {
        program_id: marginfi::ID,
        accounts: marginfi::accounts::UnwindLst {
            group: test_f.marginfi_group.key,
            marginfi_account,
            authority: payer,
            payer,
            lst_bank: lst_bank.key,
            sol_bank: sol_bank.key,
            lst_liquidity_vault,
            lst_liquidity_vault_authority: lst_vault_authority,
            sol_liquidity_vault,
            sol_liquidity_vault_authority: sol_vault_authority,
            unwind_config,
            unwind_ticket,
            unwind_stake_account,
            stake_pool: pool_accounts.stake_pool,
            validator_list: pool_accounts.validator_list,
            pool_withdraw_authority: pool_accounts.pool_withdraw_authority,
            pool_source_stake_account: pool_accounts.pool_source_stake_account,
            manager_fee_account: pool_accounts.manager_fee_account,
            pool_mint: lst_bank.mint.key,
            stake_pool_program: spl_stake_pool::id(),
            stake_program: solana_stake_interface::program::ID,
            token_program: anchor_spl::token::ID,
            system_program: system_program::ID,
            clock: solana_sdk::sysvar::clock::ID,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::UnwindLst { max_lst_in }.data(),
    };

    (ix, unwind_ticket)
}

#[tokio::test]
async fn test_unwind_lst_account_preconditions() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let sol_bank_f = test_f.get_bank(&BankMint::Sol);
    let lst_bank_f = test_f.get_bank(&BankMint::SolEquivalent);

    let lp_mfi_account_f = test_f.create_marginfi_account().await;
    let lp_token_account = test_f
        .sol_mint
        .create_token_account_and_mint_to(10_000)
        .await;
    lp_mfi_account_f
        .try_bank_deposit(lp_token_account.key, sol_bank_f, 10_000, None)
        .await?;

    let user_mfi_account_f = test_f.create_marginfi_account().await;
    let collateral_amount = 200.0;
    let collateral_wallet_balance = get_max_deposit_amount_pre_fee(collateral_amount);
    let user_collateral_token_account = lst_bank_f
        .mint
        .create_token_account_and_mint_to(collateral_wallet_balance)
        .await;
    user_mfi_account_f
        .try_bank_deposit(
            user_collateral_token_account.key,
            lst_bank_f,
            collateral_amount,
            None,
        )
        .await?;

    let sol_debt_amount = 50.0;
    let user_sol_token_account = sol_bank_f.mint.create_empty_token_account().await;
    user_mfi_account_f
        .try_bank_borrow(user_sol_token_account.key, sol_bank_f, sol_debt_amount)
        .await?;

    let user_account = user_mfi_account_f.load().await;
    let sol_bank = sol_bank_f.load().await;

    let sol_balance = user_account
        .lending_account
        .balances
        .iter()
        .find(|b| b.is_active() && b.bank_pk == sol_bank_f.key)
        .expect("SOL liability balance must exist");
    let lst_balance = user_account
        .lending_account
        .balances
        .iter()
        .find(|b| b.is_active() && b.bank_pk == lst_bank_f.key)
        .expect("collateral balance must exist");

    assert_eq!(sol_bank.pending_unwind_lamports, 0);
    assert!(I80F48::from(sol_balance.liability_shares) > I80F48::ZERO);
    assert!(I80F48::from(lst_balance.asset_shares) > I80F48::ZERO);

    Ok(())
}

#[tokio::test]
async fn test_unwind_rejects_account_without_sol_debt_atomically() -> anyhow::Result<()> {
    let (test_f, sol_bank_f, lst_bank_key, user_key, pool) =
        setup_real_unwind_case_with_positions(LAMPORTS_PER_SOL, 3_000, true, false).await?;
    let lst_bank_f = test_f.get_bank(&BankMint::SolEquivalent);
    let sol_bank_state = sol_bank_f.load().await;
    let lst_bank_state = lst_bank_f.load().await;
    let user_before = raw_account_data(&test_f, user_key).await?;
    let sol_bank_before = raw_account_data(&test_f, sol_bank_f.key).await?;
    let lst_bank_before = raw_account_data(&test_f, lst_bank_key).await?;
    let unwind_stake = Keypair::new();
    let (ix, ticket_key) = make_real_unwind_ix(
        &test_f,
        user_key,
        lst_bank_f,
        &sol_bank_f,
        lst_bank_state.liquidity_vault,
        sol_bank_state.liquidity_vault,
        &pool,
        unwind_stake.pubkey(),
        100 * LAMPORTS_PER_SOL,
        None,
    );

    let result = process_ix(&test_f, ix, &[&unwind_stake]).await;
    assert!(result.is_err(), "unwind without SOL debt must fail");
    assert_custom_error!(result.unwrap_err(), MarginfiError::NoLiabilityFound);
    assert_eq!(user_before, raw_account_data(&test_f, user_key).await?);
    assert_eq!(
        sol_bank_before,
        raw_account_data(&test_f, sol_bank_f.key).await?
    );
    assert_eq!(
        lst_bank_before,
        raw_account_data(&test_f, lst_bank_key).await?
    );
    assert!(test_f.try_load(&ticket_key).await?.is_none());
    assert!(test_f.try_load(&unwind_stake.pubkey()).await?.is_none());

    Ok(())
}

#[tokio::test]
async fn test_unwind_rejects_account_without_lst_deposit_atomically() -> anyhow::Result<()> {
    let (test_f, sol_bank_f, lst_bank_key, user_key, pool) =
        setup_real_unwind_case_with_positions(LAMPORTS_PER_SOL, 3_000, false, true).await?;
    let lst_bank_f = test_f.get_bank(&BankMint::SolEquivalent);
    let sol_bank_state = sol_bank_f.load().await;
    let lst_bank_state = lst_bank_f.load().await;
    let user_before = raw_account_data(&test_f, user_key).await?;
    let sol_bank_before = raw_account_data(&test_f, sol_bank_f.key).await?;
    let lst_bank_before = raw_account_data(&test_f, lst_bank_key).await?;
    let unwind_stake = Keypair::new();
    let (ix, ticket_key) = make_real_unwind_ix(
        &test_f,
        user_key,
        lst_bank_f,
        &sol_bank_f,
        lst_bank_state.liquidity_vault,
        sol_bank_state.liquidity_vault,
        &pool,
        unwind_stake.pubkey(),
        100 * LAMPORTS_PER_SOL,
        None,
    );

    let result = process_ix(&test_f, ix, &[&unwind_stake]).await;
    assert!(result.is_err(), "unwind without LST collateral must fail");
    assert_custom_error!(result.unwrap_err(), MarginfiError::BankAccountNotFound);
    assert_eq!(user_before, raw_account_data(&test_f, user_key).await?);
    assert_eq!(
        sol_bank_before,
        raw_account_data(&test_f, sol_bank_f.key).await?
    );
    assert_eq!(
        lst_bank_before,
        raw_account_data(&test_f, lst_bank_key).await?
    );
    assert!(test_f.try_load(&ticket_key).await?.is_none());
    assert!(test_f.try_load(&unwind_stake.pubkey()).await?.is_none());

    Ok(())
}

#[tokio::test]
async fn test_init_unwind_config_state() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let sol_bank_f = create_native_sol_bank_for_unwind(&test_f).await?;
    let (config_key, expected_bump) = unwind_config_pda(sol_bank_f.key);

    let sol_bank_initial = sol_bank_f.load().await;
    assert_eq!(sol_bank_initial.max_pending_unwind_bps, 0);
    assert!(test_f.try_load(&config_key).await?.is_none());

    let payer = test_f.payer_keypair();
    process_ix(
        &test_f,
        make_init_unwind_config_ix(&test_f, &sol_bank_f, 10_000, 1_000, 1),
        &[&payer],
    )
    .await?;

    let sol_bank_after = sol_bank_f.load().await;
    assert_eq!(sol_bank_after.max_pending_unwind_bps, 1_000);

    let config = load_unwind_config(&test_f, config_key).await?;
    assert_eq!(config.fee_multiplier_bps, 10_000);
    assert_eq!(config.min_unwind_lamports, 1);
    assert_eq!(config.bump, expected_bump);

    Ok(())
}

#[tokio::test]
async fn test_init_unwind_config_rejects_non_admin() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let sol_bank_f = create_native_sol_bank_for_unwind(&test_f).await?;
    let (config_key, _) = unwind_config_pda(sol_bank_f.key);
    let non_admin = Keypair::new();

    let payer = test_f.context.borrow().payer.pubkey();
    let fund_non_admin_ix =
        system_instruction::transfer(&payer, &non_admin.pubkey(), 1_000_000_000);
    process_ix(&test_f, fund_non_admin_ix, &[]).await?;

    let ix = Instruction {
        program_id: marginfi::ID,
        accounts: marginfi::accounts::InitUnwindConfig {
            group: test_f.marginfi_group.key,
            admin: non_admin.pubkey(),
            sol_bank: sol_bank_f.key,
            unwind_config: config_key,
            system_program: system_program::ID,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::InitUnwindConfig {
            fee_multiplier_bps: 10_000,
            max_pending_unwind_bps: 1_000,
            min_unwind_lamports: 1,
        }
        .data(),
    };

    let result = process_ix(&test_f, ix, &[&non_admin]).await;
    assert!(
        result.is_err(),
        "a non-admin must not initialize unwind config"
    );
    assert_custom_error!(result.unwrap_err(), MarginfiError::Unauthorized);

    assert!(test_f.try_load(&config_key).await?.is_none());
    assert_eq!(sol_bank_f.load().await.max_pending_unwind_bps, 0);

    Ok(())
}

#[tokio::test]
async fn test_pending_cap_configuration() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let sol_bank_f = create_native_sol_bank_for_unwind(&test_f).await?;
    let (config_key, _) = unwind_config_pda(sol_bank_f.key);
    let payer = test_f.payer_keypair();

    process_ix(
        &test_f,
        make_init_unwind_config_ix(&test_f, &sol_bank_f, 10_000, 1_000, 1),
        &[&payer],
    )
    .await?;

    process_ix(
        &test_f,
        make_update_unwind_config_ix(&test_f, &sol_bank_f, 15_000, 3_000, 2),
        &[&payer],
    )
    .await?;

    let sol_bank_after_update = sol_bank_f.load().await;
    assert_eq!(sol_bank_after_update.max_pending_unwind_bps, 3_000);
    let config_after_update = load_unwind_config(&test_f, config_key).await?;
    assert_eq!(config_after_update.fee_multiplier_bps, 15_000);
    assert_eq!(config_after_update.min_unwind_lamports, 2);

    let result = process_ix(
        &test_f,
        make_update_unwind_config_ix(&test_f, &sol_bank_f, 10_000, 5_001, 99),
        &[&payer],
    )
    .await;
    assert!(result.is_err());
    assert_custom_error!(result.unwrap_err(), MarginfiError::InvalidConfig);

    let sol_bank_after_failure = sol_bank_f.load().await;
    assert_eq!(sol_bank_after_failure.max_pending_unwind_bps, 3_000);
    let config_after_failure = load_unwind_config(&test_f, config_key).await?;
    assert_eq!(config_after_failure.fee_multiplier_bps, 15_000);
    assert_eq!(config_after_failure.min_unwind_lamports, 2);

    Ok(())
}

#[tokio::test]
async fn test_unwind_rejects_non_pyth_lst_bank_atomically() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let sol_bank_f = create_native_sol_bank_for_unwind(&test_f).await?;
    let lst_bank_f = test_f.get_bank(&BankMint::SolEquivalent);

    let sol_bank_initial = sol_bank_f.load().await;
    assert_eq!(sol_bank_initial.max_pending_unwind_bps, 0);

    process_ix(
        &test_f,
        make_init_unwind_config_ix(&test_f, &sol_bank_f, 10_000, 1_000, 1),
        &[&test_f.payer_keypair()],
    )
    .await?;

    let user_mfi_account_f = test_f.create_marginfi_account().await;
    let collateral_wallet_balance = get_max_deposit_amount_pre_fee(200.0);
    let user_collateral_token_account = lst_bank_f
        .mint
        .create_token_account_and_mint_to(collateral_wallet_balance)
        .await;
    user_mfi_account_f
        .try_bank_deposit(user_collateral_token_account.key, lst_bank_f, 200.0, None)
        .await?;

    let lst_bank_before = lst_bank_f.load().await;
    assert!(
        !matches!(lst_bank_before.config.oracle_setup, OracleSetup::PythLST),
        "the fixture bank must not be a PythLST bank for this negative test"
    );
    let sol_bank_before = sol_bank_f.load().await;
    let user_before = user_mfi_account_f.load().await;

    let lst_position_before = user_before
        .lending_account
        .balances
        .iter()
        .find(|b| b.is_active() && b.bank_pk == lst_bank_f.key)
        .expect("LST-bank collateral must exist")
        .asset_shares;

    let pool_accounts = install_dummy_pool_accounts(&test_f);
    let unwind_stake = Keypair::new();
    let (ix, ticket_key) = make_unwind_lst_ix_for_rejection(
        &test_f,
        user_mfi_account_f.key,
        lst_bank_f,
        &sol_bank_f,
        lst_bank_before.liquidity_vault,
        sol_bank_before.liquidity_vault,
        unwind_stake.pubkey(),
        pool_accounts,
        u64::MAX,
    );

    let result = process_ix(&test_f, ix, &[&unwind_stake]).await;
    assert!(result.is_err(), "unwind must reject a non-PythLST bank");
    assert_custom_error!(result.unwrap_err(), MarginfiError::IllegalAction);

    let user_after = user_mfi_account_f.load().await;
    let sol_bank_after = sol_bank_f.load().await;
    let lst_bank_after = lst_bank_f.load().await;

    assert_eq!(user_after.last_update, user_before.last_update);
    assert_eq!(
        user_after
            .lending_account
            .balances
            .iter()
            .find(|b| b.is_active() && b.bank_pk == lst_bank_f.key)
            .expect("LST collateral must remain")
            .asset_shares,
        lst_position_before
    );
    assert_eq!(
        sol_bank_after.total_liability_shares,
        sol_bank_before.total_liability_shares
    );
    assert_eq!(
        sol_bank_after.pending_unwind_lamports,
        sol_bank_before.pending_unwind_lamports
    );
    assert_eq!(
        sol_bank_after.max_pending_unwind_bps,
        sol_bank_before.max_pending_unwind_bps
    );
    assert_eq!(
        lst_bank_after.total_asset_shares,
        lst_bank_before.total_asset_shares
    );
    assert_eq!(
        lst_bank_after.collected_program_fees_outstanding,
        lst_bank_before.collected_program_fees_outstanding
    );
    assert!(test_f.try_load(&ticket_key).await?.is_none());

    Ok(())
}

#[tokio::test]
async fn test_bank_with_collateral_and_sol_positions() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let sol_bank_f = test_f.get_bank(&BankMint::Sol);
    let collateral_bank_f = test_f.get_bank(&BankMint::SolEquivalent);

    let lp_mfi_account_f = test_f.create_marginfi_account().await;
    let lp_token_account = test_f
        .sol_mint
        .create_token_account_and_mint_to(10_000)
        .await;
    lp_mfi_account_f
        .try_bank_deposit(lp_token_account.key, sol_bank_f, 10_000, None)
        .await?;

    let user_mfi_account_f = test_f.create_marginfi_account().await;
    let collateral_amount = 200.0;
    let collateral_wallet_balance = get_max_deposit_amount_pre_fee(collateral_amount);
    let user_collateral_token_account = collateral_bank_f
        .mint
        .create_token_account_and_mint_to(collateral_wallet_balance)
        .await;
    user_mfi_account_f
        .try_bank_deposit(
            user_collateral_token_account.key,
            collateral_bank_f,
            collateral_amount,
            None,
        )
        .await?;

    let user_sol_token_account = sol_bank_f.mint.create_empty_token_account().await;
    user_mfi_account_f
        .try_bank_borrow(user_sol_token_account.key, sol_bank_f, 50.0)
        .await?;

    let account = user_mfi_account_f.load().await;
    let sol_position = account
        .lending_account
        .balances
        .iter()
        .find(|b| b.is_active() && b.bank_pk == sol_bank_f.key)
        .expect("SOL liability position must exist");
    let collateral_position = account
        .lending_account
        .balances
        .iter()
        .find(|b| b.is_active() && b.bank_pk == collateral_bank_f.key)
        .expect("collateral position must exist");

    assert!(I80F48::from(sol_position.liability_shares) > I80F48::ZERO);
    assert!(I80F48::from(collateral_position.asset_shares) > I80F48::ZERO);
    assert!(I80F48::from(sol_position.asset_shares) == I80F48::ZERO);
    assert!(I80F48::from(collateral_position.liability_shares) == I80F48::ZERO);

    Ok(())
}

#[tokio::test]
async fn test_clock_epoch_tracking() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let (clock, epoch_schedule): (Clock, EpochSchedule) = {
        let ctx = test_f.context.borrow_mut();
        let clock = ctx.banks_client.get_sysvar().await?;
        let epoch_schedule = ctx.banks_client.get_sysvar().await?;
        (clock, epoch_schedule)
    };

    assert_eq!(clock.epoch, epoch_schedule.get_epoch(clock.slot));
    assert_eq!(
        clock.leader_schedule_epoch,
        epoch_schedule.get_leader_schedule_epoch(clock.slot)
    );

    Ok(())
}

#[tokio::test]
async fn test_bank_initial_state() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let sol_bank_f = test_f.get_bank(&BankMint::Sol);
    let sol_bank = sol_bank_f.load().await;

    assert_eq!(sol_bank.pending_unwind_lamports, 0);
    assert_eq!(sol_bank.max_pending_unwind_bps, 0);

    Ok(())
}

const TEST_POOL_TOKEN_SUPPLY: u64 = 1_000 * LAMPORTS_PER_SOL;
const TEST_SOL_LIQUIDITY: u64 = 1_000 * LAMPORTS_PER_SOL;
const TEST_UNWIND_DEBT_SOL: f64 = 10.0;
const TEST_LST_DEPOSIT_SOL: f64 = 100.0;
const SOLANA_PACKET_DATA_SIZE: usize = 1_232;
const MAX_UNWIND_COMPUTE_UNITS: u64 = 1_350_000;

#[derive(Clone, Copy, Debug)]
struct TestStakePoolAccounts {
    stake_pool: Pubkey,
    validator_list: Pubkey,
    pool_withdraw_authority: Pubkey,
    pool_source_stake_account: Pubkey,
    manager_fee_account: Pubkey,
    pool_mint: Pubkey,
    collateral_wallet: Pubkey,
}

async fn install_test_stake_pool(
    test_f: &TestFixture,
    lst_bank_f: &BankFixture,
) -> anyhow::Result<TestStakePoolAccounts> {
    let clock = test_f.get_clock().await;
    let rent = test_f.context.borrow_mut().banks_client.get_rent().await?;
    let stake_rent = rent.minimum_balance(StakeStateV2::size_of());

    let collateral_wallet = lst_bank_f
        .mint
        .create_token_account_and_mint_to(1_000.0)
        .await;
    let manager_fee = lst_bank_f.mint.create_empty_token_account().await;

    let stake_pool_key = Pubkey::new_unique();
    let validator_list_key = Pubkey::new_unique();
    let reserve_stake_key = Pubkey::new_unique();
    let vote_key = Pubkey::new_unique();
    let (pool_withdraw_authority, withdraw_bump) = Pubkey::find_program_address(
        &[stake_pool_key.as_ref(), b"withdraw"],
        &spl_stake_pool::id(),
    );
    let (pool_source_stake_account, _source_bump) = spl_stake_pool::find_stake_program_address(
        &spl_stake_pool::id(),
        &vote_key,
        &stake_pool_key,
        None,
    );

    let pool_manager = test_f.context.borrow().payer.pubkey();
    let pool = StakePool {
        account_type: StakePoolAccountType::StakePool,
        manager: pool_manager,
        staker: Pubkey::new_unique(),
        stake_deposit_authority: Pubkey::new_unique(),
        stake_withdraw_bump_seed: withdraw_bump,
        validator_list: validator_list_key,
        reserve_stake: reserve_stake_key,
        pool_mint: lst_bank_f.mint.key,
        manager_fee_account: manager_fee.key,
        token_program_id: spl_token::ID,
        total_lamports: TEST_POOL_TOKEN_SUPPLY,
        pool_token_supply: TEST_POOL_TOKEN_SUPPLY,
        last_update_epoch: clock.epoch,
        stake_withdrawal_fee: StakePoolFee {
            numerator: 0,
            denominator: 1,
        },
        ..StakePool::default()
    };
    let mut validator_list = ValidatorList::new(1);
    let mut validator = ValidatorStakeInfo::default();
    validator.active_stake_lamports = TEST_POOL_TOKEN_SUPPLY.into();
    validator.last_update_epoch = clock.epoch.into();
    validator.validator_seed_suffix = 0.into();
    validator.status = StakeStatus::Active.into();
    validator.vote_account_address = vote_key;
    validator_list.validators.push(validator);

    let pool_data = borsh::to_vec(&pool)?;
    let list_data = borsh::to_vec(&validator_list)?;
    anyhow::ensure!(TEST_POOL_TOKEN_SUPPLY > stake_rent + LAMPORTS_PER_SOL * 2);

    let meta = test_stake_meta(pool_withdraw_authority, stake_rent);

    let mut stake = Stake::default();
    stake.delegation.voter_pubkey = vote_key;
    stake.delegation.stake = TEST_POOL_TOKEN_SUPPLY;
    stake.delegation.activation_epoch = clock.epoch.saturating_sub(2);
    stake.delegation.deactivation_epoch = u64::MAX;
    let stake_state = StakeStateV2::Stake(meta, stake, StakeFlags::empty());
    let stake_serialized = borsh::to_vec(&stake_state)?;
    anyhow::ensure!(
        stake_serialized.len() <= StakeStateV2::size_of(),
        "stake account data does not fit StakeStateV2"
    );
    let mut source_stake_data = vec![0_u8; StakeStateV2::size_of()];
    source_stake_data[..stake_serialized.len()].copy_from_slice(&stake_serialized);

    let pool_account = Account {
        lamports: rent.minimum_balance(pool_data.len()),
        data: pool_data,
        owner: spl_stake_pool::id(),
        executable: false,
        rent_epoch: 0,
    };
    let validator_list_account = Account {
        lamports: rent.minimum_balance(list_data.len()),
        data: list_data,
        owner: spl_stake_pool::id(),
        executable: false,
        rent_epoch: 0,
    };
    let source_stake_account = Account {
        lamports: TEST_POOL_TOKEN_SUPPLY + stake_rent,
        data: source_stake_data,
        owner: STAKE_PROGRAM_ID,
        executable: false,
        rent_epoch: 0,
    };

    let reserve_meta = test_stake_meta(pool_withdraw_authority, stake_rent);
    let reserve_stake_data = stake_state_bytes(&StakeStateV2::Initialized(reserve_meta))?;
    let reserve_stake_account = Account {
        lamports: stake_rent,
        data: reserve_stake_data,
        owner: STAKE_PROGRAM_ID,
        executable: false,
        rent_epoch: 0,
    };

    {
        let mut ctx = test_f.context.borrow_mut();
        ctx.set_account(&stake_pool_key, &AccountSharedData::from(pool_account));
        ctx.set_account(
            &validator_list_key,
            &AccountSharedData::from(validator_list_account),
        );
        ctx.set_account(
            &pool_source_stake_account,
            &AccountSharedData::from(source_stake_account),
        );
        ctx.set_account(
            &reserve_stake_key,
            &AccountSharedData::from(reserve_stake_account),
        );
    }

    let mut mint_account = test_f
        .try_load(&lst_bank_f.mint.key)
        .await?
        .ok_or_else(|| anyhow::anyhow!("LST pool mint does not exist"))?;
    let mut mint_state = SplTokenMint::unpack(&mint_account.data)?;
    mint_state.mint_authority = COption::Some(pool_withdraw_authority);
    SplTokenMint::pack(mint_state, &mut mint_account.data)?;
    test_f
        .context
        .borrow_mut()
        .set_account(&lst_bank_f.mint.key, &AccountSharedData::from(mint_account));

    let mut lst_bank_account = test_f
        .try_load(&lst_bank_f.key)
        .await?
        .ok_or_else(|| anyhow::anyhow!("LST bank does not exist"))?;
    {
        let bank = bytemuck::from_bytes_mut::<Bank>(&mut lst_bank_account.data[8..]);
        bank.config.oracle_setup = OracleSetup::PythLST;
        bank.config.oracle_keys[0] = PYTH_SOL_FEED;
        bank.config.oracle_keys[1] = stake_pool_key;
    }
    test_f
        .context
        .borrow_mut()
        .set_account(&lst_bank_f.key, &AccountSharedData::from(lst_bank_account));

    test_f
        .set_pyth_oracle_timestamp(PYTH_SOL_FEED, clock.unix_timestamp)
        .await;

    let mint_after = test_f
        .try_load(&lst_bank_f.mint.key)
        .await?
        .ok_or_else(|| anyhow::anyhow!("LST pool mint disappeared"))?;
    anyhow::ensure!(
        SplTokenMint::unpack(&mint_after.data)?.supply == TEST_POOL_TOKEN_SUPPLY,
        "pool_token_supply must equal the SPL token mint supply"
    );

    Ok(TestStakePoolAccounts {
        stake_pool: stake_pool_key,
        validator_list: validator_list_key,
        pool_withdraw_authority,
        pool_source_stake_account,
        manager_fee_account: manager_fee.key,
        pool_mint: lst_bank_f.mint.key,
        collateral_wallet: collateral_wallet.key,
    })
}

async fn create_funded_wrapped_sol_account(
    test_f: &TestFixture,
    sol_bank_f: &BankFixture,
    amount_lamports: u64,
) -> anyhow::Result<Pubkey> {
    let token_account = sol_bank_f.mint.create_empty_token_account().await;
    let payer = test_f.context.borrow().payer.pubkey();
    process_ix(
        test_f,
        system_instruction::transfer(&payer, &token_account.key, amount_lamports),
        &[],
    )
    .await?;
    let sync_native_ix = spl_token::instruction::sync_native(&spl_token::ID, &token_account.key)?;
    process_ix(test_f, sync_native_ix, &[]).await?;
    Ok(token_account.key)
}

async fn prepare_unwind_user(
    test_f: &TestFixture,
    sol_bank_f: &BankFixture,
    lst_bank_f: &BankFixture,
    pool: &TestStakePoolAccounts,
    fee_multiplier_bps: u32,
    pending_cap_bps: u32,
    min_unwind_lamports: u64,
    with_lst_deposit: bool,
    with_sol_debt: bool,
) -> anyhow::Result<Pubkey> {
    let lender = test_f.create_marginfi_account().await;
    let lender_token =
        create_funded_wrapped_sol_account(test_f, sol_bank_f, TEST_SOL_LIQUIDITY).await?;
    lender
        .try_bank_deposit(
            lender_token,
            sol_bank_f,
            TEST_SOL_LIQUIDITY as f64 / LAMPORTS_PER_SOL as f64,
            None,
        )
        .await?;

    let user = test_f.create_marginfi_account().await;
    if with_lst_deposit {
        user.try_bank_deposit(
            pool.collateral_wallet,
            lst_bank_f,
            TEST_LST_DEPOSIT_SOL,
            None,
        )
        .await?;
    } else {
        let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
        let usdc_wallet = test_f
            .usdc_mint
            .create_token_account_and_mint_to(2_000)
            .await;
        user.try_bank_deposit(usdc_wallet.key, usdc_bank_f, 2_000.0, None)
            .await?;
    }

    if with_sol_debt {
        let borrow_destination = sol_bank_f.mint.create_empty_token_account().await;
        user.try_bank_borrow(borrow_destination.key, sol_bank_f, TEST_UNWIND_DEBT_SOL)
            .await?;
    }

    process_ix(
        test_f,
        make_init_unwind_config_ix(
            test_f,
            sol_bank_f,
            fee_multiplier_bps,
            pending_cap_bps,
            min_unwind_lamports,
        ),
        &[],
    )
    .await?;

    Ok(user.key)
}

fn make_real_unwind_ix(
    test_f: &TestFixture,
    user_account: Pubkey,
    lst_bank: &BankFixture,
    sol_bank: &BankFixture,
    lst_liquidity_vault: Pubkey,
    sol_liquidity_vault: Pubkey,
    pool: &TestStakePoolAccounts,
    unwind_stake_account: Pubkey,
    max_lst_in: u64,
    source_stake_override: Option<Pubkey>,
) -> (Instruction, Pubkey) {
    let payer = test_f.context.borrow().payer.pubkey();
    let (unwind_config, _) = unwind_config_pda(sol_bank.key);
    let (unwind_ticket, _) = unwind_ticket_pda(unwind_stake_account);
    let (lst_vault_authority, _) = Pubkey::find_program_address(
        &[
            LIQUIDITY_VAULT_AUTHORITY_SEED.as_bytes(),
            lst_bank.key.as_ref(),
        ],
        &marginfi::ID,
    );
    let (sol_vault_authority, _) = Pubkey::find_program_address(
        &[
            LIQUIDITY_VAULT_AUTHORITY_SEED.as_bytes(),
            sol_bank.key.as_ref(),
        ],
        &marginfi::ID,
    );
    let mut ix = Instruction {
        program_id: marginfi::ID,
        accounts: marginfi::accounts::UnwindLst {
            group: test_f.marginfi_group.key,
            marginfi_account: user_account,
            authority: payer,
            payer,
            lst_bank: lst_bank.key,
            sol_bank: sol_bank.key,
            lst_liquidity_vault,
            lst_liquidity_vault_authority: lst_vault_authority,
            sol_liquidity_vault,
            sol_liquidity_vault_authority: sol_vault_authority,
            unwind_config,
            unwind_ticket,
            unwind_stake_account,
            stake_pool: pool.stake_pool,
            validator_list: pool.validator_list,
            pool_withdraw_authority: pool.pool_withdraw_authority,
            pool_source_stake_account: source_stake_override
                .unwrap_or(pool.pool_source_stake_account),
            manager_fee_account: pool.manager_fee_account,
            pool_mint: pool.pool_mint,
            stake_pool_program: spl_stake_pool::id(),
            stake_program: STAKE_PROGRAM_ID,
            token_program: spl_token::ID,
            system_program: system_program::ID,
            clock: solana_sdk::sysvar::clock::ID,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::UnwindLst { max_lst_in }.data(),
    };

    ix.accounts.extend([
        AccountMeta::new_readonly(lst_bank.key, false),
        AccountMeta::new_readonly(PYTH_SOL_FEED, false),
        AccountMeta::new_readonly(pool.stake_pool, false),
    ]);
    (ix, unwind_ticket)
}

fn decode_marginfi_account(data: &[u8]) -> anyhow::Result<MarginfiAccount> {
    let mut data = data;
    Ok(MarginfiAccount::try_deserialize(&mut data)?)
}

async fn raw_account_data(test_f: &TestFixture, key: Pubkey) -> anyhow::Result<Vec<u8>> {
    Ok(test_f
        .try_load(&key)
        .await?
        .ok_or_else(|| anyhow::anyhow!("account {key} does not exist"))?
        .data)
}

async fn load_unwind_ticket(test_f: &TestFixture, key: Pubkey) -> anyhow::Result<UnwindTicket> {
    let account = test_f
        .try_load(&key)
        .await?
        .ok_or_else(|| anyhow::anyhow!("unwind ticket {key} does not exist"))?;
    let mut data: &[u8] = &account.data;
    Ok(UnwindTicket::try_deserialize(&mut data)?)
}

fn stake_state_bytes(state: &StakeStateV2) -> anyhow::Result<Vec<u8>> {
    let serialized = borsh::to_vec(state)?;
    anyhow::ensure!(serialized.len() <= StakeStateV2::size_of());
    let mut data = vec![0_u8; StakeStateV2::size_of()];
    data[..serialized.len()].copy_from_slice(&serialized);
    Ok(data)
}

// The stake processor still reads this serialized legacy reserve.
#[allow(deprecated)]
fn test_stake_meta(authority: Pubkey, rent_exempt_reserve: u64) -> Meta {
    let mut meta = Meta::default();
    meta.rent_exempt_reserve = rent_exempt_reserve;
    meta.authorized.staker = authority;
    meta.authorized.withdrawer = authority;
    meta
}

async fn setup_real_unwind_case(
    min_unwind_lamports: u64,
    pending_cap_bps: u32,
) -> anyhow::Result<(
    TestFixture,
    BankFixture,
    Pubkey,
    Pubkey,
    TestStakePoolAccounts,
)> {
    setup_real_unwind_case_with_positions(min_unwind_lamports, pending_cap_bps, true, true).await
}

async fn setup_real_unwind_case_with_positions(
    min_unwind_lamports: u64,
    pending_cap_bps: u32,
    with_lst_deposit: bool,
    with_sol_debt: bool,
) -> anyhow::Result<(
    TestFixture,
    BankFixture,
    Pubkey,
    Pubkey,
    TestStakePoolAccounts,
)> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let sol_bank_f = create_native_sol_bank_for_unwind(&test_f).await?;
    let lst_bank_f = test_f.get_bank(&BankMint::SolEquivalent);
    let pool = install_test_stake_pool(&test_f, lst_bank_f).await?;
    let user_key = prepare_unwind_user(
        &test_f,
        &sol_bank_f,
        lst_bank_f,
        &pool,
        10_000,
        pending_cap_bps,
        min_unwind_lamports,
        with_lst_deposit,
        with_sol_debt,
    )
    .await?;
    let lst_bank_key = lst_bank_f.key;
    Ok((test_f, sol_bank_f, lst_bank_key, user_key, pool))
}

#[tokio::test]
async fn test_unwind_lst_success_and_transaction_metrics() -> anyhow::Result<()> {
    let (test_f, sol_bank_f, lst_bank_key, user_key, pool) =
        setup_real_unwind_case(LAMPORTS_PER_SOL, 3_000).await?;
    let lst_bank_f = test_f.get_bank(&BankMint::SolEquivalent);
    let lst_bank_before = lst_bank_f.load().await;
    let sol_bank_before = sol_bank_f.load().await;
    let user_before_data = raw_account_data(&test_f, user_key).await?;
    let lst_bank_before_data = raw_account_data(&test_f, lst_bank_key).await?;
    let sol_bank_before_data = raw_account_data(&test_f, sol_bank_f.key).await?;
    let source_before = test_f
        .try_load(&pool.pool_source_stake_account)
        .await?
        .ok_or_else(|| anyhow::anyhow!("pool source stake account missing"))?;
    let lst_vault_before = load_token_account(&test_f, lst_bank_before.liquidity_vault)
        .await?
        .amount;

    let pre_user = decode_marginfi_account(&raw_account_data(&test_f, user_key).await?)?;
    let pre_sol_balance = pre_user
        .lending_account
        .balances
        .iter()
        .find(|b| b.is_active() && b.bank_pk == sol_bank_f.key)
        .ok_or_else(|| anyhow::anyhow!("SOL liability balance missing"))?;
    let debt_amount = I80F48::from(pre_sol_balance.liability_shares)
        .checked_mul(I80F48::from(sol_bank_before.liability_share_value))
        .ok_or_else(|| anyhow::anyhow!("debt amount overflow"))?;
    let expected_principal = debt_amount
        .checked_ceil()
        .and_then(|v| v.checked_to_num::<u64>())
        .ok_or_else(|| anyhow::anyhow!("debt amount conversion failed"))?;
    let debt_shares_before = I80F48::from(pre_sol_balance.liability_shares);

    let unwind_stake = Keypair::new();
    let (unwind_ix, ticket_key) = make_real_unwind_ix(
        &test_f,
        user_key,
        lst_bank_f,
        &sol_bank_f,
        lst_bank_before.liquidity_vault,
        sol_bank_before.liquidity_vault,
        &pool,
        unwind_stake.pubkey(),
        100 * LAMPORTS_PER_SOL,
        None,
    );
    let payer = test_f.payer_keypair();
    let blockhash = test_f
        .context
        .borrow_mut()
        .banks_client
        .get_latest_blockhash()
        .await?;
    let compute_budget_ix = ComputeBudgetInstruction::set_compute_unit_limit(1_400_000);
    let tx = Transaction::new_signed_with_payer(
        &[compute_budget_ix, unwind_ix],
        Some(&payer.pubkey()),
        &[&payer, &unwind_stake],
        blockhash,
    );
    let serialized_size = bincode::serialize(&tx)?.len();
    anyhow::ensure!(
        serialized_size <= SOLANA_PACKET_DATA_SIZE,
        "unwind transaction is {serialized_size} bytes, exceeding the {SOLANA_PACKET_DATA_SIZE}-byte packet limit"
    );

    let simulated = test_f
        .context
        .borrow_mut()
        .banks_client
        .simulate_transaction(tx.clone())
        .await?;
    match simulated.result.as_ref() {
        Some(Err(err)) => {
            let logs = simulated
                .simulation_details
                .as_ref()
                .map(|details| details.logs.join("\n"))
                .unwrap_or_default();
            anyhow::bail!("unwind simulation failed: {err:?}\n{logs}");
        }
        Some(Ok(())) => {}
        None => anyhow::bail!("unwind simulation did not return a transaction result"),
    }
    let compute_units = simulated
        .simulation_details
        .as_ref()
        .map(|details| details.units_consumed)
        .unwrap_or_default();
    anyhow::ensure!(compute_units > 0, "simulation did not report compute units");
    anyhow::ensure!(
        compute_units < MAX_UNWIND_COMPUTE_UNITS,
        "unwind consumed {compute_units} compute units, exceeding the {MAX_UNWIND_COMPUTE_UNITS}-CU regression guardrail"
    );
    println!(
        "unwind_lst compute units: {compute_units}/{MAX_UNWIND_COMPUTE_UNITS} guardrail (1,400,000 runtime limit); serialized transaction size: {serialized_size}/{SOLANA_PACKET_DATA_SIZE} bytes"
    );

    test_f
        .context
        .borrow_mut()
        .banks_client
        .process_transaction(tx)
        .await?;

    let sol_bank_after = sol_bank_f.load().await;
    let lst_bank_after = lst_bank_f.load().await;
    let user_after = decode_marginfi_account(&raw_account_data(&test_f, user_key).await?)?;
    let ticket = load_unwind_ticket(&test_f, ticket_key).await?;
    let current_clock = test_f.get_clock().await;
    let remaining_sol_debt = user_after
        .lending_account
        .balances
        .iter()
        .find(|b| b.is_active() && b.bank_pk == sol_bank_f.key)
        .map(|b| I80F48::from(b.liability_shares))
        .unwrap_or(I80F48::ZERO);
    assert_eq!(
        remaining_sol_debt,
        I80F48::ZERO,
        "successful unwind must repay all SOL debt shares"
    );
    assert!(
        I80F48::from(sol_bank_after.total_liability_shares)
            < I80F48::from(sol_bank_before.total_liability_shares),
        "SOL bank liability shares must decrease"
    );
    assert!(debt_shares_before > I80F48::ZERO);
    let stake_account = test_f
        .try_load(&unwind_stake.pubkey())
        .await?
        .ok_or_else(|| anyhow::anyhow!("unwind stake account was not created"))?;
    let stake_state =
        solana_borsh::v1::try_from_slice_unchecked::<StakeStateV2>(&stake_account.data)?;
    let (_, split_stake, _) = match stake_state {
        StakeStateV2::Stake(meta, stake, flags) => (meta, stake, flags),
        other => anyhow::bail!("expected delegated stake account, got {other:?}"),
    };
    assert_eq!(
        split_stake.delegation.deactivation_epoch,
        current_clock.epoch
    );

    assert_eq!(sol_bank_after.pending_unwind_lamports, expected_principal);
    assert_eq!(ticket.sol_bank, sol_bank_f.key);
    assert_eq!(ticket.stake_account, unwind_stake.pubkey());
    assert_eq!(
        ticket.rent_recipient,
        test_f.context.borrow().payer.pubkey()
    );
    assert_eq!(ticket.principal_lamports, expected_principal);
    assert_eq!(ticket.created_epoch, current_clock.epoch);
    assert_eq!(ticket.bump, unwind_ticket_pda(unwind_stake.pubkey()).1);
    let rent = test_f.context.borrow_mut().banks_client.get_rent().await?;
    assert_eq!(
        ticket.stake_rent_prefund_lamports,
        rent.minimum_balance(StakeStateV2::size_of())
    );

    let user_lst_after = user_after
        .lending_account
        .balances
        .iter()
        .find(|b| b.is_active() && b.bank_pk == lst_bank_key)
        .ok_or_else(|| anyhow::anyhow!("remaining LST collateral balance missing"))?;
    let user_before = decode_marginfi_account(&user_before_data)?;
    let user_lst_before = user_before
        .lending_account
        .balances
        .iter()
        .find(|b| b.is_active() && b.bank_pk == lst_bank_key)
        .ok_or_else(|| anyhow::anyhow!("LST collateral balance missing before unwind"))?
        .asset_shares;
    assert!(
        I80F48::from(user_lst_after.asset_shares) < I80F48::from(user_lst_before),
        "LST asset shares must decrease by the pool redemption and unwind fee"
    );
    assert!(
        I80F48::from(lst_bank_after.collected_program_fees_outstanding)
            > I80F48::from(lst_bank_before.collected_program_fees_outstanding),
        "the unwind fee must accrue to LST-bank program fees"
    );
    assert_eq!(
        I80F48::from(sol_bank_after.collected_program_fees_outstanding),
        I80F48::from(sol_bank_before.collected_program_fees_outstanding),
        "the unwind fee must not be booked to SOL lenders or SOL-bank program fees"
    );
    assert!(
        test_f.try_load(&ticket_key).await?.is_some(),
        "successful unwind must create its ticket"
    );
    assert!(
        source_before.lamports
            > test_f
                .try_load(&pool.pool_source_stake_account)
                .await?
                .ok_or_else(|| anyhow::anyhow!("pool source stake account disappeared"))?
                .lamports,
        "stake-pool CPI must split lamports out of the validator stake account"
    );
    assert!(
        load_token_account(&test_f, lst_bank_before.liquidity_vault)
            .await?
            .amount
            < lst_vault_before,
        "stake-pool CPI must burn the redeemed pool tokens from the LST liquidity vault"
    );
    assert_ne!(user_before_data, raw_account_data(&test_f, user_key).await?);
    assert_ne!(
        sol_bank_before_data,
        raw_account_data(&test_f, sol_bank_f.key).await?
    );
    assert_ne!(
        lst_bank_before_data,
        raw_account_data(&test_f, lst_bank_key).await?
    );

    let stake_before_settlement = test_f
        .try_load(&unwind_stake.pubkey())
        .await?
        .ok_or_else(|| anyhow::anyhow!("unwind stake account disappeared before settlement"))?;
    let settlement_lamports = stake_before_settlement
        .lamports
        .checked_sub(ticket.stake_rent_prefund_lamports)
        .ok_or_else(|| anyhow::anyhow!("stake balance is below its rent prefund"))?;
    anyhow::ensure!(
        settlement_lamports >= ticket.principal_lamports,
        "settleable stake value must cover the ticket principal"
    );
    let vault_before_settlement = load_token_account(&test_f, sol_bank_before.liquidity_vault)
        .await?
        .amount;

    let epoch_schedule: EpochSchedule = test_f
        .context
        .borrow_mut()
        .banks_client
        .get_sysvar()
        .await?;
    let settlement_epoch = current_clock
        .epoch
        .checked_add(3)
        .ok_or_else(|| anyhow::anyhow!("settlement epoch overflow"))?;
    let last_slot_in_settlement_epoch = epoch_schedule
        .get_first_slot_in_epoch(settlement_epoch + 1)
        .saturating_sub(1);
    test_f
        .context
        .borrow_mut()
        .warp_to_slot(last_slot_in_settlement_epoch)?;
    let settlement_clock = test_f.get_clock().await;
    anyhow::ensure!(
        settlement_clock.epoch > ticket.created_epoch,
        "settlement must occur after the unwind creation epoch"
    );
    let mut stake_history = StakeHistory::default();
    for epoch in 0..settlement_clock.epoch {
        stake_history.add(epoch, StakeHistoryEntry::default());
    }
    test_f.context.borrow().set_sysvar(&stake_history);

    let (sol_vault_authority, _) = Pubkey::find_program_address(
        &[
            LIQUIDITY_VAULT_AUTHORITY_SEED.as_bytes(),
            sol_bank_f.key.as_ref(),
        ],
        &marginfi::ID,
    );
    let settle_ix = Instruction {
        program_id: marginfi::ID,
        accounts: marginfi::accounts::SettleUnwindTicket {
            group: test_f.marginfi_group.key,
            sol_bank: sol_bank_f.key,
            unwind_ticket: ticket_key,
            stake_account: unwind_stake.pubkey(),
            rent_recipient: ticket.rent_recipient,
            sol_liquidity_vault: sol_bank_before.liquidity_vault,
            sol_liquidity_vault_authority: sol_vault_authority,
            stake_program: STAKE_PROGRAM_ID,
            clock: solana_sdk::sysvar::clock::ID,
            stake_history: solana_stake_interface::sysvar::stake_history::ID,
            token_program: spl_token::ID,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::SettleUnwindTicket {}.data(),
    };
    process_ix(&test_f, settle_ix, &[]).await?;

    let vault_after_settlement = load_token_account(&test_f, sol_bank_before.liquidity_vault)
        .await?
        .amount;
    assert_eq!(
        vault_after_settlement - vault_before_settlement,
        settlement_lamports,
        "settlement must transfer all stake value excluding the stake-account rent prefund to the SOL vault"
    );
    let sol_bank_after_settlement = sol_bank_f.load().await;
    assert_eq!(
        sol_bank_after_settlement.pending_unwind_lamports,
        sol_bank_after.pending_unwind_lamports - ticket.principal_lamports,
        "settlement must release exactly the ticket principal from pending unwind"
    );
    assert!(
        test_f.try_load(&ticket_key).await?.is_none(),
        "settlement must close the ticket produced by unwind_lst"
    );
    assert!(
        test_f.try_load(&unwind_stake.pubkey()).await?.is_none(),
        "settlement must close the deactivated stake account"
    );

    Ok(())
}

#[tokio::test]
async fn test_unwind_rejects_debt_below_minimum_atomically() -> anyhow::Result<()> {
    let (test_f, sol_bank_f, lst_bank_key, user_key, pool) =
        setup_real_unwind_case(100 * LAMPORTS_PER_SOL, 3_000).await?;
    let lst_bank_f = test_f.get_bank(&BankMint::SolEquivalent);
    let lst_bank_before = lst_bank_f.load().await;
    let sol_bank_before = sol_bank_f.load().await;
    let user_before = raw_account_data(&test_f, user_key).await?;
    let lst_bank_before_data = raw_account_data(&test_f, lst_bank_key).await?;
    let sol_bank_before_data = raw_account_data(&test_f, sol_bank_f.key).await?;
    let unwind_stake = Keypair::new();
    let (ix, ticket_key) = make_real_unwind_ix(
        &test_f,
        user_key,
        lst_bank_f,
        &sol_bank_f,
        lst_bank_before.liquidity_vault,
        sol_bank_before.liquidity_vault,
        &pool,
        unwind_stake.pubkey(),
        100 * LAMPORTS_PER_SOL,
        None,
    );
    let result = process_ix(&test_f, ix, &[&unwind_stake]).await;
    assert!(result.is_err(), "debt below minimum must be rejected");
    assert_custom_error!(result.unwrap_err(), MarginfiError::BelowMinimum);
    assert_eq!(user_before, raw_account_data(&test_f, user_key).await?);
    assert_eq!(
        lst_bank_before_data,
        raw_account_data(&test_f, lst_bank_key).await?
    );
    assert_eq!(
        sol_bank_before_data,
        raw_account_data(&test_f, sol_bank_f.key).await?
    );
    assert!(test_f.try_load(&ticket_key).await?.is_none());
    assert!(test_f.try_load(&unwind_stake.pubkey()).await?.is_none());
    Ok(())
}

#[tokio::test]
async fn test_unwind_rejects_max_lst_in_below_principal_atomically() -> anyhow::Result<()> {
    let (test_f, sol_bank_f, lst_bank_key, user_key, pool) =
        setup_real_unwind_case(LAMPORTS_PER_SOL, 3_000).await?;
    let lst_bank_f = test_f.get_bank(&BankMint::SolEquivalent);
    let sol_bank_before_state = sol_bank_f.load().await;
    let lst_bank_before_state = lst_bank_f.load().await;
    let user_before = raw_account_data(&test_f, user_key).await?;
    let sol_bank_before = raw_account_data(&test_f, sol_bank_f.key).await?;
    let lst_bank_before = raw_account_data(&test_f, lst_bank_key).await?;

    let user = decode_marginfi_account(&user_before)?;
    let sol_liability = user
        .lending_account
        .balances
        .iter()
        .find(|balance| balance.is_active() && balance.bank_pk == sol_bank_f.key)
        .ok_or_else(|| anyhow::anyhow!("SOL liability balance missing"))?;
    let debt_principal = I80F48::from(sol_liability.liability_shares)
        .checked_mul(I80F48::from(sol_bank_before_state.liability_share_value))
        .and_then(I80F48::checked_ceil)
        .and_then(I80F48::checked_to_num::<u64>)
        .ok_or_else(|| anyhow::anyhow!("debt amount conversion failed"))?;
    anyhow::ensure!(debt_principal > 0);

    let unwind_stake = Keypair::new();
    let (ix, ticket_key) = make_real_unwind_ix(
        &test_f,
        user_key,
        lst_bank_f,
        &sol_bank_f,
        lst_bank_before_state.liquidity_vault,
        sol_bank_before_state.liquidity_vault,
        &pool,
        unwind_stake.pubkey(),
        debt_principal - 1,
        None,
    );
    let result = process_ix(&test_f, ix, &[&unwind_stake]).await;
    assert!(result.is_err(), "max_lst_in below debt principal must fail");
    assert_custom_error!(result.unwrap_err(), MarginfiError::SlippageTooHigh);

    assert_eq!(user_before, raw_account_data(&test_f, user_key).await?);
    assert_eq!(
        sol_bank_before,
        raw_account_data(&test_f, sol_bank_f.key).await?
    );
    assert_eq!(
        lst_bank_before,
        raw_account_data(&test_f, lst_bank_key).await?
    );
    assert!(test_f.try_load(&ticket_key).await?.is_none());
    assert!(test_f.try_load(&unwind_stake.pubkey()).await?.is_none());
    Ok(())
}

#[tokio::test]
async fn test_unwind_rejects_pending_cap_atomically() -> anyhow::Result<()> {
    let (test_f, sol_bank_f, lst_bank_key, user_key, pool) =
        setup_real_unwind_case(LAMPORTS_PER_SOL, 0).await?;
    let lst_bank_f = test_f.get_bank(&BankMint::SolEquivalent);
    let lst_bank_state = lst_bank_f.load().await;
    let sol_bank_state = sol_bank_f.load().await;
    let user_before = raw_account_data(&test_f, user_key).await?;
    let lst_bank_before = raw_account_data(&test_f, lst_bank_key).await?;
    let sol_bank_before = raw_account_data(&test_f, sol_bank_f.key).await?;
    let unwind_stake = Keypair::new();
    let (ix, ticket_key) = make_real_unwind_ix(
        &test_f,
        user_key,
        lst_bank_f,
        &sol_bank_f,
        lst_bank_state.liquidity_vault,
        sol_bank_state.liquidity_vault,
        &pool,
        unwind_stake.pubkey(),
        100 * LAMPORTS_PER_SOL,
        None,
    );
    let result = process_ix(&test_f, ix, &[&unwind_stake]).await;
    assert!(result.is_err(), "unwind exceeding pending cap must fail");
    assert_custom_error!(
        result.unwrap_err(),
        MarginfiError::BankLiabilityCapacityExceeded
    );
    assert_eq!(user_before, raw_account_data(&test_f, user_key).await?);
    assert_eq!(
        lst_bank_before,
        raw_account_data(&test_f, lst_bank_key).await?
    );
    assert_eq!(
        sol_bank_before,
        raw_account_data(&test_f, sol_bank_f.key).await?
    );
    assert!(test_f.try_load(&ticket_key).await?.is_none());
    assert!(test_f.try_load(&unwind_stake.pubkey()).await?.is_none());
    Ok(())
}

#[tokio::test]
async fn test_unwind_stake_pool_cpi_failure_rolls_back_accounting() -> anyhow::Result<()> {
    let (test_f, sol_bank_f, lst_bank_key, user_key, pool) =
        setup_real_unwind_case(LAMPORTS_PER_SOL, 3_000).await?;
    let lst_bank_f = test_f.get_bank(&BankMint::SolEquivalent);
    let lst_bank_before_state = lst_bank_f.load().await;
    let sol_bank_before_state = sol_bank_f.load().await;
    let user_before = raw_account_data(&test_f, user_key).await?;
    let lst_bank_before = raw_account_data(&test_f, lst_bank_key).await?;
    let sol_bank_before = raw_account_data(&test_f, sol_bank_f.key).await?;
    let vault_before = load_token_account(&test_f, lst_bank_before_state.liquidity_vault)
        .await?
        .amount;

    let invalid_source = Pubkey::new_unique();
    let invalid_stake_data = stake_state_bytes(&StakeStateV2::Uninitialized)?;
    let rent = test_f.context.borrow_mut().banks_client.get_rent().await?;
    test_f.context.borrow_mut().set_account(
        &invalid_source,
        &AccountSharedData::from(Account {
            lamports: rent.minimum_balance(StakeStateV2::size_of()),
            data: invalid_stake_data,
            owner: STAKE_PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        }),
    );
    let invalid_source_before = raw_account_data(&test_f, invalid_source).await?;
    let pool_before = raw_account_data(&test_f, pool.stake_pool).await?;
    let validator_list_before = raw_account_data(&test_f, pool.validator_list).await?;
    let source_stake_before = raw_account_data(&test_f, pool.pool_source_stake_account).await?;
    let pool_mint_before = raw_account_data(&test_f, pool.pool_mint).await?;
    let manager_fee_before = raw_account_data(&test_f, pool.manager_fee_account).await?;

    let unwind_stake = Keypair::new();
    let (ix, ticket_key) = make_real_unwind_ix(
        &test_f,
        user_key,
        lst_bank_f,
        &sol_bank_f,
        lst_bank_before_state.liquidity_vault,
        sol_bank_before_state.liquidity_vault,
        &pool,
        unwind_stake.pubkey(),
        100 * LAMPORTS_PER_SOL,
        Some(invalid_source),
    );
    let payer = test_f.payer_keypair();
    let blockhash = test_f
        .context
        .borrow_mut()
        .banks_client
        .get_latest_blockhash()
        .await?;
    let tx = Transaction::new_signed_with_payer(
        &[ix.clone()],
        Some(&payer.pubkey()),
        &[&payer, &unwind_stake],
        blockhash,
    );
    let simulation = test_f
        .context
        .borrow_mut()
        .banks_client
        .simulate_transaction(tx)
        .await?;
    assert!(
        simulation
            .result
            .as_ref()
            .is_some_and(|result| result.is_err()),
        "invalid source stake account must fail"
    );
    let logs = simulation
        .simulation_details
        .as_ref()
        .map(|details| details.logs.join("\n"))
        .unwrap_or_default();
    let stake_pool_id = spl_stake_pool::id().to_string();
    anyhow::ensure!(
        logs.contains(&format!("Program {stake_pool_id} invoke")),
        "expected CPI into SPL Stake Pool; simulation logs were:\n{logs}"
    );
    anyhow::ensure!(
        logs.contains(&format!("Program {stake_pool_id} failed")),
        "expected SPL Stake Pool CPI failure; simulation logs were:\n{logs}"
    );

    let result = process_ix(&test_f, ix, &[&unwind_stake]).await;
    assert!(
        result.is_err(),
        "invalid source stake account must fail at Stake Pool CPI"
    );

    assert_eq!(user_before, raw_account_data(&test_f, user_key).await?);
    assert_eq!(
        lst_bank_before,
        raw_account_data(&test_f, lst_bank_key).await?
    );
    assert_eq!(
        sol_bank_before,
        raw_account_data(&test_f, sol_bank_f.key).await?
    );
    assert_eq!(
        load_token_account(&test_f, lst_bank_before_state.liquidity_vault)
            .await?
            .amount,
        vault_before,
        "failed CPI must not burn LST pool tokens"
    );
    assert!(test_f.try_load(&ticket_key).await?.is_none());
    assert!(test_f.try_load(&unwind_stake.pubkey()).await?.is_none());
    assert_eq!(
        invalid_source_before,
        raw_account_data(&test_f, invalid_source).await?
    );
    assert_eq!(
        pool_before,
        raw_account_data(&test_f, pool.stake_pool).await?
    );
    assert_eq!(
        validator_list_before,
        raw_account_data(&test_f, pool.validator_list).await?
    );
    assert_eq!(
        source_stake_before,
        raw_account_data(&test_f, pool.pool_source_stake_account).await?
    );
    assert_eq!(
        pool_mint_before,
        raw_account_data(&test_f, pool.pool_mint).await?
    );
    assert_eq!(
        manager_fee_before,
        raw_account_data(&test_f, pool.manager_fee_account).await?
    );
    Ok(())
}

async fn seed_settlement_ticket(
    test_f: &TestFixture,
    sol_bank_f: &BankFixture,
    stake_key: Pubkey,
    principal: u64,
    surplus: u64,
    created_epoch: u64,
    deactivation_epoch: u64,
) -> anyhow::Result<Pubkey> {
    let clock = test_f.get_clock().await;
    let rent = test_f.context.borrow_mut().banks_client.get_rent().await?;
    let (vault_authority, _) = Pubkey::find_program_address(
        &[
            LIQUIDITY_VAULT_AUTHORITY_SEED.as_bytes(),
            sol_bank_f.key.as_ref(),
        ],
        &marginfi::ID,
    );
    let stake_rent = rent.minimum_balance(StakeStateV2::size_of());
    let meta = test_stake_meta(vault_authority, stake_rent);

    let mut stake = Stake::default();
    stake.delegation.stake = principal + surplus;
    stake.delegation.activation_epoch = deactivation_epoch.saturating_sub(1);
    stake.delegation.deactivation_epoch = deactivation_epoch;
    let stake_state = StakeStateV2::Stake(meta, stake, StakeFlags::empty());
    let stake_account = Account {
        lamports: stake_rent + principal + surplus,
        data: stake_state_bytes(&stake_state)?,
        owner: STAKE_PROGRAM_ID,
        executable: false,
        rent_epoch: 0,
    };
    test_f
        .context
        .borrow_mut()
        .set_account(&stake_key, &AccountSharedData::from(stake_account));

    let mut bank_account = test_f
        .try_load(&sol_bank_f.key)
        .await?
        .ok_or_else(|| anyhow::anyhow!("SOL bank missing"))?;
    {
        let bank = bytemuck::from_bytes_mut::<Bank>(&mut bank_account.data[8..]);
        bank.total_asset_shares = I80F48::from_num(1_000_000_000_000i64).into();
        bank.asset_share_value = I80F48::ONE.into();
        bank.total_liability_shares = I80F48::ZERO.into();
        bank.liability_share_value = I80F48::ONE.into();
        bank.pending_unwind_lamports = principal;
        bank.last_update = clock.unix_timestamp;
    }
    test_f
        .context
        .borrow_mut()
        .set_account(&sol_bank_f.key, &AccountSharedData::from(bank_account));

    let (ticket_key, bump) = unwind_ticket_pda(stake_key);
    let ticket = UnwindTicket {
        sol_bank: sol_bank_f.key,
        stake_account: stake_key,
        rent_recipient: test_f.context.borrow().payer.pubkey(),
        principal_lamports: principal,
        stake_rent_prefund_lamports: stake_rent,
        created_epoch,
        bump,
    };
    let mut ticket_data = Vec::new();
    ticket.try_serialize(&mut ticket_data)?;
    let ticket_account = Account {
        lamports: rent.minimum_balance(ticket_data.len()),
        data: ticket_data,
        owner: marginfi::ID,
        executable: false,
        rent_epoch: 0,
    };
    test_f
        .context
        .borrow_mut()
        .set_account(&ticket_key, &AccountSharedData::from(ticket_account));
    Ok(ticket_key)
}

fn make_settle_unwind_ticket_ix(
    test_f: &TestFixture,
    sol_bank_f: &BankFixture,
    ticket_key: Pubkey,
    stake_key: Pubkey,
    rent_recipient: Pubkey,
    vault_key: Pubkey,
) -> Instruction {
    let (vault_authority, _) = Pubkey::find_program_address(
        &[
            LIQUIDITY_VAULT_AUTHORITY_SEED.as_bytes(),
            sol_bank_f.key.as_ref(),
        ],
        &marginfi::ID,
    );
    Instruction {
        program_id: marginfi::ID,
        accounts: marginfi::accounts::SettleUnwindTicket {
            group: test_f.marginfi_group.key,
            sol_bank: sol_bank_f.key,
            unwind_ticket: ticket_key,
            stake_account: stake_key,
            rent_recipient,
            sol_liquidity_vault: vault_key,
            sol_liquidity_vault_authority: vault_authority,
            stake_program: STAKE_PROGRAM_ID,
            clock: solana_sdk::sysvar::clock::ID,
            stake_history: solana_stake_interface::sysvar::stake_history::ID,
            token_program: spl_token::ID,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::SettleUnwindTicket {}.data(),
    }
}

#[tokio::test]
async fn test_settle_unwind_ticket_rejects_before_deactivation_epoch() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let sol_bank_f = create_native_sol_bank_for_unwind(&test_f).await?;
    let initial_clock = test_f.get_clock().await;
    let epoch_schedule: EpochSchedule = test_f
        .context
        .borrow_mut()
        .banks_client
        .get_sysvar()
        .await?;
    test_f
        .context
        .borrow_mut()
        .warp_to_slot(epoch_schedule.get_first_slot_in_epoch(initial_clock.epoch + 1))?;
    let clock = test_f.get_clock().await;
    anyhow::ensure!(clock.epoch > initial_clock.epoch);
    let stake_key = Pubkey::new_unique();
    let principal = 100_000_000;
    let ticket_key = seed_settlement_ticket(
        &test_f,
        &sol_bank_f,
        stake_key,
        principal,
        0,
        clock.epoch - 1,
        clock.epoch,
    )
    .await?;
    let bank_before = raw_account_data(&test_f, sol_bank_f.key).await?;
    let ticket_before = raw_account_data(&test_f, ticket_key).await?;
    let vault_key = sol_bank_f.load().await.liquidity_vault;
    let vault_before = load_token_account(&test_f, vault_key).await?.amount;
    let ix = make_settle_unwind_ticket_ix(
        &test_f,
        &sol_bank_f,
        ticket_key,
        stake_key,
        test_f.context.borrow().payer.pubkey(),
        vault_key,
    );
    let result = process_ix(&test_f, ix, &[]).await;
    assert!(
        result.is_err(),
        "settlement must fail before the stake deactivation epoch has passed"
    );
    assert_custom_error!(result.unwrap_err(), MarginfiError::EpochNotOver);
    assert_eq!(
        bank_before,
        raw_account_data(&test_f, sol_bank_f.key).await?
    );
    assert_eq!(ticket_before, raw_account_data(&test_f, ticket_key).await?);
    assert_eq!(
        vault_before,
        load_token_account(&test_f, vault_key).await?.amount
    );
    Ok(())
}

#[tokio::test]
async fn test_settle_unwind_ticket_credits_vault_releases_pending_and_closes_ticket(
) -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let sol_bank_f = create_native_sol_bank_for_unwind(&test_f).await?;

    const PRINCIPAL_LAMPORTS: u64 = 100_000_000;
    const SURPLUS_LAMPORTS: u64 = 1_000_000;
    const ASSET_SHARES: u64 = 1_000_000_000_000;

    let target_epoch = test_f.get_clock().await.epoch + 3;
    let epoch_schedule: EpochSchedule = test_f
        .context
        .borrow_mut()
        .banks_client
        .get_sysvar()
        .await?;
    let last_slot_in_target_epoch = epoch_schedule
        .get_first_slot_in_epoch(target_epoch + 1)
        .saturating_sub(1);
    test_f
        .context
        .borrow_mut()
        .warp_to_slot(last_slot_in_target_epoch)?;
    let clock = test_f.get_clock().await;
    anyhow::ensure!(clock.epoch > 1);
    let deactivation_epoch = clock.epoch - 2;

    let mut stake_history = StakeHistory::default();
    for epoch in 0..clock.epoch {
        stake_history.add(epoch, StakeHistoryEntry::default());
    }
    test_f.context.borrow().set_sysvar(&stake_history);

    let stake_key = Pubkey::new_unique();
    let ticket_key = seed_settlement_ticket(
        &test_f,
        &sol_bank_f,
        stake_key,
        PRINCIPAL_LAMPORTS,
        SURPLUS_LAMPORTS,
        clock.epoch - 1,
        deactivation_epoch,
    )
    .await?;
    let sol_bank_before = sol_bank_f.load().await;
    let vault_before = load_token_account(&test_f, sol_bank_before.liquidity_vault)
        .await?
        .amount;
    let ix = make_settle_unwind_ticket_ix(
        &test_f,
        &sol_bank_f,
        ticket_key,
        stake_key,
        test_f.context.borrow().payer.pubkey(),
        sol_bank_before.liquidity_vault,
    );
    process_ix(&test_f, ix, &[]).await?;

    let vault_after = load_token_account(&test_f, sol_bank_before.liquidity_vault)
        .await?
        .amount;
    assert_eq!(
        vault_after - vault_before,
        PRINCIPAL_LAMPORTS + SURPLUS_LAMPORTS
    );
    let sol_bank_after = sol_bank_f.load().await;
    assert_eq!(sol_bank_after.pending_unwind_lamports, 0);
    let expected_share_value =
        I80F48::ONE + I80F48::from_num(SURPLUS_LAMPORTS) / I80F48::from_num(ASSET_SHARES);
    assert_eq!(
        I80F48::from(sol_bank_after.asset_share_value),
        expected_share_value
    );
    assert!(test_f.try_load(&ticket_key).await?.is_none());
    Ok(())
}
