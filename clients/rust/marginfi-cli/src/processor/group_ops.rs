use {
    crate::{
        config::Config,
        output,
        profile::Profile,
        utils::{
            find_bank_vault_authority_pda, find_bank_vault_pda, find_fee_state_pda,
            load_observation_account_metas, send_tx,
        },
        RatePointArg,
    },
    anchor_client::anchor_lang::{InstructionData, ToAccountMetas},
    anchor_spl::token_2022::spl_token_2022,
    anyhow::{bail, Context, Result},
    fixed::types::I80F48,
    log::info,
    marginfi::{
        state::bank::{BankImpl, BankVaultType},
        utils::NumTraitsWithTolerance,
    },
    marginfi_type_crate::{
        constants::ZERO_AMOUNT_THRESHOLD,
        types::{
            make_points, Bank, BankConfigCompact, BankOperationalState, InterestRateConfig,
            MarginfiAccount, MarginfiGroup, RatePoint, WrappedI80F48, CURVE_POINTS,
            INTEREST_CURVE_SEVEN_POINT,
        },
    },
    solana_client::{
        rpc_client::RpcClient,
        rpc_filter::{Memcmp, RpcFilterType},
    },
    solana_sdk::{
        instruction::{AccountMeta, Instruction},
        program_pack::Pack,
        pubkey::Pubkey,
        signature::Keypair,
        signer::Signer,
        system_program,
    },
    std::{collections::HashMap, mem::size_of},
};

// --------------------------------------------------------------------------------------------------------------------
// marginfi group
// --------------------------------------------------------------------------------------------------------------------

pub fn group_get(config: Config, marginfi_group: Option<Pubkey>) -> Result<()> {
    let json = config.json_output;
    if let Some(marginfi_group) = marginfi_group {
        let group: MarginfiGroup = config.mfi_program.account(marginfi_group)?;
        output::print_group_detail(&marginfi_group, &group, json);
        if !json {
            println!("--------\nBanks:");
        }
        print_group_banks(config, marginfi_group)?;
    } else {
        group_get_all(config)?;
    }
    Ok(())
}

pub fn group_get_all(config: Config) -> Result<()> {
    let json = config.json_output;
    let accounts: Vec<(Pubkey, MarginfiGroup)> = config.mfi_program.accounts(vec![])?;

    for (address, group) in &accounts {
        output::print_group_detail(address, group, json);
    }

    Ok(())
}

pub fn print_group_banks(config: Config, marginfi_group: Pubkey) -> Result<()> {
    let json = config.json_output;
    let banks = config
        .mfi_program
        .accounts::<Bank>(vec![RpcFilterType::Memcmp(Memcmp::new_raw_bytes(
            8 + size_of::<Pubkey>() + size_of::<u8>(),
            marginfi_group.to_bytes().to_vec(),
        ))])?;

    output::print_banks_table(&banks, json);

    Ok(())
}

