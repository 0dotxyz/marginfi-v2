use anchor_lang::{InstructionData, ToAccountMetas};
use fixtures::{assert_custom_error, prelude::*};
use marginfi::prelude::MarginfiError;
use marginfi_type_crate::types::{FeeState, MarginfiGroup, PanicState};
use pretty_assertions::assert_eq;
use solana_program_test::*;
use solana_sdk::{
    instruction::Instruction, pubkey::Pubkey, signature::Keypair, signer::Signer,
    transaction::Transaction,
};

#[tokio::test]
async fn test_panic_pause_and_unpause_instructions() -> anyhow::Result<()> {
    let test_f = TestFixture::new(None).await;
    let marginfi_group = &test_f.marginfi_group;

    let fee_state: FeeState = test_f.load_and_deserialize(&marginfi_group.fee_state).await;
    assert!(!fee_state.panic_state.is_paused_flag());
    assert_eq!(fee_state.panic_state.daily_pause_count, 0);
    assert_eq!(fee_state.panic_state.consecutive_pause_count, 0);
    assert_eq!(fee_state.pause_delegate_admin, Pubkey::default());

    marginfi_group.try_panic_pause().await?;

    let fee_state: FeeState = test_f.load_and_deserialize(&marginfi_group.fee_state).await;
    assert!(fee_state.panic_state.is_paused_flag());
    assert_eq!(fee_state.panic_state.daily_pause_count, 1);
    assert_eq!(fee_state.panic_state.consecutive_pause_count, 1);
    assert_eq!(fee_state.panic_state.pause_start_timestamp, 0);

    marginfi_group.try_panic_unpause().await?;

    let fee_state: FeeState = test_f.load_and_deserialize(&marginfi_group.fee_state).await;
    assert!(!fee_state.panic_state.is_paused_flag());
    assert_eq!(fee_state.panic_state.consecutive_pause_count, 0);
    assert_eq!(fee_state.panic_state.pause_start_timestamp, 0);
    assert_eq!(fee_state.panic_state.daily_pause_count, 1);

    Ok(())
}

#[tokio::test]
async fn test_pause_delegate_admin_can_pause_and_unpause() -> anyhow::Result<()> {
    let test_f = TestFixture::new(None).await;
    let marginfi_group = &test_f.marginfi_group;
    let pause_delegate_admin = Keypair::new();
    create_system_account_if_missing(test_f.context.clone(), pause_delegate_admin.pubkey()).await;

    marginfi_group
        .try_set_pause_delegate_admin(Some(pause_delegate_admin.pubkey()))
        .await?;

    let fee_state: FeeState = test_f.load_and_deserialize(&marginfi_group.fee_state).await;
    assert_eq!(
        fee_state.pause_delegate_admin,
        pause_delegate_admin.pubkey()
    );

    marginfi_group
        .try_panic_pause_with_authority(&pause_delegate_admin)
        .await?;

    let fee_state: FeeState = test_f.load_and_deserialize(&marginfi_group.fee_state).await;
    assert!(fee_state.panic_state.is_paused_flag());

    marginfi_group
        .try_panic_unpause_with_authority(&pause_delegate_admin)
        .await?;

    let fee_state: FeeState = test_f.load_and_deserialize(&marginfi_group.fee_state).await;
    assert!(!fee_state.panic_state.is_paused_flag());

    Ok(())
}

#[tokio::test]
async fn test_pause_delegate_admin_revoke_and_unauthorized_signer() -> anyhow::Result<()> {
    let test_f = TestFixture::new(None).await;
    let marginfi_group = &test_f.marginfi_group;
    let pause_delegate_admin = Keypair::new();
    let unauthorized = Keypair::new();
    create_system_account_if_missing(test_f.context.clone(), pause_delegate_admin.pubkey()).await;
    create_system_account_if_missing(test_f.context.clone(), unauthorized.pubkey()).await;

    let res = marginfi_group
        .try_panic_pause_with_authority(&unauthorized)
        .await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::Unauthorized);

    marginfi_group
        .try_set_pause_delegate_admin(Some(pause_delegate_admin.pubkey()))
        .await?;
    marginfi_group.try_set_pause_delegate_admin(None).await?;

    let fee_state: FeeState = test_f.load_and_deserialize(&marginfi_group.fee_state).await;
    assert_eq!(fee_state.pause_delegate_admin, Pubkey::default());

    let res = marginfi_group
        .try_panic_pause_with_authority(&pause_delegate_admin)
        .await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::Unauthorized);

    Ok(())
}

