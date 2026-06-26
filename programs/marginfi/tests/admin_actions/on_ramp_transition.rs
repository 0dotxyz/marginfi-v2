use fixtures::test::{TestFixture, TestSettings};
use marginfi_type_crate::{
    constants::{STAKED_ORACLE_DISABLED, STAKED_ORACLE_PRICE_USES_ONRAMP},
    types::OnRampTransition,
};
use solana_program_test::tokio;

#[tokio::test]
async fn on_ramp_transition_flags_test() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let settings_pre_transition = test_f.marginfi_group.load_staked_settings().await;
    assert_eq!(settings_pre_transition.flags, 0);
    assert_eq!(
        settings_pre_transition.on_ramp_transition(),
        OnRampTransition::PreTransition
    );

    test_f.marginfi_group.try_disable_staked_oracles().await?;

    let stake_disabled_settings = test_f.marginfi_group.load_staked_settings().await;
    assert_eq!(stake_disabled_settings.flags, STAKED_ORACLE_DISABLED);
    assert_eq!(
        stake_disabled_settings.on_ramp_transition(),
        OnRampTransition::StakeOraclesDisabled
    );

    test_f
        .marginfi_group
        .try_enable_staked_oracle_onramp()
        .await?;

    let onramp_enabled_settings = test_f.marginfi_group.load_staked_settings().await;
    assert_eq!(
        onramp_enabled_settings.flags,
        STAKED_ORACLE_PRICE_USES_ONRAMP
    );
    assert_eq!(
        onramp_enabled_settings.on_ramp_transition(),
        OnRampTransition::OnRampEnabled
    );

    Ok(())
}
