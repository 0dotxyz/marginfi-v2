use marginfi_type_crate::ix_builders;
use {
    crate::{
        config::Config,
        profile::Profile,
        utils::{find_fee_state_pda, send_tx},
    },
    anyhow::{anyhow, Result},
    marginfi_type_crate::constants::STAKED_SETTINGS_SEED,
    solana_sdk::pubkey::Pubkey,
};

pub fn panic_unpause_permissionless(config: Config) -> Result<()> {
    let fee_state = find_fee_state_pda(&config.program_id).0;

    let ix = ix_builders::with_program_id(
        ix_builders::admin::panic_unpause_permissionless(
            &ix_builders::admin::PanicUnpausePermissionless { fee_state },
        ),
        config.program_id,
    );

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

    let ix = ix_builders::with_program_id(
        ix_builders::admin::propagate_staked_settings(
            &ix_builders::admin::PropagateStakedSettings {
                marginfi_group,
                staked_settings,
                bank: bank_pk,
            },
        ),
        config.program_id,
    );

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(&config, vec![ix], &signing_keypairs)?;
    println!("Staked settings propagated (sig: {})", sig);

    Ok(())
}

pub fn propagate_fee(config: Config, marginfi_group: Pubkey) -> Result<()> {
    let fee_state_pubkey = find_fee_state_pda(&config.program_id).0;

    let ix = ix_builders::with_program_id(
        ix_builders::admin::propagate_fee_state(&ix_builders::admin::PropagateFeeState {
            fee_state: fee_state_pubkey,
            marginfi_group,
        }),
        config.program_id,
    );

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(&config, vec![ix], &signing_keypairs)?;
    println!("Fee propagated (sig: {})", sig);

    Ok(())
}
