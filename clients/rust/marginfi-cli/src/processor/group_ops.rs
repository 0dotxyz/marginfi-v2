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
    borsh::BorshDeserialize,
    fixed::types::I80F48,
    log::info,
    marginfi::{
        constants::SPL_SINGLE_POOL_ID,
        state::{
            bank::{BankImpl, BankVaultType},
            bank_config::BankConfigImpl,
        },
        utils::NumTraitsWithTolerance,
    },
    marginfi_type_crate::{
        constants::{STAKED_SETTINGS_SEED, ZERO_AMOUNT_THRESHOLD},
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
        stake::state::StakeStateV2,
        system_program,
    },
    std::{collections::HashMap, mem::size_of},
};

pub struct StandardBankCreateRequest {
    pub group: Pubkey,
    pub bank_mint: Pubkey,
    pub seed: Option<u64>,
    pub asset_weight_init: f64,
    pub asset_weight_maint: f64,
    pub liability_weight_init: f64,
    pub liability_weight_maint: f64,
    pub deposit_limit_ui: u64,
    pub borrow_limit_ui: u64,
    pub zero_util_rate: u32,
    pub hundred_util_rate: u32,
    pub points: Vec<RatePointArg>,
    pub insurance_fee_fixed_apr: f64,
    pub insurance_ir_fee: f64,
    pub protocol_fixed_fee_apr: f64,
    pub protocol_ir_fee: f64,
    pub protocol_origination_fee: f64,
    pub risk_tier: crate::RiskTierArg,
    pub oracle_max_age: u16,
    pub oracle_max_confidence: u32,
    pub asset_tag: u8,
    pub global_fee_wallet: Option<Pubkey>,
    pub oracle: Pubkey,
    pub oracle_type: u8,
}

pub struct StakedBankCreateRequest {
    pub group: Pubkey,
    pub stake_pool: Pubkey,
    pub seed: Option<u64>,
}

pub struct GroupCreateConfigRequest {
    pub emode_admin: Option<Pubkey>,
    pub curve_admin: Option<Pubkey>,
    pub limit_admin: Option<Pubkey>,
    pub flow_admin: Option<Pubkey>,
    pub emissions_admin: Option<Pubkey>,
    pub metadata_admin: Option<Pubkey>,
    pub risk_admin: Option<Pubkey>,
    pub emode_max_init_leverage: Option<f64>,
    pub emode_max_maint_leverage: Option<f64>,
}

impl GroupCreateConfigRequest {
    fn is_empty(&self) -> bool {
        self.emode_admin.is_none()
            && self.curve_admin.is_none()
            && self.limit_admin.is_none()
            && self.flow_admin.is_none()
            && self.emissions_admin.is_none()
            && self.metadata_admin.is_none()
            && self.risk_admin.is_none()
            && self.emode_max_init_leverage.is_none()
            && self.emode_max_maint_leverage.is_none()
    }
}

// --------------------------------------------------------------------------------------------------------------------
// marginfi group
// --------------------------------------------------------------------------------------------------------------------

pub fn group_get(config: Config, marginfi_group: Option<Pubkey>) -> Result<()> {
    let json = config.json_output;
    if let Some(marginfi_group) = marginfi_group {
        let group: MarginfiGroup = config.mfi_program.account(marginfi_group)?;
        if json {
            let banks = load_all_banks(&config, Some(marginfi_group))?;
            let val = serde_json::json!({
                "group": output::group_detail_json(&marginfi_group, &group),
                "banks": output::banks_table_json(&banks),
            });
            println!("{}", serde_json::to_string_pretty(&val)?);
        } else {
            output::print_group_detail(&marginfi_group, &group, false);
            println!("--------\nBanks:");
            print_group_banks(config, marginfi_group)?;
        }
    } else {
        group_get_all(config)?;
    }
    Ok(())
}

