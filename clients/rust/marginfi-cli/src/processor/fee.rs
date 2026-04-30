use {
    crate::{
        config::Config,
        profile::Profile,
        utils::{find_fee_state_pda, send_tx},
    },
    anchor_client::anchor_lang::{InstructionData, ToAccountMetas},
    anyhow::{anyhow, Result},
    fixed::types::I80F48,
    marginfi_type_crate::{
        constants::STAKED_SETTINGS_SEED,
        types::{RiskTier, WrappedI80F48},
    },
    solana_sdk::{instruction::Instruction, pubkey::Pubkey, system_program},
};

pub fn initialize_fee_state(
    config: Config,
    admin: Pubkey,
    fee_wallet: Pubkey,
    bank_init_flat_sol_fee: u32,
    liquidation_flat_sol_fee: u32,
    program_fee_fixed: f64,
    program_fee_rate: f64,
    liquidation_max_fee: f64,
    order_init_flat_sol_fee: u32,
    order_execution_max_fee: f64,
) -> Result<()> {
    let program_fee_fixed: WrappedI80F48 = I80F48::from_num(program_fee_fixed).into();
    let program_fee_rate: WrappedI80F48 = I80F48::from_num(program_fee_rate).into();
    let liquidation_max_fee: WrappedI80F48 = I80F48::from_num(liquidation_max_fee).into();
    let order_execution_max_fee: WrappedI80F48 = I80F48::from_num(order_execution_max_fee).into();

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
            order_init_flat_sol_fee,
            order_execution_max_fee,
        })
        .instructions()?;

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(&config, initialize_fee_state_ixs, &signing_keypairs)?;
    println!("Fee state initialized (sig: {})", sig);

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
    order_init_flat_sol_fee: u32,
    order_execution_max_fee: f64,
) -> Result<()> {
    let program_fee_fixed: WrappedI80F48 = I80F48::from_num(program_fee_fixed).into();
    let program_fee_rate: WrappedI80F48 = I80F48::from_num(program_fee_rate).into();
    let liquidation_max_fee: WrappedI80F48 = I80F48::from_num(liquidation_max_fee).into();
    let order_execution_max_fee: WrappedI80F48 = I80F48::from_num(order_execution_max_fee).into();

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
            order_init_flat_sol_fee,
            order_execution_max_fee,
        })
        .instructions()?;

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(&config, edit_fee_state_ixs, &signing_keypairs)?;
    println!("Fee state edited (sig: {})", sig);

    Ok(())
}

pub fn config_group_fee(config: Config, profile: Profile, enable_program_fee: bool) -> Result<()> {
    let marginfi_group_pubkey = profile.marginfi_group.ok_or_else(|| {
        anyhow!(
            "Marginfi group does not exist for profile [{}]",
            profile.name
        )
    })?;

    let fee_state_pubkey = find_fee_state_pda(&config.program_id).0;

    let config_group_fee_ixs_builder = config.mfi_program.request();

    let config_group_fee_ixs = config_group_fee_ixs_builder
        .accounts(marginfi::accounts::ConfigGroupFee {
            marginfi_group: marginfi_group_pubkey,
            global_fee_admin: config.authority(),
            fee_state: fee_state_pubkey,
        })
        .args(marginfi::instruction::ConfigGroupFee { enable_program_fee })
        .instructions()?;

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(&config, config_group_fee_ixs, &signing_keypairs)?;
    println!("Config group fee updated (sig: {})", sig);

    Ok(())
}

pub fn set_pause_delegate_admin(
    config: Config,
    pause_delegate_admin: Option<Pubkey>,
) -> Result<()> {
    let fee_state_pubkey = find_fee_state_pda(&config.program_id).0;

    let set_pause_delegate_admin_ixs = config
        .mfi_program
        .request()
        .accounts(marginfi::accounts::SetPauseDelegateAdmin {
            global_fee_admin: config.authority(),
            fee_state: fee_state_pubkey,
        })
        .args(marginfi::instruction::SetPauseDelegateAdmin {
            new_pause_delegate_admin: pause_delegate_admin,
        })
        .instructions()?;

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(&config, set_pause_delegate_admin_ixs, &signing_keypairs)?;
    println!("Pause delegate admin updated (sig: {})", sig);

    Ok(())
}

pub fn panic_pause(config: Config) -> Result<()> {
    let fee_state = find_fee_state_pda(&config.program_id).0;

    let ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::PanicPause {
            pause_authority: config.authority(),
            fee_state,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::PanicPause {}.data(),
    };

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(&config, vec![ix], &signing_keypairs)?;
    println!("Protocol paused (sig: {})", sig);

    Ok(())
}

pub fn panic_unpause(config: Config) -> Result<()> {
    let fee_state = find_fee_state_pda(&config.program_id).0;

    let ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::PanicUnpause {
            pause_authority: config.authority(),
            fee_state,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::PanicUnpause {}.data(),
    };

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(&config, vec![ix], &signing_keypairs)?;
    println!("Protocol unpaused (sig: {})", sig);

    Ok(())
}