#[tokio::test]
async fn test_pause_delegate_admin_cannot_edit_fee_state() -> anyhow::Result<()> {
    let test_f = TestFixture::new(None).await;
    let marginfi_group = &test_f.marginfi_group;
    let pause_delegate_admin = Keypair::new();
    create_system_account_if_missing(test_f.context.clone(), pause_delegate_admin.pubkey()).await;

    marginfi_group
        .try_set_pause_delegate_admin(Some(pause_delegate_admin.pubkey()))
        .await?;

    let fee_state: FeeState = test_f.load_and_deserialize(&marginfi_group.fee_state).await;
    let ix = Instruction {
        program_id: marginfi::ID,
        accounts: marginfi::accounts::EditFeeState {
            global_fee_admin: pause_delegate_admin.pubkey(),
            fee_state: marginfi_group.fee_state,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::EditGlobalFeeState {
            admin: fee_state.global_fee_admin,
            fee_wallet: fee_state.global_fee_wallet,
            bank_init_flat_sol_fee: fee_state.bank_init_flat_sol_fee,
            liquidation_flat_sol_fee: fee_state.liquidation_flat_sol_fee,
            order_init_flat_sol_fee: fee_state.order_init_flat_sol_fee,
            program_fee_fixed: fee_state.program_fee_fixed,
            program_fee_rate: fee_state.program_fee_rate,
            liquidation_max_fee: fee_state.liquidation_max_fee,
            order_execution_max_fee: fee_state.order_execution_max_fee,
        }
        .data(),
    };

    let payer = test_f.payer_keypair();
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &pause_delegate_admin],
        latest_blockhash(&test_f.context).await,
    );

    let res = test_f
        .context
        .borrow_mut()
        .banks_client
        .process_transaction(tx)
        .await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::Unauthorized);

    Ok(())
}

#[tokio::test]
async fn test_panic_daily_limits_with_instructions() -> anyhow::Result<()> {
    let test_f = TestFixture::new(None).await;
    let marginfi_group = &test_f.marginfi_group;

    marginfi_group.try_panic_pause().await?;

    let fee_state: FeeState = test_f.load_and_deserialize(&marginfi_group.fee_state).await;
    assert_eq!(fee_state.panic_state.daily_pause_count, 1);
    assert_eq!(fee_state.panic_state.consecutive_pause_count, 1);
    assert!(fee_state.panic_state.is_paused_flag());

    marginfi_group.try_panic_unpause().await?;

    let fee_state: FeeState = test_f.load_and_deserialize(&marginfi_group.fee_state).await;
    assert_eq!(fee_state.panic_state.daily_pause_count, 1);
    assert_eq!(fee_state.panic_state.consecutive_pause_count, 0);
    assert!(!fee_state.panic_state.is_paused_flag());

    Ok(())
}

#[tokio::test]
async fn test_panic_state_cache_with_instructions() -> anyhow::Result<()> {
    let test_f = TestFixture::new(None).await;
    let marginfi_group = &test_f.marginfi_group;

    let marginfi_group_state: MarginfiGroup =
        test_f.load_and_deserialize(&marginfi_group.key).await;
    assert!(!marginfi_group_state.panic_state_cache.is_paused_flag());

    marginfi_group.try_panic_pause().await?;

    let fee_state: FeeState = test_f.load_and_deserialize(&marginfi_group.fee_state).await;
    assert!(fee_state.panic_state.is_paused_flag());

    let current_time = fee_state.panic_state.pause_start_timestamp;
    assert!(!fee_state
        .panic_state
        .is_expired(current_time + PanicState::PAUSE_DURATION_SECONDS - 1));
    assert!(fee_state
        .panic_state
        .is_expired(current_time + PanicState::PAUSE_DURATION_SECONDS));

    Ok(())
}

#[tokio::test]
async fn test_protocol_pause_check_with_instructions() -> anyhow::Result<()> {
    let test_f = TestFixture::new(None).await;
    let marginfi_group = &test_f.marginfi_group;

    let group_state: MarginfiGroup = test_f.load_and_deserialize(&marginfi_group.key).await;
    assert!(!group_state.panic_state_cache.is_paused_flag());

    marginfi_group.try_panic_pause().await?;

    // Get the fee state and test expiration logic
    let fee_state: FeeState = test_f.load_and_deserialize(&marginfi_group.fee_state).await;
    assert!(fee_state.panic_state.is_paused_flag());

    let current_time = fee_state.panic_state.pause_start_timestamp;
    let expired_time = current_time + PanicState::PAUSE_DURATION_SECONDS + 1;
    assert!(fee_state.panic_state.is_expired(expired_time));

    Ok(())
}
