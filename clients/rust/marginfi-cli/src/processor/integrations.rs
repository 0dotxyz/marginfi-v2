use {
    super::load_all_banks,
    crate::{
        config::Config,
        profile::Profile,
        utils::{
            build_kamino_refresh_obligation_ix, build_kamino_refresh_reserve_ix,
            derive_juplend_cpi_accounts, derive_juplend_cpi_accounts_for_lending,
            find_bank_vault_authority_pda, find_bank_vault_pda, find_fee_state_pda,
            load_observation_account_metas, load_observation_account_metas_close_last, send_tx,
            EXP_10_I80F48, JUPLEND_LENDING_PROGRAM_ID,
        },
    },
    anchor_client::anchor_lang::{InstructionData, ToAccountMetas},
    anyhow::{Context, Result},
    fixed::types::I80F48,
    marginfi::state::{bank::BankVaultType, bank_config::BankConfigImpl},
    marginfi_type_crate::{
        pdas::{
            derive_kamino_lending_market_authority, derive_kamino_user_state, FARMS_PROGRAM_ID,
            KAMINO_PROGRAM_ID,
        },
        types::{Bank, MarginfiAccount, OracleSetup},
    },
    solana_sdk::{
        instruction::{AccountMeta, Instruction},
        pubkey::Pubkey,
        system_program, sysvar,
    },
    std::collections::HashMap,
};

const DRIFT_PROGRAM_ID: Pubkey = solana_sdk::pubkey!("dRiftyHA39MWEi3m9aunc5MzRF1JYuBsbn6VPcn33UH");

struct KaminoInitAccounts {
    lending_market_authority: Pubkey,
    reserve_liquidity_supply: Pubkey,
    reserve_collateral_mint: Pubkey,
    reserve_destination_deposit_collateral: Pubkey,
    user_metadata: Pubkey,
    obligation_farm_user_state: Option<Pubkey>,
    reserve_farm_state: Option<Pubkey>,
}

/// Derive Kamino refresh oracle accounts from a bank's oracle setup.
///
/// Returns `(pyth_oracle, switchboard_price_oracle, switchboard_twap_oracle, scope_prices)`.
fn kamino_refresh_oracle_accounts(
    bank: &Bank,
) -> (
    Option<Pubkey>,
    Option<Pubkey>,
    Option<Pubkey>,
    Option<Pubkey>,
) {
    let keys = &bank.config.oracle_keys;
    match bank.config.oracle_setup {
        OracleSetup::KaminoPythPush => (Some(keys[0]), None, None, None),
        OracleSetup::KaminoSwitchboardPull => (None, None, None, Some(keys[0])),
        _ => (None, None, None, None),
    }
}

/// Build the pair of Kamino refresh instructions (refreshReserve + refreshObligation)
/// that must be prepended before any Kamino deposit or withdraw.
fn build_kamino_refresh_ixs(bank: &Bank, lending_market: Pubkey) -> Vec<Instruction> {
    let (pyth_oracle, switchboard_price, switchboard_twap, scope_prices) =
        kamino_refresh_oracle_accounts(bank);
    let reserve = bank.integration_acc_1;
    let obligation = bank.integration_acc_2;

    vec![
        build_kamino_refresh_reserve_ix(
            reserve,
            lending_market,
            pyth_oracle,
            switchboard_price,
            switchboard_twap,
            scope_prices,
        ),
        build_kamino_refresh_obligation_ix(obligation, lending_market, reserve),
    ]
}

fn build_signer_ata_ix(
    config: &Config,
    owner: &Pubkey,
    mint: &Pubkey,
    token_program: &Pubkey,
) -> Instruction {
    spl_associated_token_account::instruction::create_associated_token_account_idempotent(
        &config.explicit_fee_payer(),
        owner,
        mint,
        token_program,
    )
}

fn load_withdraw_observation_metas(
    config: &Config,
    marginfi_account_pk: Pubkey,
    group: Pubkey,
    close_bank: Option<Pubkey>,
) -> Result<Vec<AccountMeta>> {
    let banks = HashMap::from_iter(load_all_banks(config, Some(group))?);
    let marginfi_account = config
        .mfi_program
        .account::<MarginfiAccount>(marginfi_account_pk)?;

    Ok(match close_bank {
        Some(close_bank) => load_observation_account_metas_close_last(
            &marginfi_account,
            &banks,
            vec![],
            vec![],
            close_bank,
        ),
        None => load_observation_account_metas(&marginfi_account, &banks, vec![], vec![]),
    })
}