pub fn group_get_all(config: Config) -> Result<()> {
    let json = config.json_output;
    let accounts: Vec<(Pubkey, MarginfiGroup)> = config.mfi_program.accounts(vec![])?;

    if json {
        let vals = accounts
            .iter()
            .map(|(address, group)| output::group_detail_json(address, group))
            .collect::<Vec<_>>();
        println!("{}", serde_json::to_string_pretty(&vals)?);
    } else {
        for (address, group) in &accounts {
            output::print_group_detail(address, group, false);
        }
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
    create_config: GroupCreateConfigRequest,
) -> Result<()> {
    let authority = config.authority();
    let final_admin = admin.unwrap_or(authority);

    if profile.marginfi_group.is_some() && !override_existing_profile_group {
        bail!(
            "Marginfi group already exists for profile [{}]",
            profile.name
        );
    }

    let marginfi_group_keypair = Keypair::new();
    let needs_post_create_config = !create_config.is_empty();
    let init_admin = if needs_post_create_config {
        authority
    } else {
        final_admin
    };

    let init_marginfi_group_ixs_builder = config.mfi_program.request();

    let mut signing_keypairs = config.get_signers(false);
    signing_keypairs.push(&marginfi_group_keypair);

    let init_marginfi_group_ixs = init_marginfi_group_ixs_builder
        .accounts(marginfi::accounts::MarginfiGroupInitialize {
            marginfi_group: marginfi_group_keypair.pubkey(),
            admin: init_admin,
            fee_state: find_fee_state_pda(&config.program_id).0,
            system_program: system_program::id(),
        })
        .args(marginfi::instruction::MarginfiGroupInitialize {})
        .instructions()?;

    let mut group_ixs = init_marginfi_group_ixs;

    if needs_post_create_config {
        let configure_ixs = config
            .mfi_program
            .request()
            .accounts(marginfi::accounts::MarginfiGroupConfigure {
                marginfi_group: marginfi_group_keypair.pubkey(),
                admin: authority,
            })
            .args(marginfi::instruction::MarginfiGroupConfigure {
                new_admin: (final_admin != authority).then_some(final_admin),
                new_emode_admin: create_config.emode_admin,
                new_curve_admin: create_config.curve_admin,
                new_limit_admin: create_config.limit_admin,
                new_flow_admin: create_config.flow_admin,
                new_emissions_admin: create_config.emissions_admin,
                new_metadata_admin: create_config.metadata_admin,
                new_risk_admin: create_config.risk_admin,
                emode_max_init_leverage: create_config
                    .emode_max_init_leverage
                    .map(|value| I80F48::from_num(value).into()),
                emode_max_maint_leverage: create_config
                    .emode_max_maint_leverage
                    .map(|value| I80F48::from_num(value).into()),
                same_asset_emode_init_leverage: None,
                same_asset_emode_maint_leverage: None,
            })
            .instructions()?;
        group_ixs.extend(configure_ixs);
    }

    let sig = send_tx(&config, group_ixs, &signing_keypairs)?;
    println!("marginfi group created (sig: {})", sig);

    if config.send_tx {
        let mut profile = profile;
        profile.set_marginfi_group(marginfi_group_keypair.pubkey())?;
    }

    Ok(())
}

pub fn group_configure(
    config: Config,
    profile: Profile,
    new_admin: Option<Pubkey>,
    new_emode_admin: Option<Pubkey>,
    new_curve_admin: Option<Pubkey>,
    new_limit_admin: Option<Pubkey>,
    new_flow_admin: Option<Pubkey>,
    new_emissions_admin: Option<Pubkey>,
    new_metadata_admin: Option<Pubkey>,
    new_risk_admin: Option<Pubkey>,
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
            new_admin,
            new_emode_admin,
            new_curve_admin,
            new_limit_admin,
            new_flow_admin,
            new_emissions_admin,
            new_metadata_admin,
            new_risk_admin,
            emode_max_init_leverage: emode_max_init_leverage
                .map(|value| I80F48::from_num(value).into()),
            emode_max_maint_leverage: emode_max_maint_leverage
                .map(|value| I80F48::from_num(value).into()),
            same_asset_emode_init_leverage: None,
            same_asset_emode_maint_leverage: None,
        })
        .instructions()?;

    let sig = send_tx(&config, configure_marginfi_group_ixs, &signing_keypairs)?;
    println!("marginfi group configured (sig: {})", sig);

    Ok(())
}

