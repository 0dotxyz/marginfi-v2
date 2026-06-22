use fixtures::test::{TestFixture, TestSettings};
use marginfi_type_crate::types::OnRampTransition;
use solana_program_test::tokio;

#[tokio::test]
async fn on_ramp_transition_flags_test() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let group_pre_transition = test_f.marginfi_group.load().await;
    assert_eq!(group_pre_transition.group_flags, 0);
    assert_eq!(
        group_pre_transition.on_ramp_transition(),
        OnRampTransition::PreTransition
    );

    test_f.marginfi_group.try_disable_staked_oracles().await?;

    let stake_disabled_group = test_f.marginfi_group.load().await;
    assert_eq!(stake_disabled_group.group_flags, 2);
    assert_eq!(
        stake_disabled_group.on_ramp_transition(),
        OnRampTransition::StakeOraclesDisabled
    );

    test_f
        .marginfi_group
        .try_enable_staked_oracle_onramp()
        .await?;

    let onramp_enabled_group = test_f.marginfi_group.load().await;
    assert_eq!(onramp_enabled_group.group_flags, 4);
    assert_eq!(
        onramp_enabled_group.on_ramp_transition(),
        OnRampTransition::OnRampEnabled
    );

    Ok(())
}