fn derive_kamino_init_accounts(
    rpc_client: &solana_client::rpc_client::RpcClient,
    reserve: Pubkey,
    lending_market: Pubkey,
    _reserve_oracle: Pubkey,
    _oracle_setup: u8,
    obligation: Pubkey,
    liquidity_vault_authority: Pubkey,
) -> Result<KaminoInitAccounts> {
    use kamino_mocks::state::MinimalReserve;

    let reserve_data = rpc_client.get_account_data(&reserve)?;
    let reserve_size = std::mem::size_of::<MinimalReserve>();
    if reserve_data.len() < 8 + reserve_size {
        anyhow::bail!(
            "Kamino reserve account {} data too small ({} bytes)",
            reserve,
            reserve_data.len()
        );
    }
    let reserve_state: &MinimalReserve = bytemuck::from_bytes(&reserve_data[8..8 + reserve_size]);

    let (lending_market_authority, _) = derive_kamino_lending_market_authority(&lending_market);
    let (reserve_liquidity_supply, _) = Pubkey::find_program_address(
        &[b"reserve_liq_supply", reserve.as_ref()],
        &KAMINO_PROGRAM_ID,
    );
    let (reserve_collateral_mint, _) = Pubkey::find_program_address(
        &[b"reserve_coll_mint", reserve.as_ref()],
        &KAMINO_PROGRAM_ID,
    );
    let (reserve_destination_deposit_collateral, _) = Pubkey::find_program_address(
        &[b"reserve_coll_supply", reserve.as_ref()],
        &KAMINO_PROGRAM_ID,
    );
    let (user_metadata, _) = Pubkey::find_program_address(
        &[b"user_meta", liquidity_vault_authority.as_ref()],
        &KAMINO_PROGRAM_ID,
    );

    let reserve_farm_state = (reserve_state.farm_collateral != Pubkey::default())
        .then_some(reserve_state.farm_collateral);
    let obligation_farm_user_state =
        reserve_farm_state.map(|farm_state| derive_kamino_user_state(&farm_state, &obligation).0);

    Ok(KaminoInitAccounts {
        lending_market_authority,
        reserve_liquidity_supply,
        reserve_collateral_mint,
        reserve_destination_deposit_collateral,
        user_metadata,
        obligation_farm_user_state,
        reserve_farm_state,
    })
}

pub fn resolve_integration_bank_seed(
    config: &Config,
    group: Pubkey,
    mint: Pubkey,
    requested_seed: Option<u64>,
) -> Result<u64> {
    let rpc_client = config.mfi_program.rpc();

    if let Some(seed) = requested_seed {
        let (bank_pda, _) = Pubkey::find_program_address(
            &[group.as_ref(), mint.as_ref(), &seed.to_le_bytes()],
            &config.program_id,
        );
        if rpc_client.get_account(&bank_pda).is_ok() {
            anyhow::bail!(
                "seed {} already in use for mint {} in group {} (bank {})",
                seed,
                mint,
                group,
                bank_pda
            );
        }
        return Ok(seed);
    }

    for seed in 0..u64::MAX {
        let (bank_pda, _) = Pubkey::find_program_address(
            &[group.as_ref(), mint.as_ref(), &seed.to_le_bytes()],
            &config.program_id,
        );
        if rpc_client.get_account(&bank_pda).is_err() {
            return Ok(seed);
        }
    }

    anyhow::bail!(
        "unable to find a free bank seed for mint {} in group {}",
        mint,
        group
    );
}

pub fn find_existing_integration_bank(
    config: &Config,
    group: Pubkey,
    mint: Pubkey,
    asset_tag: u8,
    integration_acc_1: Pubkey,
) -> Result<Option<Pubkey>> {
    for (bank_pk, bank) in crate::processor::load_all_banks(config, Some(group))? {
        if bank.mint == mint
            && bank.config.asset_tag == asset_tag
            && bank.integration_acc_1 == integration_acc_1
        {
            return Ok(Some(bank_pk));
        }
    }

    Ok(None)
}

#[allow(clippy::too_many_arguments)]
fn build_kamino_init_obligation_ix(
    config: &Config,
    fee_payer: Pubkey,
    bank_pk: Pubkey,
    bank: &Bank,
    amount: u64,
    lending_market: Pubkey,
    lending_market_authority: Pubkey,
    reserve_liquidity_supply: Pubkey,
    reserve_collateral_mint: Pubkey,
    reserve_destination_deposit_collateral: Pubkey,
    user_metadata: Pubkey,
    _pyth_oracle: Option<Pubkey>,
    _switchboard_price_oracle: Option<Pubkey>,
    _switchboard_twap_oracle: Option<Pubkey>,
    _scope_prices: Option<Pubkey>,
    obligation_farm_user_state: Option<Pubkey>,
    reserve_farm_state: Option<Pubkey>,
    signer_token_account: Pubkey,
    token_program: Pubkey,
) -> Result<Instruction> {
    let (liquidity_vault_authority, _) =
        find_bank_vault_authority_pda(&bank_pk, BankVaultType::Liquidity, &config.program_id);

    Ok(Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::KaminoInitObligation {
            fee_payer,
            bank: bank_pk,
            signer_token_account,
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
    })
}

fn build_drift_init_user_ix(
    config: &Config,
    fee_payer: Pubkey,
    bank_pk: Pubkey,
    bank: &Bank,
    amount: u64,
    drift_state: Pubkey,
    drift_spot_market_vault: Pubkey,
    drift_oracle: Option<Pubkey>,
    signer_token_account: Pubkey,
    token_program: Pubkey,
) -> Result<Instruction> {
    let (liquidity_vault_authority, _) =
        find_bank_vault_authority_pda(&bank_pk, BankVaultType::Liquidity, &config.program_id);

    Ok(Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::DriftInitUser {
            fee_payer,
            signer_token_account,
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
    })
}