pub fn create_standard_bank(config: Config, request: StandardBankCreateRequest) -> Result<()> {
    let rpc_client = config.mfi_program.rpc();
    let global_fee_wallet = match request.global_fee_wallet {
        Some(pubkey) => pubkey,
        None => {
            let fee_state_pk = find_fee_state_pda(&config.program_id).0;
            let fee_state: marginfi_type_crate::types::FeeState =
                config.mfi_program.account(fee_state_pk)?;
            fee_state.global_fee_wallet
        }
    };

    let asset_weight_init: WrappedI80F48 = I80F48::from_num(request.asset_weight_init).into();
    let asset_weight_maint: WrappedI80F48 = I80F48::from_num(request.asset_weight_maint).into();
    let liability_weight_init: WrappedI80F48 =
        I80F48::from_num(request.liability_weight_init).into();
    let liability_weight_maint: WrappedI80F48 =
        I80F48::from_num(request.liability_weight_maint).into();

    let optimal_utilization_rate: WrappedI80F48 = I80F48::ZERO.into();
    let plateau_interest_rate: WrappedI80F48 = I80F48::ZERO.into();
    let max_interest_rate: WrappedI80F48 = I80F48::ZERO.into();
    let insurance_fee_fixed_apr: WrappedI80F48 =
        I80F48::from_num(request.insurance_fee_fixed_apr).into();
    let insurance_ir_fee: WrappedI80F48 = I80F48::from_num(request.insurance_ir_fee).into();
    let protocol_fixed_fee_apr: WrappedI80F48 =
        I80F48::from_num(request.protocol_fixed_fee_apr).into();
    let protocol_ir_fee: WrappedI80F48 = I80F48::from_num(request.protocol_ir_fee).into();
    let protocol_origination_fee: WrappedI80F48 =
        I80F48::from_num(request.protocol_origination_fee).into();

    let mint_account = rpc_client.get_account(&request.bank_mint)?;
    let token_program = mint_account.owner;
    let mint = spl_token_2022::state::Mint::unpack(
        &mint_account.data[..spl_token_2022::state::Mint::LEN],
    )?;
    let deposit_limit = request.deposit_limit_ui * 10_u64.pow(mint.decimals as u32);
    let borrow_limit = request.borrow_limit_ui * 10_u64.pow(mint.decimals as u32);

    let pts_raw: Vec<RatePoint> = request
        .points
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
        protocol_fixed_fee_apr,
        protocol_ir_fee,
        protocol_origination_fee,
        zero_util_rate: request.zero_util_rate,
        hundred_util_rate: request.hundred_util_rate,
        points,
        curve_type: INTEREST_CURVE_SEVEN_POINT,
        ..InterestRateConfig::default()
    };
    let compact_config = BankConfigCompact {
        asset_weight_init,
        asset_weight_maint,
        liability_weight_init,
        liability_weight_maint,
        deposit_limit,
        borrow_limit,
        interest_rate_config: interest_rate_config.into(),
        operational_state: BankOperationalState::Operational,
        risk_tier: request.risk_tier.into(),
        oracle_max_age: request.oracle_max_age,
        oracle_max_confidence: request.oracle_max_confidence,
        asset_tag: request.asset_tag,
        ..BankConfigCompact::default()
    };
    let bank_config = marginfi_type_crate::types::BankConfig::from(compact_config);
    bank_config
        .validate()
        .context("invalid standard bank config")?;

    let bank_seed = resolve_bank_seed(
        &rpc_client,
        &config.program_id,
        request.group,
        request.bank_mint,
        request.seed,
    )?;
    let signing_keypairs = config.get_signers(true);
    let add_bank_ixs: Vec<Instruction> = create_bank_ix_with_seed(
        &config,
        request.group,
        request.bank_mint,
        token_program,
        asset_weight_init,
        asset_weight_maint,
        liability_weight_init,
        liability_weight_maint,
        deposit_limit,
        borrow_limit,
        interest_rate_config,
        request.risk_tier,
        request.oracle_max_age,
        request.oracle_max_confidence,
        request.asset_tag,
        global_fee_wallet,
        request.oracle,
        request.oracle_type,
        bank_seed,
    )?;

    let sig = send_tx(&config, add_bank_ixs, &signing_keypairs)?;
    println!("bank created (sig: {})", sig);

    Ok(())
}

