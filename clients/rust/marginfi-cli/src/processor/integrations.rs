use {
    crate::{
        config::Config,
        profile::Profile,
        utils::{
            derive_juplend_cpi_accounts, find_bank_vault_authority_pda, find_fee_state_pda,
            send_tx, EXP_10_I80F48, JUPLEND_LENDING_PROGRAM_ID,
        },
    },
    anchor_client::anchor_lang::{InstructionData, ToAccountMetas},
    anyhow::{Context, Result},
    fixed::types::I80F48,
    marginfi::state::bank::BankVaultType,
    marginfi_type_crate::types::Bank,
    solana_sdk::{
        instruction::Instruction, pubkey::Pubkey, signer::Signer, system_program, sysvar,
    },
};

// Known program IDs for integrations
const KAMINO_PROGRAM_ID: Pubkey =
    solana_sdk::pubkey!("KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD");
const FARMS_PROGRAM_ID: Pubkey =
    solana_sdk::pubkey!("FarmsPZpWu9i7Kky8tPN37rs2TpmMrAZrC7S7vJa91Hr");
const DRIFT_PROGRAM_ID: Pubkey = solana_sdk::pubkey!("dRiftyHA39MWEi3m9aunc5MzRF1JYuBsbn6VPcn33UH");

// ---------------------------------------------------------------------------
// Kamino
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
pub fn kamino_init_obligation(
    _profile: &Profile,
    config: &Config,
    bank_pk: Pubkey,
    amount: u64,
    lending_market: Pubkey,
    lending_market_authority: Pubkey,
    reserve_liquidity_supply: Pubkey,
    reserve_collateral_mint: Pubkey,
    reserve_destination_deposit_collateral: Pubkey,
    user_metadata: Pubkey,
    pyth_oracle: Option<Pubkey>,
    switchboard_price_oracle: Option<Pubkey>,
    switchboard_twap_oracle: Option<Pubkey>,
    scope_prices: Option<Pubkey>,
    obligation_farm_user_state: Option<Pubkey>,
    reserve_farm_state: Option<Pubkey>,
) -> Result<()> {
    let rpc_client = config.mfi_program.rpc();
    let signer = config.get_non_ms_authority_keypair()?;
    let bank = config.mfi_program.account::<Bank>(bank_pk)?;

    let (liquidity_vault_authority, _) =
        find_bank_vault_authority_pda(&bank_pk, BankVaultType::Liquidity, &config.program_id);

    let bank_mint_account = rpc_client.get_account(&bank.mint)?;
    let token_program = bank_mint_account.owner;

    let user_ata = anchor_spl::associated_token::get_associated_token_address_with_program_id(
        &signer.pubkey(),
        &bank.mint,
        &token_program,
    );

    let ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::KaminoInitObligation {
            fee_payer: signer.pubkey(),
            bank: bank_pk,
            signer_token_account: user_ata,
            liquidity_vault_authority,
            liquidity_vault: bank.liquidity_vault,
            integration_acc_2: bank.integration_acc_2,
            user_metadata,
            lending_market,
            lending_market_authority,
            integration_acc_1: bank.integration_acc_1,
            mint: bank.mint,
            reserve_liquidity_supply,
            reserve_collateral_mint,
            reserve_destination_deposit_collateral,
            pyth_oracle,
            switchboard_price_oracle,
            switchboard_twap_oracle,
            scope_prices,
            obligation_farm_user_state,
            reserve_farm_state,
            kamino_program: KAMINO_PROGRAM_ID,
            farms_program: FARMS_PROGRAM_ID,
            collateral_token_program: anchor_spl::token::ID,
            liquidity_token_program: token_program,
            instruction_sysvar_account: sysvar::instructions::ID,
            rent: sysvar::rent::ID,
            system_program: system_program::ID,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::KaminoInitObligation { amount }.data(),
    };

    let sig = send_tx(config, vec![ix], &[signer])?;
    println!("Kamino init obligation successful: {sig}");

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn kamino_deposit(
    profile: &Profile,
    config: &Config,
    bank_pk: Pubkey,
    ui_amount: f64,
    lending_market: Pubkey,
    lending_market_authority: Pubkey,
    reserve_liquidity_supply: Pubkey,
    reserve_collateral_mint: Pubkey,
    reserve_destination_deposit_collateral: Pubkey,
    obligation_farm_user_state: Option<Pubkey>,
    reserve_farm_state: Option<Pubkey>,
) -> Result<()> {
    let rpc_client = config.mfi_program.rpc();
    let signer = config.get_non_ms_authority_keypair()?;
    let marginfi_account_pk = profile.get_marginfi_account();
    let bank = config.mfi_program.account::<Bank>(bank_pk)?;

    let amount = (I80F48::from_num(ui_amount) * EXP_10_I80F48[bank.mint_decimals as usize])
        .floor()
        .to_num::<u64>();

    let (liquidity_vault_authority, _) =
        find_bank_vault_authority_pda(&bank_pk, BankVaultType::Liquidity, &config.program_id);

    let bank_mint_account = rpc_client.get_account(&bank.mint)?;
    let token_program = bank_mint_account.owner;

    let user_ata = anchor_spl::associated_token::get_associated_token_address_with_program_id(
        &signer.pubkey(),
        &bank.mint,
        &token_program,
    );

    let ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::KaminoDeposit {
            group: profile
                .marginfi_group
                .context("marginfi group not set in profile")?,
            marginfi_account: marginfi_account_pk,
            authority: signer.pubkey(),
            bank: bank_pk,
            signer_token_account: user_ata,
            liquidity_vault_authority,
            liquidity_vault: bank.liquidity_vault,
            integration_acc_2: bank.integration_acc_2,
            lending_market,
            lending_market_authority,
            integration_acc_1: bank.integration_acc_1,
            mint: bank.mint,
            reserve_liquidity_supply,
            reserve_collateral_mint,
            reserve_destination_deposit_collateral,
            obligation_farm_user_state,
            reserve_farm_state,
            kamino_program: KAMINO_PROGRAM_ID,
            farms_program: FARMS_PROGRAM_ID,
            collateral_token_program: anchor_spl::token::ID,
            liquidity_token_program: token_program,
            instruction_sysvar_account: sysvar::instructions::ID,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::KaminoDeposit { amount }.data(),
    };

    let sig = send_tx(config, vec![ix], &[signer])?;
    println!("Kamino deposit successful: {sig}");

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn kamino_withdraw(
    profile: &Profile,
    config: &Config,
    bank_pk: Pubkey,
    ui_amount: f64,
    withdraw_all: bool,
    lending_market: Pubkey,
    lending_market_authority: Pubkey,
    reserve_liquidity_supply: Pubkey,
    reserve_collateral_mint: Pubkey,
    reserve_source_collateral: Pubkey,
    obligation_farm_user_state: Option<Pubkey>,
    reserve_farm_state: Option<Pubkey>,
) -> Result<()> {
    let rpc_client = config.mfi_program.rpc();
    let signer = config.get_non_ms_authority_keypair()?;
    let marginfi_account_pk = profile.get_marginfi_account();
    let bank = config.mfi_program.account::<Bank>(bank_pk)?;

    let amount = (I80F48::from_num(ui_amount) * EXP_10_I80F48[bank.mint_decimals as usize])
        .floor()
        .to_num::<u64>();

    let (liquidity_vault_authority, _) =
        find_bank_vault_authority_pda(&bank_pk, BankVaultType::Liquidity, &config.program_id);

    let bank_mint_account = rpc_client.get_account(&bank.mint)?;
    let token_program = bank_mint_account.owner;

    let user_ata = anchor_spl::associated_token::get_associated_token_address_with_program_id(
        &signer.pubkey(),
        &bank.mint,
        &token_program,
    );

    let ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::KaminoWithdraw {
            group: profile
                .marginfi_group
                .context("marginfi group not set in profile")?,
            marginfi_account: marginfi_account_pk,
            authority: signer.pubkey(),
            bank: bank_pk,
            destination_token_account: user_ata,
            liquidity_vault_authority,
            liquidity_vault: bank.liquidity_vault,
            integration_acc_2: bank.integration_acc_2,
            lending_market,
            lending_market_authority,
            integration_acc_1: bank.integration_acc_1,
            reserve_liquidity_mint: bank.mint,
            reserve_liquidity_supply,
            reserve_collateral_mint,
            reserve_source_collateral,
            obligation_farm_user_state,
            reserve_farm_state,
            kamino_program: KAMINO_PROGRAM_ID,
            farms_program: FARMS_PROGRAM_ID,
            collateral_token_program: anchor_spl::token::ID,
            liquidity_token_program: token_program,
            instruction_sysvar_account: sysvar::instructions::ID,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::KaminoWithdraw {
            amount,
            withdraw_all: if withdraw_all { Some(true) } else { None },
        }
        .data(),
    };

    let sig = send_tx(config, vec![ix], &[signer])?;
    println!("Kamino withdraw successful: {sig}");

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn kamino_harvest_reward(
    config: &Config,
    bank_pk: Pubkey,
    reward_index: u64,
    user_state: Pubkey,
    farm_state: Pubkey,
    global_config: Pubkey,
    reward_mint: Pubkey,
    user_reward_ata: Pubkey,
    rewards_vault: Pubkey,
    rewards_treasury_vault: Pubkey,
    farm_vaults_authority: Pubkey,
    scope_prices: Option<Pubkey>,
) -> Result<()> {
    let rpc_client = config.mfi_program.rpc();
    let signer = config.get_non_ms_authority_keypair()?;
    let bank = config.mfi_program.account::<Bank>(bank_pk)?;

    let (liquidity_vault_authority, _) =
        find_bank_vault_authority_pda(&bank_pk, BankVaultType::Liquidity, &config.program_id);
    let (fee_state, _) = find_fee_state_pda(&config.program_id);

    let reward_mint_account = rpc_client.get_account(&reward_mint)?;
    let reward_token_program = reward_mint_account.owner;

    let fee_state_data = config
        .mfi_program
        .account::<marginfi_type_crate::types::FeeState>(fee_state)?;
    let destination_token_account =
        anchor_spl::associated_token::get_associated_token_address_with_program_id(
            &fee_state_data.global_fee_wallet,
            &reward_mint,
            &reward_token_program,
        );

    let _ = bank; // bank was loaded to validate it exists

    let ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::KaminoHarvestReward {
            bank: bank_pk,
            fee_state,
            destination_token_account,
            liquidity_vault_authority,
            user_state,
            farm_state,
            global_config,
            reward_mint,
            user_reward_ata,
            rewards_vault,
            rewards_treasury_vault,
            farm_vaults_authority,
            scope_prices,
            farms_program: FARMS_PROGRAM_ID,
            token_program: reward_token_program,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::KaminoHarvestReward { reward_index }.data(),
    };

    let sig = send_tx(config, vec![ix], &[signer])?;
    println!("Kamino harvest reward successful: {sig}");

    Ok(())
}

// ---------------------------------------------------------------------------
// Drift
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
pub fn drift_init_user(
    _profile: &Profile,
    config: &Config,
    bank_pk: Pubkey,
    amount: u64,
    drift_state: Pubkey,
    drift_spot_market_vault: Pubkey,
    drift_oracle: Option<Pubkey>,
) -> Result<()> {
    let rpc_client = config.mfi_program.rpc();
    let signer = config.get_non_ms_authority_keypair()?;
    let bank = config.mfi_program.account::<Bank>(bank_pk)?;

    let (liquidity_vault_authority, _) =
        find_bank_vault_authority_pda(&bank_pk, BankVaultType::Liquidity, &config.program_id);

    let bank_mint_account = rpc_client.get_account(&bank.mint)?;
    let token_program = bank_mint_account.owner;

    let user_ata = anchor_spl::associated_token::get_associated_token_address_with_program_id(
        &signer.pubkey(),
        &bank.mint,
        &token_program,
    );

    let ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::DriftInitUser {
            fee_payer: signer.pubkey(),
            signer_token_account: user_ata,
            bank: bank_pk,
            liquidity_vault_authority,
            liquidity_vault: bank.liquidity_vault,
            mint: bank.mint,
            integration_acc_3: bank.integration_acc_3,
            integration_acc_2: bank.integration_acc_2,
            drift_state,
            integration_acc_1: bank.integration_acc_1,
            drift_spot_market_vault,
            drift_oracle,
            drift_program: DRIFT_PROGRAM_ID,
            token_program,
            rent: sysvar::rent::ID,
            system_program: system_program::ID,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::DriftInitUser { amount }.data(),
    };

    let sig = send_tx(config, vec![ix], &[signer])?;
    println!("Drift init user successful: {sig}");

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn drift_deposit(
    profile: &Profile,
    config: &Config,
    bank_pk: Pubkey,
    ui_amount: f64,
    drift_state: Pubkey,
    drift_spot_market_vault: Pubkey,
    drift_oracle: Option<Pubkey>,
) -> Result<()> {
    let rpc_client = config.mfi_program.rpc();
    let signer = config.get_non_ms_authority_keypair()?;
    let marginfi_account_pk = profile.get_marginfi_account();
    let bank = config.mfi_program.account::<Bank>(bank_pk)?;

    let amount = (I80F48::from_num(ui_amount) * EXP_10_I80F48[bank.mint_decimals as usize])
        .floor()
        .to_num::<u64>();

    let (liquidity_vault_authority, _) =
        find_bank_vault_authority_pda(&bank_pk, BankVaultType::Liquidity, &config.program_id);

    let bank_mint_account = rpc_client.get_account(&bank.mint)?;
    let token_program = bank_mint_account.owner;

    let user_ata = anchor_spl::associated_token::get_associated_token_address_with_program_id(
        &signer.pubkey(),
        &bank.mint,
        &token_program,
    );

    let ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::DriftDeposit {
            group: profile
                .marginfi_group
                .context("marginfi group not set in profile")?,
            marginfi_account: marginfi_account_pk,
            authority: signer.pubkey(),
            bank: bank_pk,
            drift_oracle,
            liquidity_vault_authority,
            liquidity_vault: bank.liquidity_vault,
            signer_token_account: user_ata,
            drift_state,
            integration_acc_2: bank.integration_acc_2,
            integration_acc_3: bank.integration_acc_3,
            integration_acc_1: bank.integration_acc_1,
            drift_spot_market_vault,
            mint: bank.mint,
            drift_program: DRIFT_PROGRAM_ID,
            token_program,
            system_program: system_program::ID,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::DriftDeposit { amount }.data(),
    };

    let sig = send_tx(config, vec![ix], &[signer])?;
    println!("Drift deposit successful: {sig}");

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn drift_withdraw(
    profile: &Profile,
    config: &Config,
    bank_pk: Pubkey,
    ui_amount: f64,
    withdraw_all: bool,
    drift_state: Pubkey,
    drift_spot_market_vault: Pubkey,
    drift_oracle: Option<Pubkey>,
    drift_signer: Pubkey,
    drift_reward_oracle: Option<Pubkey>,
    drift_reward_spot_market: Option<Pubkey>,
    drift_reward_mint: Option<Pubkey>,
    drift_reward_oracle_2: Option<Pubkey>,
    drift_reward_spot_market_2: Option<Pubkey>,
    drift_reward_mint_2: Option<Pubkey>,
) -> Result<()> {
    let rpc_client = config.mfi_program.rpc();
    let signer = config.get_non_ms_authority_keypair()?;
    let marginfi_account_pk = profile.get_marginfi_account();
    let bank = config.mfi_program.account::<Bank>(bank_pk)?;

    let amount = (I80F48::from_num(ui_amount) * EXP_10_I80F48[bank.mint_decimals as usize])
        .floor()
        .to_num::<u64>();

    let (liquidity_vault_authority, _) =
        find_bank_vault_authority_pda(&bank_pk, BankVaultType::Liquidity, &config.program_id);

    let bank_mint_account = rpc_client.get_account(&bank.mint)?;
    let token_program = bank_mint_account.owner;

    let user_ata = anchor_spl::associated_token::get_associated_token_address_with_program_id(
        &signer.pubkey(),
        &bank.mint,
        &token_program,
    );

    let ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::DriftWithdraw {
            group: profile
                .marginfi_group
                .context("marginfi group not set in profile")?,
            marginfi_account: marginfi_account_pk,
            authority: signer.pubkey(),
            bank: bank_pk,
            drift_oracle,
            liquidity_vault_authority,
            liquidity_vault: bank.liquidity_vault,
            destination_token_account: user_ata,
            drift_state,
            integration_acc_2: bank.integration_acc_2,
            integration_acc_3: bank.integration_acc_3,
            integration_acc_1: bank.integration_acc_1,
            drift_spot_market_vault,
            drift_reward_oracle,
            drift_reward_spot_market,
            drift_reward_mint,
            drift_reward_oracle_2,
            drift_reward_spot_market_2,
            drift_reward_mint_2,
            drift_signer,
            mint: bank.mint,
            drift_program: DRIFT_PROGRAM_ID,
            token_program,
            system_program: system_program::ID,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::DriftWithdraw {
            amount,
            withdraw_all: if withdraw_all { Some(true) } else { None },
        }
        .data(),
    };

    let sig = send_tx(config, vec![ix], &[signer])?;
    println!("Drift withdraw successful: {sig}");

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn drift_harvest_reward(
    config: &Config,
    bank_pk: Pubkey,
    drift_state: Pubkey,
    drift_signer: Pubkey,
    harvest_drift_spot_market: Pubkey,
    harvest_drift_spot_market_vault: Pubkey,
    reward_mint: Pubkey,
) -> Result<()> {
    let rpc_client = config.mfi_program.rpc();
    let signer = config.get_non_ms_authority_keypair()?;
    let bank = config.mfi_program.account::<Bank>(bank_pk)?;

    let (liquidity_vault_authority, _) =
        find_bank_vault_authority_pda(&bank_pk, BankVaultType::Liquidity, &config.program_id);
    let (fee_state, _) = find_fee_state_pda(&config.program_id);

    let reward_mint_account = rpc_client.get_account(&reward_mint)?;
    let reward_token_program = reward_mint_account.owner;

    let fee_state_data = config
        .mfi_program
        .account::<marginfi_type_crate::types::FeeState>(fee_state)?;

    let intermediary_token_account =
        anchor_spl::associated_token::get_associated_token_address_with_program_id(
            &liquidity_vault_authority,
            &reward_mint,
            &reward_token_program,
        );

    let destination_token_account =
        anchor_spl::associated_token::get_associated_token_address_with_program_id(
            &fee_state_data.global_fee_wallet,
            &reward_mint,
            &reward_token_program,
        );

    let _ = bank; // bank was loaded to validate it exists

    let ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::DriftHarvestReward {
            bank: bank_pk,
            fee_state,
            liquidity_vault_authority,
            intermediary_token_account,
            destination_token_account,
            drift_state,
            integration_acc_2: bank.integration_acc_2,
            integration_acc_3: bank.integration_acc_3,
            harvest_drift_spot_market,
            harvest_drift_spot_market_vault,
            drift_signer,
            reward_mint,
            drift_program: DRIFT_PROGRAM_ID,
            token_program: reward_token_program,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::DriftHarvestReward {}.data(),
    };

    let sig = send_tx(config, vec![ix], &[signer])?;
    println!("Drift harvest reward successful: {sig}");

    Ok(())
}

// ---------------------------------------------------------------------------
// JupLend
// ---------------------------------------------------------------------------

pub fn juplend_init_position(
    _profile: &Profile,
    config: &Config,
    bank_pk: Pubkey,
    amount: u64,
) -> Result<()> {
    let rpc_client = config.mfi_program.rpc();
    let signer = config.get_non_ms_authority_keypair()?;
    let bank = config.mfi_program.account::<Bank>(bank_pk)?;

    let (liquidity_vault_authority, _) =
        find_bank_vault_authority_pda(&bank_pk, BankVaultType::Liquidity, &config.program_id);

    let jl = derive_juplend_cpi_accounts(&rpc_client, &bank, &liquidity_vault_authority)?;

    let bank_mint_account = rpc_client.get_account(&bank.mint)?;
    let token_program = bank_mint_account.owner;

    let user_ata = anchor_spl::associated_token::get_associated_token_address_with_program_id(
        &signer.pubkey(),
        &bank.mint,
        &token_program,
    );

    let ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::JuplendInitPosition {
            fee_payer: signer.pubkey(),
            signer_token_account: user_ata,
            bank: bank_pk,
            liquidity_vault_authority,
            liquidity_vault: bank.liquidity_vault,
            mint: bank.mint,
            integration_acc_1: bank.integration_acc_1,
            f_token_mint: jl.f_token_mint,
            integration_acc_2: bank.integration_acc_2,
            lending_admin: jl.lending_admin,
            supply_token_reserves_liquidity: jl.supply_token_reserves_liquidity,
            lending_supply_position_on_liquidity: jl.lending_supply_position_on_liquidity,
            rate_model: jl.rate_model,
            vault: jl.vault,
            liquidity: jl.liquidity,
            liquidity_program: jl.liquidity_program,
            rewards_rate_model: jl.rewards_rate_model,
            juplend_program: JUPLEND_LENDING_PROGRAM_ID,
            token_program,
            associated_token_program: anchor_spl::associated_token::ID,
            system_program: system_program::ID,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::JuplendInitPosition { amount }.data(),
    };

    let sig = send_tx(config, vec![ix], &[signer])?;
    println!("JupLend init position successful: {sig}");

    Ok(())
}

pub fn juplend_deposit(
    profile: &Profile,
    config: &Config,
    bank_pk: Pubkey,
    ui_amount: f64,
) -> Result<()> {
    let rpc_client = config.mfi_program.rpc();
    let signer = config.get_non_ms_authority_keypair()?;
    let marginfi_account_pk = profile.get_marginfi_account();
    let bank = config.mfi_program.account::<Bank>(bank_pk)?;

    let amount = (I80F48::from_num(ui_amount) * EXP_10_I80F48[bank.mint_decimals as usize])
        .floor()
        .to_num::<u64>();

    let (liquidity_vault_authority, _) =
        find_bank_vault_authority_pda(&bank_pk, BankVaultType::Liquidity, &config.program_id);

    let jl = derive_juplend_cpi_accounts(&rpc_client, &bank, &liquidity_vault_authority)?;

    let bank_mint_account = rpc_client.get_account(&bank.mint)?;
    let token_program = bank_mint_account.owner;

    let user_ata = anchor_spl::associated_token::get_associated_token_address_with_program_id(
        &signer.pubkey(),
        &bank.mint,
        &token_program,
    );

    let ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::JuplendDeposit {
            group: profile
                .marginfi_group
                .context("marginfi group not set in profile")?,
            marginfi_account: marginfi_account_pk,
            authority: signer.pubkey(),
            bank: bank_pk,
            signer_token_account: user_ata,
            liquidity_vault_authority,
            liquidity_vault: bank.liquidity_vault,
            mint: bank.mint,
            integration_acc_1: bank.integration_acc_1,
            f_token_mint: jl.f_token_mint,
            integration_acc_2: bank.integration_acc_2,
            lending_admin: jl.lending_admin,
            supply_token_reserves_liquidity: jl.supply_token_reserves_liquidity,
            lending_supply_position_on_liquidity: jl.lending_supply_position_on_liquidity,
            rate_model: jl.rate_model,
            vault: jl.vault,
            liquidity: jl.liquidity,
            liquidity_program: jl.liquidity_program,
            rewards_rate_model: jl.rewards_rate_model,
            juplend_program: JUPLEND_LENDING_PROGRAM_ID,
            token_program,
            associated_token_program: anchor_spl::associated_token::ID,
            system_program: system_program::ID,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::JuplendDeposit { amount }.data(),
    };

    let sig = send_tx(config, vec![ix], &[signer])?;
    println!("JupLend deposit successful: {sig}");

    Ok(())
}

pub fn juplend_withdraw(
    profile: &Profile,
    config: &Config,
    bank_pk: Pubkey,
    ui_amount: f64,
    withdraw_all: bool,
) -> Result<()> {
    let rpc_client = config.mfi_program.rpc();
    let signer = config.get_non_ms_authority_keypair()?;
    let marginfi_account_pk = profile.get_marginfi_account();
    let bank = config.mfi_program.account::<Bank>(bank_pk)?;

    let amount = (I80F48::from_num(ui_amount) * EXP_10_I80F48[bank.mint_decimals as usize])
        .floor()
        .to_num::<u64>();

    let (liquidity_vault_authority, _) =
        find_bank_vault_authority_pda(&bank_pk, BankVaultType::Liquidity, &config.program_id);

    let jl = derive_juplend_cpi_accounts(&rpc_client, &bank, &liquidity_vault_authority)?;

    let bank_mint_account = rpc_client.get_account(&bank.mint)?;
    let token_program = bank_mint_account.owner;

    let user_ata = anchor_spl::associated_token::get_associated_token_address_with_program_id(
        &signer.pubkey(),
        &bank.mint,
        &token_program,
    );

    let ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::JuplendWithdraw {
            group: profile
                .marginfi_group
                .context("marginfi group not set in profile")?,
            marginfi_account: marginfi_account_pk,
            authority: signer.pubkey(),
            bank: bank_pk,
            destination_token_account: user_ata,
            liquidity_vault_authority,
            mint: bank.mint,
            integration_acc_1: bank.integration_acc_1,
            f_token_mint: jl.f_token_mint,
            integration_acc_2: bank.integration_acc_2,
            integration_acc_3: bank.integration_acc_3,
            lending_admin: jl.lending_admin,
            supply_token_reserves_liquidity: jl.supply_token_reserves_liquidity,
            lending_supply_position_on_liquidity: jl.lending_supply_position_on_liquidity,
            rate_model: jl.rate_model,
            vault: jl.vault,
            claim_account: jl.claim_account,
            liquidity: jl.liquidity,
            liquidity_program: jl.liquidity_program,
            rewards_rate_model: jl.rewards_rate_model,
            juplend_program: JUPLEND_LENDING_PROGRAM_ID,
            token_program,
            associated_token_program: anchor_spl::associated_token::ID,
            system_program: system_program::ID,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::JuplendWithdraw {
            amount,
            withdraw_all: if withdraw_all { Some(true) } else { None },
        }
        .data(),
    };

    let sig = send_tx(config, vec![ix], &[signer])?;
    println!("JupLend withdraw successful: {sig}");

    Ok(())
}