pub fn panic_unpause_permissionless(config: Config) -> Result<()> {
    let fee_state = find_fee_state_pda(&config.program_id).0;

    let ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::PanicUnpausePermissionless { fee_state }
            .to_account_metas(Some(true)),
        data: marginfi::instruction::PanicUnpausePermissionless {}.data(),
    };

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(&config, vec![ix], &signing_keypairs)?;
    println!("Protocol unpaused permissionlessly (sig: {})", sig);

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn init_staked_settings(
    config: Config,
    profile: Profile,
    oracle: Pubkey,
    asset_weight_init: f64,
    asset_weight_maint: f64,
    deposit_limit: u64,
    total_asset_value_init_limit: u64,
    oracle_max_age: u16,
    risk_tier: RiskTier,
) -> Result<()> {
    let marginfi_group = profile
        .marginfi_group
        .ok_or_else(|| anyhow!("Marginfi group not specified in profile [{}]", profile.name))?;

    let (staked_settings, _bump) = Pubkey::find_program_address(
        &[STAKED_SETTINGS_SEED.as_bytes(), marginfi_group.as_ref()],
        &config.program_id,
    );

    let ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::InitStakedSettings {
            marginfi_group,
            admin: config.authority(),
            fee_payer: config.explicit_fee_payer(),
            staked_settings,
            system_program: system_program::id(),
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::InitStakedSettings {
            settings: marginfi::instructions::StakedSettingsConfig {
                oracle,
                asset_weight_init: I80F48::from_num(asset_weight_init).into(),
                asset_weight_maint: I80F48::from_num(asset_weight_maint).into(),
                deposit_limit,
                total_asset_value_init_limit,
                oracle_max_age,
                risk_tier,
            },
        }
        .data(),
    };

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(&config, vec![ix], &signing_keypairs)?;
    println!("Staked settings initialized (sig: {})", sig);

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn edit_staked_settings(
    config: Config,
    profile: Profile,
    oracle: Option<Pubkey>,
    asset_weight_init: Option<f64>,
    asset_weight_maint: Option<f64>,
    deposit_limit: Option<u64>,
    total_asset_value_init_limit: Option<u64>,
    oracle_max_age: Option<u16>,
    risk_tier: Option<RiskTier>,
) -> Result<()> {
    let marginfi_group = profile
        .marginfi_group
        .ok_or_else(|| anyhow!("Marginfi group not specified in profile [{}]", profile.name))?;

    let (staked_settings, _bump) = Pubkey::find_program_address(
        &[STAKED_SETTINGS_SEED.as_bytes(), marginfi_group.as_ref()],
        &config.program_id,
    );

    let ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::EditStakedSettings {
            marginfi_group,
            admin: config.authority(),
            staked_settings,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::EditStakedSettings {
            settings: marginfi::instructions::StakedSettingsEditConfig {
                oracle,
                asset_weight_init: asset_weight_init.map(|v| I80F48::from_num(v).into()),
                asset_weight_maint: asset_weight_maint.map(|v| I80F48::from_num(v).into()),
                deposit_limit,
                total_asset_value_init_limit,
                oracle_max_age,
                risk_tier,
            },
        }
        .data(),
    };

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(&config, vec![ix], &signing_keypairs)?;
    println!("Staked settings edited (sig: {})", sig);

    Ok(())
}

pub fn propagate_staked_settings(config: Config, profile: Profile, bank_pk: Pubkey) -> Result<()> {
    let marginfi_group = profile
        .marginfi_group
        .ok_or_else(|| anyhow!("Marginfi group not specified in profile [{}]", profile.name))?;

    let (staked_settings, _bump) = Pubkey::find_program_address(
        &[STAKED_SETTINGS_SEED.as_bytes(), marginfi_group.as_ref()],
        &config.program_id,
    );

    let ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::PropagateStakedSettings {
            marginfi_group,
            staked_settings,
            bank: bank_pk,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::PropagateStakedSettings {}.data(),
    };

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(&config, vec![ix], &signing_keypairs)?;
    println!("Staked settings propagated (sig: {})", sig);

    Ok(())
}

pub fn configure_group_rate_limits(
    config: Config,
    profile: Profile,
    hourly_max_outflow_usd: Option<u64>,
    daily_max_outflow_usd: Option<u64>,
) -> Result<()> {
    let marginfi_group = profile
        .marginfi_group
        .ok_or_else(|| anyhow!("Marginfi group not specified in profile [{}]", profile.name))?;

    let ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::ConfigureGroupRateLimits {
            marginfi_group,
            admin: config.authority(),
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::ConfigureGroupRateLimits {
            hourly_max_outflow_usd,
            daily_max_outflow_usd,
        }
        .data(),
    };

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(&config, vec![ix], &signing_keypairs)?;
    println!("Group rate limits configured (sig: {})", sig);

    Ok(())
}

pub fn configure_deleverage_withdrawal_limit(
    config: Config,
    profile: Profile,
    daily_limit: u32,
) -> Result<()> {
    let marginfi_group = profile
        .marginfi_group
        .ok_or_else(|| anyhow!("Marginfi group not specified in profile [{}]", profile.name))?;

    let ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::ConfigureDeleverageWithdrawalLimit {
            marginfi_group,
            admin: config.authority(),
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::ConfigureDeleverageWithdrawalLimit { limit: daily_limit }
            .data(),
    };

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(&config, vec![ix], &signing_keypairs)?;
    println!("Deleverage withdrawal limit configured (sig: {})", sig);

    Ok(())
}

/// Note: doing this one group at a time is tedious, consider running the script instead.
pub fn propagate_fee(config: Config, marginfi_group: Pubkey) -> Result<()> {
    let fee_state_pubkey = find_fee_state_pda(&config.program_id).0;

    let propagate_fee_ixs_builder = config.mfi_program.request();

    let propagate_fee_ixs = propagate_fee_ixs_builder
        .accounts(marginfi::accounts::PropagateFee {
            fee_state: fee_state_pubkey,
            marginfi_group,
        })
        .args(marginfi::instruction::PropagateFeeState {})
        .instructions()?;

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(&config, propagate_fee_ixs, &signing_keypairs)?;
    println!("Fee propagated (sig: {})", sig);

    Ok(())
}