fn resolve_bank_seed(
    rpc_client: &RpcClient,
    program_id: &Pubkey,
    group: Pubkey,
    bank_mint: Pubkey,
    requested_seed: Option<u64>,
) -> Result<u64> {
    use solana_sdk::commitment_config::CommitmentConfig;

    if let Some(seed) = requested_seed {
        let (bank_pda, _) = Pubkey::find_program_address(
            &[group.as_ref(), bank_mint.as_ref(), &seed.to_le_bytes()],
            program_id,
        );
        if rpc_client
            .get_account_with_commitment(&bank_pda, CommitmentConfig::default())?
            .value
            .is_some()
        {
            bail!(
                "seed {} already in use for mint {} in group {} (bank {})",
                seed,
                bank_mint,
                group,
                bank_pda
            );
        }
        return Ok(seed);
    }

    for seed in 0..u64::MAX {
        let (bank_pda, _) = Pubkey::find_program_address(
            &[group.as_ref(), bank_mint.as_ref(), &seed.to_le_bytes()],
            program_id,
        );
        if rpc_client
            .get_account_with_commitment(&bank_pda, CommitmentConfig::default())?
            .value
            .is_none()
        {
            return Ok(seed);
        }
    }

    bail!(
        "unable to find a free seed for mint {} in group {}",
        bank_mint,
        group
    )
}

fn derive_staked_bank_dependencies(stake_pool: &Pubkey) -> (Pubkey, Pubkey) {
    let (bank_mint, _) =
        Pubkey::find_program_address(&[b"mint", stake_pool.as_ref()], &SPL_SINGLE_POOL_ID);
    let (sol_pool, _) =
        Pubkey::find_program_address(&[b"stake", stake_pool.as_ref()], &SPL_SINGLE_POOL_ID);
    (bank_mint, sol_pool)
}

fn find_next_staked_bank_seed(
    rpc_client: &RpcClient,
    program_id: &Pubkey,
    group: Pubkey,
    bank_mint: Pubkey,
    requested_seed: Option<u64>,
) -> Result<(Pubkey, u64)> {
    use solana_sdk::commitment_config::CommitmentConfig;

    if let Some(seed) = requested_seed {
        let (bank_pda, _) = Pubkey::find_program_address(
            &[group.as_ref(), bank_mint.as_ref(), &seed.to_le_bytes()],
            program_id,
        );
        if rpc_client
            .get_account_with_commitment(&bank_pda, CommitmentConfig::default())?
            .value
            .is_some()
        {
            bail!(
                "seed {} already in use for mint {} in group {} (bank {})",
                seed,
                bank_mint,
                group,
                bank_pda
            );
        }
        return Ok((bank_pda, seed));
    }

    for seed in 0..u64::MAX {
        let (bank_pda, _) = Pubkey::find_program_address(
            &[group.as_ref(), bank_mint.as_ref(), &seed.to_le_bytes()],
            program_id,
        );
        if rpc_client
            .get_account_with_commitment(&bank_pda, CommitmentConfig::default())?
            .value
            .is_none()
        {
            return Ok((bank_pda, seed));
        }
    }

    bail!("unable to find a free seed for staked bank")
}

fn load_staked_settings_oracle(config: &Config, group: Pubkey) -> Result<(Pubkey, Pubkey)> {
    let staked_settings = Pubkey::find_program_address(
        &[STAKED_SETTINGS_SEED.as_bytes(), group.as_ref()],
        &config.program_id,
    )
    .0;
    let account = config
        .mfi_program
        .rpc()
        .get_account(&staked_settings)
        .with_context(|| format!("failed to load staked settings for group {}", group))?;

    let expected_len = 8 + 256;
    if account.data.len() < expected_len {
        bail!(
            "staked settings account {} too short: got {} bytes, expected at least {}",
            staked_settings,
            account.data.len(),
            expected_len
        );
    }

    let oracle = Pubkey::try_from(&account.data[72..104])
        .map_err(|_| anyhow::anyhow!("failed to decode staked settings oracle"))?;

    Ok((staked_settings, oracle))
}

