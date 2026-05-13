use {
    crate::{
        config::Config,
        profile::Profile,
        utils::{find_fee_state_pda, send_tx},
    },
    anchor_client::anchor_lang::{InstructionData, ToAccountMetas},
    anyhow::{anyhow, Result},
    marginfi_type_crate::constants::STAKED_SETTINGS_SEED,
    solana_sdk::{instruction::Instruction, pubkey::Pubkey},
};

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
