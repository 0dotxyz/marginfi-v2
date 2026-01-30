pub mod admin;
pub mod emissions;
pub mod group;
pub mod oracle;

use {
    crate::{
        config::Config,
        profile::{self, get_cli_config_dir, load_profile, CliConfig, Profile},
        utils::{
            bank_to_oracle_key, calc_emissions_rate, find_bank_emssions_auth_pda,
            find_bank_emssions_token_account_pda, find_bank_vault_authority_pda,
            find_bank_vault_pda, find_fee_state_pda, load_observation_account_metas,
            process_transaction, EXP_10_I80F48,
        },
        RatePointArg,
    },
    anchor_client::{
        anchor_lang::{AnchorDeserialize, InstructionData, ToAccountMetas},
        Cluster,
    },
    anchor_spl::token_2022::spl_token_2022,
    anyhow::{anyhow, bail, Result},
    fixed::types::I80F48,
    log::info,
    marginfi::{
        state::{
            bank::{BankImpl, BankVaultType},
            bank_config::BankConfigImpl,
            marginfi_account::BankAccountWrapper,
            price::{
                parse_swb_ignore_alignment, LitePullFeedAccountData, OraclePriceFeedAdapter,
                PriceAdapter,
            },
        },
        utils::NumTraitsWithTolerance,
    },
    marginfi_type_crate::{
        constants::{
            EMISSIONS_FLAG_BORROW_ACTIVE, EMISSIONS_FLAG_LENDING_ACTIVE, ZERO_AMOUNT_THRESHOLD,
        },
        types::{
            make_points, BalanceSide, Bank, BankConfigCompact, BankConfigOpt, BankOperationalState,
            InterestRateConfig, MarginfiAccount, MarginfiGroup, OracleSetup, RatePoint,
            WrappedI80F48, CURVE_POINTS, INTEREST_CURVE_SEVEN_POINT,
        },
    },
    pyth_solana_receiver_sdk::price_update::PriceUpdateV2,
    solana_client::{
        rpc_client::RpcClient,
        rpc_filter::{Memcmp, RpcFilterType},
    },
    solana_sdk::{
        account::{ReadableAccount, WritableAccount},
        account_info::IntoAccountInfo,
        clock::Clock,
        commitment_config::CommitmentLevel,
        compute_budget::ComputeBudgetInstruction,
        instruction::{AccountMeta, Instruction},
        message::Message,
        program_pack::Pack,
        pubkey::Pubkey,
        signature::Keypair,
        signer::Signer,
        system_program,
        transaction::Transaction,
    },
    spl_associated_token_account::{
        get_associated_token_address, instruction::create_associated_token_account_idempotent,
    },
    std::{
        cell::RefCell,
        collections::HashMap,
        fs, io,
        mem::size_of,
        ops::{Neg, Not},
        time::{Duration, SystemTime, UNIX_EPOCH},
    },
    switchboard_on_demand::PullFeedAccountData,
};

// --------------------------------------------------------------------------------------------------------------------
// marginfi group
// --------------------------------------------------------------------------------------------------------------------

pub fn group_get(config: Config, marginfi_group: Option<Pubkey>) -> Result<()> {
    if let Some(marginfi_group) = marginfi_group {
        println!("Address: {marginfi_group}");
        println!("=============");
        print_group_banks(config, marginfi_group)?;
    } else {
        group_get_all(config)?;
    }
    Ok(())
}

pub fn group_get_all(config: Config) -> Result<()> {
    let accounts: Vec<(Pubkey, MarginfiGroup)> = config.mfi_program.accounts(vec![])?;

    accounts
        .iter()
        .for_each(|(address, group)| print_group(address, group));

    Ok(())
}

fn print_group(address: &Pubkey, group: &MarginfiGroup) {
    println!(
        r#"
Group: {}
Admin: {}
"#,
        address, group.admin
    );
}

pub fn print_group_banks(config: Config, marginfi_group: Pubkey) -> Result<()> {
    let banks = config
        .mfi_program
        .accounts::<Bank>(vec![RpcFilterType::Memcmp(Memcmp::new_raw_bytes(
            8 + size_of::<Pubkey>() + size_of::<u8>(),
            marginfi_group.to_bytes().to_vec(),
        ))])?;

    println!("--------\nBanks:");

    banks
        .iter()
        .for_each(|(address, bank)| print_bank(address, bank));

    Ok(())
}