pub fn create_staked_bank(config: Config, request: StakedBankCreateRequest) -> Result<()> {
    let rpc_client = config.mfi_program.rpc();
    let (bank_mint, sol_pool) = derive_staked_bank_dependencies(&request.stake_pool);
    let sol_pool_account = rpc_client
        .get_account(&sol_pool)
        .with_context(|| format!("failed to load derived SOL pool {}", sol_pool))?;
    if sol_pool_account.owner != solana_sdk::stake::program::id() {
        bail!(
            "derived SOL pool {} has unexpected owner {}",
            sol_pool,
            sol_pool_account.owner
        );
    }
    let sol_pool_state = StakeStateV2::try_from_slice(&sol_pool_account.data)
        .with_context(|| format!("failed to decode stake state for SOL pool {}", sol_pool))?;
    let validator_vote_account = sol_pool_state
        .delegation()
        .map(|delegation| delegation.voter_pubkey)
        .context("derived SOL pool is not in delegated stake state")?;
    let token_program = rpc_client
        .get_account(&bank_mint)
        .with_context(|| format!("failed to load derived LST mint {}", bank_mint))?
        .owner;
    let (staked_settings, oracle) = load_staked_settings_oracle(&config, request.group)?;
    let (bank_pda, bank_seed) = find_next_staked_bank_seed(
        &rpc_client,
        &config.program_id,
        request.group,
        bank_mint,
        request.seed,
    )?;

    let mut add_bank_ixs = config
        .mfi_program
        .request()
        .accounts(marginfi::accounts::LendingPoolAddBankPermissionless {
            marginfi_group: request.group,
            staked_settings,
            fee_payer: config.explicit_fee_payer(),
            bank_mint,
            sol_pool,
            stake_pool: request.stake_pool,
            validator_vote_account,
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
        .args(marginfi::instruction::LendingPoolAddBankPermissionless { bank_seed })
        .instructions()?;

    let ix = add_bank_ixs
        .first_mut()
        .context("failed to build staked bank instruction")?;
    ix.accounts.extend([
        AccountMeta::new_readonly(oracle, false),
        AccountMeta::new_readonly(bank_mint, false),
        AccountMeta::new_readonly(sol_pool, false),
    ]);

    let signing_keypairs = config.get_signers(true);
    let sig = send_tx(&config, add_bank_ixs, &signing_keypairs)?;
    println!("staked bank created (sig: {})", sig);
    println!("Bank address (PDA): {}", bank_pda);
    println!("Derived LST mint: {}", bank_mint);
    println!("Derived SOL pool: {}", sol_pool);
    println!("Derived validator vote account: {}", validator_vote_account);

    Ok(())
}

pub fn group_clone_bank(
    config: Config,
    group: Pubkey,
    source_bank: Pubkey,
    bank_mint: Pubkey,
    bank_seed: u64,
) -> Result<()> {
    let mint_account = config.mfi_program.rpc().get_account(&bank_mint)?;
    let token_program = mint_account.owner;

    let (bank_pda, _) = Pubkey::find_program_address(
        &[group.as_ref(), bank_mint.as_ref(), &bank_seed.to_le_bytes()],
        &config.program_id,
    );

    let clone_bank_ixs = config
        .mfi_program
        .request()
        .accounts(marginfi::accounts::LendingPoolCloneBank {
            marginfi_group: group,
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
    group: Pubkey,
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
    oracle_max_confidence: u32,
    asset_tag: u8,
    global_fee_wallet: Pubkey,
    oracle: Pubkey,
    oracle_type: u8,
    bank_seed: u64,
) -> Result<Vec<Instruction>> {
    let (bank_pda, _) = Pubkey::find_program_address(
        [group.as_ref(), bank_mint.as_ref(), &bank_seed.to_le_bytes()].as_slice(),
        &config.program_id,
    );

    let add_bank_ixs_builder = config.mfi_program.request();
    let mut add_bank_ixs = add_bank_ixs_builder
        .accounts(marginfi::accounts::LendingPoolAddBankWithSeed {
            marginfi_group: group,
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
            fee_payer: config.explicit_fee_payer(),
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
                oracle_max_confidence,
                asset_tag,
                ..BankConfigCompact::default()
            },
            bank_seed,
        })
        .instructions()?;

    // Chain oracle configuration in the same transaction
    let oracle_meta = AccountMeta::new_readonly(oracle, false);
    let configure_oracle_ixs = config
        .mfi_program
        .request()
        .accounts(marginfi::accounts::LendingPoolConfigureBankOracle {
            group,
            admin: config.authority(),
            bank: bank_pda,
        })
        .args(marginfi::instruction::LendingPoolConfigureBankOracle {
            setup: oracle_type,
            oracle,
        })
        .instructions()?;
    let mut oracle_ix = configure_oracle_ixs.into_iter().next().unwrap();
    oracle_ix.accounts.push(oracle_meta);
    add_bank_ixs.push(oracle_ix);

    println!("Bank address (PDA): {}", bank_pda);

    Ok(add_bank_ixs)
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