pub fn load_all_banks(
    config: &Config,
    marginfi_group: Option<Pubkey>,
) -> Result<Vec<(Pubkey, Bank)>> {
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

pub fn group_create(
    config: Config,
    profile: Profile,
    admin: Option<Pubkey>,
    override_existing_profile_group: bool,
) -> Result<()> {
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
            fee_state: find_fee_state_pda(&config.program_id).0,
            system_program: system_program::id(),
        })
        .args(marginfi::instruction::MarginfiGroupInitialize {})
        .instructions()?;

    let sig = send_tx(&config, init_marginfi_group_ixs, &signing_keypairs)?;
    println!("marginfi group created (sig: {})", sig);

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
    emode_max_init_leverage: Option<f64>,
    emode_max_maint_leverage: Option<f64>,
) -> Result<()> {
    if profile.marginfi_group.is_none() {
        bail!("Marginfi group not specified in profile [{}]", profile.name);
    }

    let signing_keypairs = config.get_signers(false);
    let configure_marginfi_group_ixs_builder = config.mfi_program.request();

    let configure_marginfi_group_ixs = configure_marginfi_group_ixs_builder
        .accounts(marginfi::accounts::MarginfiGroupConfigure {
            marginfi_group: profile
                .marginfi_group
                .context("marginfi group not set in profile")?,
            admin: config.authority(),
        })
        .args(marginfi::instruction::MarginfiGroupConfigure {
            new_admin: Some(new_admin),
            new_emode_admin: Some(new_emode_admin),
            new_curve_admin: Some(new_curve_admin),
            new_limit_admin: Some(new_limit_admin),
            new_emissions_admin: Some(new_emissions_admin),
            new_metadata_admin: Some(new_metadata_admin),
            new_risk_admin: Some(new_risk_admin),
            emode_max_init_leverage: emode_max_init_leverage
                .map(|value| I80F48::from_num(value).into()),
            emode_max_maint_leverage: emode_max_maint_leverage
                .map(|value| I80F48::from_num(value).into()),
        })
        .instructions()?;

    let sig = send_tx(&config, configure_marginfi_group_ixs, &signing_keypairs)?;
    println!("marginfi group configured (sig: {})", sig);

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
    _compute_unit_price: Option<u64>,
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

    let sig = send_tx(&config, add_bank_ixs, &signing_keypairs)?;
    println!("bank created (sig: {})", sig);

    Ok(())
}

pub fn group_clone_bank(
    config: Config,
    profile: Profile,
    source_bank: Pubkey,
    bank_mint: Pubkey,
    bank_seed: u64,
) -> Result<()> {
    let marginfi_group = profile
        .marginfi_group
        .context("marginfi group not set in profile")?;

    let mint_account = config.mfi_program.rpc().get_account(&bank_mint)?;
    let token_program = mint_account.owner;

    let (bank_pda, _) = Pubkey::find_program_address(
        &[
            marginfi_group.as_ref(),
            bank_mint.as_ref(),
            &bank_seed.to_le_bytes(),
        ],
        &config.program_id,
    );

    let clone_bank_ixs = config
        .mfi_program
        .request()
        .accounts(marginfi::accounts::LendingPoolCloneBank {
            marginfi_group,
            admin: config.authority(),
            fee_payer: config.explicit_fee_payer(),
            bank_mint,
            source_bank,
            bank: bank_pda,
            liquidity_vault_authority: find_bank_vault_authority_pda(
                &bank_pda,
                BankVaultType::Liquidity,
                &config.program_id,
            )
            .0,
            liquidity_vault: find_bank_vault_pda(
                &bank_pda,
                BankVaultType::Liquidity,
                &config.program_id,
            )
            .0,
            insurance_vault_authority: find_bank_vault_authority_pda(
                &bank_pda,
                BankVaultType::Insurance,
                &config.program_id,
            )
            .0,
            insurance_vault: find_bank_vault_pda(
                &bank_pda,
                BankVaultType::Insurance,
                &config.program_id,
            )
            .0,
            fee_vault_authority: find_bank_vault_authority_pda(
                &bank_pda,
                BankVaultType::Fee,
                &config.program_id,
            )
            .0,
            fee_vault: find_bank_vault_pda(&bank_pda, BankVaultType::Fee, &config.program_id).0,
            token_program,
            system_program: system_program::id(),
        })
        .args(marginfi::instruction::LendingPoolCloneBank { bank_seed })
        .instructions()?;

    let signing_keypairs = config.get_signers(true);
    let sig = send_tx(&config, clone_bank_ixs, &signing_keypairs)?;
    println!("bank cloned (sig: {}, bank: {})", sig, bank_pda);

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
    let group_key = profile
        .marginfi_group
        .context("marginfi group not set in profile")?;

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
            marginfi_group: profile
                .marginfi_group
                .context("marginfi group not set in profile")?,
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
            marginfi_group: profile
                .marginfi_group
                .context("marginfi group not set in profile")?,
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
        Some(
            profile
                .marginfi_group
                .context("marginfi group not set in profile")?,
        ),
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
        Some(
            profile
                .marginfi_group
                .context("marginfi group not set in profile")?,
        ),
    )?);
    let marginfi_account = config
        .mfi_program
        .account::<MarginfiAccount>(marginfi_account_pk)?;

    let target_banks = marginfi_account
        .lending_account
        .balances
        .iter()
        .filter_map(|balance| {
            if !balance.is_active() {
                return None;
            }
            let bank = banks.get(&balance.bank_pk)?;
            let liability_amount = bank
                .get_liability_amount(balance.liability_shares.into())
                .ok()?;
            liability_amount
                .is_positive_with_tolerance(ZERO_AMOUNT_THRESHOLD)
                .then_some(balance.bank_pk)
        })
        .collect::<Vec<Pubkey>>();

    for bank_pk in target_banks {
        handle_bankruptcy_for_an_account(
            config,
            &profile,
            &rpc_client,
            &banks,
            marginfi_account_pk,
            &marginfi_account,
            bank_pk,
        )?;
    }

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

    let bank = banks.get(&bank_pk).context("bank not found")?;

    let bank_mint_account = rpc_client.get_account(&bank.mint)?;
    let token_program = bank_mint_account.owner;
    let mut handle_bankruptcy_ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::LendingPoolHandleBankruptcy {
            group: profile
                .marginfi_group
                .context("marginfi group not set in profile")?,
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

    let signing_keypairs = config.get_signers(false);

    let sig = send_tx(config, vec![handle_bankruptcy_ix], &signing_keypairs)?;
    println!("Bankruptcy handled (sig: {})", sig);

    Ok(())
}