fn build_juplend_init_position_ix(
    config: &Config,
    fee_payer: Pubkey,
    bank_pk: Pubkey,
    bank: &Bank,
    amount: u64,
    jl: &crate::utils::JuplendCpiAccounts,
    signer_token_account: Pubkey,
    token_program: Pubkey,
) -> Result<Instruction> {
    let (liquidity_vault_authority, _) =
        find_bank_vault_authority_pda(&bank_pk, BankVaultType::Liquidity, &config.program_id);

    Ok(Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::JuplendInitPosition {
            fee_payer,
            signer_token_account,
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
    })
}

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
    let authority = config.authority();
    let bank = config.mfi_program.account::<Bank>(bank_pk)?;

    let bank_mint_account = rpc_client.get_account(&bank.mint)?;
    let token_program = bank_mint_account.owner;

    let user_ata = anchor_spl::associated_token::get_associated_token_address_with_program_id(
        &authority,
        &bank.mint,
        &token_program,
    );

    let create_ata_ix = build_signer_ata_ix(config, &authority, &bank.mint, &token_program);
    let ix = build_kamino_init_obligation_ix(
        config,
        authority,
        bank_pk,
        &bank,
        amount,
        lending_market,
        lending_market_authority,
        reserve_liquidity_supply,
        reserve_collateral_mint,
        reserve_destination_deposit_collateral,
        user_metadata,
        pyth_oracle,
        switchboard_price_oracle,
        switchboard_twap_oracle,
        scope_prices,
        obligation_farm_user_state,
        reserve_farm_state,
        user_ata,
        token_program,
    )?;

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(config, vec![create_ata_ix, ix], &signing_keypairs)?;
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
    let authority = config.authority();
    let marginfi_account_pk = profile.get_marginfi_account()?;
    let marginfi_account = config
        .mfi_program
        .account::<MarginfiAccount>(marginfi_account_pk)?;
    let bank = config.mfi_program.account::<Bank>(bank_pk)?;
    let group = marginfi_account.group;

    if bank.group != group {
        anyhow::bail!("Bank does not belong to group");
    }

    let amount = (I80F48::from_num(ui_amount) * EXP_10_I80F48[bank.mint_decimals as usize])
        .floor()
        .to_num::<u64>();

    let (liquidity_vault_authority, _) =
        find_bank_vault_authority_pda(&bank_pk, BankVaultType::Liquidity, &config.program_id);

    let bank_mint_account = rpc_client.get_account(&bank.mint)?;
    let token_program = bank_mint_account.owner;

    let user_ata = anchor_spl::associated_token::get_associated_token_address_with_program_id(
        &authority,
        &bank.mint,
        &token_program,
    );

    let ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::KaminoDeposit {
            group,
            marginfi_account: marginfi_account_pk,
            authority,
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
        data: marginfi::instruction::KaminoDeposit {
            amount,
            refresh_reserve: Some(false),
        }
        .data(),
    };

    // Prepend Kamino refresh instructions to ensure reserve/obligation are non-stale
    let mut ixs = build_kamino_refresh_ixs(&bank, lending_market);
    ixs.push(ix);

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(config, ixs, &signing_keypairs)?;
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
    let authority = config.authority();
    let marginfi_account_pk = profile.get_marginfi_account()?;
    let marginfi_account = config
        .mfi_program
        .account::<MarginfiAccount>(marginfi_account_pk)?;
    let bank = config.mfi_program.account::<Bank>(bank_pk)?;
    let group = marginfi_account.group;

    if bank.group != group {
        anyhow::bail!("Bank does not belong to group");
    }

    let collateral_decimals = rpc_client
        .get_token_supply(&reserve_collateral_mint)?
        .decimals;

    let amount = (I80F48::from_num(ui_amount) * EXP_10_I80F48[collateral_decimals as usize])
        .floor()
        .to_num::<u64>();

    let (liquidity_vault_authority, _) =
        find_bank_vault_authority_pda(&bank_pk, BankVaultType::Liquidity, &config.program_id);

    let bank_mint_account = rpc_client.get_account(&bank.mint)?;
    let token_program = bank_mint_account.owner;

    let user_ata = anchor_spl::associated_token::get_associated_token_address_with_program_id(
        &authority,
        &bank.mint,
        &token_program,
    );
    let observation_metas = load_withdraw_observation_metas(
        config,
        marginfi_account_pk,
        group,
        withdraw_all.then_some(bank_pk),
    )?;
    let flags = if withdraw_all {
        Some(0b0000_0001u8)
    } else {
        None
    };

    let mut ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::KaminoWithdraw {
            group,
            marginfi_account: marginfi_account_pk,
            authority,
            bank: bank_pk,
            destination_token_account: user_ata,
            liquidity_vault_authority,
            liquidity_vault: bank.liquidity_vault,
            integration_acc_2: bank.integration_acc_2,
            lending_market,
            lending_market_authority,
            integration_acc_1: bank.integration_acc_1,
            mint: bank.mint,
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
        data: marginfi::instruction::KaminoWithdraw { amount, flags }.data(),
    };
    ix.accounts.extend(observation_metas);

    let create_ata_ix = build_signer_ata_ix(config, &authority, &bank.mint, &token_program);

    // Prepend Kamino refresh instructions to ensure reserve/obligation are non-stale
    let mut ixs = build_kamino_refresh_ixs(&bank, lending_market);
    ixs.push(create_ata_ix);
    ixs.push(ix);

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(config, ixs, &signing_keypairs)?;
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

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(config, vec![ix], &signing_keypairs)?;
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
    let authority = config.authority();
    let bank = config.mfi_program.account::<Bank>(bank_pk)?;

    let bank_mint_account = rpc_client.get_account(&bank.mint)?;
    let token_program = bank_mint_account.owner;

    let user_ata = anchor_spl::associated_token::get_associated_token_address_with_program_id(
        &authority,
        &bank.mint,
        &token_program,
    );

    let create_ata_ix = build_signer_ata_ix(config, &authority, &bank.mint, &token_program);
    let ix = build_drift_init_user_ix(
        config,
        authority,
        bank_pk,
        &bank,
        amount,
        drift_state,
        drift_spot_market_vault,
        drift_oracle,
        user_ata,
        token_program,
    )?;

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(config, vec![create_ata_ix, ix], &signing_keypairs)?;
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
    let authority = config.authority();
    let marginfi_account_pk = profile.get_marginfi_account()?;
    let marginfi_account = config
        .mfi_program
        .account::<MarginfiAccount>(marginfi_account_pk)?;
    let bank = config.mfi_program.account::<Bank>(bank_pk)?;
    let group = marginfi_account.group;

    if bank.group != group {
        anyhow::bail!("Bank does not belong to group");
    }

    let amount = (I80F48::from_num(ui_amount) * EXP_10_I80F48[bank.mint_decimals as usize])
        .floor()
        .to_num::<u64>();

    let (liquidity_vault_authority, _) =
        find_bank_vault_authority_pda(&bank_pk, BankVaultType::Liquidity, &config.program_id);

    let bank_mint_account = rpc_client.get_account(&bank.mint)?;
    let token_program = bank_mint_account.owner;

    let user_ata = anchor_spl::associated_token::get_associated_token_address_with_program_id(
        &authority,
        &bank.mint,
        &token_program,
    );

    let ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::DriftDeposit {
            group,
            marginfi_account: marginfi_account_pk,
            authority,
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

    let create_ata_ix = build_signer_ata_ix(config, &authority, &bank.mint, &token_program);
    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(config, vec![create_ata_ix, ix], &signing_keypairs)?;
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
    let authority = config.authority();
    let marginfi_account_pk = profile.get_marginfi_account()?;
    let marginfi_account = config
        .mfi_program
        .account::<MarginfiAccount>(marginfi_account_pk)?;
    let bank = config.mfi_program.account::<Bank>(bank_pk)?;
    let group = marginfi_account.group;

    if bank.group != group {
        anyhow::bail!("Bank does not belong to group");
    }

    let amount = (I80F48::from_num(ui_amount) * EXP_10_I80F48[bank.mint_decimals as usize])
        .floor()
        .to_num::<u64>();

    let (liquidity_vault_authority, _) =
        find_bank_vault_authority_pda(&bank_pk, BankVaultType::Liquidity, &config.program_id);

    let bank_mint_account = rpc_client.get_account(&bank.mint)?;
    let token_program = bank_mint_account.owner;

    let user_ata = anchor_spl::associated_token::get_associated_token_address_with_program_id(
        &authority,
        &bank.mint,
        &token_program,
    );
    let observation_metas = load_withdraw_observation_metas(
        config,
        marginfi_account_pk,
        group,
        withdraw_all.then_some(bank_pk),
    )?;

    let mut ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::DriftWithdraw {
            group,
            marginfi_account: marginfi_account_pk,
            authority,
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
    ix.accounts.extend(observation_metas);

    let create_ata_ix = build_signer_ata_ix(config, &authority, &bank.mint, &token_program);

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(config, vec![create_ata_ix, ix], &signing_keypairs)?;
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

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(config, vec![ix], &signing_keypairs)?;
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
    let authority = config.authority();
    let bank = config.mfi_program.account::<Bank>(bank_pk)?;

    let (liquidity_vault_authority, _) =
        find_bank_vault_authority_pda(&bank_pk, BankVaultType::Liquidity, &config.program_id);

    let jl = derive_juplend_cpi_accounts(&rpc_client, &bank, &liquidity_vault_authority)?;

    let bank_mint_account = rpc_client.get_account(&bank.mint)?;
    let token_program = bank_mint_account.owner;

    let user_ata = anchor_spl::associated_token::get_associated_token_address_with_program_id(
        &authority,
        &bank.mint,
        &token_program,
    );

    let create_ata_ix = build_signer_ata_ix(config, &authority, &bank.mint, &token_program);
    let create_withdraw_intermediary_ata_ix = build_signer_ata_ix(
        config,
        &liquidity_vault_authority,
        &bank.mint,
        &token_program,
    );
    let ix = build_juplend_init_position_ix(
        config,
        authority,
        bank_pk,
        &bank,
        amount,
        &jl,
        user_ata,
        token_program,
    )?;

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(
        config,
        vec![create_ata_ix, create_withdraw_intermediary_ata_ix, ix],
        &signing_keypairs,
    )?;
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
    let authority = config.authority();
    let marginfi_account_pk = profile.get_marginfi_account()?;
    let marginfi_account = config
        .mfi_program
        .account::<MarginfiAccount>(marginfi_account_pk)?;
    let bank = config.mfi_program.account::<Bank>(bank_pk)?;
    let group = marginfi_account.group;

    if bank.group != group {
        anyhow::bail!("Bank does not belong to group");
    }

    let amount = (I80F48::from_num(ui_amount) * EXP_10_I80F48[bank.mint_decimals as usize])
        .floor()
        .to_num::<u64>();

    let (liquidity_vault_authority, _) =
        find_bank_vault_authority_pda(&bank_pk, BankVaultType::Liquidity, &config.program_id);

    let jl = derive_juplend_cpi_accounts(&rpc_client, &bank, &liquidity_vault_authority)?;

    let bank_mint_account = rpc_client.get_account(&bank.mint)?;
    let token_program = bank_mint_account.owner;

    let user_ata = anchor_spl::associated_token::get_associated_token_address_with_program_id(
        &authority,
        &bank.mint,
        &token_program,
    );

    let ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::JuplendDeposit {
            group,
            marginfi_account: marginfi_account_pk,
            authority,
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

    let create_ata_ix = build_signer_ata_ix(config, &authority, &bank.mint, &token_program);
    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(config, vec![create_ata_ix, ix], &signing_keypairs)?;
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
    let authority = config.authority();
    let marginfi_account_pk = profile.get_marginfi_account()?;
    let marginfi_account = config
        .mfi_program
        .account::<MarginfiAccount>(marginfi_account_pk)?;
    let bank = config.mfi_program.account::<Bank>(bank_pk)?;
    let group = marginfi_account.group;

    if bank.group != group {
        anyhow::bail!("Bank does not belong to group");
    }

    let amount = (I80F48::from_num(ui_amount) * EXP_10_I80F48[bank.mint_decimals as usize])
        .floor()
        .to_num::<u64>();

    let (liquidity_vault_authority, _) =
        find_bank_vault_authority_pda(&bank_pk, BankVaultType::Liquidity, &config.program_id);

    let jl = derive_juplend_cpi_accounts(&rpc_client, &bank, &liquidity_vault_authority)?;

    let bank_mint_account = rpc_client.get_account(&bank.mint)?;
    let token_program = bank_mint_account.owner;

    let user_ata = anchor_spl::associated_token::get_associated_token_address_with_program_id(
        &authority,
        &bank.mint,
        &token_program,
    );
    let observation_metas = load_withdraw_observation_metas(
        config,
        marginfi_account_pk,
        group,
        withdraw_all.then_some(bank_pk),
    )?;

    let mut ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::JuplendWithdraw {
            group,
            marginfi_account: marginfi_account_pk,
            authority,
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
    ix.accounts.extend(observation_metas);

    let create_ata_ix = build_signer_ata_ix(config, &authority, &bank.mint, &token_program);
    let create_withdraw_intermediary_ata_ix = build_signer_ata_ix(
        config,
        &liquidity_vault_authority,
        &bank.mint,
        &token_program,
    );

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(
        config,
        vec![create_ata_ix, create_withdraw_intermediary_ata_ix, ix],
        &signing_keypairs,
    )?;
    println!("JupLend withdraw successful: {sig}");

    Ok(())
}

// ---------------------------------------------------------------------------
// Add-bank processors (integration bank creation)
// ---------------------------------------------------------------------------

pub struct KaminoBankCreateRequest {
    pub group: Pubkey,
    pub bank_mint: Pubkey,
    pub seed: u64,
    pub oracle: Pubkey,
    pub reserve_oracle: Pubkey,
    pub oracle_setup: u8,
    pub kamino_reserve: Pubkey,
    pub kamino_market: Pubkey,
    pub asset_weight_init: f64,
    pub asset_weight_maint: f64,
    pub deposit_limit: u64,
    pub total_asset_value_init_limit: u64,
    pub oracle_max_age: u16,
    pub oracle_max_confidence: u32,
    pub risk_tier: marginfi_type_crate::types::RiskTier,
    pub config_flags: u8,
    pub init_deposit_amount: u64,
    pub token_program: Pubkey,
}

pub struct DriftBankCreateRequest {
    pub group: Pubkey,
    pub bank_mint: Pubkey,
    pub seed: u64,
    pub oracle: Pubkey,
    pub oracle_setup: u8,
    pub drift_market_index: u16,
    pub asset_weight_init: f64,
    pub asset_weight_maint: f64,
    pub deposit_limit: u64,
    pub total_asset_value_init_limit: u64,
    pub oracle_max_age: u16,
    pub oracle_max_confidence: u32,
    pub risk_tier: marginfi_type_crate::types::RiskTier,
    pub config_flags: u8,
    pub drift_oracle: Option<Pubkey>,
    pub init_deposit_amount: u64,
    pub token_program: Pubkey,
}

pub struct JuplendBankCreateRequest {
    pub group: Pubkey,
    pub bank_mint: Pubkey,
    pub seed: u64,
    pub oracle: Pubkey,
    pub oracle_setup: u8,
    pub juplend_lending: Pubkey,
    pub f_token_mint: Pubkey,
    pub asset_weight_init: f64,
    pub asset_weight_maint: f64,
    pub deposit_limit: u64,
    pub total_asset_value_init_limit: u64,
    pub oracle_max_age: u16,
    pub oracle_max_confidence: u32,
    pub risk_tier: marginfi_type_crate::types::RiskTier,
    pub config_flags: u8,
    pub init_deposit_amount: u64,
    pub token_program: Pubkey,
}

pub fn kamino_add_bank(config: &Config, request: KaminoBankCreateRequest) -> Result<()> {
    use marginfi::state::kamino::KaminoConfigCompact;
    use marginfi_type_crate::types::{BankOperationalState, OracleSetup};
    let rpc_client = config.mfi_program.rpc();

    let (bank_pda, _) = Pubkey::find_program_address(
        &[
            request.group.as_ref(),
            request.bank_mint.as_ref(),
            &request.seed.to_le_bytes(),
        ],
        &config.program_id,
    );

    let oracle_setup_enum = match request.oracle_setup {
        11 => OracleSetup::KaminoPythPush,
        12 => OracleSetup::KaminoSwitchboardPull,
        _ => anyhow::bail!(
            "Invalid Kamino oracle setup: {}. Use 11 (KaminoPythPush) or 12 (KaminoSwitchboardPull)",
            request.oracle_setup
        ),
    };

    let bank_config = KaminoConfigCompact {
        oracle: request.oracle,
        asset_weight_init: I80F48::from_num(request.asset_weight_init).into(),
        asset_weight_maint: I80F48::from_num(request.asset_weight_maint).into(),
        deposit_limit: request.deposit_limit,
        oracle_setup: oracle_setup_enum,
        operational_state: BankOperationalState::Operational,
        risk_tier: request.risk_tier,
        config_flags: request.config_flags,
        total_asset_value_init_limit: request.total_asset_value_init_limit,
        oracle_max_age: request.oracle_max_age,
        oracle_max_confidence: request.oracle_max_confidence,
    };
    bank_config
        .to_bank_config(request.kamino_reserve)
        .validate()
        .context("invalid Kamino bank config")?;

    let liquidity_vault_authority =
        find_bank_vault_authority_pda(&bank_pda, BankVaultType::Liquidity, &config.program_id).0;
    let liquidity_vault =
        find_bank_vault_pda(&bank_pda, BankVaultType::Liquidity, &config.program_id).0;
    let insurance_vault_authority =
        find_bank_vault_authority_pda(&bank_pda, BankVaultType::Insurance, &config.program_id).0;
    let insurance_vault =
        find_bank_vault_pda(&bank_pda, BankVaultType::Insurance, &config.program_id).0;
    let fee_vault_authority =
        find_bank_vault_authority_pda(&bank_pda, BankVaultType::Fee, &config.program_id).0;
    let fee_vault = find_bank_vault_pda(&bank_pda, BankVaultType::Fee, &config.program_id).0;

    // Kamino obligation PDA
    let (obligation_pda, _) = Pubkey::find_program_address(
        &[
            &[0u8],
            &[0u8],
            liquidity_vault_authority.as_ref(),
            request.kamino_market.as_ref(),
            solana_sdk::system_program::id().as_ref(),
            solana_sdk::system_program::id().as_ref(),
        ],
        &KAMINO_PROGRAM_ID,
    );

    let oracle_meta = AccountMeta::new_readonly(request.oracle, false);
    let reserve_meta = AccountMeta::new_readonly(request.kamino_reserve, false);
    let fee_payer = config.explicit_fee_payer();
    let signer_token_account =
        anchor_spl::associated_token::get_associated_token_address_with_program_id(
            &fee_payer,
            &request.bank_mint,
            &request.token_program,
        );
    let create_ata_ix = build_signer_ata_ix(
        config,
        &fee_payer,
        &request.bank_mint,
        &request.token_program,
    );
    let init_accounts = derive_kamino_init_accounts(
        &rpc_client,
        request.kamino_reserve,
        request.kamino_market,
        request.reserve_oracle,
        request.oracle_setup,
        obligation_pda,
        liquidity_vault_authority,
    )?;

    let add_bank_ixs = config
        .mfi_program
        .request()
        .accounts(marginfi::accounts::LendingPoolAddBankKamino {
            group: request.group,
            admin: config.authority(),
            fee_payer,
            bank_mint: request.bank_mint,
            bank: bank_pda,
            integration_acc_1: request.kamino_reserve,
            integration_acc_2: obligation_pda,
            liquidity_vault_authority,
            liquidity_vault,
            insurance_vault_authority,
            insurance_vault,
            fee_vault_authority,
            fee_vault,
            token_program: request.token_program,
            system_program: system_program::id(),
        })
        .args(marginfi::instruction::LendingPoolAddBankKamino {
            bank_config,
            bank_seed: request.seed,
        })
        .instructions()?;

    let mut ixs = add_bank_ixs;
    ixs[0].accounts.push(oracle_meta);
    ixs[0].accounts.push(reserve_meta);
    ixs.push(create_ata_ix);
    ixs.push(Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::KaminoInitObligation {
            fee_payer,
            bank: bank_pda,
            signer_token_account,
            liquidity_vault_authority,
            liquidity_vault,
            integration_acc_2: obligation_pda,
            user_metadata: init_accounts.user_metadata,
            lending_market: request.kamino_market,
            lending_market_authority: init_accounts.lending_market_authority,
            integration_acc_1: request.kamino_reserve,
            mint: request.bank_mint,
            reserve_liquidity_supply: init_accounts.reserve_liquidity_supply,
            reserve_collateral_mint: init_accounts.reserve_collateral_mint,
            reserve_destination_deposit_collateral: init_accounts
                .reserve_destination_deposit_collateral,
            obligation_farm_user_state: init_accounts.obligation_farm_user_state,
            reserve_farm_state: init_accounts.reserve_farm_state,
            kamino_program: KAMINO_PROGRAM_ID,
            farms_program: FARMS_PROGRAM_ID,
            collateral_token_program: anchor_spl::token::ID,
            liquidity_token_program: request.token_program,
            instruction_sysvar_account: sysvar::instructions::ID,
            rent: sysvar::rent::ID,
            system_program: system_program::ID,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::KaminoInitObligation {
            amount: request.init_deposit_amount,
        }
        .data(),
    });

    let signing_keypairs = config.get_signers(true);
    let sig = send_tx(config, ixs, &signing_keypairs)?;
    println!("Kamino bank created (bank: {}, sig: {})", bank_pda, sig);
    Ok(())
}

pub fn drift_add_bank(config: &Config, request: DriftBankCreateRequest) -> Result<()> {
    use marginfi::state::drift::DriftConfigCompact;
    use marginfi_type_crate::types::{BankOperationalState, OracleSetup};

    let (bank_pda, _) = Pubkey::find_program_address(
        &[
            request.group.as_ref(),
            request.bank_mint.as_ref(),
            &request.seed.to_le_bytes(),
        ],
        &config.program_id,
    );

    let oracle_setup_enum = match request.oracle_setup {
        9 => OracleSetup::DriftPythPull,
        10 => OracleSetup::DriftSwitchboardPull,
        _ => anyhow::bail!(
            "Invalid Drift oracle setup: {}. Use 9 (DriftPythPull) or 10 (DriftSwitchboardPull)",
            request.oracle_setup
        ),
    };

    let bank_config = DriftConfigCompact {
        oracle: request.oracle,
        asset_weight_init: I80F48::from_num(request.asset_weight_init).into(),
        asset_weight_maint: I80F48::from_num(request.asset_weight_maint).into(),
        deposit_limit: request.deposit_limit,
        oracle_setup: oracle_setup_enum,
        operational_state: BankOperationalState::Operational,
        risk_tier: request.risk_tier,
        config_flags: request.config_flags,
        total_asset_value_init_limit: request.total_asset_value_init_limit,
        oracle_max_age: request.oracle_max_age,
        oracle_max_confidence: request.oracle_max_confidence,
    };

    let liquidity_vault_authority =
        find_bank_vault_authority_pda(&bank_pda, BankVaultType::Liquidity, &config.program_id).0;
    let liquidity_vault =
        find_bank_vault_pda(&bank_pda, BankVaultType::Liquidity, &config.program_id).0;
    let insurance_vault_authority =
        find_bank_vault_authority_pda(&bank_pda, BankVaultType::Insurance, &config.program_id).0;
    let insurance_vault =
        find_bank_vault_pda(&bank_pda, BankVaultType::Insurance, &config.program_id).0;
    let fee_vault_authority =
        find_bank_vault_authority_pda(&bank_pda, BankVaultType::Fee, &config.program_id).0;
    let fee_vault = find_bank_vault_pda(&bank_pda, BankVaultType::Fee, &config.program_id).0;

    // Derive Drift spot market PDA
    let (drift_spot_market, _) = Pubkey::find_program_address(
        &[b"spot_market", &request.drift_market_index.to_le_bytes()],
        &DRIFT_PROGRAM_ID,
    );
    bank_config
        .to_bank_config(drift_spot_market)
        .validate()
        .context("invalid Drift bank config")?;

    // Derive Drift user PDA (sub_account_id = 0)
    let (drift_user, _) = Pubkey::find_program_address(
        &[
            b"user",
            liquidity_vault_authority.as_ref(),
            &0u16.to_le_bytes(),
        ],
        &DRIFT_PROGRAM_ID,
    );

    // Derive Drift user stats PDA
    let (drift_user_stats, _) = Pubkey::find_program_address(
        &[b"user_stats", liquidity_vault_authority.as_ref()],
        &DRIFT_PROGRAM_ID,
    );

    let oracle_meta = AccountMeta::new_readonly(request.oracle, false);
    let spot_market_meta = AccountMeta::new_readonly(drift_spot_market, false);
    let fee_payer = config.explicit_fee_payer();
    let signer_token_account =
        anchor_spl::associated_token::get_associated_token_address_with_program_id(
            &fee_payer,
            &request.bank_mint,
            &request.token_program,
        );
    let create_ata_ix = build_signer_ata_ix(
        config,
        &fee_payer,
        &request.bank_mint,
        &request.token_program,
    );
    let (drift_state, _) = Pubkey::find_program_address(&[b"drift_state"], &DRIFT_PROGRAM_ID);
    let (drift_spot_market_vault, _) = Pubkey::find_program_address(
        &[
            b"spot_market_vault",
            &request.drift_market_index.to_le_bytes(),
        ],
        &DRIFT_PROGRAM_ID,
    );

    let add_bank_ixs = config
        .mfi_program
        .request()
        .accounts(marginfi::accounts::LendingPoolAddBankDrift {
            group: request.group,
            admin: config.authority(),
            fee_payer,
            bank_mint: request.bank_mint,
            bank: bank_pda,
            integration_acc_1: drift_spot_market,
            integration_acc_2: drift_user,
            integration_acc_3: drift_user_stats,
            liquidity_vault_authority,
            liquidity_vault,
            insurance_vault_authority,
            insurance_vault,
            fee_vault_authority,
            fee_vault,
            token_program: request.token_program,
            system_program: system_program::id(),
        })
        .args(marginfi::instruction::LendingPoolAddBankDrift {
            bank_config,
            bank_seed: request.seed,
        })
        .instructions()?;

    let mut ixs = add_bank_ixs;
    ixs[0].accounts.push(oracle_meta);
    ixs[0].accounts.push(spot_market_meta);
    ixs.push(create_ata_ix);
    ixs.push(Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::DriftInitUser {
            fee_payer,
            signer_token_account,
            bank: bank_pda,
            liquidity_vault_authority,
            liquidity_vault,
            mint: request.bank_mint,
            integration_acc_3: drift_user_stats,
            integration_acc_2: drift_user,
            drift_state,
            integration_acc_1: drift_spot_market,
            drift_spot_market_vault,
            drift_oracle: request.drift_oracle,
            drift_program: DRIFT_PROGRAM_ID,
            token_program: request.token_program,
            rent: sysvar::rent::ID,
            system_program: system_program::ID,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::DriftInitUser {
            amount: request.init_deposit_amount,
        }
        .data(),
    });

    let signing_keypairs = config.get_signers(true);
    let sig = send_tx(config, ixs, &signing_keypairs)?;
    println!("Drift bank created (bank: {}, sig: {})", bank_pda, sig);
    Ok(())
}

pub fn juplend_add_bank(config: &Config, request: JuplendBankCreateRequest) -> Result<()> {
    use juplend_mocks::state::Lending;
    use marginfi::state::juplend::JuplendConfigCompact;
    use marginfi_type_crate::types::OracleSetup;
    let rpc_client = config.mfi_program.rpc();

    let (bank_pda, _) = Pubkey::find_program_address(
        &[
            request.group.as_ref(),
            request.bank_mint.as_ref(),
            &request.seed.to_le_bytes(),
        ],
        &config.program_id,
    );

    let oracle_setup_enum = match request.oracle_setup {
        15 => OracleSetup::JuplendPythPull,
        16 => OracleSetup::JuplendSwitchboardPull,
        _ => anyhow::bail!(
            "Invalid JupLend oracle setup: {}. Use 15 (JuplendPythPull) or 16 (JuplendSwitchboardPull)",
            request.oracle_setup
        ),
    };

    let bank_config = JuplendConfigCompact {
        oracle: request.oracle,
        asset_weight_init: I80F48::from_num(request.asset_weight_init).into(),
        asset_weight_maint: I80F48::from_num(request.asset_weight_maint).into(),
        deposit_limit: request.deposit_limit,
        oracle_setup: oracle_setup_enum,
        risk_tier: request.risk_tier,
        config_flags: request.config_flags,
        total_asset_value_init_limit: request.total_asset_value_init_limit,
        oracle_max_age: request.oracle_max_age,
        oracle_max_confidence: request.oracle_max_confidence,
    };
    bank_config
        .to_bank_config(request.juplend_lending)
        .validate()
        .context("invalid JupLend bank config")?;

    let liquidity_vault_authority =
        find_bank_vault_authority_pda(&bank_pda, BankVaultType::Liquidity, &config.program_id).0;
    let liquidity_vault =
        find_bank_vault_pda(&bank_pda, BankVaultType::Liquidity, &config.program_id).0;
    let insurance_vault_authority =
        find_bank_vault_authority_pda(&bank_pda, BankVaultType::Insurance, &config.program_id).0;
    let insurance_vault =
        find_bank_vault_pda(&bank_pda, BankVaultType::Insurance, &config.program_id).0;
    let fee_vault_authority =
        find_bank_vault_authority_pda(&bank_pda, BankVaultType::Fee, &config.program_id).0;
    let fee_vault = find_bank_vault_pda(&bank_pda, BankVaultType::Fee, &config.program_id).0;

    // JupLend fToken vault PDA
    let (f_token_vault, _) =
        Pubkey::find_program_address(&[b"f_token_vault", bank_pda.as_ref()], &config.program_id);

    let oracle_meta = AccountMeta::new_readonly(request.oracle, false);
    let lending_meta = AccountMeta::new_readonly(request.juplend_lending, false);
    let fee_payer = config.explicit_fee_payer();
    let signer_token_account =
        anchor_spl::associated_token::get_associated_token_address_with_program_id(
            &fee_payer,
            &request.bank_mint,
            &request.token_program,
        );
    let create_ata_ix = build_signer_ata_ix(
        config,
        &fee_payer,
        &request.bank_mint,
        &request.token_program,
    );
    let create_withdraw_intermediary_ata_ix = build_signer_ata_ix(
        config,
        &liquidity_vault_authority,
        &request.bank_mint,
        &request.token_program,
    );
    let lending_data = rpc_client.get_account_data(&request.juplend_lending)?;
    let lending_size = std::mem::size_of::<Lending>();
    if lending_data.len() < 8 + lending_size {
        anyhow::bail!(
            "JupLend lending account {} data too small ({} bytes)",
            request.juplend_lending,
            lending_data.len()
        );
    }
    let lending: &Lending = bytemuck::from_bytes(&lending_data[8..8 + lending_size]);
    let jl = derive_juplend_cpi_accounts_for_lending(
        lending,
        &request.bank_mint,
        &request.token_program,
        &liquidity_vault_authority,
    );

    let add_bank_ixs = config
        .mfi_program
        .request()
        .accounts(marginfi::accounts::LendingPoolAddBankJuplend {
            group: request.group,
            admin: config.authority(),
            fee_payer,
            bank_mint: request.bank_mint,
            bank: bank_pda,
            integration_acc_1: request.juplend_lending,
            liquidity_vault_authority,
            liquidity_vault,
            insurance_vault_authority,
            insurance_vault,
            fee_vault_authority,
            fee_vault,
            f_token_mint: request.f_token_mint,
            integration_acc_2: f_token_vault,
            token_program: request.token_program,
            system_program: system_program::id(),
        })
        .args(marginfi::instruction::LendingPoolAddBankJuplend {
            bank_config,
            bank_seed: request.seed,
        })
        .instructions()?;

    let mut ixs = add_bank_ixs;
    ixs[0].accounts.push(oracle_meta);
    ixs[0].accounts.push(lending_meta);
    ixs.push(create_ata_ix);
    ixs.push(create_withdraw_intermediary_ata_ix);
    ixs.push(Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::JuplendInitPosition {
            fee_payer,
            signer_token_account,
            bank: bank_pda,
            liquidity_vault_authority,
            liquidity_vault,
            mint: request.bank_mint,
            integration_acc_1: request.juplend_lending,
            f_token_mint: jl.f_token_mint,
            integration_acc_2: f_token_vault,
            lending_admin: jl.lending_admin,
            supply_token_reserves_liquidity: jl.supply_token_reserves_liquidity,
            lending_supply_position_on_liquidity: jl.lending_supply_position_on_liquidity,
            rate_model: jl.rate_model,
            vault: jl.vault,
            liquidity: jl.liquidity,
            liquidity_program: jl.liquidity_program,
            rewards_rate_model: jl.rewards_rate_model,
            juplend_program: JUPLEND_LENDING_PROGRAM_ID,
            token_program: request.token_program,
            associated_token_program: anchor_spl::associated_token::ID,
            system_program: system_program::ID,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::JuplendInitPosition {
            amount: request.init_deposit_amount,
        }
        .data(),
    });

    let signing_keypairs = config.get_signers(true);
    let sig = send_tx(config, ixs, &signing_keypairs)?;
    println!("JupLend bank created (bank: {}, sig: {})", bank_pda, sig);
    Ok(())
}