fn print_bank(address: &Pubkey, bank: &Bank) {
    println!(
        r#"
Group: {},
Bank: {}
Mint: {},
Total Deposits: {}
Total Liabilities: {}
Config:
  State: {:?}
  Risk Tier: {:?}
  USD Soft limit: {:?}
  Asset:
    Weight Init: {:?}, Maint: {:?}
    Limit: {}
  Liab:
    Weight Init: {:?}, Maint: {:?}
    Limit: {}
  Interest Rate Config:
    Curve: opt_ur: {:?} pl_ir: {:?} max_ir: {:?}
    Fees - Insurance: ir: {:?} fix: {:?}, Group: ir: {:?} fix: {:?}
  Oracle Setup:
    Type: {:?}
    Keys: {:#?}
    Max Age: {:#?}s
Emissions:
  Flags: 0b{:b}
  Rate: {:?}
  Mint: {:?}
  Remaining: {:?}
Unclaimed
  Fees: {:?}
  Insurance: {:?}
Last Update: {:?}h ago ({})
"#,
        bank.group,
        address,
        bank.mint,
        bank.get_asset_amount(bank.total_asset_shares.into())
            .unwrap()
            / EXP_10_I80F48[bank.mint_decimals as usize],
        bank.get_liability_amount(bank.total_liability_shares.into())
            .unwrap()
            / EXP_10_I80F48[bank.mint_decimals as usize],
        bank.config.operational_state,
        bank.config.risk_tier,
        bank.config.total_asset_value_init_limit,
        bank.config.asset_weight_init,
        bank.config.asset_weight_maint,
        I80F48::from_num(bank.config.deposit_limit) / EXP_10_I80F48[bank.mint_decimals as usize],
        bank.config.liability_weight_init,
        bank.config.liability_weight_maint,
        I80F48::from_num(bank.config.borrow_limit) / EXP_10_I80F48[bank.mint_decimals as usize],
        bank.config.interest_rate_config.optimal_utilization_rate,
        bank.config.interest_rate_config.plateau_interest_rate,
        bank.config.interest_rate_config.max_interest_rate,
        bank.config.interest_rate_config.insurance_fee_fixed_apr,
        bank.config.interest_rate_config.insurance_ir_fee,
        bank.config.interest_rate_config.protocol_fixed_fee_apr,
        bank.config.interest_rate_config.protocol_ir_fee,
        bank.config.oracle_setup,
        bank.config.oracle_keys,
        bank.config.get_oracle_max_age(),
        bank.flags,
        I80F48::from(bank.emissions_rate),
        bank.emissions_mint,
        I80F48::from(bank.emissions_remaining),
        I80F48::from(bank.collected_group_fees_outstanding)
            / EXP_10_I80F48[bank.mint_decimals as usize],
        I80F48::from(bank.collected_insurance_fees_outstanding)
            / EXP_10_I80F48[bank.mint_decimals as usize],
        SystemTime::now()
            .duration_since(UNIX_EPOCH + Duration::from_secs(bank.last_update as u64))
            .unwrap()
            .as_secs_f32()
            / 3600_f32,
        bank.last_update
    )
}

pub fn group_create(
    config: Config,
    profile: Profile,
    admin: Option<Pubkey>,
    override_existing_profile_group: bool,
) -> Result<()> {
    let rpc_client = config.mfi_program.rpc();
    let admin = admin.unwrap_or_else(|| config.authority());

    if profile.marginfi_group.is_some() && !override_existing_profile_group {
        bail!(
            "Marginfi group already exists for profile [{}]",
            profile.name
        );
    }

    let marginfi_group_keypair = Keypair::new();

    let init_marginfi_group_ixs_builder = config.mfi_program.request();

    let mut signing_keypairs = config.get_signers(false);
    signing_keypairs.push(&marginfi_group_keypair);

    let init_marginfi_group_ixs = init_marginfi_group_ixs_builder
        .accounts(marginfi::accounts::MarginfiGroupInitialize {
            marginfi_group: marginfi_group_keypair.pubkey(),
            admin,
            fee_state: find_fee_state_pda(&marginfi::ID).0,
            system_program: system_program::id(),
        })
        .args(marginfi::instruction::MarginfiGroupInitialize {})
        .instructions()?;

    let recent_blockhash = rpc_client.get_latest_blockhash().unwrap();
    let message = Message::new(&init_marginfi_group_ixs, Some(&config.authority()));
    let mut transaction = Transaction::new_unsigned(message);
    transaction.partial_sign(&signing_keypairs, recent_blockhash);

    match process_transaction(&transaction, &rpc_client, config.get_tx_mode()) {
        Ok(sig) => println!("marginfi group created (sig: {})", sig),
        Err(err) => {
            println!("Error during marginfi group creation:\n{:#?}", err);
            return Err(anyhow!("Error during marginfi group creation"));
        }
    };

    let mut profile = profile;
    profile.set_marginfi_group(marginfi_group_keypair.pubkey())?;

    Ok(())
}

pub fn group_configure(
    config: Config,
    profile: Profile,
    new_admin: Pubkey,
    new_emode_admin: Pubkey,
    new_curve_admin: Pubkey,
    new_limit_admin: Pubkey,
    new_emissions_admin: Pubkey,
    new_metadata_admin: Pubkey,
    new_risk_admin: Pubkey,
) -> Result<()> {
    let rpc_client = config.mfi_program.rpc();

    if profile.marginfi_group.is_none() {
        bail!("Marginfi group not specified in profile [{}]", profile.name);
    }

    let signing_keypairs = config.get_signers(false);
    let configure_marginfi_group_ixs_builder = config
        .mfi_program
        .request()
        .signer(*signing_keypairs.first().unwrap());

    let configure_marginfi_group_ixs = configure_marginfi_group_ixs_builder
        .accounts(marginfi::accounts::MarginfiGroupConfigure {
            marginfi_group: profile.marginfi_group.unwrap(),
            admin: config.authority(),
        })
        .args(marginfi::instruction::MarginfiGroupConfigure {
            new_admin,
            new_emode_admin,
            new_curve_admin,
            new_limit_admin,
            new_emissions_admin,
            new_metadata_admin,
            new_risk_admin,
            emode_max_init_leverage: None,
            emode_max_maint_leverage: None,
        })
        .instructions()?;

    let recent_blockhash = rpc_client.get_latest_blockhash().unwrap();
    let message = Message::new(&configure_marginfi_group_ixs, Some(&config.authority()));
    let mut transaction = Transaction::new_unsigned(message);
    transaction.partial_sign(&signing_keypairs, recent_blockhash);

    match process_transaction(&transaction, &rpc_client, config.get_tx_mode()) {
        Ok(sig) => println!("marginfi group created (sig: {})", sig),
        Err(err) => println!("Error during marginfi group creation:\n{:#?}", err),
    };

    Ok(())
}

#[allow(clippy::too_many_arguments)]

pub fn group_add_bank(
    config: Config,
    profile: Profile,
    bank_mint: Pubkey,
    seed: bool,
    asset_weight_init: f64,
    asset_weight_maint: f64,
    liability_weight_init: f64,
    liability_weight_maint: f64,
    deposit_limit_ui: u64,
    borrow_limit_ui: u64,
    zero_util_rate: u32,
    hundred_util_rate: u32,
    points: Vec<RatePointArg>,
    insurance_fee_fixed_apr: f64,
    insurance_ir_fee: f64,
    group_fixed_fee_apr: f64,
    group_ir_fee: f64,
    risk_tier: crate::RiskTierArg,
    oracle_max_age: u16,
    compute_unit_price: Option<u64>,
    global_fee_wallet: Pubkey,
) -> Result<()> {
    let rpc_client = config.mfi_program.rpc();

    if profile.marginfi_group.is_none() {
        bail!("Marginfi group not specified in profile [{}]", profile.name);
    }

    let asset_weight_init: WrappedI80F48 = I80F48::from_num(asset_weight_init).into();
    let asset_weight_maint: WrappedI80F48 = I80F48::from_num(asset_weight_maint).into();
    let liability_weight_init: WrappedI80F48 = I80F48::from_num(liability_weight_init).into();
    let liability_weight_maint: WrappedI80F48 = I80F48::from_num(liability_weight_maint).into();

    let optimal_utilization_rate: WrappedI80F48 = I80F48::ZERO.into();
    let plateau_interest_rate: WrappedI80F48 = I80F48::ZERO.into();
    let max_interest_rate: WrappedI80F48 = I80F48::ZERO.into();
    let insurance_fee_fixed_apr: WrappedI80F48 = I80F48::from_num(insurance_fee_fixed_apr).into();
    let insurance_ir_fee: WrappedI80F48 = I80F48::from_num(insurance_ir_fee).into();
    let group_fixed_fee_apr: WrappedI80F48 = I80F48::from_num(group_fixed_fee_apr).into();
    let group_ir_fee: WrappedI80F48 = I80F48::from_num(group_ir_fee).into();

    let mint_account = rpc_client.get_account(&bank_mint)?;
    let token_program = mint_account.owner;
    let mint = spl_token_2022::state::Mint::unpack(
        &mint_account.data[..spl_token_2022::state::Mint::LEN],
    )?;
    let deposit_limit = deposit_limit_ui * 10_u64.pow(mint.decimals as u32);
    let borrow_limit = borrow_limit_ui * 10_u64.pow(mint.decimals as u32);

    let pts_raw: Vec<RatePoint> = points
        .iter()
        .map(|p| RatePoint {
            util: p.util,
            rate: p.rate,
        })
        .collect();
    let points: [RatePoint; CURVE_POINTS] = make_points(&pts_raw);

    let interest_rate_config = InterestRateConfig {
        optimal_utilization_rate,
        plateau_interest_rate,
        max_interest_rate,
        insurance_fee_fixed_apr,
        insurance_ir_fee,
        protocol_fixed_fee_apr: group_fixed_fee_apr,
        protocol_ir_fee: group_ir_fee,
        zero_util_rate,
        hundred_util_rate,
        points,
        curve_type: INTEREST_CURVE_SEVEN_POINT,
        ..InterestRateConfig::default()
    };

    // Create signing keypairs -- if the PDA is used, no explicit fee payer.
    let mut signing_keypairs = config.get_signers(false);

    let bank_keypair = Keypair::new();
    if !seed {
        signing_keypairs.push(&bank_keypair);
    }

    // Generate the PDA for the bank keypair if the seed bool is set
    // Issue tx with the seed
    let add_bank_ixs: Vec<Instruction> = if seed {
        create_bank_ix_with_seed(
            &config,
            profile,
            &rpc_client,
            bank_mint,
            token_program,
            asset_weight_init,
            asset_weight_maint,
            liability_weight_init,
            liability_weight_maint,
            deposit_limit,
            borrow_limit,
            interest_rate_config,
            risk_tier,
            oracle_max_age,
            global_fee_wallet,
        )?
    } else {
        create_bank_ix(
            &config,
            profile,
            bank_mint,
            token_program,
            &bank_keypair,
            asset_weight_init,
            asset_weight_maint,
            liability_weight_init,
            liability_weight_maint,
            deposit_limit,
            borrow_limit,
            interest_rate_config,
            risk_tier,
            oracle_max_age,
            global_fee_wallet,
        )?
    };

    let mut ixs = vec![ComputeBudgetInstruction::set_compute_unit_price(
        compute_unit_price.unwrap_or(1),
    )];
    ixs.extend(add_bank_ixs);

    let recent_blockhash = rpc_client.get_latest_blockhash().unwrap();
    let message = Message::new(&ixs, None);
    let mut transaction = Transaction::new_unsigned(message);
    transaction.partial_sign(&signing_keypairs, recent_blockhash);

    match process_transaction(&transaction, &rpc_client, config.get_tx_mode()) {
        Ok(sig) => println!("bank created (sig: {})", sig),
        Err(err) => println!("Error during bank creation:\n{:#?}", err),
    };

    Ok(())
}

#[allow(clippy::too_many_arguments)]

fn create_bank_ix_with_seed(
    config: &Config,
    profile: Profile,
    rpc_client: &RpcClient,
    bank_mint: Pubkey,
    token_program: Pubkey,
    asset_weight_init: WrappedI80F48,
    asset_weight_maint: WrappedI80F48,
    liability_weight_init: WrappedI80F48,
    liability_weight_maint: WrappedI80F48,
    deposit_limit: u64,
    borrow_limit: u64,
    interest_rate_config: InterestRateConfig,
    risk_tier: crate::RiskTierArg,
    oracle_max_age: u16,
    global_fee_wallet: Pubkey,
) -> Result<Vec<Instruction>> {
    use solana_sdk::commitment_config::CommitmentConfig;

    let mut bank_pda = Pubkey::default();
    let mut bank_seed: u64 = u64::default();
    let group_key = profile.marginfi_group.unwrap();

    // Iterate through to find the next canonical seed
    for i in 0..u64::MAX {
        println!("Seed option enabled -- generating a PDA account");
        let (pda, _) = Pubkey::find_program_address(
            [group_key.as_ref(), bank_mint.as_ref(), &i.to_le_bytes()].as_slice(),
            &config.program_id,
        );
        if rpc_client
            .get_account_with_commitment(&pda, CommitmentConfig::default())?
            .value
            .is_none()
        {
            // Bank address is free
            println!("Succesffuly generated a PDA account");
            bank_pda = pda;
            bank_seed = i;
            break;
        }
    }

    let add_bank_ixs_builder = config.mfi_program.request();
    let add_bank_ixs = add_bank_ixs_builder
        .accounts(marginfi::accounts::LendingPoolAddBankWithSeed {
            marginfi_group: profile.marginfi_group.unwrap(),
            admin: config.authority(),
            bank_mint,
            bank: bank_pda,
            fee_vault: find_bank_vault_pda(&bank_pda, BankVaultType::Fee, &config.program_id).0,
            fee_vault_authority: find_bank_vault_authority_pda(
                &bank_pda,
                BankVaultType::Fee,
                &config.program_id,
            )
            .0,
            insurance_vault: find_bank_vault_pda(
                &bank_pda,
                BankVaultType::Insurance,
                &config.program_id,
            )
            .0,
            insurance_vault_authority: find_bank_vault_authority_pda(
                &bank_pda,
                BankVaultType::Insurance,
                &config.program_id,
            )
            .0,
            liquidity_vault: find_bank_vault_pda(
                &bank_pda,
                BankVaultType::Liquidity,
                &config.program_id,
            )
            .0,
            liquidity_vault_authority: find_bank_vault_authority_pda(
                &bank_pda,
                BankVaultType::Liquidity,
                &config.program_id,
            )
            .0,
            token_program,
            system_program: system_program::id(),
            fee_payer: config.authority(),
            fee_state: find_fee_state_pda(&config.program_id).0,
            global_fee_wallet,
        })
        .args(marginfi::instruction::LendingPoolAddBankWithSeed {
            bank_config: BankConfigCompact {
                asset_weight_init,
                asset_weight_maint,
                liability_weight_init,
                liability_weight_maint,
                deposit_limit,
                borrow_limit,
                interest_rate_config: interest_rate_config.into(),
                operational_state: BankOperationalState::Operational,
                risk_tier: risk_tier.into(),
                oracle_max_age,
                ..BankConfigCompact::default()
            },
            bank_seed,
        })
        .instructions()?;

    println!("Bank address (PDA): {}", bank_pda);

    Ok(add_bank_ixs)
}

#[allow(clippy::too_many_arguments)]

fn create_bank_ix(
    config: &Config,
    profile: Profile,
    bank_mint: Pubkey,
    token_program: Pubkey,
    bank_keypair: &Keypair,
    asset_weight_init: WrappedI80F48,
    asset_weight_maint: WrappedI80F48,
    liability_weight_init: WrappedI80F48,
    liability_weight_maint: WrappedI80F48,
    deposit_limit: u64,
    borrow_limit: u64,
    interest_rate_config: InterestRateConfig,
    risk_tier: crate::RiskTierArg,
    oracle_max_age: u16,
    global_fee_wallet: Pubkey,
) -> Result<Vec<Instruction>> {
    let add_bank_ixs_builder = config.mfi_program.request();
    let add_bank_ixs = add_bank_ixs_builder
        .accounts(marginfi::accounts::LendingPoolAddBank {
            marginfi_group: profile.marginfi_group.unwrap(),
            admin: config.authority(),
            bank: bank_keypair.pubkey(),
            bank_mint,
            fee_vault: find_bank_vault_pda(
                &bank_keypair.pubkey(),
                BankVaultType::Fee,
                &config.program_id,
            )
            .0,
            fee_vault_authority: find_bank_vault_authority_pda(
                &bank_keypair.pubkey(),
                BankVaultType::Fee,
                &config.program_id,
            )
            .0,
            insurance_vault: find_bank_vault_pda(
                &bank_keypair.pubkey(),
                BankVaultType::Insurance,
                &config.program_id,
            )
            .0,
            insurance_vault_authority: find_bank_vault_authority_pda(
                &bank_keypair.pubkey(),
                BankVaultType::Insurance,
                &config.program_id,
            )
            .0,
            liquidity_vault: find_bank_vault_pda(
                &bank_keypair.pubkey(),
                BankVaultType::Liquidity,
                &config.program_id,
            )
            .0,
            liquidity_vault_authority: find_bank_vault_authority_pda(
                &bank_keypair.pubkey(),
                BankVaultType::Liquidity,
                &config.program_id,
            )
            .0,
            token_program,
            system_program: system_program::id(),
            fee_payer: config.explicit_fee_payer(),
            fee_state: find_fee_state_pda(&config.program_id).0,
            global_fee_wallet,
        })
        .args(marginfi::instruction::LendingPoolAddBank {
            bank_config: BankConfigCompact {
                asset_weight_init,
                asset_weight_maint,
                liability_weight_init,
                liability_weight_maint,
                deposit_limit,
                borrow_limit,
                interest_rate_config: interest_rate_config.into(),
                operational_state: BankOperationalState::Operational,
                risk_tier: risk_tier.into(),
                oracle_max_age,
                ..BankConfigCompact::default()
            },
        })
        .instructions()?;

    println!("Bank address: {}", bank_keypair.pubkey());

    Ok(add_bank_ixs)
}

#[allow(clippy::too_many_arguments, dead_code)]

pub fn group_handle_bankruptcy(
    config: &Config,
    profile: Profile,
    bank_pk: Pubkey,
    marginfi_account_pk: Pubkey,
) -> Result<()> {
    let rpc_client = config.mfi_program.rpc();

    if profile.marginfi_group.is_none() {
        bail!("Marginfi group not specified in profile [{}]", profile.name);
    }

    let banks = HashMap::from_iter(load_all_banks(
        config,
        Some(profile.marginfi_group.unwrap()),
    )?);

    let marginfi_account = config
        .mfi_program
        .account::<MarginfiAccount>(marginfi_account_pk)?;

    handle_bankruptcy_for_an_account(
        config,
        &profile,
        &rpc_client,
        &banks,
        marginfi_account_pk,
        &marginfi_account,
        bank_pk,
    )?;

    Ok(())
}

#[allow(dead_code)]
pub fn group_auto_handle_bankruptcy_for_an_account(
    config: &Config,
    profile: Profile,
    marginfi_account_pk: Pubkey,
) -> Result<()> {
    let rpc_client = config.mfi_program.rpc();

    if profile.marginfi_group.is_none() {
        bail!("Marginfi group not specified in profile [{}]", profile.name);
    }

    let banks = HashMap::from_iter(load_all_banks(
        config,
        Some(profile.marginfi_group.unwrap()),
    )?);
    let marginfi_account = config
        .mfi_program
        .account::<MarginfiAccount>(marginfi_account_pk)?;

    marginfi_account
        .lending_account
        .balances
        .iter()
        .filter(|b| {
            b.is_active()
                && banks
                    .get(&b.bank_pk)
                    .unwrap()
                    .get_liability_amount(b.liability_shares.into())
                    .unwrap()
                    .is_positive_with_tolerance(ZERO_AMOUNT_THRESHOLD)
        })
        .map(|b| b.bank_pk)
        .collect::<Vec<Pubkey>>()
        .iter()
        .for_each(|bank_pk| {
            handle_bankruptcy_for_an_account(
                config,
                &profile,
                &rpc_client,
                &banks,
                marginfi_account_pk,
                &marginfi_account,
                *bank_pk,
            )
            .unwrap();
        });

    Ok(())
}

#[allow(dead_code)]
fn handle_bankruptcy_for_an_account(
    config: &Config,
    profile: &Profile,
    rpc_client: &RpcClient,
    banks: &HashMap<Pubkey, Bank>,
    marginfi_account_pk: Pubkey,
    marginfi_account: &MarginfiAccount,
    bank_pk: Pubkey,
) -> Result<()> {
    println!("Handling bankruptcy for bank {}", bank_pk);

    let bank = banks.get(&bank_pk).unwrap();

    let bank_mint_account = rpc_client.get_account(&bank.mint)?;
    let token_program = bank_mint_account.owner;
    let mut handle_bankruptcy_ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::LendingPoolHandleBankruptcy {
            group: profile.marginfi_group.unwrap(),
            signer: config.authority(),
            bank: bank_pk,
            marginfi_account: marginfi_account_pk,
            liquidity_vault: find_bank_vault_pda(
                &bank_pk,
                BankVaultType::Liquidity,
                &config.program_id,
            )
            .0,
            insurance_vault: find_bank_vault_pda(
                &bank_pk,
                BankVaultType::Insurance,
                &config.program_id,
            )
            .0,
            insurance_vault_authority: find_bank_vault_authority_pda(
                &bank_pk,
                BankVaultType::Insurance,
                &config.program_id,
            )
            .0,
            token_program,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::LendingPoolHandleBankruptcy {}.data(),
    };

    if token_program == anchor_spl::token_2022::ID {
        handle_bankruptcy_ix
            .accounts
            .push(AccountMeta::new_readonly(bank.mint, false));
    }
    handle_bankruptcy_ix
        .accounts
        .extend(load_observation_account_metas(
            marginfi_account,
            banks,
            vec![bank_pk],
            vec![],
        ));

    let recent_blockhash = rpc_client.get_latest_blockhash().unwrap();

    let signing_keypairs = config.get_signers(false);

    let message = Message::new(&[handle_bankruptcy_ix], Some(&config.authority()));
    let mut transaction = Transaction::new_unsigned(message);
    transaction.partial_sign(&signing_keypairs, recent_blockhash);

    match process_transaction(&transaction, rpc_client, config.get_tx_mode()) {
        Ok(sig) => println!("Bankruptcy handled (sig: {})", sig),
        Err(err) => println!("Error during bankruptcy handling:\n{:#?}", err),
    };

    Ok(())
}

const BANKRUPTCY_CHUNKS: usize = 4;

pub fn handle_bankruptcy_for_accounts(
    config: &Config,
    profile: &Profile,
    accounts: Vec<Pubkey>,
) -> Result<()> {
    let mut instructions = vec![];
    let rpc_client = config.mfi_program.rpc();

    let banks = HashMap::from_iter(load_all_banks(
        config,
        Some(profile.marginfi_group.unwrap()),
    )?);

    for account in accounts {
        let marginfi_account = config
            .mfi_program
            .account::<MarginfiAccount>(account)
            .unwrap();

        marginfi_account
            .lending_account
            .balances
            .iter()
            .filter(|b| {
                b.is_active()
                    && banks
                        .get(&b.bank_pk)
                        .unwrap()
                        .get_liability_amount(b.liability_shares.into())
                        .unwrap()
                        .is_positive_with_tolerance(ZERO_AMOUNT_THRESHOLD)
            })
            .map(|b| b.bank_pk)
            .collect::<Vec<Pubkey>>()
            .iter()
            .for_each(|bank_pk| {
                instructions.push(
                    make_bankruptcy_ix(
                        config,
                        profile,
                        &banks,
                        account,
                        &marginfi_account,
                        *bank_pk,
                    )
                    .unwrap(),
                );
            });
    }

    println!("Handling {} bankruptcies", instructions.len());

    let chunks = instructions.chunks(BANKRUPTCY_CHUNKS);

    for chunk in chunks {
        let recent_blockhash = rpc_client.get_latest_blockhash().unwrap();

        let signing_keypairs = config.get_signers(false);

        let mut ixs = vec![ComputeBudgetInstruction::set_compute_unit_limit(1_400_000)];
        ixs.extend_from_slice(chunk);

        let message = Message::new(&ixs, Some(&config.authority()));

        let mut transaction = Transaction::new_unsigned(message);
        transaction.partial_sign(&signing_keypairs, recent_blockhash);

        match process_transaction(&transaction, &rpc_client, config.get_tx_mode()) {
            Ok(sig) => println!("Bankruptcy handled (sig: {})", sig),
            Err(err) => println!("Error during bankruptcy handling:\n{:#?}", err),
        };
    }

    Ok(())
}

fn make_bankruptcy_ix(
    config: &Config,
    profile: &Profile,
    banks: &HashMap<Pubkey, Bank>,
    marginfi_account_pk: Pubkey,
    marginfi_account: &MarginfiAccount,
    bank_pk: Pubkey,
) -> Result<Instruction> {
    println!("Handling bankruptcy for bank {}", bank_pk);
    let rpc_client = config.mfi_program.rpc();

    let bank = banks.get(&bank_pk).unwrap();

    let bank_mint_account = rpc_client.get_account(&bank.mint)?;
    let token_program = bank_mint_account.owner;
    let mut handle_bankruptcy_ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::LendingPoolHandleBankruptcy {
            group: profile.marginfi_group.unwrap(),
            signer: config.fee_payer.pubkey(),
            bank: bank_pk,
            marginfi_account: marginfi_account_pk,
            liquidity_vault: find_bank_vault_pda(
                &bank_pk,
                BankVaultType::Liquidity,
                &config.program_id,
            )
            .0,
            insurance_vault: find_bank_vault_pda(
                &bank_pk,
                BankVaultType::Insurance,
                &config.program_id,
            )
            .0,
            insurance_vault_authority: find_bank_vault_authority_pda(
                &bank_pk,
                BankVaultType::Insurance,
                &config.program_id,
            )
            .0,
            token_program,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::LendingPoolHandleBankruptcy {}.data(),
    };

    if token_program == anchor_spl::token_2022::ID {
        handle_bankruptcy_ix
            .accounts
            .push(AccountMeta::new_readonly(bank.mint, false));
    }
    handle_bankruptcy_ix
        .accounts
        .extend(load_observation_account_metas(
            marginfi_account,
            banks,
            vec![bank_pk],
            vec![],
        ));

    Ok(handle_bankruptcy_ix)
}

pub fn initialize_fee_state(
    config: Config,
    admin: Pubkey,
    fee_wallet: Pubkey,
    bank_init_flat_sol_fee: u32,
    liquidation_flat_sol_fee: u32,
    program_fee_fixed: f64,
    program_fee_rate: f64,
    liquidation_max_fee: f64,
) -> Result<()> {
    let program_fee_fixed: WrappedI80F48 = I80F48::from_num(program_fee_fixed).into();
    let program_fee_rate: WrappedI80F48 = I80F48::from_num(program_fee_rate).into();
    let liquidation_max_fee: WrappedI80F48 = I80F48::from_num(liquidation_max_fee).into();

    let rpc_client = config.mfi_program.rpc();

    let fee_state_pubkey = find_fee_state_pda(&config.program_id).0;

    let initialize_fee_state_ixs_builder = config.mfi_program.request();

    let initialize_fee_state_ixs = initialize_fee_state_ixs_builder
        .accounts(marginfi::accounts::InitFeeState {
            payer: config.authority(),
            fee_state: fee_state_pubkey,
            system_program: system_program::id(),
        })
        .args(marginfi::instruction::InitGlobalFeeState {
            admin,
            fee_wallet,
            bank_init_flat_sol_fee,
            liquidation_flat_sol_fee,
            program_fee_fixed,
            program_fee_rate,
            liquidation_max_fee,
        })
        .instructions()?;

    let recent_blockhash = rpc_client.get_latest_blockhash().unwrap();
    let message = Message::new(&initialize_fee_state_ixs, Some(&config.authority()));
    let mut transaction = Transaction::new_unsigned(message);
    transaction.partial_sign(&config.get_signers(false), recent_blockhash);

    match process_transaction(&transaction, &rpc_client, config.get_tx_mode()) {
        Ok(sig) => println!("Fee state initialized (sig: {})", sig),
        Err(err) => {
            println!("Error during fee state initialization:\n{:#?}", err);
            return Err(anyhow!("Error during fee state initialization"));
        }
    };

    Ok(())
}

pub fn edit_fee_state(
    config: Config,
    new_admin: Pubkey,
    fee_wallet: Pubkey,
    bank_init_flat_sol_fee: u32,
    liquidation_flat_sol_fee: u32,
    program_fee_fixed: f64,
    program_fee_rate: f64,
    liquidation_max_fee: f64,
) -> Result<()> {
    let program_fee_fixed: WrappedI80F48 = I80F48::from_num(program_fee_fixed).into();
    let program_fee_rate: WrappedI80F48 = I80F48::from_num(program_fee_rate).into();
    let liquidation_max_fee: WrappedI80F48 = I80F48::from_num(liquidation_max_fee).into();

    let rpc_client = config.mfi_program.rpc();

    let fee_state_pubkey = find_fee_state_pda(&config.program_id).0;

    let edit_fee_state_ixs_builder = config.mfi_program.request();

    let edit_fee_state_ixs = edit_fee_state_ixs_builder
        .accounts(marginfi::accounts::EditFeeState {
            global_fee_admin: config.authority(),
            fee_state: fee_state_pubkey,
        })
        .args(marginfi::instruction::EditGlobalFeeState {
            admin: new_admin,
            fee_wallet,
            bank_init_flat_sol_fee,
            liquidation_flat_sol_fee,
            program_fee_fixed,
            program_fee_rate,
            liquidation_max_fee,
        })
        .instructions()?;

    let recent_blockhash = rpc_client.get_latest_blockhash().unwrap();
    let message = Message::new(&edit_fee_state_ixs, Some(&config.authority()));
    let mut transaction = Transaction::new_unsigned(message);
    transaction.partial_sign(&config.get_signers(false), recent_blockhash);

    match process_transaction(&transaction, &rpc_client, config.get_tx_mode()) {
        Ok(sig) => println!("Fee state edited (sig: {})", sig),
        Err(err) => {
            println!("Error during fee state edit:\n{:#?}", err);
            return Err(anyhow!("Error during fee state edit"));
        }
    };

    Ok(())
}

pub fn config_group_fee(config: Config, profile: Profile, enable_program_fee: bool) -> Result<()> {
    let rpc_client = config.mfi_program.rpc();
    let marginfi_group_pubkey = profile.marginfi_group.ok_or_else(|| {
        anyhow!(
            "Marginfi group does not exist for profile [{}]",
            profile.name
        )
    })?;

    let fee_state_pubkey = find_fee_state_pda(&profile.program_id.unwrap()).0;

    let config_group_fee_ixs_builder = config.mfi_program.request();

    let config_group_fee_ixs = config_group_fee_ixs_builder
        .accounts(marginfi::accounts::ConfigGroupFee {
            marginfi_group: marginfi_group_pubkey,
            global_fee_admin: config.authority(),
            fee_state: fee_state_pubkey,
        })
        .args(marginfi::instruction::ConfigGroupFee { enable_program_fee })
        .instructions()?;

    let recent_blockhash = rpc_client.get_latest_blockhash().unwrap();
    let message = Message::new(&config_group_fee_ixs, Some(&config.authority()));
    let mut transaction = Transaction::new_unsigned(message);
    transaction.partial_sign(&config.get_signers(false), recent_blockhash);

    match process_transaction(&transaction, &rpc_client, config.get_tx_mode()) {
        Ok(sig) => println!("Config group fee updated (sig: {})", sig),
        Err(err) => {
            println!("Error during config group fee update:\n{:#?}", err);
            return Err(anyhow!("Error during config group fee update"));
        }
    };

    Ok(())
}

/// Note: doing this one group at a time is tedious, consider running the script instead.
pub fn propagate_fee(config: Config, marginfi_group: Pubkey) -> Result<()> {
    let rpc_client = config.mfi_program.rpc();

    let fee_state_pubkey = find_fee_state_pda(&config.program_id).0;

    let propagate_fee_ixs_builder = config.mfi_program.request();

    let propagate_fee_ixs = propagate_fee_ixs_builder
        .accounts(marginfi::accounts::PropagateFee {
            fee_state: fee_state_pubkey,
            marginfi_group,
        })
        .args(marginfi::instruction::PropagateFeeState {})
        .instructions()?;

    let recent_blockhash = rpc_client.get_latest_blockhash().unwrap();
    let message = Message::new(&propagate_fee_ixs, None);
    let mut transaction = Transaction::new_unsigned(message);
    transaction.partial_sign(&config.get_signers(false), recent_blockhash);

    match process_transaction(&transaction, &rpc_client, config.get_tx_mode()) {
        Ok(sig) => println!("Fee propagated (sig: {})", sig),
        Err(err) => {
            println!("Error during fee propagation:\n{:#?}", err);
            return Err(anyhow!("Error during fee propagation"));
        }
    };

    Ok(())
}

// --------------------------------------------------------------------------------------------------------------------
// bank
// --------------------------------------------------------------------------------------------------------------------

pub fn bank_get(config: Config, bank_pk: Option<Pubkey>) -> Result<()> {
    let rpc_client = config.mfi_program.rpc();

    if let Some(address) = bank_pk {
        let mut bank: Bank = config.mfi_program.account(address)?;
        let group: MarginfiGroup = config.mfi_program.account(bank.group)?;

        let current_timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards");
        let current_timestamp = current_timestamp.as_secs() as i64;

        bank.accrue_interest(current_timestamp, &group)?;
        bank.update_bank_cache(&group, None)?;
        println!(" Cranking interest at: {:?}", current_timestamp);

        print_bank(&address, &bank);

        let liquidity_vault_balance =
            rpc_client.get_token_account_balance(&bank.liquidity_vault)?;
        let fee_vault_balance = rpc_client.get_token_account_balance(&bank.fee_vault)?;
        let insurance_vault_balance =
            rpc_client.get_token_account_balance(&bank.insurance_vault)?;

        println!("=============");
        println!("Token balances:");
        println!(
            "\tliquidity vault: {} (native: {})",
            liquidity_vault_balance.ui_amount.unwrap(),
            liquidity_vault_balance.amount
        );
        println!(
            "\tfee vault: {} (native: {})",
            fee_vault_balance.ui_amount.unwrap(),
            fee_vault_balance.amount
        );
        println!(
            "\tinsurance vault: {} (native: {})",
            insurance_vault_balance.ui_amount.unwrap(),
            insurance_vault_balance.amount
        );
        if bank.emissions_mint != Pubkey::default() {
            let emissions_token_account = find_bank_emssions_token_account_pda(
                address,
                bank.emissions_mint,
                config.program_id,
            )
            .0;
            let emissions_vault_balance =
                rpc_client.get_token_account_balance(&emissions_token_account)?;
            println!(
                "\temissions vault: {} (native: {} - TA: {})",
                emissions_vault_balance.ui_amount.unwrap(),
                emissions_vault_balance.amount,
                emissions_token_account
            );
        }
    } else {
        group_get_all(config)?;
    }
    Ok(())
}

fn load_all_banks(config: &Config, marginfi_group: Option<Pubkey>) -> Result<Vec<(Pubkey, Bank)>> {
    info!("Loading banks for group {:?}", marginfi_group);
    let filters = match marginfi_group {
        Some(marginfi_group) => vec![RpcFilterType::Memcmp(Memcmp::new_raw_bytes(
            8 + size_of::<Pubkey>() + size_of::<u8>(),
            marginfi_group.to_bytes().to_vec(),
        ))],
        None => vec![],
    };

    let banks_with_addresses = config.mfi_program.accounts::<Bank>(filters)?;

    Ok(banks_with_addresses)
}

pub fn bank_get_all(config: Config, marginfi_group: Option<Pubkey>) -> Result<()> {
    let accounts = load_all_banks(&config, marginfi_group)?;
    for (address, state) in accounts {
        print_bank(&address, &state);
    }
    Ok(())
}

pub fn bank_inspect_price_oracle(config: Config, bank_pk: Pubkey) -> Result<()> {
    use marginfi::state::price::{OraclePriceType, PriceBias};

    let bank: Bank = config.mfi_program.account(bank_pk)?;
    let opfa = match bank.config.oracle_setup {
        OracleSetup::Fixed => OraclePriceFeedAdapter::try_from_bank_with_max_age(
            &bank,
            &[],
            &Clock::default(),
            u64::MAX,
        )
        .unwrap(),
        _ => {
            let mut price_oracle_account = config
                .mfi_program
                .rpc()
                .get_account(&bank.config.oracle_keys[0])?;
            let price_oracle_ai =
                (&bank.config.oracle_keys[0], &mut price_oracle_account).into_account_info();

            OraclePriceFeedAdapter::try_from_bank_with_max_age(
                &bank,
                &[price_oracle_ai],
                &Clock::default(),
                u64::MAX,
            )
            .unwrap()
        }
    };

    let (real_price, maint_asset_price, maint_liab_price, init_asset_price, init_liab_price) = (
        opfa.get_price_of_type_ignore_conf(OraclePriceType::RealTime, None)?,
        opfa.get_price_of_type_ignore_conf(OraclePriceType::RealTime, Some(PriceBias::Low))?,
        opfa.get_price_of_type_ignore_conf(OraclePriceType::RealTime, Some(PriceBias::High))?,
        opfa.get_price_of_type_ignore_conf(OraclePriceType::TimeWeighted, Some(PriceBias::Low))?,
        opfa.get_price_of_type_ignore_conf(OraclePriceType::TimeWeighted, Some(PriceBias::High))?,
    );

    let keys = bank
        .config
        .oracle_keys
        .iter()
        .filter(|k| k != &&Pubkey::default())
        .collect::<Vec<_>>();

    println!(
        r##"
Oracle Setup: {setup:?}
Oracle Keys: {keys:#?}
Prince:
    Realtime: {real_price}
    Maint: {maint_asset_price} (asset) {maint_liab_price} (liab)
    Init: {init_asset_price} (asset) {init_liab_price} (liab)
    "##,
        setup = bank.config.oracle_setup,
        keys = keys,
        real_price = real_price,
        maint_asset_price = maint_asset_price,
        maint_liab_price = maint_liab_price,
        init_asset_price = init_asset_price,
        init_liab_price = init_liab_price,
    );

    Ok(())
}

pub fn show_oracle_ages(config: Config, only_stale: bool) -> Result<()> {
    let banks = config
        .mfi_program
        .accounts::<Bank>(vec![RpcFilterType::Memcmp(Memcmp::new_raw_bytes(
            8 + size_of::<Pubkey>() + size_of::<u8>(),
            solana_sdk::pubkey!("4qp6Fx6tnZkY5Wropq9wUYgtFxXKwE6viZxFHg3rdAG8")
                .to_bytes()
                .to_vec(),
        ))])?;

    let (pyth_feeds, swb_feeds): (Vec<_>, Vec<_>) = banks
        .into_iter()
        .map(|(_, b)| {
            (
                b.config.oracle_setup,
                b.config.oracle_max_age,
                b.mint,
                *b.config.oracle_keys.clone().first().unwrap(),
            )
        })
        .partition(|(setup, _, _, _)| {
            matches!(
                setup,
                OracleSetup::PythPushOracle
                    | OracleSetup::KaminoPythPush
                    | OracleSetup::StakedWithPythPush
            )
        });

    let pyth_feeds = pyth_feeds
        .into_iter()
        .map(|(_, max_age, mint, key)| (max_age, mint, key))
        .collect::<Vec<_>>();

    let swb_feeds = swb_feeds
        .into_iter()
        .filter(|(setup, _, _, _)| matches!(setup, OracleSetup::SwitchboardPull))
        .map(|(_, max_age, mint, key)| (max_age, mint, key))
        .collect::<Vec<_>>();

    let mut pyth_max_ages: HashMap<Pubkey, (u16, f64)> = HashMap::from_iter(
        pyth_feeds
            .iter()
            .map(|(max_age, mint, _)| (*max_age, *mint))
            .map(|(max_age, mint)| (mint, (max_age, 0f64))),
    );
    let mut swb_max_ages: HashMap<Pubkey, (u16, f64)> = HashMap::from_iter(
        swb_feeds
            .iter()
            .map(|(max_age, mint, _)| (*max_age, *mint))
            .map(|(max_age, mint)| (mint, (max_age, 0f64))),
    );

    loop {
        let pyth_keys = pyth_feeds
            .iter()
            .map(|(_, _, key)| *key)
            .collect::<Vec<_>>();
        let pyth_mints = pyth_feeds
            .iter()
            .map(|(_, key, _)| *key)
            .collect::<Vec<_>>();
        let pyth_max_age = pyth_feeds
            .iter()
            .map(|(max_age, _, _)| *max_age)
            .collect::<Vec<_>>();
        let pyth_feed_accounts = config
            .mfi_program
            .rpc()
            .get_multiple_accounts(pyth_keys.as_slice())?
            .into_iter()
            .zip(pyth_mints)
            .zip(pyth_max_age)
            .map(|((maybe_account, mint), max_age)| {
                let account = maybe_account.unwrap();
                let price_update = PriceUpdateV2::deserialize(&mut &account.data()[8..]).unwrap();

                (mint, price_update, max_age)
            })
            .collect::<Vec<_>>();

        let swb_keys = swb_feeds.iter().map(|(_, _, key)| *key).collect::<Vec<_>>();
        let swb_mints = swb_feeds.iter().map(|(_, key, _)| *key).collect::<Vec<_>>();
        let swb_max_age = swb_feeds
            .iter()
            .map(|(max_age, _, _)| *max_age)
            .collect::<Vec<_>>();
        let swb_feed_accounts = config
            .mfi_program
            .rpc()
            .get_multiple_accounts(swb_keys.as_slice())?
            .into_iter()
            .zip(swb_mints)
            .zip(swb_max_age)
            .map(|((maybe_account, mint), max_age)| {
                let mut account = maybe_account.unwrap();
                let data = account.data_as_mut_slice();
                let cell = RefCell::new(data);
                let feed: PullFeedAccountData = parse_swb_ignore_alignment(cell.borrow()).unwrap();
                let lite_feed = LitePullFeedAccountData::from(&feed);

                (mint, lite_feed, max_age)
            })
            .collect::<Vec<_>>();

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs() as i64;

        let mut pyth_ages = pyth_feed_accounts
            .iter()
            .map(|(mint, pa, _)| ((now - pa.price_message.publish_time) as f64 / 60f64, *mint))
            .collect::<Vec<_>>();
        pyth_ages.sort_by(|(a, _), (b, _)| b.partial_cmp(a).unwrap());

        let mut swb_ages = swb_feed_accounts
            .iter()
            .map(|(mint, pa, _)| ((now - pa.last_update_timestamp) as f64 / 60f64, *mint))
            .collect::<Vec<_>>();
        swb_ages.sort_by(|(a, _), (b, _)| b.partial_cmp(a).unwrap());

        println!("Pyth");
        for (pa, mint) in pyth_ages.into_iter() {
            let (max_age, max_duration) = pyth_max_ages.get_mut(&mint).unwrap();
            *max_duration = pa.max(*max_duration);

            let max_age = if *max_age == 0 {
                1f64
            } else {
                *max_age as f64 / 60f64
            };

            if only_stale && *max_duration < max_age {
                continue;
            }
            println!(
                "- {:?}: {:.2}min (max: {:.2}min) - allowed: {:.2}min",
                mint, pa, max_duration, max_age
            );
        }
        println!("Switchboard");
        for (pa, mint) in swb_ages.into_iter() {
            let (max_age, max_duration) = swb_max_ages.get_mut(&mint).unwrap();
            *max_duration = pa.max(*max_duration);

            let max_age = if *max_age == 0 {
                1f64
            } else {
                *max_age as f64 / 60f64
            };

            if only_stale && *max_duration < max_age {
                continue;
            }
            println!(
                "- {:?}: {:.2}min (max: {:.2}min) - allowed: {:.2}min",
                mint, pa, max_duration, max_age
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]

pub fn bank_setup_emissions(
    config: &Config,
    profile: &Profile,
    bank: Pubkey,
    deposits: bool,
    borrows: bool,
    mint: Pubkey,
    rate: f64,
    total: f64,
) -> Result<()> {
    let rpc_client = config.mfi_program.rpc();

    let mut flags = 0;

    if deposits {
        flags |= EMISSIONS_FLAG_LENDING_ACTIVE;
    }

    if borrows {
        flags |= EMISSIONS_FLAG_BORROW_ACTIVE;
    }

    let emissions_mint_account = config.mfi_program.rpc().get_account(&mint).unwrap();
    let token_program = emissions_mint_account.owner;

    let funding_account_ata =
        anchor_spl::associated_token::get_associated_token_address_with_program_id(
            &config.authority(),
            &mint,
            &token_program,
        );

    let emissions_mint = spl_token_2022::state::Mint::unpack(
        &emissions_mint_account.data[..spl_token_2022::state::Mint::LEN],
    )
    .unwrap();
    let emissions_mint_decimals = emissions_mint.decimals;

    let total_emissions = (total * 10u64.pow(emissions_mint_decimals as u32) as f64) as u64;
    let rate = crate::utils::calc_emissions_rate(rate, emissions_mint_decimals);

    println!(
        "Native rate: {} tokens per 1 bank token (UI) per YEAR",
        rate
    );
    println!("Emissions flag: {:b}", flags);
    println!("Total native emissions: {}", total_emissions);

    // Get (y or n) input from user
    println!("Is this correct? (y/n)");
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    let input = input.trim();

    if input != "y" {
        println!("Aborting");
        return Ok(());
    }

    let ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::LendingPoolSetupEmissions {
            group: profile.marginfi_group.expect("marginfi group not set"),
            delegate_emissions_admin: config.authority(),
            bank,
            emissions_mint: mint,
            emissions_auth: find_bank_emssions_auth_pda(bank, mint, config.program_id).0,
            emissions_token_account: find_bank_emssions_token_account_pda(
                bank,
                mint,
                config.program_id,
            )
            .0,
            emissions_funding_account: funding_account_ata,
            token_program,
            system_program: system_program::id(),
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::LendingPoolSetupEmissions {
            flags,
            rate,
            total_emissions,
        }
        .data(),
    };

    let recent_blockhash = rpc_client.get_latest_blockhash().unwrap();
    let signing_keypairs = config.get_signers(false);

    let message = Message::new(&[ix], Some(&config.authority()));
    let mut transaction = Transaction::new_unsigned(message);
    transaction.partial_sign(&signing_keypairs, recent_blockhash);

    match process_transaction(&transaction, &rpc_client, config.get_tx_mode()) {
        Ok(sig) => println!("Tx succeded (sig: {})", sig),
        Err(err) => println!("Error :\n{:#?}", err),
    };

    Ok(())
}

#[allow(clippy::too_many_arguments)]

pub fn bank_update_emissions(
    config: &Config,
    profile: &Profile,
    bank_pk: Pubkey,
    deposits: bool,
    borrows: bool,
    disable: bool,
    rate: Option<f64>,
    additional_emissions: Option<f64>,
) -> Result<()> {
    assert!(!(disable && (deposits || borrows)));

    let rpc_client = config.mfi_program.rpc();

    let bank = config
        .mfi_program
        .account::<Bank>(bank_pk)
        .unwrap_or_else(|_| panic!("Bank {} not found", bank_pk));

    let emission_mint = bank.emissions_mint;
    let funding_account_ata = get_associated_token_address(&config.authority(), &emission_mint);

    let emissions_mint_decimals = config
        .mfi_program
        .rpc()
        .get_account(&emission_mint)
        .unwrap();

    let emissions_mint_decimals = spl_token_2022::state::Mint::unpack(
        &emissions_mint_decimals.data[..spl_token_2022::state::Mint::LEN],
    )
    .unwrap()
    .decimals;

    let emissions_rate = rate.map(|rate| calc_emissions_rate(rate, emissions_mint_decimals));
    let additional_emissions = additional_emissions
        .map(|emissions| (emissions * 10u64.pow(emissions_mint_decimals as u32) as f64) as u64);
    let emissions_flags = if disable {
        Some(0)
    } else if deposits || borrows {
        let mut flags = 0;

        if deposits {
            flags |= EMISSIONS_FLAG_LENDING_ACTIVE;
        }

        if borrows {
            flags |= EMISSIONS_FLAG_BORROW_ACTIVE;
        }

        Some(flags)
    } else {
        None
    };

    println!(
        "Changes:\n\tRate: {:?}\n\tAdditional emissions: {:?}\n\tFlags: {:?}",
        emissions_rate.map(|rate| format!("{} tokens per 1M bank tokens per YEAR", rate)),
        additional_emissions,
        emissions_flags.map(|flags| format!("{:b}", flags)),
    );

    // Get (y or n) input from user
    println!("Is this correct? (y/n)");

    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    let input = input.trim();

    if input != "y" {
        println!("Aborting");
        return Ok(());
    }

    let ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::LendingPoolUpdateEmissionsParameters {
            group: profile.marginfi_group.expect("marginfi group not set"),
            delegate_emissions_admin: config.authority(),
            bank: bank_pk,
            emissions_mint: emission_mint,
            emissions_token_account: find_bank_emssions_token_account_pda(
                bank_pk,
                emission_mint,
                config.program_id,
            )
            .0,
            emissions_funding_account: funding_account_ata,
            token_program: spl_token::id(),
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::LendingPoolUpdateEmissionsParameters {
            emissions_flags,
            emissions_rate,
            additional_emissions,
        }
        .data(),
    };

    let recent_blockhash = rpc_client.get_latest_blockhash().unwrap();
    let signing_keypairs = config.get_signers(false);

    let message = Message::new(&[ix], Some(&config.authority()));
    let mut transaction = Transaction::new_unsigned(message);
    transaction.partial_sign(&signing_keypairs, recent_blockhash);

    match process_transaction(&transaction, &rpc_client, config.get_tx_mode()) {
        Ok(sig) => println!("Tx succeded (sig: {})", sig),
        Err(err) => println!("Error:\n{:#?}", err),
    };

    Ok(())
}

pub fn bank_configure(
    config: Config,
    profile: Profile,
    bank_pk: Pubkey,
    bank_config_opt: BankConfigOpt,
) -> Result<()> {
    let rpc_client = config.mfi_program.rpc();

    let configure_bank_ixs_builder = config.mfi_program.request();
    let signing_keypairs = config.get_signers(false);

    let configure_bank_ixs = configure_bank_ixs_builder
        .accounts(marginfi::accounts::LendingPoolConfigureBank {
            group: profile.marginfi_group.unwrap(),
            admin: config.authority(),
            bank: bank_pk,
        })
        .args(marginfi::instruction::LendingPoolConfigureBank {
            bank_config_opt: bank_config_opt.clone(),
        })
        .instructions()?;

    let recent_blockhash = rpc_client.get_latest_blockhash().unwrap();
    let message = Message::new(&configure_bank_ixs, Some(&config.authority()));
    let mut transaction = Transaction::new_unsigned(message);
    transaction.partial_sign(&signing_keypairs, recent_blockhash);

    let sig = process_transaction(&transaction, &rpc_client, config.get_tx_mode())?;

    println!("Transaction signature: {}", sig);

    Ok(())
}

pub fn bank_configure_oracle(
    config: Config,
    profile: Profile,
    bank_pk: Pubkey,
    setup: u8,
    oracle: Pubkey,
) -> Result<()> {
    let rpc_client = config.mfi_program.rpc();

    let configure_bank_ixs_builder = config.mfi_program.request();
    let signing_keypairs = config.get_signers(false);

    let extra_accounts = vec![AccountMeta::new_readonly(oracle, false)];

    let mut configure_bank_ixs = configure_bank_ixs_builder
        .accounts(marginfi::accounts::LendingPoolConfigureBankOracle {
            group: profile.marginfi_group.unwrap(),
            admin: config.authority(),
            bank: bank_pk,
        })
        .args(marginfi::instruction::LendingPoolConfigureBankOracle { setup, oracle })
        .instructions()?;

    configure_bank_ixs[0].accounts.extend(extra_accounts);

    let recent_blockhash = rpc_client.get_latest_blockhash().unwrap();
    let message = Message::new(&configure_bank_ixs, Some(&config.authority()));
    let mut transaction = Transaction::new_unsigned(message);
    transaction.partial_sign(&signing_keypairs, recent_blockhash);

    let sig = process_transaction(&transaction, &rpc_client, config.get_tx_mode())?;

    println!("Transaction signature: {}", sig);

    Ok(())
}

// --------------------------------------------------------------------------------------------------------------------
// Profile
// --------------------------------------------------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
pub fn create_profile(
    name: String,
    cluster: Cluster,
    keypair_path: String,
    multisig: Option<Pubkey>,
    rpc_url: String,
    program_id: Option<Pubkey>,
    commitment: Option<CommitmentLevel>,
    marginfi_group: Option<Pubkey>,
    marginfi_account: Option<Pubkey>,
) -> Result<()> {
    let cli_config_dir = get_cli_config_dir();
    let profile = Profile::new(
        name,
        cluster,
        keypair_path,
        multisig,
        rpc_url,
        program_id,
        commitment,
        marginfi_group,
        marginfi_account,
    );
    if !cli_config_dir.exists() {
        fs::create_dir(&cli_config_dir)?;

        let cli_config_file = cli_config_dir.join("config.json");

        fs::write(
            cli_config_file,
            serde_json::to_string(&CliConfig {
                profile_name: profile.name.clone(),
            })?,
        )?;
    }

    let cli_profiles_dir = cli_config_dir.join("profiles");

    if !cli_profiles_dir.exists() {
        fs::create_dir(&cli_profiles_dir)?;
    }

    let profile_file = cli_profiles_dir.join(profile.name.clone() + ".json");
    if profile_file.exists() {
        return Err(anyhow!("Profile {} already exists", profile.name));
    }

    println!("Creating profile {profile:#?}");

    fs::write(&profile_file, serde_json::to_string(&profile)?)?;

    Ok(())
}

pub fn show_profile() -> Result<()> {
    let profile = load_profile()?;
    println!("{profile:?}");
    Ok(())
}

pub fn set_profile(name: String) -> Result<()> {
    let cli_config_dir = get_cli_config_dir();
    let cli_config_file = cli_config_dir.join("config.json");

    if !cli_config_file.exists() {
        return Err(anyhow!("Profiles not configured, run `mfi profile create`"));
    }

    let profile_file = cli_config_dir.join("profiles").join(format!("{name}.json"));

    if !profile_file.exists() {
        return Err(anyhow!("Profile {} does not exist", name));
    }

    let cli_config = fs::read_to_string(&cli_config_file)?;
    let mut cli_config: CliConfig = serde_json::from_str(&cli_config)?;

    cli_config.profile_name = name;

    fs::write(&cli_config_file, serde_json::to_string(&cli_config)?)?;

    Ok(())
}

pub fn list_profiles() -> Result<()> {
    let cli_config_dir = get_cli_config_dir();
    let cli_profiles_dir = cli_config_dir.join("profiles");

    if !cli_profiles_dir.exists() {
        return Err(anyhow!("Profiles not configured, run `mfi profile create`"));
    }

    let mut profiles = fs::read_dir(&cli_profiles_dir)?
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect::<Vec<String>>();

    if profiles.is_empty() {
        println!("No profiles exist");
    }

    let cli_config = serde_json::from_str::<CliConfig>(&fs::read_to_string(
        cli_config_dir.join("config.json"),
    )?)?;

    println!("Current profile: {}", cli_config.profile_name);

    profiles.sort();

    println!("Found {} profiles", profiles.len());
    for profile in profiles {
        println!("{profile}");
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn configure_profile(
    name: String,
    new_name: Option<String>,
    cluster: Option<Cluster>,
    keypair_path: Option<String>,
    multisig: Option<Pubkey>,
    rpc_url: Option<String>,
    program_id: Option<Pubkey>,
    commitment: Option<CommitmentLevel>,
    group: Option<Pubkey>,
    account: Option<Pubkey>,
) -> Result<()> {
    let mut profile = profile::load_profile_by_name(&name)?;
    let using_new_name = new_name.is_some();
    profile.config(
        new_name,
        cluster,
        keypair_path,
        multisig,
        rpc_url,
        program_id,
        commitment,
        group,
        account,
    )?;

    if using_new_name {
        if let Err(e) = profile::delete_profile_by_name(&name) {
            println!("failed to delete old profile {name}: {e:?}");
            return Err(e);
        }
    }

    Ok(())
}

pub fn delete_profile(name: String) -> Result<()> {
    profile::delete_profile_by_name(&name)
}

// --------------------------------------------------------------------------------------------------------------------
// Marginfi Accounts
// --------------------------------------------------------------------------------------------------------------------

pub fn marginfi_account_list(profile: Profile, config: &Config) -> Result<()> {
    let group = profile.marginfi_group.expect("Missing marginfi group");
    let authority = config.authority();

    let banks = HashMap::from_iter(load_all_banks(config, Some(group))?);

    let accounts = config.mfi_program.accounts::<MarginfiAccount>(vec![
        RpcFilterType::Memcmp(Memcmp::new_raw_bytes(8, group.to_bytes().to_vec())),
        RpcFilterType::Memcmp(Memcmp::new_raw_bytes(8 + 32, authority.to_bytes().to_vec())),
    ])?;

    if accounts.is_empty() {
        println!("No marginfi accounts found");
    }

    for (address, marginfi_account) in accounts {
        print_account(
            address,
            marginfi_account,
            banks.clone(),
            profile
                .marginfi_account
                .map_or(false, |default_account| default_account == address),
        )?;
    }

    Ok(())
}

pub fn print_account(
    address: Pubkey,
    marginfi_account: MarginfiAccount,
    banks: HashMap<Pubkey, Bank>,
    default: bool,
) -> Result<()> {
    println!(
        "Address: {} {}",
        address,
        if default { "(default)" } else { "" }
    );
    println!("Lending Account Balances:");
    marginfi_account
        .lending_account
        .get_active_balances_iter()
        .for_each(|balance| {
            let bank = banks.get(&balance.bank_pk).expect("Bank not found");
            let balance_amount = if balance.is_empty(BalanceSide::Assets).not() {
                let native_value = bank.get_asset_amount(balance.asset_shares.into()).unwrap();

                native_value / EXP_10_I80F48[bank.mint_decimals as usize]
            } else if balance.is_empty(BalanceSide::Liabilities).not() {
                let native_value = bank
                    .get_liability_amount(balance.liability_shares.into())
                    .unwrap();

                (native_value / EXP_10_I80F48[bank.mint_decimals as usize]).neg()
            } else {
                I80F48::ZERO
            };

            let mut bank = *bank;
            let mut balance = *balance;

            let mut baw = BankAccountWrapper {
                bank: &mut bank,
                balance: &mut balance,
            };

            // Current timestamp
            let current_timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();

            baw.claim_emissions(current_timestamp).unwrap();

            println!(
                "\tBalance: {:.3}, Bank: {} (mint: {}), Emissions: {}",
                balance_amount,
                balance.bank_pk,
                bank.mint,
                I80F48::from(balance.emissions_outstanding)
            )
        });
    Ok(())
}

pub fn marginfi_account_use(
    mut profile: Profile,
    config: &Config,
    marginfi_account_pk: Pubkey,
) -> Result<()> {
    let group = profile.marginfi_group.expect("Missing marginfi group");
    let authority = config.authority();

    let marginfi_account = config
        .mfi_program
        .account::<MarginfiAccount>(marginfi_account_pk)?;

    if marginfi_account.group != group {
        return Err(anyhow!("Marginfi account does not belong to group"));
    }

    if marginfi_account.authority != authority {
        return Err(anyhow!("Marginfi account does not belong to authority"));
    }

    profile.config(
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        Some(marginfi_account_pk),
    )?;

    println!("Default marginfi account set to: {marginfi_account_pk}");

    Ok(())
}

/// Print the marginfi account for the provided address or the default marginfi account if none is provided
///
/// If marginfi account address is provided use the group in the marginfi account data, otherwise use the profile defaults
pub fn marginfi_account_get(
    profile: Profile,
    config: &Config,
    marginfi_account_pk: Option<Pubkey>,
) -> Result<()> {
    let marginfi_account_pk =
        marginfi_account_pk.unwrap_or_else(|| profile.marginfi_account.unwrap());

    let marginfi_account = config
        .mfi_program
        .account::<MarginfiAccount>(marginfi_account_pk)?;

    let group = marginfi_account.group;

    let banks = HashMap::from_iter(load_all_banks(config, Some(group))?);

    print_account(marginfi_account_pk, marginfi_account, banks, false)?;

    Ok(())
}

pub fn marginfi_account_deposit(
    profile: &Profile,
    config: &Config,
    bank_pk: Pubkey,
    ui_amount: f64,
    deposit_up_to_limit: Option<bool>,
) -> Result<()> {
    let rpc_client = config.mfi_program.rpc();
    let signer = config.get_non_ms_authority_keypair()?;
    let marginfi_account_pk = profile.get_marginfi_account();

    let bank = config.mfi_program.account::<Bank>(bank_pk)?;

    let amount = (I80F48::from_num(ui_amount) * EXP_10_I80F48[bank.mint_decimals as usize])
        .floor()
        .to_num::<u64>();

    // Check that bank belongs to the correct group
    if bank.group != profile.marginfi_group.unwrap() {
        bail!("Bank does not belong to group")
    }

    let bank_mint_account = rpc_client.get_account(&bank.mint)?;
    let token_program = bank_mint_account.owner;

    let deposit_ata = anchor_spl::associated_token::get_associated_token_address_with_program_id(
        &signer.pubkey(),
        &bank.mint,
        &token_program,
    );

    let mut ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::LendingAccountDeposit {
            group: profile.marginfi_group.unwrap(),
            marginfi_account: marginfi_account_pk,
            authority: signer.pubkey(),
            bank: bank_pk,
            signer_token_account: deposit_ata,
            liquidity_vault: bank.liquidity_vault,
            token_program,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::LendingAccountDeposit {
            amount,
            deposit_up_to_limit,
        }
        .data(),
    };
    if token_program == anchor_spl::token_2022::ID {
        ix.accounts
            .push(AccountMeta::new_readonly(bank.mint, false));
    }

    let recent_blockhash = rpc_client.get_latest_blockhash().unwrap();
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&signer.pubkey()),
        &[signer],
        recent_blockhash,
    );

    match process_transaction(&tx, &rpc_client, config.get_tx_mode()) {
        Ok(sig) => println!("Deposit successful: {sig}"),
        Err(err) => println!("Error during deposit:\n{err:#?}"),
    }

    Ok(())
}

pub fn marginfi_account_withdraw(
    profile: &Profile,
    config: &Config,
    bank_pk: Pubkey,
    ui_amount: f64,
    withdraw_all: bool,
) -> Result<()> {
    let signer = config.get_non_ms_authority_keypair()?;

    let rpc_client = config.mfi_program.rpc();

    let marginfi_account_pk = profile.get_marginfi_account();

    let banks = HashMap::from_iter(load_all_banks(
        config,
        Some(profile.marginfi_group.unwrap()),
    )?);
    let bank = banks.get(&bank_pk).expect("Bank not found");

    let marginfi_account = config
        .mfi_program
        .account::<MarginfiAccount>(marginfi_account_pk)?;

    let amount = (I80F48::from_num(ui_amount) * EXP_10_I80F48[bank.mint_decimals as usize])
        .floor()
        .to_num::<u64>();

    // Check that bank belongs to the correct group
    if bank.group != profile.marginfi_group.unwrap() {
        bail!("Bank does not belong to group")
    }

    let bank_mint_account = rpc_client.get_account(&bank.mint)?;
    let token_program = bank_mint_account.owner;

    let withdraw_ata = anchor_spl::associated_token::get_associated_token_address_with_program_id(
        &signer.pubkey(),
        &bank.mint,
        &token_program,
    );

    let mut ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::LendingAccountWithdraw {
            group: profile.marginfi_group.unwrap(),
            marginfi_account: marginfi_account_pk,
            authority: signer.pubkey(),
            bank: bank_pk,
            liquidity_vault: bank.liquidity_vault,
            token_program,
            destination_token_account: withdraw_ata,
            bank_liquidity_vault_authority: find_bank_vault_authority_pda(
                &bank_pk,
                BankVaultType::Liquidity,
                &config.program_id,
            )
            .0,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::LendingAccountWithdraw {
            amount,
            withdraw_all: if withdraw_all { Some(true) } else { None },
        }
        .data(),
    };

    if token_program == anchor_spl::token_2022::ID {
        ix.accounts
            .push(AccountMeta::new_readonly(bank.mint, false));
    }
    ix.accounts.extend(load_observation_account_metas(
        &marginfi_account,
        &banks,
        vec![],
        if withdraw_all { vec![bank_pk] } else { vec![] },
    ));

    let create_ide_ata_ix = create_associated_token_account_idempotent(
        &signer.pubkey(),
        &signer.pubkey(),
        &bank.mint,
        &token_program,
    );

    let recent_blockhash = rpc_client.get_latest_blockhash().unwrap();
    let tx = Transaction::new_signed_with_payer(
        &[create_ide_ata_ix, ix],
        Some(&signer.pubkey()),
        &[signer],
        recent_blockhash,
    );

    match process_transaction(&tx, &rpc_client, config.get_tx_mode()) {
        Ok(sig) => println!("Withdraw successful: {sig}"),
        Err(err) => println!("Error during withdraw:\n{err:#?}"),
    }

    Ok(())
}

pub fn marginfi_account_borrow(
    profile: &Profile,
    config: &Config,
    bank_pk: Pubkey,
    ui_amount: f64,
) -> Result<()> {
    let signer = config.get_non_ms_authority_keypair()?;

    let rpc_client = config.mfi_program.rpc();

    let marginfi_account_pk = profile.get_marginfi_account();

    let banks = HashMap::from_iter(load_all_banks(
        config,
        Some(profile.marginfi_group.unwrap()),
    )?);
    let bank = banks.get(&bank_pk).expect("Bank not found");

    let marginfi_account = config
        .mfi_program
        .account::<MarginfiAccount>(marginfi_account_pk)?;

    let amount = (I80F48::from_num(ui_amount) * EXP_10_I80F48[bank.mint_decimals as usize])
        .floor()
        .to_num::<u64>();

    // Check that bank belongs to the correct group
    if bank.group != profile.marginfi_group.unwrap() {
        bail!("Bank does not belong to group")
    }

    let bank_mint_account = rpc_client.get_account(&bank.mint)?;
    let token_program = bank_mint_account.owner;

    let borrow_ata = anchor_spl::associated_token::get_associated_token_address_with_program_id(
        &signer.pubkey(),
        &bank.mint,
        &token_program,
    );

    let mut ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::LendingAccountBorrow {
            group: profile.marginfi_group.unwrap(),
            marginfi_account: marginfi_account_pk,
            authority: signer.pubkey(),
            bank: bank_pk,
            liquidity_vault: bank.liquidity_vault,
            token_program,
            destination_token_account: borrow_ata,
            bank_liquidity_vault_authority: find_bank_vault_authority_pda(
                &bank_pk,
                BankVaultType::Liquidity,
                &config.program_id,
            )
            .0,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::LendingAccountBorrow { amount }.data(),
    };

    if token_program == anchor_spl::token_2022::ID {
        ix.accounts
            .push(AccountMeta::new_readonly(bank.mint, false));
    }
    ix.accounts.extend(load_observation_account_metas(
        &marginfi_account,
        &banks,
        vec![bank_pk],
        vec![],
    ));

    let create_ide_ata_ix = create_associated_token_account_idempotent(
        &signer.pubkey(),
        &signer.pubkey(),
        &bank.mint,
        &token_program,
    );

    let recent_blockhash = rpc_client.get_latest_blockhash().unwrap();
    let tx = Transaction::new_signed_with_payer(
        &[create_ide_ata_ix, ix],
        Some(&signer.pubkey()),
        &[signer],
        recent_blockhash,
    );

    match process_transaction(&tx, &rpc_client, config.get_tx_mode()) {
        Ok(sig) => println!("Borrow successful: {sig}"),
        Err(err) => println!("Error during borrow:\n{err:#?}"),
    }

    Ok(())
}

pub fn marginfi_account_liquidate(
    profile: &Profile,
    config: &Config,
    liquidatee_marginfi_account_pk: Pubkey,
    asset_bank_pk: Pubkey,
    liability_bank_pk: Pubkey,
    ui_asset_amount: f64,
) -> Result<()> {
    let signer = config.get_non_ms_authority_keypair()?;

    let rpc_client = config.mfi_program.rpc();

    let marginfi_account_pk = profile.get_marginfi_account();

    let banks = HashMap::from_iter(load_all_banks(
        config,
        Some(profile.marginfi_group.unwrap()),
    )?);
    let asset_bank = banks.get(&asset_bank_pk).expect("Asset bank not found");
    let liability_bank = banks
        .get(&liability_bank_pk)
        .expect("Liability bank not found");

    let marginfi_account = config
        .mfi_program
        .account::<MarginfiAccount>(marginfi_account_pk)?;

    let liquidatee_marginfi_account = config
        .mfi_program
        .account::<MarginfiAccount>(liquidatee_marginfi_account_pk)?;

    let asset_amount = (I80F48::from_num(ui_asset_amount)
        * EXP_10_I80F48[asset_bank.mint_decimals as usize])
        .floor()
        .to_num::<u64>();

    // Check that banks belong to the correct group
    if asset_bank.group != profile.marginfi_group.unwrap() {
        bail!("Asset bank does not belong to group")
    }
    if liability_bank.group != profile.marginfi_group.unwrap() {
        bail!("Liability bank does not belong to group")
    }

    let liability_mint_account = rpc_client.get_account(&liability_bank.mint)?;
    let token_program = liability_mint_account.owner;

    let liquidator_accounts = load_observation_account_metas(
        &marginfi_account,
        &banks,
        vec![liability_bank_pk, asset_bank_pk],
        vec![],
    );
    let liquidatee_accounts =
        load_observation_account_metas(&liquidatee_marginfi_account, &banks, vec![], vec![]);

    let mut ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::LendingAccountLiquidate {
            group: profile.marginfi_group.unwrap(),
            asset_bank: asset_bank_pk,
            liab_bank: liability_bank_pk,
            liquidator_marginfi_account: marginfi_account_pk,
            authority: signer.pubkey(),
            liquidatee_marginfi_account: liquidatee_marginfi_account_pk,
            bank_liquidity_vault_authority: find_bank_vault_authority_pda(
                &liability_bank_pk,
                BankVaultType::Liquidity,
                &config.program_id,
            )
            .0,
            bank_liquidity_vault: liability_bank.liquidity_vault,
            bank_insurance_vault: liability_bank.insurance_vault,
            token_program,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::LendingAccountLiquidate {
            asset_amount,
            liquidatee_accounts: liquidatee_accounts.len() as u8,
            liquidator_accounts: liquidator_accounts.len() as u8,
        }
        .data(),
    };

    let oracle_accounts = vec![asset_bank, liability_bank].into_iter().map(|bank| {
        let oracle_key = bank_to_oracle_key(&bank.config);
        AccountMeta::new_readonly(oracle_key, false)
    });

    ix.accounts.extend(oracle_accounts);

    let oracle_accounts = vec![asset_bank, liability_bank].into_iter().map(|bank| {
        let oracle_key = bank_to_oracle_key(&bank.config);
        AccountMeta::new_readonly(oracle_key, false)
    });

    ix.accounts.extend(oracle_accounts);

    if token_program == anchor_spl::token_2022::ID {
        ix.accounts
            .push(AccountMeta::new_readonly(liability_bank.mint, false));
    }
    ix.accounts.push(AccountMeta {
        pubkey: asset_bank.config.oracle_keys[0],
        is_signer: false,
        is_writable: false,
    });
    ix.accounts.push(AccountMeta {
        pubkey: liability_bank.config.oracle_keys[0],
        is_signer: false,
        is_writable: false,
    });
    ix.accounts.extend(liquidator_accounts);
    ix.accounts.extend(liquidatee_accounts);

    let cu_ix = ComputeBudgetInstruction::set_compute_unit_limit(1_400_000);

    let recent_blockhash = rpc_client.get_latest_blockhash().unwrap();
    let tx = Transaction::new_signed_with_payer(
        &[ix, cu_ix],
        Some(&signer.pubkey()),
        &[signer],
        recent_blockhash,
    );

    match process_transaction(&tx, &config.mfi_program.rpc(), config.get_tx_mode()) {
        Ok(sig) => println!("Liquidation successful: {sig}"),
        Err(err) => println!("Error during liquidation:\n{err:#?}"),
    }

    Ok(())
}

pub fn marginfi_account_create(profile: &Profile, config: &Config) -> Result<()> {
    let signer = config.get_non_ms_authority_keypair()?;

    let rpc_client = config.mfi_program.rpc();

    let marginfi_account_key = Keypair::new();

    let ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::MarginfiAccountInitialize {
            marginfi_group: profile.marginfi_group.unwrap(),
            marginfi_account: marginfi_account_key.pubkey(),
            system_program: system_program::ID,
            authority: signer.pubkey(),
            fee_payer: signer.pubkey(),
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::MarginfiAccountInitialize.data(),
    };

    let recent_blockhash = rpc_client.get_latest_blockhash().unwrap();
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&signer.pubkey()),
        &[signer, &marginfi_account_key],
        recent_blockhash,
    );

    let marginfi_account_pk = marginfi_account_key.pubkey();

    match process_transaction(&tx, &config.mfi_program.rpc(), config.get_tx_mode()) {
        Ok(_sig) => print!("{marginfi_account_pk}"),
        Err(err) => println!("Error during initialize:\n{err:#?}"),
    }

    let mut profile = profile.clone();

    profile.config(
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        Some(marginfi_account_key.pubkey()),
    )?;

    Ok(())
}

pub fn marginfi_account_close(profile: &Profile, config: &Config) -> Result<()> {
    let signer = config.get_non_ms_authority_keypair()?;

    let rpc_client = config.mfi_program.rpc();

    let marginfi_account_pk = profile.get_marginfi_account();
    println!("Closing marginfi account {}", marginfi_account_pk);

    let ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::MarginfiAccountClose {
            marginfi_account: marginfi_account_pk,
            authority: signer.pubkey(),
            fee_payer: signer.pubkey(),
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::MarginfiAccountClose.data(),
    };

    let recent_blockhash = rpc_client.get_latest_blockhash().unwrap();
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&signer.pubkey()),
        &[signer],
        recent_blockhash,
    );

    match process_transaction(&tx, &config.mfi_program.rpc(), config.get_tx_mode()) {
        Ok(sig) => println!("Marginfi account closed successfully (sig: {})", sig),
        Err(err) => println!("Error during marginfi account closure:\n{:#?}", err),
    };

    Ok(())
}