const BANKRUPTCY_CHUNKS: usize = 4;

pub fn handle_bankruptcy_for_accounts(
    config: &Config,
    profile: &Profile,
    accounts: Vec<Pubkey>,
) -> Result<()> {
    let mut instructions = vec![];

    let banks = HashMap::from_iter(load_all_banks(
        config,
        Some(
            profile
                .marginfi_group
                .context("marginfi group not set in profile")?,
        ),
    )?);

    for account in accounts {
        let marginfi_account = config
            .mfi_program
            .account::<MarginfiAccount>(account)
            .context("failed to fetch marginfi account")?;

        let account_bankrupt_banks = marginfi_account
            .lending_account
            .balances
            .iter()
            .filter_map(|balance| {
                if !balance.is_active() {
                    return None;
                }
                let bank = banks.get(&balance.bank_pk)?;
                let liability_amount = bank
                    .get_liability_amount(balance.liability_shares.into())
                    .ok()?;
                liability_amount
                    .is_positive_with_tolerance(ZERO_AMOUNT_THRESHOLD)
                    .then_some(balance.bank_pk)
            })
            .collect::<Vec<Pubkey>>();

        for bank_pk in account_bankrupt_banks {
            instructions.push(make_bankruptcy_ix(
                config,
                profile,
                &banks,
                account,
                &marginfi_account,
                bank_pk,
            )?);
        }
    }

    println!("Handling {} bankruptcies", instructions.len());

    let chunks = instructions.chunks(BANKRUPTCY_CHUNKS);

    for chunk in chunks {
        let signing_keypairs = config.get_signers(false);
        let ixs = chunk.to_vec();

        let sig = send_tx(config, ixs, &signing_keypairs)?;
        println!("Bankruptcy handled (sig: {})", sig);
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

    let bank = banks.get(&bank_pk).context("bank not found")?;

    let bank_mint_account = rpc_client.get_account(&bank.mint)?;
    let token_program = bank_mint_account.owner;
    let mut handle_bankruptcy_ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::LendingPoolHandleBankruptcy {
            group: profile
                .marginfi_group
                .context("marginfi group not set in profile")?,
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
