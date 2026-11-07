//! Compares each SDK builder with the program’s Anchor-generated instruction and accounts.
use anchor_lang::{InstructionData, ToAccountMetas};
use bytemuck::Zeroable;
use marginfi_type_crate::types::*;
use solana_sdk::{instruction::Instruction, pubkey::Pubkey};

fn key(n: u32) -> Pubkey {
    let mut b = [0u8; 32];
    b[..4].copy_from_slice(&n.to_le_bytes());
    Pubkey::new_from_array(b)
}
fn wi(n: i64) -> WrappedI80F48 {
    fixed::types::I80F48::from_num(n).into()
}

#[test]
fn all_builders_match_anchor() {
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::AdminCloseAccount {
                group: key(100),
                marginfi_account: key(101),
                global_fee_wallet: key(102),
                rebalance_fee_pool: key(103),
            }
            .to_account_metas(None),
            data: marginfi::instruction::AdminCloseAccount {}.data(),
        };
        let actual = marginfi_type_crate::ix_builders::account::admin_close_account(
            &marginfi_type_crate::ix_builders::account::AdminCloseAccount {
                group: key(100),
                marginfi_account: key(101),
                global_fee_wallet: key(102),
                rebalance_fee_pool: key(103),
            },
        );
        assert_eq!(expected, actual, "admin_close_account optional_none=False");
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::ConfigGroupFee {
                marginfi_group: key(100),
                global_fee_admin: key(101),
                fee_state: key(102),
                instruction_sysvar: key(103),
            }
            .to_account_metas(None),
            data: marginfi::instruction::ConfigGroupFee {
                enable_program_fee: true,
            }
            .data(),
        };
        let actual = marginfi_type_crate::ix_builders::admin::config_group_fee(
            &marginfi_type_crate::ix_builders::admin::ConfigGroupFee {
                marginfi_group: key(100),
                global_fee_admin: key(101),
                fee_state: key(102),
                instruction_sysvar: key(103),
            },
            true,
        );
        assert_eq!(expected, actual, "config_group_fee optional_none=False");
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::ConfigureBankRateLimits {
                group: key(100),
                admin: key(101),
                bank: key(102),
                instruction_sysvar: key(103),
            }
            .to_account_metas(None),
            data: marginfi::instruction::ConfigureBankRateLimits {
                hourly_max_outflow: Some(20u64),
                daily_max_outflow: Some(21u64),
            }
            .data(),
        };
        let actual = marginfi_type_crate::ix_builders::admin::configure_bank_rate_limits(
            &marginfi_type_crate::ix_builders::admin::ConfigureBankRateLimits {
                group: key(100),
                admin: key(101),
                bank: key(102),
                instruction_sysvar: key(103),
            },
            Some(20u64),
            Some(21u64),
        );
        assert_eq!(
            expected, actual,
            "configure_bank_rate_limits optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::ConfigureDeleverageWithdrawalLimit {
                marginfi_group: key(100),
                admin: key(101),
                instruction_sysvar: key(102),
            }
            .to_account_metas(None),
            data: marginfi::instruction::ConfigureDeleverageWithdrawalLimit { limit: 20u32 }.data(),
        };
        let actual = marginfi_type_crate::ix_builders::admin::configure_deleverage_withdrawal_limit(
            &marginfi_type_crate::ix_builders::admin::ConfigureDeleverageWithdrawalLimit {
                marginfi_group: key(100),
                admin: key(101),
                instruction_sysvar: key(102),
            },
            20u32,
        );
        assert_eq!(
            expected, actual,
            "configure_deleverage_withdrawal_limit optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::ConfigureGroupRateLimits {
                marginfi_group: key(100),
                admin: key(101),
                instruction_sysvar: key(102),
            }
            .to_account_metas(None),
            data: marginfi::instruction::ConfigureGroupRateLimits {
                hourly_max_outflow_usd: Some(20u64),
                daily_max_outflow_usd: Some(21u64),
            }
            .data(),
        };
        let actual = marginfi_type_crate::ix_builders::admin::configure_group_rate_limits(
            &marginfi_type_crate::ix_builders::admin::ConfigureGroupRateLimits {
                marginfi_group: key(100),
                admin: key(101),
                instruction_sysvar: key(102),
            },
            Some(20u64),
            Some(21u64),
        );
        assert_eq!(
            expected, actual,
            "configure_group_rate_limits optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::DisableStakedOracles {
                group: key(100),
                governance_admin: key(101),
                staked_settings: key(102),
                instruction_sysvar: key(103),
            }
            .to_account_metas(None),
            data: marginfi::instruction::DisableStakedOracles {}.data(),
        };
        let actual = marginfi_type_crate::ix_builders::admin::disable_staked_oracles(
            &marginfi_type_crate::ix_builders::admin::DisableStakedOracles {
                group: key(100),
                governance_admin: key(101),
                staked_settings: key(102),
                instruction_sysvar: key(103),
            },
        );
        assert_eq!(
            expected, actual,
            "disable_staked_oracles optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::DriftClaimBadDebt {
                payer: key(100),
                bank: key(101),
                fee_state: key(102),
                liquidity_vault_authority: key(103),
                integration_acc_2: key(104),
                integration_acc_3: key(105),
                distributor: key(106),
                claim_status: key(107),
                from: key(108),
                claim_mint: key(109),
                global_fee_wallet: key(110),
                claimant_token_account: key(111),
                destination_token_account: key(112),
                merkle_distributor_program: key(113),
                associated_token_program: key(114),
                token_program: key(115),
                system_program: key(116),
            }
            .to_account_metas(None),
            data: marginfi::instruction::DriftClaimBadDebt {
                amount: 20u64,
                proof: vec![[21u8; 32]],
            }
            .data(),
        };
        let actual = marginfi_type_crate::ix_builders::drift::drift_claim_bad_debt(
            &marginfi_type_crate::ix_builders::drift::DriftClaimBadDebt {
                payer: key(100),
                bank: key(101),
                fee_state: key(102),
                liquidity_vault_authority: key(103),
                integration_acc_2: key(104),
                integration_acc_3: key(105),
                distributor: key(106),
                claim_status: key(107),
                from: key(108),
                claim_mint: key(109),
                global_fee_wallet: key(110),
                claimant_token_account: key(111),
                destination_token_account: key(112),
                merkle_distributor_program: key(113),
                associated_token_program: key(114),
                token_program: key(115),
                system_program: key(116),
            },
            20u64,
            vec![[21u8; 32]],
        );
        assert_eq!(expected, actual, "drift_claim_bad_debt optional_none=False");
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::DriftDeposit {
                group: key(100),
                marginfi_account: key(101),
                authority: key(102),
                bank: key(103),
                drift_oracle: Some(key(104)),
                liquidity_vault_authority: key(105),
                liquidity_vault: key(106),
                signer_token_account: key(107),
                drift_state: key(108),
                integration_acc_2: key(109),
                integration_acc_3: key(110),
                integration_acc_1: key(111),
                drift_spot_market_vault: key(112),
                mint: key(113),
                drift_program: key(114),
                token_program: key(115),
                system_program: key(116),
            }
            .to_account_metas(None),
            data: marginfi::instruction::DriftDeposit { amount: 20u64 }.data(),
        };
        let actual = marginfi_type_crate::ix_builders::drift::drift_deposit(
            &marginfi_type_crate::ix_builders::drift::DriftDeposit {
                group: key(100),
                marginfi_account: key(101),
                authority: key(102),
                bank: key(103),
                drift_oracle: Some(key(104)),
                liquidity_vault_authority: key(105),
                liquidity_vault: key(106),
                signer_token_account: key(107),
                drift_state: key(108),
                integration_acc_2: key(109),
                integration_acc_3: key(110),
                integration_acc_1: key(111),
                drift_spot_market_vault: key(112),
                mint: key(113),
                drift_program: key(114),
                token_program: key(115),
                system_program: key(116),
            },
            20u64,
        );
        assert_eq!(expected, actual, "drift_deposit optional_none=False");
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::DriftDeposit {
                group: key(100),
                marginfi_account: key(101),
                authority: key(102),
                bank: key(103),
                drift_oracle: None,
                liquidity_vault_authority: key(105),
                liquidity_vault: key(106),
                signer_token_account: key(107),
                drift_state: key(108),
                integration_acc_2: key(109),
                integration_acc_3: key(110),
                integration_acc_1: key(111),
                drift_spot_market_vault: key(112),
                mint: key(113),
                drift_program: key(114),
                token_program: key(115),
                system_program: key(116),
            }
            .to_account_metas(None),
            data: marginfi::instruction::DriftDeposit { amount: 20u64 }.data(),
        };
        let actual = marginfi_type_crate::ix_builders::drift::drift_deposit(
            &marginfi_type_crate::ix_builders::drift::DriftDeposit {
                group: key(100),
                marginfi_account: key(101),
                authority: key(102),
                bank: key(103),
                drift_oracle: None,
                liquidity_vault_authority: key(105),
                liquidity_vault: key(106),
                signer_token_account: key(107),
                drift_state: key(108),
                integration_acc_2: key(109),
                integration_acc_3: key(110),
                integration_acc_1: key(111),
                drift_spot_market_vault: key(112),
                mint: key(113),
                drift_program: key(114),
                token_program: key(115),
                system_program: key(116),
            },
            20u64,
        );
        assert_eq!(expected, actual, "drift_deposit optional_none=True");
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::DriftHarvestReward {
                bank: key(100),
                fee_state: key(101),
                liquidity_vault_authority: key(102),
                intermediary_token_account: key(103),
                destination_token_account: key(104),
                drift_state: key(105),
                integration_acc_2: key(106),
                integration_acc_3: key(107),
                harvest_drift_spot_market: key(108),
                harvest_drift_spot_market_vault: key(109),
                drift_signer: key(110),
                reward_mint: key(111),
                drift_program: key(112),
                token_program: key(113),
            }
            .to_account_metas(None),
            data: marginfi::instruction::DriftHarvestReward {}.data(),
        };
        let actual = marginfi_type_crate::ix_builders::drift::drift_harvest_reward(
            &marginfi_type_crate::ix_builders::drift::DriftHarvestReward {
                bank: key(100),
                fee_state: key(101),
                liquidity_vault_authority: key(102),
                intermediary_token_account: key(103),
                destination_token_account: key(104),
                drift_state: key(105),
                integration_acc_2: key(106),
                integration_acc_3: key(107),
                harvest_drift_spot_market: key(108),
                harvest_drift_spot_market_vault: key(109),
                drift_signer: key(110),
                reward_mint: key(111),
                drift_program: key(112),
                token_program: key(113),
            },
        );
        assert_eq!(expected, actual, "drift_harvest_reward optional_none=False");
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::DriftInitUser {
                fee_payer: key(100),
                signer_token_account: key(101),
                bank: key(102),
                liquidity_vault_authority: key(103),
                liquidity_vault: key(104),
                mint: key(105),
                integration_acc_3: key(106),
                integration_acc_2: key(107),
                drift_state: key(108),
                integration_acc_1: key(109),
                drift_spot_market_vault: key(110),
                drift_oracle: Some(key(111)),
                drift_program: key(112),
                token_program: key(113),
                rent: key(114),
                system_program: key(115),
            }
            .to_account_metas(None),
            data: marginfi::instruction::DriftInitUser { amount: 20u64 }.data(),
        };
        let actual = marginfi_type_crate::ix_builders::drift::drift_init_user(
            &marginfi_type_crate::ix_builders::drift::DriftInitUser {
                fee_payer: key(100),
                signer_token_account: key(101),
                bank: key(102),
                liquidity_vault_authority: key(103),
                liquidity_vault: key(104),
                mint: key(105),
                integration_acc_3: key(106),
                integration_acc_2: key(107),
                drift_state: key(108),
                integration_acc_1: key(109),
                drift_spot_market_vault: key(110),
                drift_oracle: Some(key(111)),
                drift_program: key(112),
                token_program: key(113),
                rent: key(114),
                system_program: key(115),
            },
            20u64,
        );
        assert_eq!(expected, actual, "drift_init_user optional_none=False");
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::DriftInitUser {
                fee_payer: key(100),
                signer_token_account: key(101),
                bank: key(102),
                liquidity_vault_authority: key(103),
                liquidity_vault: key(104),
                mint: key(105),
                integration_acc_3: key(106),
                integration_acc_2: key(107),
                drift_state: key(108),
                integration_acc_1: key(109),
                drift_spot_market_vault: key(110),
                drift_oracle: None,
                drift_program: key(112),
                token_program: key(113),
                rent: key(114),
                system_program: key(115),
            }
            .to_account_metas(None),
            data: marginfi::instruction::DriftInitUser { amount: 20u64 }.data(),
        };
        let actual = marginfi_type_crate::ix_builders::drift::drift_init_user(
            &marginfi_type_crate::ix_builders::drift::DriftInitUser {
                fee_payer: key(100),
                signer_token_account: key(101),
                bank: key(102),
                liquidity_vault_authority: key(103),
                liquidity_vault: key(104),
                mint: key(105),
                integration_acc_3: key(106),
                integration_acc_2: key(107),
                drift_state: key(108),
                integration_acc_1: key(109),
                drift_spot_market_vault: key(110),
                drift_oracle: None,
                drift_program: key(112),
                token_program: key(113),
                rent: key(114),
                system_program: key(115),
            },
            20u64,
        );
        assert_eq!(expected, actual, "drift_init_user optional_none=True");
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::DriftWithdraw {
                group: key(100),
                marginfi_account: key(101),
                authority: key(102),
                bank: key(103),
                drift_oracle: Some(key(104)),
                liquidity_vault_authority: key(105),
                liquidity_vault: key(106),
                destination_token_account: key(107),
                drift_state: key(108),
                integration_acc_2: key(109),
                integration_acc_3: key(110),
                integration_acc_1: key(111),
                drift_spot_market_vault: key(112),
                drift_reward_oracle: Some(key(113)),
                drift_reward_spot_market: Some(key(114)),
                drift_reward_mint: Some(key(115)),
                drift_reward_oracle_2: Some(key(116)),
                drift_reward_spot_market_2: Some(key(117)),
                drift_reward_mint_2: Some(key(118)),
                drift_signer: key(119),
                mint: key(120),
                drift_program: key(121),
                token_program: key(122),
                system_program: key(123),
            }
            .to_account_metas(None),
            data: marginfi::instruction::DriftWithdraw {
                amount: 20u64,
                withdraw_all: Some(true),
            }
            .data(),
        };
        let actual = marginfi_type_crate::ix_builders::drift::drift_withdraw(
            &marginfi_type_crate::ix_builders::drift::DriftWithdraw {
                group: key(100),
                marginfi_account: key(101),
                authority: key(102),
                bank: key(103),
                drift_oracle: Some(key(104)),
                liquidity_vault_authority: key(105),
                liquidity_vault: key(106),
                destination_token_account: key(107),
                drift_state: key(108),
                integration_acc_2: key(109),
                integration_acc_3: key(110),
                integration_acc_1: key(111),
                drift_spot_market_vault: key(112),
                drift_reward_oracle: Some(key(113)),
                drift_reward_spot_market: Some(key(114)),
                drift_reward_mint: Some(key(115)),
                drift_reward_oracle_2: Some(key(116)),
                drift_reward_spot_market_2: Some(key(117)),
                drift_reward_mint_2: Some(key(118)),
                drift_signer: key(119),
                mint: key(120),
                drift_program: key(121),
                token_program: key(122),
                system_program: key(123),
            },
            20u64,
            Some(true),
        );
        assert_eq!(expected, actual, "drift_withdraw optional_none=False");
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::DriftWithdraw {
                group: key(100),
                marginfi_account: key(101),
                authority: key(102),
                bank: key(103),
                drift_oracle: None,
                liquidity_vault_authority: key(105),
                liquidity_vault: key(106),
                destination_token_account: key(107),
                drift_state: key(108),
                integration_acc_2: key(109),
                integration_acc_3: key(110),
                integration_acc_1: key(111),
                drift_spot_market_vault: key(112),
                drift_reward_oracle: None,
                drift_reward_spot_market: None,
                drift_reward_mint: None,
                drift_reward_oracle_2: None,
                drift_reward_spot_market_2: None,
                drift_reward_mint_2: None,
                drift_signer: key(119),
                mint: key(120),
                drift_program: key(121),
                token_program: key(122),
                system_program: key(123),
            }
            .to_account_metas(None),
            data: marginfi::instruction::DriftWithdraw {
                amount: 20u64,
                withdraw_all: Some(true),
            }
            .data(),
        };
        let actual = marginfi_type_crate::ix_builders::drift::drift_withdraw(
            &marginfi_type_crate::ix_builders::drift::DriftWithdraw {
                group: key(100),
                marginfi_account: key(101),
                authority: key(102),
                bank: key(103),
                drift_oracle: None,
                liquidity_vault_authority: key(105),
                liquidity_vault: key(106),
                destination_token_account: key(107),
                drift_state: key(108),
                integration_acc_2: key(109),
                integration_acc_3: key(110),
                integration_acc_1: key(111),
                drift_spot_market_vault: key(112),
                drift_reward_oracle: None,
                drift_reward_spot_market: None,
                drift_reward_mint: None,
                drift_reward_oracle_2: None,
                drift_reward_spot_market_2: None,
                drift_reward_mint_2: None,
                drift_signer: key(119),
                mint: key(120),
                drift_program: key(121),
                token_program: key(122),
                system_program: key(123),
            },
            20u64,
            Some(true),
        );
        assert_eq!(expected, actual, "drift_withdraw optional_none=True");
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::EditFeeStatePremium {
                global_fee_admin: key(100),
                fee_state: key(101),
                instruction_sysvar: key(102),
            }
            .to_account_metas(None),
            data: marginfi::instruction::EditFeeStatePremium {
                premium_wallet: key(20),
            }
            .data(),
        };
        let actual = marginfi_type_crate::ix_builders::admin::edit_fee_state_premium(
            &marginfi_type_crate::ix_builders::admin::EditFeeStatePremium {
                global_fee_admin: key(100),
                fee_state: key(101),
                instruction_sysvar: key(102),
            },
            key(20),
        );
        assert_eq!(
            expected, actual,
            "edit_fee_state_premium optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::EditFeeState {
                global_fee_admin: key(100),
                fee_state: key(101),
                instruction_sysvar: key(102),
            }
            .to_account_metas(None),
            data: marginfi::instruction::EditGlobalFeeState {
                admin: Some(key(20)),
                fee_wallet: Some(key(21)),
                bank_init_flat_sol_fee: Some(22u32),
                liquidation_flat_sol_fee: Some(23u32),
                order_init_flat_sol_fee: Some(24u32),
                program_fee_fixed: Some(wi(25)),
                program_fee_rate: Some(wi(26)),
                liquidation_max_fee: Some(wi(27)),
                order_execution_max_fee: Some(wi(28)),
                pause_delegate_admin: Some(key(29)),
                account_transfer_fee: Some(30u32),
            }
            .data(),
        };
        let actual = marginfi_type_crate::ix_builders::admin::edit_global_fee_state(
            &marginfi_type_crate::ix_builders::admin::EditGlobalFeeState {
                global_fee_admin: key(100),
                fee_state: key(101),
                instruction_sysvar: key(102),
            },
            Some(key(20)),
            Some(key(21)),
            Some(22u32),
            Some(23u32),
            Some(24u32),
            Some(wi(25)),
            Some(wi(26)),
            Some(wi(27)),
            Some(wi(28)),
            Some(key(29)),
            Some(30u32),
        );
        assert_eq!(
            expected, actual,
            "edit_global_fee_state optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::EditStakedSettings {
                marginfi_group: key(100),
                governance_admin: key(101),
                staked_settings: key(102),
                instruction_sysvar: key(103),
            }
            .to_account_metas(None),
            data: marginfi::instruction::EditStakedSettings {
                settings: StakedSettingsEditConfig::default(),
            }
            .data(),
        };
        let actual = marginfi_type_crate::ix_builders::admin::edit_staked_settings(
            &marginfi_type_crate::ix_builders::admin::EditStakedSettings {
                marginfi_group: key(100),
                governance_admin: key(101),
                staked_settings: key(102),
                instruction_sysvar: key(103),
            },
            StakedSettingsEditConfig::default(),
        );
        assert_eq!(expected, actual, "edit_staked_settings optional_none=False");
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::EnableStakedOracleOnramp {
                group: key(100),
                governance_admin: key(101),
                staked_settings: key(102),
                instruction_sysvar: key(103),
            }
            .to_account_metas(None),
            data: marginfi::instruction::EnableStakedOracleOnramp {}.data(),
        };
        let actual = marginfi_type_crate::ix_builders::admin::enable_staked_oracle_onramp(
            &marginfi_type_crate::ix_builders::admin::EnableStakedOracleOnramp {
                group: key(100),
                governance_admin: key(101),
                staked_settings: key(102),
                instruction_sysvar: key(103),
            },
        );
        assert_eq!(
            expected, actual,
            "enable_staked_oracle_onramp optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::EndDeleverage {
                marginfi_account: key(100),
                liquidation_record: key(101),
                group: key(102),
                risk_admin: key(103),
                instruction_sysvar: key(104),
            }
            .to_account_metas(None),
            data: marginfi::instruction::EndDeleverage {}.data(),
        };
        let actual = marginfi_type_crate::ix_builders::liquidation::end_deleverage(
            &marginfi_type_crate::ix_builders::liquidation::EndDeleverage {
                marginfi_account: key(100),
                liquidation_record: key(101),
                group: key(102),
                risk_admin: key(103),
                instruction_sysvar: key(104),
            },
        );
        assert_eq!(expected, actual, "end_deleverage optional_none=False");
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::EndLiquidation {
                marginfi_account: key(100),
                liquidation_record: key(101),
                group: key(102),
                liquidation_receiver: key(103),
                fee_state: key(104),
                global_fee_wallet: key(105),
                system_program: key(106),
                fee_payer: Some(key(107)),
            }
            .to_account_metas(None),
            data: marginfi::instruction::EndLiquidation {}.data(),
        };
        let actual = marginfi_type_crate::ix_builders::liquidation::end_liquidation(
            &marginfi_type_crate::ix_builders::liquidation::EndLiquidation {
                marginfi_account: key(100),
                liquidation_record: key(101),
                group: key(102),
                liquidation_receiver: key(103),
                fee_state: key(104),
                global_fee_wallet: key(105),
                system_program: key(106),
                fee_payer: Some(key(107)),
            },
        );
        assert_eq!(expected, actual, "end_liquidation optional_none=False");
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::EndLiquidation {
                marginfi_account: key(100),
                liquidation_record: key(101),
                group: key(102),
                liquidation_receiver: key(103),
                fee_state: key(104),
                global_fee_wallet: key(105),
                system_program: key(106),
                fee_payer: None,
            }
            .to_account_metas(None),
            data: marginfi::instruction::EndLiquidation {}.data(),
        };
        let actual = marginfi_type_crate::ix_builders::liquidation::end_liquidation(
            &marginfi_type_crate::ix_builders::liquidation::EndLiquidation {
                marginfi_account: key(100),
                liquidation_record: key(101),
                group: key(102),
                liquidation_receiver: key(103),
                fee_state: key(104),
                global_fee_wallet: key(105),
                system_program: key(106),
                fee_payer: None,
            },
        );
        assert_eq!(expected, actual, "end_liquidation optional_none=True");
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::InitBankMetadata {
                bank: key(100),
                fee_payer: key(101),
                metadata: key(102),
                system_program: key(103),
            }
            .to_account_metas(None),
            data: marginfi::instruction::InitBankMetadata {}.data(),
        };
        let actual = marginfi_type_crate::ix_builders::pool::init_bank_metadata(
            &marginfi_type_crate::ix_builders::pool::InitBankMetadata {
                bank: key(100),
                fee_payer: key(101),
                metadata: key(102),
                system_program: key(103),
            },
        );
        assert_eq!(expected, actual, "init_bank_metadata optional_none=False");
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::InitFeeState {
                payer: key(100),
                fee_state: key(101),
                system_program: key(102),
            }
            .to_account_metas(None),
            data: marginfi::instruction::InitGlobalFeeState {
                admin: key(20),
                fee_wallet: key(21),
                bank_init_flat_sol_fee: 22u32,
                liquidation_flat_sol_fee: 23u32,
                order_init_flat_sol_fee: 24u32,
                program_fee_fixed: wi(25),
                program_fee_rate: wi(26),
                liquidation_max_fee: wi(27),
                order_execution_max_fee: wi(28),
            }
            .data(),
        };
        let actual = marginfi_type_crate::ix_builders::admin::init_global_fee_state(
            &marginfi_type_crate::ix_builders::admin::InitGlobalFeeState {
                payer: key(100),
                fee_state: key(101),
                system_program: key(102),
            },
            key(20),
            key(21),
            22u32,
            23u32,
            24u32,
            wi(25),
            wi(26),
            wi(27),
            wi(28),
        );
        assert_eq!(
            expected, actual,
            "init_global_fee_state optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::InitStakedSettings {
                marginfi_group: key(100),
                governance_admin: key(101),
                fee_payer: key(102),
                staked_settings: key(103),
                system_program: key(104),
                instruction_sysvar: key(105),
            }
            .to_account_metas(None),
            data: marginfi::instruction::InitStakedSettings {
                settings: StakedSettingsConfig::default(),
            }
            .data(),
        };
        let actual = marginfi_type_crate::ix_builders::admin::init_staked_settings(
            &marginfi_type_crate::ix_builders::admin::InitStakedSettings {
                marginfi_group: key(100),
                governance_admin: key(101),
                fee_payer: key(102),
                staked_settings: key(103),
                system_program: key(104),
                instruction_sysvar: key(105),
            },
            StakedSettingsConfig::default(),
        );
        assert_eq!(expected, actual, "init_staked_settings optional_none=False");
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::JuplendDeposit {
                group: key(100),
                marginfi_account: key(101),
                authority: key(102),
                bank: key(103),
                signer_token_account: key(104),
                liquidity_vault_authority: key(105),
                liquidity_vault: key(106),
                mint: key(107),
                integration_acc_1: key(108),
                f_token_mint: key(109),
                integration_acc_2: key(110),
                lending_admin: key(111),
                supply_token_reserves_liquidity: key(112),
                lending_supply_position_on_liquidity: key(113),
                rate_model: key(114),
                vault: key(115),
                liquidity: key(116),
                liquidity_program: key(117),
                rewards_rate_model: key(118),
                juplend_program: key(119),
                token_program: key(120),
                associated_token_program: key(121),
                system_program: key(122),
            }
            .to_account_metas(None),
            data: marginfi::instruction::JuplendDeposit { amount: 20u64 }.data(),
        };
        let actual = marginfi_type_crate::ix_builders::juplend::juplend_deposit(
            &marginfi_type_crate::ix_builders::juplend::JuplendDeposit {
                group: key(100),
                marginfi_account: key(101),
                authority: key(102),
                bank: key(103),
                signer_token_account: key(104),
                liquidity_vault_authority: key(105),
                liquidity_vault: key(106),
                mint: key(107),
                integration_acc_1: key(108),
                f_token_mint: key(109),
                integration_acc_2: key(110),
                lending_admin: key(111),
                supply_token_reserves_liquidity: key(112),
                lending_supply_position_on_liquidity: key(113),
                rate_model: key(114),
                vault: key(115),
                liquidity: key(116),
                liquidity_program: key(117),
                rewards_rate_model: key(118),
                juplend_program: key(119),
                token_program: key(120),
                associated_token_program: key(121),
                system_program: key(122),
            },
            20u64,
        );
        assert_eq!(expected, actual, "juplend_deposit optional_none=False");
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::JuplendInitPosition {
                fee_payer: key(100),
                signer_token_account: key(101),
                bank: key(102),
                liquidity_vault_authority: key(103),
                liquidity_vault: key(104),
                mint: key(105),
                integration_acc_1: key(106),
                f_token_mint: key(107),
                integration_acc_2: key(108),
                lending_admin: key(109),
                supply_token_reserves_liquidity: key(110),
                lending_supply_position_on_liquidity: key(111),
                rate_model: key(112),
                vault: key(113),
                liquidity: key(114),
                liquidity_program: key(115),
                rewards_rate_model: key(116),
                juplend_program: key(117),
                token_program: key(118),
                associated_token_program: key(119),
                system_program: key(120),
            }
            .to_account_metas(None),
            data: marginfi::instruction::JuplendInitPosition { amount: 20u64 }.data(),
        };
        let actual = marginfi_type_crate::ix_builders::juplend::juplend_init_position(
            &marginfi_type_crate::ix_builders::juplend::JuplendInitPosition {
                fee_payer: key(100),
                signer_token_account: key(101),
                bank: key(102),
                liquidity_vault_authority: key(103),
                liquidity_vault: key(104),
                mint: key(105),
                integration_acc_1: key(106),
                f_token_mint: key(107),
                integration_acc_2: key(108),
                lending_admin: key(109),
                supply_token_reserves_liquidity: key(110),
                lending_supply_position_on_liquidity: key(111),
                rate_model: key(112),
                vault: key(113),
                liquidity: key(114),
                liquidity_program: key(115),
                rewards_rate_model: key(116),
                juplend_program: key(117),
                token_program: key(118),
                associated_token_program: key(119),
                system_program: key(120),
            },
            20u64,
        );
        assert_eq!(
            expected, actual,
            "juplend_init_position optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::JuplendWithdraw {
                group: key(100),
                marginfi_account: key(101),
                authority: key(102),
                bank: key(103),
                destination_token_account: key(104),
                liquidity_vault_authority: key(105),
                mint: key(106),
                integration_acc_1: key(107),
                f_token_mint: key(108),
                integration_acc_2: key(109),
                integration_acc_3: key(110),
                lending_admin: key(111),
                supply_token_reserves_liquidity: key(112),
                lending_supply_position_on_liquidity: key(113),
                rate_model: key(114),
                vault: key(115),
                claim_account: key(116),
                liquidity: key(117),
                liquidity_program: key(118),
                rewards_rate_model: key(119),
                juplend_program: key(120),
                token_program: key(121),
                associated_token_program: key(122),
                system_program: key(123),
            }
            .to_account_metas(None),
            data: marginfi::instruction::JuplendWithdraw {
                amount: 20u64,
                withdraw_all: Some(true),
            }
            .data(),
        };
        let actual = marginfi_type_crate::ix_builders::juplend::juplend_withdraw(
            &marginfi_type_crate::ix_builders::juplend::JuplendWithdraw {
                group: key(100),
                marginfi_account: key(101),
                authority: key(102),
                bank: key(103),
                destination_token_account: key(104),
                liquidity_vault_authority: key(105),
                mint: key(106),
                integration_acc_1: key(107),
                f_token_mint: key(108),
                integration_acc_2: key(109),
                integration_acc_3: key(110),
                lending_admin: key(111),
                supply_token_reserves_liquidity: key(112),
                lending_supply_position_on_liquidity: key(113),
                rate_model: key(114),
                vault: key(115),
                claim_account: key(116),
                liquidity: key(117),
                liquidity_program: key(118),
                rewards_rate_model: key(119),
                juplend_program: key(120),
                token_program: key(121),
                associated_token_program: key(122),
                system_program: key(123),
            },
            20u64,
            Some(true),
        );
        assert_eq!(expected, actual, "juplend_withdraw optional_none=False");
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::KaminoDeposit {
                group: key(100),
                marginfi_account: key(101),
                authority: key(102),
                bank: key(103),
                signer_token_account: key(104),
                liquidity_vault_authority: key(105),
                liquidity_vault: key(106),
                integration_acc_2: key(107),
                lending_market: key(108),
                lending_market_authority: key(109),
                integration_acc_1: key(110),
                mint: key(111),
                reserve_liquidity_supply: key(112),
                reserve_collateral_mint: key(113),
                reserve_destination_deposit_collateral: key(114),
                obligation_farm_user_state: Some(key(115)),
                reserve_farm_state: Some(key(116)),
                kamino_program: key(117),
                farms_program: key(118),
                collateral_token_program: key(119),
                liquidity_token_program: key(120),
                instruction_sysvar_account: key(121),
            }
            .to_account_metas(None),
            data: marginfi::instruction::KaminoDeposit {
                amount: 20u64,
                refresh_reserve: Some(true),
            }
            .data(),
        };
        let actual = marginfi_type_crate::ix_builders::kamino::kamino_deposit(
            &marginfi_type_crate::ix_builders::kamino::KaminoDeposit {
                group: key(100),
                marginfi_account: key(101),
                authority: key(102),
                bank: key(103),
                signer_token_account: key(104),
                liquidity_vault_authority: key(105),
                liquidity_vault: key(106),
                integration_acc_2: key(107),
                lending_market: key(108),
                lending_market_authority: key(109),
                integration_acc_1: key(110),
                mint: key(111),
                reserve_liquidity_supply: key(112),
                reserve_collateral_mint: key(113),
                reserve_destination_deposit_collateral: key(114),
                obligation_farm_user_state: Some(key(115)),
                reserve_farm_state: Some(key(116)),
                kamino_program: key(117),
                farms_program: key(118),
                collateral_token_program: key(119),
                liquidity_token_program: key(120),
                instruction_sysvar_account: key(121),
            },
            20u64,
            Some(true),
        );
        assert_eq!(expected, actual, "kamino_deposit optional_none=False");
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::KaminoDeposit {
                group: key(100),
                marginfi_account: key(101),
                authority: key(102),
                bank: key(103),
                signer_token_account: key(104),
                liquidity_vault_authority: key(105),
                liquidity_vault: key(106),
                integration_acc_2: key(107),
                lending_market: key(108),
                lending_market_authority: key(109),
                integration_acc_1: key(110),
                mint: key(111),
                reserve_liquidity_supply: key(112),
                reserve_collateral_mint: key(113),
                reserve_destination_deposit_collateral: key(114),
                obligation_farm_user_state: None,
                reserve_farm_state: None,
                kamino_program: key(117),
                farms_program: key(118),
                collateral_token_program: key(119),
                liquidity_token_program: key(120),
                instruction_sysvar_account: key(121),
            }
            .to_account_metas(None),
            data: marginfi::instruction::KaminoDeposit {
                amount: 20u64,
                refresh_reserve: Some(true),
            }
            .data(),
        };
        let actual = marginfi_type_crate::ix_builders::kamino::kamino_deposit(
            &marginfi_type_crate::ix_builders::kamino::KaminoDeposit {
                group: key(100),
                marginfi_account: key(101),
                authority: key(102),
                bank: key(103),
                signer_token_account: key(104),
                liquidity_vault_authority: key(105),
                liquidity_vault: key(106),
                integration_acc_2: key(107),
                lending_market: key(108),
                lending_market_authority: key(109),
                integration_acc_1: key(110),
                mint: key(111),
                reserve_liquidity_supply: key(112),
                reserve_collateral_mint: key(113),
                reserve_destination_deposit_collateral: key(114),
                obligation_farm_user_state: None,
                reserve_farm_state: None,
                kamino_program: key(117),
                farms_program: key(118),
                collateral_token_program: key(119),
                liquidity_token_program: key(120),
                instruction_sysvar_account: key(121),
            },
            20u64,
            Some(true),
        );
        assert_eq!(expected, actual, "kamino_deposit optional_none=True");
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::KaminoHarvestReward {
                bank: key(100),
                fee_state: key(101),
                destination_token_account: key(102),
                liquidity_vault_authority: key(103),
                user_state: key(104),
                farm_state: key(105),
                global_config: key(106),
                reward_mint: key(107),
                user_reward_ata: key(108),
                rewards_vault: key(109),
                rewards_treasury_vault: key(110),
                farm_vaults_authority: key(111),
                scope_prices: Some(key(112)),
                farms_program: key(113),
                token_program: key(114),
            }
            .to_account_metas(None),
            data: marginfi::instruction::KaminoHarvestReward {
                reward_index: 20u64,
            }
            .data(),
        };
        let actual = marginfi_type_crate::ix_builders::kamino::kamino_harvest_reward(
            &marginfi_type_crate::ix_builders::kamino::KaminoHarvestReward {
                bank: key(100),
                fee_state: key(101),
                destination_token_account: key(102),
                liquidity_vault_authority: key(103),
                user_state: key(104),
                farm_state: key(105),
                global_config: key(106),
                reward_mint: key(107),
                user_reward_ata: key(108),
                rewards_vault: key(109),
                rewards_treasury_vault: key(110),
                farm_vaults_authority: key(111),
                scope_prices: Some(key(112)),
                farms_program: key(113),
                token_program: key(114),
            },
            20u64,
        );
        assert_eq!(
            expected, actual,
            "kamino_harvest_reward optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::KaminoHarvestReward {
                bank: key(100),
                fee_state: key(101),
                destination_token_account: key(102),
                liquidity_vault_authority: key(103),
                user_state: key(104),
                farm_state: key(105),
                global_config: key(106),
                reward_mint: key(107),
                user_reward_ata: key(108),
                rewards_vault: key(109),
                rewards_treasury_vault: key(110),
                farm_vaults_authority: key(111),
                scope_prices: None,
                farms_program: key(113),
                token_program: key(114),
            }
            .to_account_metas(None),
            data: marginfi::instruction::KaminoHarvestReward {
                reward_index: 20u64,
            }
            .data(),
        };
        let actual = marginfi_type_crate::ix_builders::kamino::kamino_harvest_reward(
            &marginfi_type_crate::ix_builders::kamino::KaminoHarvestReward {
                bank: key(100),
                fee_state: key(101),
                destination_token_account: key(102),
                liquidity_vault_authority: key(103),
                user_state: key(104),
                farm_state: key(105),
                global_config: key(106),
                reward_mint: key(107),
                user_reward_ata: key(108),
                rewards_vault: key(109),
                rewards_treasury_vault: key(110),
                farm_vaults_authority: key(111),
                scope_prices: None,
                farms_program: key(113),
                token_program: key(114),
            },
            20u64,
        );
        assert_eq!(expected, actual, "kamino_harvest_reward optional_none=True");
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::KaminoInitObligation {
                fee_payer: key(100),
                bank: key(101),
                signer_token_account: key(102),
                liquidity_vault_authority: key(103),
                liquidity_vault: key(104),
                integration_acc_2: key(105),
                user_metadata: key(106),
                lending_market: key(107),
                lending_market_authority: key(108),
                integration_acc_1: key(109),
                mint: key(110),
                reserve_liquidity_supply: key(111),
                reserve_collateral_mint: key(112),
                reserve_destination_deposit_collateral: key(113),
                obligation_farm_user_state: Some(key(114)),
                reserve_farm_state: Some(key(115)),
                kamino_program: key(116),
                farms_program: key(117),
                collateral_token_program: key(118),
                liquidity_token_program: key(119),
                instruction_sysvar_account: key(120),
                rent: key(121),
                system_program: key(122),
            }
            .to_account_metas(None),
            data: marginfi::instruction::KaminoInitObligation { amount: 20u64 }.data(),
        };
        let actual = marginfi_type_crate::ix_builders::kamino::kamino_init_obligation(
            &marginfi_type_crate::ix_builders::kamino::KaminoInitObligation {
                fee_payer: key(100),
                bank: key(101),
                signer_token_account: key(102),
                liquidity_vault_authority: key(103),
                liquidity_vault: key(104),
                integration_acc_2: key(105),
                user_metadata: key(106),
                lending_market: key(107),
                lending_market_authority: key(108),
                integration_acc_1: key(109),
                mint: key(110),
                reserve_liquidity_supply: key(111),
                reserve_collateral_mint: key(112),
                reserve_destination_deposit_collateral: key(113),
                obligation_farm_user_state: Some(key(114)),
                reserve_farm_state: Some(key(115)),
                kamino_program: key(116),
                farms_program: key(117),
                collateral_token_program: key(118),
                liquidity_token_program: key(119),
                instruction_sysvar_account: key(120),
                rent: key(121),
                system_program: key(122),
            },
            20u64,
        );
        assert_eq!(
            expected, actual,
            "kamino_init_obligation optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::KaminoInitObligation {
                fee_payer: key(100),
                bank: key(101),
                signer_token_account: key(102),
                liquidity_vault_authority: key(103),
                liquidity_vault: key(104),
                integration_acc_2: key(105),
                user_metadata: key(106),
                lending_market: key(107),
                lending_market_authority: key(108),
                integration_acc_1: key(109),
                mint: key(110),
                reserve_liquidity_supply: key(111),
                reserve_collateral_mint: key(112),
                reserve_destination_deposit_collateral: key(113),
                obligation_farm_user_state: None,
                reserve_farm_state: None,
                kamino_program: key(116),
                farms_program: key(117),
                collateral_token_program: key(118),
                liquidity_token_program: key(119),
                instruction_sysvar_account: key(120),
                rent: key(121),
                system_program: key(122),
            }
            .to_account_metas(None),
            data: marginfi::instruction::KaminoInitObligation { amount: 20u64 }.data(),
        };
        let actual = marginfi_type_crate::ix_builders::kamino::kamino_init_obligation(
            &marginfi_type_crate::ix_builders::kamino::KaminoInitObligation {
                fee_payer: key(100),
                bank: key(101),
                signer_token_account: key(102),
                liquidity_vault_authority: key(103),
                liquidity_vault: key(104),
                integration_acc_2: key(105),
                user_metadata: key(106),
                lending_market: key(107),
                lending_market_authority: key(108),
                integration_acc_1: key(109),
                mint: key(110),
                reserve_liquidity_supply: key(111),
                reserve_collateral_mint: key(112),
                reserve_destination_deposit_collateral: key(113),
                obligation_farm_user_state: None,
                reserve_farm_state: None,
                kamino_program: key(116),
                farms_program: key(117),
                collateral_token_program: key(118),
                liquidity_token_program: key(119),
                instruction_sysvar_account: key(120),
                rent: key(121),
                system_program: key(122),
            },
            20u64,
        );
        assert_eq!(
            expected, actual,
            "kamino_init_obligation optional_none=True"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::KaminoWithdraw {
                group: key(100),
                marginfi_account: key(101),
                authority: key(102),
                bank: key(103),
                destination_token_account: key(104),
                liquidity_vault_authority: key(105),
                liquidity_vault: key(106),
                integration_acc_2: key(107),
                lending_market: key(108),
                lending_market_authority: key(109),
                integration_acc_1: key(110),
                mint: key(111),
                reserve_liquidity_supply: key(112),
                reserve_collateral_mint: key(113),
                reserve_source_collateral: key(114),
                obligation_farm_user_state: Some(key(115)),
                reserve_farm_state: Some(key(116)),
                kamino_program: key(117),
                farms_program: key(118),
                collateral_token_program: key(119),
                liquidity_token_program: key(120),
                instruction_sysvar_account: key(121),
            }
            .to_account_metas(None),
            data: marginfi::instruction::KaminoWithdraw {
                amount: 20u64,
                flags: Some(21u8),
            }
            .data(),
        };
        let actual = marginfi_type_crate::ix_builders::kamino::kamino_withdraw(
            &marginfi_type_crate::ix_builders::kamino::KaminoWithdraw {
                group: key(100),
                marginfi_account: key(101),
                authority: key(102),
                bank: key(103),
                destination_token_account: key(104),
                liquidity_vault_authority: key(105),
                liquidity_vault: key(106),
                integration_acc_2: key(107),
                lending_market: key(108),
                lending_market_authority: key(109),
                integration_acc_1: key(110),
                mint: key(111),
                reserve_liquidity_supply: key(112),
                reserve_collateral_mint: key(113),
                reserve_source_collateral: key(114),
                obligation_farm_user_state: Some(key(115)),
                reserve_farm_state: Some(key(116)),
                kamino_program: key(117),
                farms_program: key(118),
                collateral_token_program: key(119),
                liquidity_token_program: key(120),
                instruction_sysvar_account: key(121),
            },
            20u64,
            Some(21u8),
        );
        assert_eq!(expected, actual, "kamino_withdraw optional_none=False");
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::KaminoWithdraw {
                group: key(100),
                marginfi_account: key(101),
                authority: key(102),
                bank: key(103),
                destination_token_account: key(104),
                liquidity_vault_authority: key(105),
                liquidity_vault: key(106),
                integration_acc_2: key(107),
                lending_market: key(108),
                lending_market_authority: key(109),
                integration_acc_1: key(110),
                mint: key(111),
                reserve_liquidity_supply: key(112),
                reserve_collateral_mint: key(113),
                reserve_source_collateral: key(114),
                obligation_farm_user_state: None,
                reserve_farm_state: None,
                kamino_program: key(117),
                farms_program: key(118),
                collateral_token_program: key(119),
                liquidity_token_program: key(120),
                instruction_sysvar_account: key(121),
            }
            .to_account_metas(None),
            data: marginfi::instruction::KaminoWithdraw {
                amount: 20u64,
                flags: Some(21u8),
            }
            .data(),
        };
        let actual = marginfi_type_crate::ix_builders::kamino::kamino_withdraw(
            &marginfi_type_crate::ix_builders::kamino::KaminoWithdraw {
                group: key(100),
                marginfi_account: key(101),
                authority: key(102),
                bank: key(103),
                destination_token_account: key(104),
                liquidity_vault_authority: key(105),
                liquidity_vault: key(106),
                integration_acc_2: key(107),
                lending_market: key(108),
                lending_market_authority: key(109),
                integration_acc_1: key(110),
                mint: key(111),
                reserve_liquidity_supply: key(112),
                reserve_collateral_mint: key(113),
                reserve_source_collateral: key(114),
                obligation_farm_user_state: None,
                reserve_farm_state: None,
                kamino_program: key(117),
                farms_program: key(118),
                collateral_token_program: key(119),
                liquidity_token_program: key(120),
                instruction_sysvar_account: key(121),
            },
            20u64,
            Some(21u8),
        );
        assert_eq!(expected, actual, "kamino_withdraw optional_none=True");
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::LendingAccountBorrow {
                group: key(100),
                marginfi_account: key(101),
                authority: key(102),
                bank: key(103),
                destination_token_account: key(104),
                bank_liquidity_vault_authority: key(105),
                liquidity_vault: key(106),
                token_program: key(107),
            }
            .to_account_metas(None),
            data: marginfi::instruction::LendingAccountBorrow { amount: 20u64 }.data(),
        };
        let actual = marginfi_type_crate::ix_builders::lending::lending_account_borrow(
            &marginfi_type_crate::ix_builders::lending::LendingAccountBorrow {
                group: key(100),
                marginfi_account: key(101),
                authority: key(102),
                bank: key(103),
                destination_token_account: key(104),
                bank_liquidity_vault_authority: key(105),
                liquidity_vault: key(106),
                token_program: key(107),
            },
            20u64,
        );
        assert_eq!(
            expected, actual,
            "lending_account_borrow optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::LendingAccountCloseBalance {
                group: key(100),
                marginfi_account: key(101),
                authority: key(102),
                bank: key(103),
            }
            .to_account_metas(None),
            data: marginfi::instruction::LendingAccountCloseBalance {}.data(),
        };
        let actual = marginfi_type_crate::ix_builders::lending::lending_account_close_balance(
            &marginfi_type_crate::ix_builders::lending::LendingAccountCloseBalance {
                group: key(100),
                marginfi_account: key(101),
                authority: key(102),
                bank: key(103),
            },
        );
        assert_eq!(
            expected, actual,
            "lending_account_close_balance optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::LendingAccountDeposit {
                group: key(100),
                marginfi_account: key(101),
                authority: key(102),
                bank: key(103),
                signer_token_account: key(104),
                liquidity_vault: key(105),
                token_program: key(106),
            }
            .to_account_metas(None),
            data: marginfi::instruction::LendingAccountDeposit {
                amount: 20u64,
                deposit_up_to_limit: Some(true),
            }
            .data(),
        };
        let actual = marginfi_type_crate::ix_builders::lending::lending_account_deposit(
            &marginfi_type_crate::ix_builders::lending::LendingAccountDeposit {
                group: key(100),
                marginfi_account: key(101),
                authority: key(102),
                bank: key(103),
                signer_token_account: key(104),
                liquidity_vault: key(105),
                token_program: key(106),
            },
            20u64,
            Some(true),
        );
        assert_eq!(
            expected, actual,
            "lending_account_deposit optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::LendingAccountEndFlashloan {
                marginfi_account: key(100),
                group: key(101),
                authority: key(102),
            }
            .to_account_metas(None),
            data: marginfi::instruction::LendingAccountEndFlashloan {}.data(),
        };
        let actual = marginfi_type_crate::ix_builders::lending::lending_account_end_flashloan(
            &marginfi_type_crate::ix_builders::lending::LendingAccountEndFlashloan {
                marginfi_account: key(100),
                group: key(101),
                authority: key(102),
            },
        );
        assert_eq!(
            expected, actual,
            "lending_account_end_flashloan optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::LendingAccountLiquidate {
                group: key(100),
                asset_bank: key(101),
                liab_bank: key(102),
                liquidator_marginfi_account: key(103),
                authority: key(104),
                liquidatee_marginfi_account: key(105),
                bank_liquidity_vault_authority: key(106),
                bank_liquidity_vault: key(107),
                bank_insurance_vault: key(108),
                token_program: key(109),
            }
            .to_account_metas(None),
            data: marginfi::instruction::LendingAccountLiquidate {
                asset_amount: 20u64,
                liquidatee_accounts: 21u8,
                liquidator_accounts: 22u8,
            }
            .data(),
        };
        let actual = marginfi_type_crate::ix_builders::lending::lending_account_liquidate(
            &marginfi_type_crate::ix_builders::lending::LendingAccountLiquidate {
                group: key(100),
                asset_bank: key(101),
                liab_bank: key(102),
                liquidator_marginfi_account: key(103),
                authority: key(104),
                liquidatee_marginfi_account: key(105),
                bank_liquidity_vault_authority: key(106),
                bank_liquidity_vault: key(107),
                bank_insurance_vault: key(108),
                token_program: key(109),
            },
            20u64,
            21u8,
            22u8,
        );
        assert_eq!(
            expected, actual,
            "lending_account_liquidate optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::PulseHealth {
                marginfi_account: key(100),
                group: key(101),
            }
            .to_account_metas(None),
            data: marginfi::instruction::LendingAccountPulseHealth {}.data(),
        };
        let actual = marginfi_type_crate::ix_builders::lending::lending_account_pulse_health(
            &marginfi_type_crate::ix_builders::lending::LendingAccountPulseHealth {
                marginfi_account: key(100),
                group: key(101),
            },
        );
        assert_eq!(
            expected, actual,
            "lending_account_pulse_health optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::LendingAccountRepay {
                group: key(100),
                marginfi_account: key(101),
                authority: key(102),
                bank: key(103),
                signer_token_account: key(104),
                liquidity_vault: key(105),
                token_program: key(106),
            }
            .to_account_metas(None),
            data: marginfi::instruction::LendingAccountRepay {
                amount: 20u64,
                repay_all: Some(true),
            }
            .data(),
        };
        let actual = marginfi_type_crate::ix_builders::lending::lending_account_repay(
            &marginfi_type_crate::ix_builders::lending::LendingAccountRepay {
                group: key(100),
                marginfi_account: key(101),
                authority: key(102),
                bank: key(103),
                signer_token_account: key(104),
                liquidity_vault: key(105),
                token_program: key(106),
            },
            20u64,
            Some(true),
        );
        assert_eq!(
            expected, actual,
            "lending_account_repay optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::LendingAccountStartFlashloan {
                marginfi_account: key(100),
                authority: key(101),
                ixs_sysvar: key(102),
            }
            .to_account_metas(None),
            data: marginfi::instruction::LendingAccountStartFlashloan { end_index: 20u64 }.data(),
        };
        let actual = marginfi_type_crate::ix_builders::lending::lending_account_start_flashloan(
            &marginfi_type_crate::ix_builders::lending::LendingAccountStartFlashloan {
                marginfi_account: key(100),
                authority: key(101),
                ixs_sysvar: key(102),
            },
            20u64,
        );
        assert_eq!(
            expected, actual,
            "lending_account_start_flashloan optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::LendingAccountWithdraw {
                group: key(100),
                marginfi_account: key(101),
                authority: key(102),
                bank: key(103),
                destination_token_account: key(104),
                bank_liquidity_vault_authority: key(105),
                liquidity_vault: key(106),
                token_program: key(107),
            }
            .to_account_metas(None),
            data: marginfi::instruction::LendingAccountWithdraw {
                amount: 20u64,
                withdraw_all: Some(true),
            }
            .data(),
        };
        let actual = marginfi_type_crate::ix_builders::lending::lending_account_withdraw(
            &marginfi_type_crate::ix_builders::lending::LendingAccountWithdraw {
                group: key(100),
                marginfi_account: key(101),
                authority: key(102),
                bank: key(103),
                destination_token_account: key(104),
                bank_liquidity_vault_authority: key(105),
                liquidity_vault: key(106),
                token_program: key(107),
            },
            20u64,
            Some(true),
        );
        assert_eq!(
            expected, actual,
            "lending_account_withdraw optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::LendingPoolAccrueBankInterest {
                group: key(100),
                bank: key(101),
            }
            .to_account_metas(None),
            data: marginfi::instruction::LendingPoolAccrueBankInterest {}.data(),
        };
        let actual = marginfi_type_crate::ix_builders::pool::lending_pool_accrue_bank_interest(
            &marginfi_type_crate::ix_builders::pool::LendingPoolAccrueBankInterest {
                group: key(100),
                bank: key(101),
            },
        );
        assert_eq!(
            expected, actual,
            "lending_pool_accrue_bank_interest optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::LendingPoolAddBank {
                marginfi_group: key(100),
                governance_admin: key(101),
                fee_payer: key(102),
                fee_state: key(103),
                global_fee_wallet: key(104),
                bank_mint: key(105),
                bank: key(106),
                liquidity_vault_authority: key(107),
                liquidity_vault: key(108),
                insurance_vault_authority: key(109),
                insurance_vault: key(110),
                fee_vault_authority: key(111),
                fee_vault: key(112),
                token_program: key(113),
                system_program: key(114),
                instruction_sysvar: key(115),
            }
            .to_account_metas(None),
            data: marginfi::instruction::LendingPoolAddBank {
                bank_config: BankConfigCompact::default(),
            }
            .data(),
        };
        let actual = marginfi_type_crate::ix_builders::pool::lending_pool_add_bank(
            &marginfi_type_crate::ix_builders::pool::LendingPoolAddBank {
                marginfi_group: key(100),
                governance_admin: key(101),
                fee_payer: key(102),
                fee_state: key(103),
                global_fee_wallet: key(104),
                bank_mint: key(105),
                bank: key(106),
                liquidity_vault_authority: key(107),
                liquidity_vault: key(108),
                insurance_vault_authority: key(109),
                insurance_vault: key(110),
                fee_vault_authority: key(111),
                fee_vault: key(112),
                token_program: key(113),
                system_program: key(114),
                instruction_sysvar: key(115),
            },
            BankConfigCompact::default(),
        );
        assert_eq!(
            expected, actual,
            "lending_pool_add_bank optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::LendingPoolAddBankDrift {
                group: key(100),
                governance_admin: key(101),
                fee_payer: key(102),
                bank_mint: key(103),
                bank: key(104),
                integration_acc_1: key(105),
                integration_acc_2: key(106),
                integration_acc_3: key(107),
                liquidity_vault_authority: key(108),
                liquidity_vault: key(109),
                insurance_vault_authority: key(110),
                insurance_vault: key(111),
                fee_vault_authority: key(112),
                fee_vault: key(113),
                token_program: key(114),
                system_program: key(115),
                instruction_sysvar: key(116),
            }
            .to_account_metas(None),
            data: marginfi::instruction::LendingPoolAddBankDrift {
                bank_config: DriftConfigCompact::default(),
                bank_seed: 21u64,
            }
            .data(),
        };
        let actual = marginfi_type_crate::ix_builders::drift::lending_pool_add_bank_drift(
            &marginfi_type_crate::ix_builders::drift::LendingPoolAddBankDrift {
                group: key(100),
                governance_admin: key(101),
                fee_payer: key(102),
                bank_mint: key(103),
                bank: key(104),
                integration_acc_1: key(105),
                integration_acc_2: key(106),
                integration_acc_3: key(107),
                liquidity_vault_authority: key(108),
                liquidity_vault: key(109),
                insurance_vault_authority: key(110),
                insurance_vault: key(111),
                fee_vault_authority: key(112),
                fee_vault: key(113),
                token_program: key(114),
                system_program: key(115),
                instruction_sysvar: key(116),
            },
            DriftConfigCompact::default(),
            21u64,
        );
        assert_eq!(
            expected, actual,
            "lending_pool_add_bank_drift optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::LendingPoolAddBankJuplend {
                group: key(100),
                governance_admin: key(101),
                fee_payer: key(102),
                bank_mint: key(103),
                bank: key(104),
                integration_acc_1: key(105),
                liquidity_vault_authority: key(106),
                liquidity_vault: key(107),
                insurance_vault_authority: key(108),
                insurance_vault: key(109),
                fee_vault_authority: key(110),
                fee_vault: key(111),
                f_token_mint: key(112),
                integration_acc_2: key(113),
                token_program: key(114),
                system_program: key(115),
                instruction_sysvar: key(116),
            }
            .to_account_metas(None),
            data: marginfi::instruction::LendingPoolAddBankJuplend {
                bank_config: JuplendConfigCompact::default(),
                bank_seed: 21u64,
            }
            .data(),
        };
        let actual = marginfi_type_crate::ix_builders::juplend::lending_pool_add_bank_juplend(
            &marginfi_type_crate::ix_builders::juplend::LendingPoolAddBankJuplend {
                group: key(100),
                governance_admin: key(101),
                fee_payer: key(102),
                bank_mint: key(103),
                bank: key(104),
                integration_acc_1: key(105),
                liquidity_vault_authority: key(106),
                liquidity_vault: key(107),
                insurance_vault_authority: key(108),
                insurance_vault: key(109),
                fee_vault_authority: key(110),
                fee_vault: key(111),
                f_token_mint: key(112),
                integration_acc_2: key(113),
                token_program: key(114),
                system_program: key(115),
                instruction_sysvar: key(116),
            },
            JuplendConfigCompact::default(),
            21u64,
        );
        assert_eq!(
            expected, actual,
            "lending_pool_add_bank_juplend optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::LendingPoolAddBankKamino {
                group: key(100),
                governance_admin: key(101),
                fee_payer: key(102),
                bank_mint: key(103),
                bank: key(104),
                integration_acc_1: key(105),
                integration_acc_2: key(106),
                liquidity_vault_authority: key(107),
                liquidity_vault: key(108),
                insurance_vault_authority: key(109),
                insurance_vault: key(110),
                fee_vault_authority: key(111),
                fee_vault: key(112),
                token_program: key(113),
                system_program: key(114),
                instruction_sysvar: key(115),
            }
            .to_account_metas(None),
            data: marginfi::instruction::LendingPoolAddBankKamino {
                bank_config: KaminoConfigCompact::default(),
                bank_seed: 21u64,
            }
            .data(),
        };
        let actual = marginfi_type_crate::ix_builders::kamino::lending_pool_add_bank_kamino(
            &marginfi_type_crate::ix_builders::kamino::LendingPoolAddBankKamino {
                group: key(100),
                governance_admin: key(101),
                fee_payer: key(102),
                bank_mint: key(103),
                bank: key(104),
                integration_acc_1: key(105),
                integration_acc_2: key(106),
                liquidity_vault_authority: key(107),
                liquidity_vault: key(108),
                insurance_vault_authority: key(109),
                insurance_vault: key(110),
                fee_vault_authority: key(111),
                fee_vault: key(112),
                token_program: key(113),
                system_program: key(114),
                instruction_sysvar: key(115),
            },
            KaminoConfigCompact::default(),
            21u64,
        );
        assert_eq!(
            expected, actual,
            "lending_pool_add_bank_kamino optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::LendingPoolAddBankPermissionless {
                marginfi_group: key(100),
                staked_settings: key(101),
                fee_payer: key(102),
                bank_mint: key(103),
                sol_pool: key(104),
                pool_onramp: key(105),
                stake_pool: key(106),
                validator_vote_account: key(107),
                bank: key(108),
                liquidity_vault_authority: key(109),
                liquidity_vault: key(110),
                insurance_vault_authority: key(111),
                insurance_vault: key(112),
                fee_vault_authority: key(113),
                fee_vault: key(114),
                token_program: key(115),
                system_program: key(116),
            }
            .to_account_metas(None),
            data: marginfi::instruction::LendingPoolAddBankPermissionless { bank_seed: 20u64 }
                .data(),
        };
        let actual = marginfi_type_crate::ix_builders::pool::lending_pool_add_bank_permissionless(
            &marginfi_type_crate::ix_builders::pool::LendingPoolAddBankPermissionless {
                marginfi_group: key(100),
                staked_settings: key(101),
                fee_payer: key(102),
                bank_mint: key(103),
                sol_pool: key(104),
                pool_onramp: key(105),
                stake_pool: key(106),
                validator_vote_account: key(107),
                bank: key(108),
                liquidity_vault_authority: key(109),
                liquidity_vault: key(110),
                insurance_vault_authority: key(111),
                insurance_vault: key(112),
                fee_vault_authority: key(113),
                fee_vault: key(114),
                token_program: key(115),
                system_program: key(116),
            },
            20u64,
        );
        assert_eq!(
            expected, actual,
            "lending_pool_add_bank_permissionless optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::LendingPoolAddBankSolend {
                group: key(100),
                governance_admin: key(101),
                fee_payer: key(102),
                bank_mint: key(103),
                bank: key(104),
                integration_acc_1: key(105),
                integration_acc_2: key(106),
                liquidity_vault_authority: key(107),
                liquidity_vault: key(108),
                insurance_vault_authority: key(109),
                insurance_vault: key(110),
                fee_vault_authority: key(111),
                fee_vault: key(112),
                token_program: key(113),
                system_program: key(114),
                instruction_sysvar: key(115),
            }
            .to_account_metas(None),
            data: marginfi::instruction::LendingPoolAddBankSolend {
                bank_config: SolendConfigCompact::default(),
                bank_seed: 21u64,
            }
            .data(),
        };
        let actual = marginfi_type_crate::ix_builders::solend::lending_pool_add_bank_solend(
            &marginfi_type_crate::ix_builders::solend::LendingPoolAddBankSolend {
                group: key(100),
                governance_admin: key(101),
                fee_payer: key(102),
                bank_mint: key(103),
                bank: key(104),
                integration_acc_1: key(105),
                integration_acc_2: key(106),
                liquidity_vault_authority: key(107),
                liquidity_vault: key(108),
                insurance_vault_authority: key(109),
                insurance_vault: key(110),
                fee_vault_authority: key(111),
                fee_vault: key(112),
                token_program: key(113),
                system_program: key(114),
                instruction_sysvar: key(115),
            },
            SolendConfigCompact::default(),
            21u64,
        );
        assert_eq!(
            expected, actual,
            "lending_pool_add_bank_solend optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::LendingPoolAddBankWithSeed {
                marginfi_group: key(100),
                governance_admin: key(101),
                fee_payer: key(102),
                fee_state: key(103),
                global_fee_wallet: key(104),
                bank_mint: key(105),
                bank: key(106),
                liquidity_vault_authority: key(107),
                liquidity_vault: key(108),
                insurance_vault_authority: key(109),
                insurance_vault: key(110),
                fee_vault_authority: key(111),
                fee_vault: key(112),
                token_program: key(113),
                system_program: key(114),
                instruction_sysvar: key(115),
            }
            .to_account_metas(None),
            data: marginfi::instruction::LendingPoolAddBankWithSeed {
                bank_config: BankConfigCompact::default(),
                bank_seed: 21u64,
            }
            .data(),
        };
        let actual = marginfi_type_crate::ix_builders::pool::lending_pool_add_bank_with_seed(
            &marginfi_type_crate::ix_builders::pool::LendingPoolAddBankWithSeed {
                marginfi_group: key(100),
                governance_admin: key(101),
                fee_payer: key(102),
                fee_state: key(103),
                global_fee_wallet: key(104),
                bank_mint: key(105),
                bank: key(106),
                liquidity_vault_authority: key(107),
                liquidity_vault: key(108),
                insurance_vault_authority: key(109),
                insurance_vault: key(110),
                fee_vault_authority: key(111),
                fee_vault: key(112),
                token_program: key(113),
                system_program: key(114),
                instruction_sysvar: key(115),
            },
            BankConfigCompact::default(),
            21u64,
        );
        assert_eq!(
            expected, actual,
            "lending_pool_add_bank_with_seed optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::LendingPoolBackfillBankIsT22Flag {
                bank: key(100),
                group: key(101),
                mint: key(102),
            }
            .to_account_metas(None),
            data: marginfi::instruction::LendingPoolBackfillBankIsT22Flag {
                bank_seed: Some(20u64),
            }
            .data(),
        };
        let actual = marginfi_type_crate::ix_builders::pool::lending_pool_backfill_bank_is_t22_flag(
            &marginfi_type_crate::ix_builders::pool::LendingPoolBackfillBankIsT22Flag {
                bank: key(100),
                group: key(101),
                mint: key(102),
            },
            Some(20u64),
        );
        assert_eq!(
            expected, actual,
            "lending_pool_backfill_bank_is_t22_flag optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::LendingPoolBackfillStakedBankValidatorVoteAccount {
                bank: key(100),
                validator_vote_account: key(101),
            }
            .to_account_metas(None),
            data: marginfi::instruction::LendingPoolBackfillStakedBankValidatorVoteAccount {}
                .data(),
        };
        let actual = marginfi_type_crate::ix_builders::pool::lending_pool_backfill_staked_bank_validator_vote_account(&marginfi_type_crate::ix_builders::pool::LendingPoolBackfillStakedBankValidatorVoteAccount { bank: key(100), validator_vote_account: key(101) });
        assert_eq!(
            expected, actual,
            "lending_pool_backfill_staked_bank_validator_vote_account optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::LendingPoolClearCircuitBreaker {
                group: key(100),
                authority: key(101),
                bank: key(102),
                instruction_sysvar: key(103),
            }
            .to_account_metas(None),
            data: marginfi::instruction::LendingPoolClearCircuitBreaker {
                reseed_reference: true,
            }
            .data(),
        };
        let actual = marginfi_type_crate::ix_builders::pool::lending_pool_clear_circuit_breaker(
            &marginfi_type_crate::ix_builders::pool::LendingPoolClearCircuitBreaker {
                group: key(100),
                authority: key(101),
                bank: key(102),
                instruction_sysvar: key(103),
            },
            true,
        );
        assert_eq!(
            expected, actual,
            "lending_pool_clear_circuit_breaker optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::LendingPoolCloneBank {
                marginfi_group: key(100),
                governance_admin: key(101),
                fee_payer: key(102),
                bank_mint: key(103),
                source_bank: key(104),
                bank: key(105),
                liquidity_vault_authority: key(106),
                liquidity_vault: key(107),
                insurance_vault_authority: key(108),
                insurance_vault: key(109),
                fee_vault_authority: key(110),
                fee_vault: key(111),
                token_program: key(112),
                system_program: key(113),
                instruction_sysvar: key(114),
            }
            .to_account_metas(None),
            data: marginfi::instruction::LendingPoolCloneBank { bank_seed: 20u64 }.data(),
        };
        let actual = marginfi_type_crate::ix_builders::pool::lending_pool_clone_bank(
            &marginfi_type_crate::ix_builders::pool::LendingPoolCloneBank {
                marginfi_group: key(100),
                governance_admin: key(101),
                fee_payer: key(102),
                bank_mint: key(103),
                source_bank: key(104),
                bank: key(105),
                liquidity_vault_authority: key(106),
                liquidity_vault: key(107),
                insurance_vault_authority: key(108),
                insurance_vault: key(109),
                fee_vault_authority: key(110),
                fee_vault: key(111),
                token_program: key(112),
                system_program: key(113),
                instruction_sysvar: key(114),
            },
            20u64,
        );
        assert_eq!(
            expected, actual,
            "lending_pool_clone_bank optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::LendingPoolCloneEmode {
                group: key(100),
                governance_admin: key(101),
                copy_from_bank: key(102),
                copy_to_bank: key(103),
                instruction_sysvar: key(104),
            }
            .to_account_metas(None),
            data: marginfi::instruction::LendingPoolCloneEmode {}.data(),
        };
        let actual = marginfi_type_crate::ix_builders::pool::lending_pool_clone_emode(
            &marginfi_type_crate::ix_builders::pool::LendingPoolCloneEmode {
                group: key(100),
                governance_admin: key(101),
                copy_from_bank: key(102),
                copy_to_bank: key(103),
                instruction_sysvar: key(104),
            },
        );
        assert_eq!(
            expected, actual,
            "lending_pool_clone_emode optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::LendingPoolCloseBank {
                group: key(100),
                bank: key(101),
                admin: key(102),
                instruction_sysvar: key(103),
            }
            .to_account_metas(None),
            data: marginfi::instruction::LendingPoolCloseBank {
                force_close: Some(true),
            }
            .data(),
        };
        let actual = marginfi_type_crate::ix_builders::pool::lending_pool_close_bank(
            &marginfi_type_crate::ix_builders::pool::LendingPoolCloseBank {
                group: key(100),
                bank: key(101),
                admin: key(102),
                instruction_sysvar: key(103),
            },
            Some(true),
        );
        assert_eq!(
            expected, actual,
            "lending_pool_close_bank optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::LendingPoolCollectBankFees {
                group: key(100),
                bank: key(101),
                liquidity_vault_authority: key(102),
                liquidity_vault: key(103),
                insurance_vault: key(104),
                fee_vault: key(105),
                fee_state: key(106),
                fee_ata: key(107),
                token_program: key(108),
            }
            .to_account_metas(None),
            data: marginfi::instruction::LendingPoolCollectBankFees {}.data(),
        };
        let actual = marginfi_type_crate::ix_builders::pool::lending_pool_collect_bank_fees(
            &marginfi_type_crate::ix_builders::pool::LendingPoolCollectBankFees {
                group: key(100),
                bank: key(101),
                liquidity_vault_authority: key(102),
                liquidity_vault: key(103),
                insurance_vault: key(104),
                fee_vault: key(105),
                fee_state: key(106),
                fee_ata: key(107),
                token_program: key(108),
            },
        );
        assert_eq!(
            expected, actual,
            "lending_pool_collect_bank_fees optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::LendingPoolCollectBankPremiumFees {
                group: key(100),
                bank: key(101),
                liquidity_vault_authority: key(102),
                liquidity_vault: key(103),
                fee_state: key(104),
                premium_ata: key(105),
                token_program: key(106),
            }
            .to_account_metas(None),
            data: marginfi::instruction::LendingPoolCollectBankPremiumFees {}.data(),
        };
        let actual = marginfi_type_crate::ix_builders::pool::lending_pool_collect_bank_premium_fees(
            &marginfi_type_crate::ix_builders::pool::LendingPoolCollectBankPremiumFees {
                group: key(100),
                bank: key(101),
                liquidity_vault_authority: key(102),
                liquidity_vault: key(103),
                fee_state: key(104),
                premium_ata: key(105),
                token_program: key(106),
            },
        );
        assert_eq!(
            expected, actual,
            "lending_pool_collect_bank_premium_fees optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::LendingPoolConfigureBank {
                group: key(100),
                admin: key(101),
                bank: key(102),
                instruction_sysvar: key(103),
            }
            .to_account_metas(None),
            data: marginfi::instruction::LendingPoolConfigureBank {
                bank_config_opt: BankConfigFast::default(),
            }
            .data(),
        };
        let actual = marginfi_type_crate::ix_builders::pool::lending_pool_configure_bank(
            &marginfi_type_crate::ix_builders::pool::LendingPoolConfigureBank {
                group: key(100),
                admin: key(101),
                bank: key(102),
                instruction_sysvar: key(103),
            },
            BankConfigFast::default(),
        );
        assert_eq!(
            expected, actual,
            "lending_pool_configure_bank optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::LendingPoolConfigureBankEmode {
                group: key(100),
                governance_admin: key(101),
                bank: key(102),
                instruction_sysvar: key(103),
            }
            .to_account_metas(None),
            data: marginfi::instruction::LendingPoolConfigureBankEmode {
                emode_tag: 20u16,
                entries: [{
                    let mut x = EmodeEntry::zeroed();
                    x.collateral_bank_emode_tag = 17;
                    x
                }; 10],
            }
            .data(),
        };
        let actual = marginfi_type_crate::ix_builders::pool::lending_pool_configure_bank_emode(
            &marginfi_type_crate::ix_builders::pool::LendingPoolConfigureBankEmode {
                group: key(100),
                governance_admin: key(101),
                bank: key(102),
                instruction_sysvar: key(103),
            },
            20u16,
            [{
                let mut x = EmodeEntry::zeroed();
                x.collateral_bank_emode_tag = 17;
                x
            }; 10],
        );
        assert_eq!(
            expected, actual,
            "lending_pool_configure_bank_emode optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::LendingPoolConfigureBankGov {
                group: key(100),
                governance_admin: key(101),
                bank: key(102),
                instruction_sysvar: key(103),
            }
            .to_account_metas(None),
            data: marginfi::instruction::LendingPoolConfigureBankGov {
                bank_config_opt: BankConfigGov::default(),
            }
            .data(),
        };
        let actual = marginfi_type_crate::ix_builders::pool::lending_pool_configure_bank_gov(
            &marginfi_type_crate::ix_builders::pool::LendingPoolConfigureBankGov {
                group: key(100),
                governance_admin: key(101),
                bank: key(102),
                instruction_sysvar: key(103),
            },
            BankConfigGov::default(),
        );
        assert_eq!(
            expected, actual,
            "lending_pool_configure_bank_gov optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::LendingPoolConfigureBankInterestOnly {
                group: key(100),
                delegate_curve_admin: key(101),
                bank: key(102),
                instruction_sysvar: key(103),
            }
            .to_account_metas(None),
            data: marginfi::instruction::LendingPoolConfigureBankInterestOnly {
                interest_rate_config: InterestRateConfigOpt::default(),
            }
            .data(),
        };
        let actual =
            marginfi_type_crate::ix_builders::pool::lending_pool_configure_bank_interest_only(
                &marginfi_type_crate::ix_builders::pool::LendingPoolConfigureBankInterestOnly {
                    group: key(100),
                    delegate_curve_admin: key(101),
                    bank: key(102),
                    instruction_sysvar: key(103),
                },
                InterestRateConfigOpt::default(),
            );
        assert_eq!(
            expected, actual,
            "lending_pool_configure_bank_interest_only optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::LendingPoolConfigureBankLimitsOnly {
                group: key(100),
                delegate_limit_admin: key(101),
                bank: key(102),
                instruction_sysvar: key(103),
            }
            .to_account_metas(None),
            data: marginfi::instruction::LendingPoolConfigureBankLimitsOnly {
                deposit_limit: Some(20u64),
                borrow_limit: Some(21u64),
                total_asset_value_init_limit: Some(22u64),
            }
            .data(),
        };
        let actual =
            marginfi_type_crate::ix_builders::pool::lending_pool_configure_bank_limits_only(
                &marginfi_type_crate::ix_builders::pool::LendingPoolConfigureBankLimitsOnly {
                    group: key(100),
                    delegate_limit_admin: key(101),
                    bank: key(102),
                    instruction_sysvar: key(103),
                },
                Some(20u64),
                Some(21u64),
                Some(22u64),
            );
        assert_eq!(
            expected, actual,
            "lending_pool_configure_bank_limits_only optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::LendingPoolConfigureBankOracle {
                group: key(100),
                governance_admin: key(101),
                bank: key(102),
                instruction_sysvar: key(103),
            }
            .to_account_metas(None),
            data: marginfi::instruction::LendingPoolConfigureBankOracle {
                setup: 20u8,
                oracle: key(21),
            }
            .data(),
        };
        let actual = marginfi_type_crate::ix_builders::pool::lending_pool_configure_bank_oracle(
            &marginfi_type_crate::ix_builders::pool::LendingPoolConfigureBankOracle {
                group: key(100),
                governance_admin: key(101),
                bank: key(102),
                instruction_sysvar: key(103),
            },
            20u8,
            key(21),
        );
        assert_eq!(
            expected, actual,
            "lending_pool_configure_bank_oracle optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::LendingPoolConfigureBankOracle {
                group: key(100),
                governance_admin: key(101),
                bank: key(102),
                instruction_sysvar: key(103),
            }
            .to_account_metas(None),
            data: marginfi::instruction::LendingPoolConfigureBankOracleScope {
                oracle: key(20),
                entry_index: 21u16,
            }
            .data(),
        };
        let actual =
            marginfi_type_crate::ix_builders::pool::lending_pool_configure_bank_oracle_scope(
                &marginfi_type_crate::ix_builders::pool::LendingPoolConfigureBankOracleScope {
                    group: key(100),
                    governance_admin: key(101),
                    bank: key(102),
                    instruction_sysvar: key(103),
                },
                key(20),
                21u16,
            );
        assert_eq!(
            expected, actual,
            "lending_pool_configure_bank_oracle_scope optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::LendingPoolConfigureBankPremium {
                group: key(100),
                admin: key(101),
                bank: key(102),
                instruction_sysvar: key(103),
            }
            .to_account_metas(None),
            data: marginfi::instruction::LendingPoolConfigureBankPremium {
                premium_tag: 20u16,
                active: true,
            }
            .data(),
        };
        let actual = marginfi_type_crate::ix_builders::pool::lending_pool_configure_bank_premium(
            &marginfi_type_crate::ix_builders::pool::LendingPoolConfigureBankPremium {
                group: key(100),
                admin: key(101),
                bank: key(102),
                instruction_sysvar: key(103),
            },
            20u16,
            true,
        );
        assert_eq!(
            expected, actual,
            "lending_pool_configure_bank_premium optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::LendingPoolConfigureGroupPremium {
                group: key(100),
                admin: key(101),
                instruction_sysvar: key(102),
            }
            .to_account_metas(None),
            data: marginfi::instruction::LendingPoolConfigureGroupPremium {
                collateral_tag: 20u16,
                liability_tag: 21u16,
                rate: 22u32,
            }
            .data(),
        };
        let actual = marginfi_type_crate::ix_builders::pool::lending_pool_configure_group_premium(
            &marginfi_type_crate::ix_builders::pool::LendingPoolConfigureGroupPremium {
                group: key(100),
                admin: key(101),
                instruction_sysvar: key(102),
            },
            20u16,
            21u16,
            22u32,
        );
        assert_eq!(
            expected, actual,
            "lending_pool_configure_group_premium optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::LendingPoolEmissionsDeposit {
                group: key(100),
                bank: key(101),
                mint: key(102),
                emissions_funding_account: key(103),
                depositor: key(104),
                liquidity_vault: key(105),
                token_program: key(106),
            }
            .to_account_metas(None),
            data: marginfi::instruction::LendingPoolEmissionsDeposit { amount: 20u64 }.data(),
        };
        let actual = marginfi_type_crate::ix_builders::pool::lending_pool_emissions_deposit(
            &marginfi_type_crate::ix_builders::pool::LendingPoolEmissionsDeposit {
                group: key(100),
                bank: key(101),
                mint: key(102),
                emissions_funding_account: key(103),
                depositor: key(104),
                liquidity_vault: key(105),
                token_program: key(106),
            },
            20u64,
        );
        assert_eq!(
            expected, actual,
            "lending_pool_emissions_deposit optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::LendingPoolForceTokenlessRepayComplete {
                group: key(100),
                risk_admin: key(101),
                bank: key(102),
                instruction_sysvar: key(103),
            }
            .to_account_metas(None),
            data: marginfi::instruction::LendingPoolForceTokenlessRepayComplete {}.data(),
        };
        let actual =
            marginfi_type_crate::ix_builders::pool::lending_pool_force_tokenless_repay_complete(
                &marginfi_type_crate::ix_builders::pool::LendingPoolForceTokenlessRepayComplete {
                    group: key(100),
                    risk_admin: key(101),
                    bank: key(102),
                    instruction_sysvar: key(103),
                },
            );
        assert_eq!(
            expected, actual,
            "lending_pool_force_tokenless_repay_complete optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::LendingPoolHandleBankruptcy {
                group: key(100),
                signer: key(101),
                bank: key(102),
                marginfi_account: key(103),
                liquidity_vault: key(104),
                insurance_vault: key(105),
                insurance_vault_authority: key(106),
                token_program: key(107),
                instruction_sysvar: key(108),
            }
            .to_account_metas(None),
            data: marginfi::instruction::LendingPoolHandleBankruptcy {}.data(),
        };
        let actual = marginfi_type_crate::ix_builders::pool::lending_pool_handle_bankruptcy(
            &marginfi_type_crate::ix_builders::pool::LendingPoolHandleBankruptcy {
                group: key(100),
                signer: key(101),
                bank: key(102),
                marginfi_account: key(103),
                liquidity_vault: key(104),
                insurance_vault: key(105),
                insurance_vault_authority: key(106),
                token_program: key(107),
                instruction_sysvar: key(108),
            },
        );
        assert_eq!(
            expected, actual,
            "lending_pool_handle_bankruptcy optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::LendingPoolInitSameAssetEmodeRegistry {
                group: key(100),
                governance_admin: key(101),
                same_asset_emode_registry: key(102),
                system_program: key(103),
                instruction_sysvar: key(104),
            }
            .to_account_metas(None),
            data: marginfi::instruction::LendingPoolInitSameAssetEmodeRegistry {}.data(),
        };
        let actual =
            marginfi_type_crate::ix_builders::pool::lending_pool_init_same_asset_emode_registry(
                &marginfi_type_crate::ix_builders::pool::LendingPoolInitSameAssetEmodeRegistry {
                    group: key(100),
                    governance_admin: key(101),
                    same_asset_emode_registry: key(102),
                    system_program: key(103),
                    instruction_sysvar: key(104),
                },
            );
        assert_eq!(
            expected, actual,
            "lending_pool_init_same_asset_emode_registry optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::LendingPoolPulseBankPriceCache {
                group: key(100),
                bank: key(101),
            }
            .to_account_metas(None),
            data: marginfi::instruction::LendingPoolPulseBankPriceCache {}.data(),
        };
        let actual = marginfi_type_crate::ix_builders::pool::lending_pool_pulse_bank_price_cache(
            &marginfi_type_crate::ix_builders::pool::LendingPoolPulseBankPriceCache {
                group: key(100),
                bank: key(101),
            },
        );
        assert_eq!(
            expected, actual,
            "lending_pool_pulse_bank_price_cache optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::LendingPoolResizeBankAccount {
                bank: key(100),
                payer: key(101),
                system_program: key(102),
            }
            .to_account_metas(None),
            data: marginfi::instruction::LendingPoolResizeBankAccount {}.data(),
        };
        let actual = marginfi_type_crate::ix_builders::pool::lending_pool_resize_bank_account(
            &marginfi_type_crate::ix_builders::pool::LendingPoolResizeBankAccount {
                bank: key(100),
                payer: key(101),
                system_program: key(102),
            },
        );
        assert_eq!(
            expected, actual,
            "lending_pool_resize_bank_account optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::LendingPoolResizeGroupAccount {
                group: key(100),
                payer: key(101),
                system_program: key(102),
            }
            .to_account_metas(None),
            data: marginfi::instruction::LendingPoolResizeGroupAccount {}.data(),
        };
        let actual = marginfi_type_crate::ix_builders::pool::lending_pool_resize_group_account(
            &marginfi_type_crate::ix_builders::pool::LendingPoolResizeGroupAccount {
                group: key(100),
                payer: key(101),
                system_program: key(102),
            },
        );
        assert_eq!(
            expected, actual,
            "lending_pool_resize_group_account optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::LendingPoolSetBankSameAssetEmodeEligibility {
                group: key(100),
                governance_admin: key(101),
                bank: key(102),
                same_asset_emode_registry: key(103),
                instruction_sysvar: key(104),
            }
            .to_account_metas(None),
            data: marginfi::instruction::LendingPoolSetBankSameAssetEmodeEligibility {
                enabled: true,
            }
            .data(),
        };
        let actual = marginfi_type_crate::ix_builders::pool::lending_pool_set_bank_same_asset_emode_eligibility(&marginfi_type_crate::ix_builders::pool::LendingPoolSetBankSameAssetEmodeEligibility { group: key(100), governance_admin: key(101), bank: key(102), same_asset_emode_registry: key(103), instruction_sysvar: key(104) }, true);
        assert_eq!(
            expected, actual,
            "lending_pool_set_bank_same_asset_emode_eligibility optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::LendingPoolSetOraclePrice {
                group: key(100),
                governance_admin: key(101),
                bank: key(102),
                instruction_sysvar: key(103),
            }
            .to_account_metas(None),
            data: marginfi::instruction::LendingPoolSetOraclePrice {
                price: wi(20),
                setup: 21u8,
            }
            .data(),
        };
        let actual = marginfi_type_crate::ix_builders::pool::lending_pool_set_oracle_price(
            &marginfi_type_crate::ix_builders::pool::LendingPoolSetOraclePrice {
                group: key(100),
                governance_admin: key(101),
                bank: key(102),
                instruction_sysvar: key(103),
            },
            wi(20),
            21u8,
        );
        assert_eq!(
            expected, actual,
            "lending_pool_set_oracle_price optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::LendingPoolUpdateFeesDestinationAccount {
                group: key(100),
                bank: key(101),
                admin: key(102),
                destination_account: key(103),
                instruction_sysvar: key(104),
            }
            .to_account_metas(None),
            data: marginfi::instruction::LendingPoolUpdateFeesDestinationAccount {}.data(),
        };
        let actual =
            marginfi_type_crate::ix_builders::pool::lending_pool_update_fees_destination_account(
                &marginfi_type_crate::ix_builders::pool::LendingPoolUpdateFeesDestinationAccount {
                    group: key(100),
                    bank: key(101),
                    admin: key(102),
                    destination_account: key(103),
                    instruction_sysvar: key(104),
                },
            );
        assert_eq!(
            expected, actual,
            "lending_pool_update_fees_destination_account optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::LendingPoolWithdrawFees {
                group: key(100),
                bank: key(101),
                admin: key(102),
                fee_vault: key(103),
                fee_vault_authority: key(104),
                dst_token_account: key(105),
                token_program: key(106),
                instruction_sysvar: key(107),
            }
            .to_account_metas(None),
            data: marginfi::instruction::LendingPoolWithdrawFees { amount: 20u64 }.data(),
        };
        let actual = marginfi_type_crate::ix_builders::pool::lending_pool_withdraw_fees(
            &marginfi_type_crate::ix_builders::pool::LendingPoolWithdrawFees {
                group: key(100),
                bank: key(101),
                admin: key(102),
                fee_vault: key(103),
                fee_vault_authority: key(104),
                dst_token_account: key(105),
                token_program: key(106),
                instruction_sysvar: key(107),
            },
            20u64,
        );
        assert_eq!(
            expected, actual,
            "lending_pool_withdraw_fees optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::LendingPoolWithdrawFeesPermissionless {
                group: key(100),
                bank: key(101),
                fee_vault: key(102),
                fee_vault_authority: key(103),
                fees_destination_account: key(104),
                token_program: key(105),
            }
            .to_account_metas(None),
            data: marginfi::instruction::LendingPoolWithdrawFeesPermissionless { amount: 20u64 }
                .data(),
        };
        let actual =
            marginfi_type_crate::ix_builders::pool::lending_pool_withdraw_fees_permissionless(
                &marginfi_type_crate::ix_builders::pool::LendingPoolWithdrawFeesPermissionless {
                    group: key(100),
                    bank: key(101),
                    fee_vault: key(102),
                    fee_vault_authority: key(103),
                    fees_destination_account: key(104),
                    token_program: key(105),
                },
                20u64,
            );
        assert_eq!(
            expected, actual,
            "lending_pool_withdraw_fees_permissionless optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::LendingPoolWithdrawInsurance {
                group: key(100),
                bank: key(101),
                admin: key(102),
                insurance_vault: key(103),
                insurance_vault_authority: key(104),
                dst_token_account: key(105),
                token_program: key(106),
                instruction_sysvar: key(107),
            }
            .to_account_metas(None),
            data: marginfi::instruction::LendingPoolWithdrawInsurance { amount: 20u64 }.data(),
        };
        let actual = marginfi_type_crate::ix_builders::pool::lending_pool_withdraw_insurance(
            &marginfi_type_crate::ix_builders::pool::LendingPoolWithdrawInsurance {
                group: key(100),
                bank: key(101),
                admin: key(102),
                insurance_vault: key(103),
                insurance_vault_authority: key(104),
                dst_token_account: key(105),
                token_program: key(106),
                instruction_sysvar: key(107),
            },
            20u64,
        );
        assert_eq!(
            expected, actual,
            "lending_pool_withdraw_insurance optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::MarginfiAccountClose {
                marginfi_account: key(100),
                authority: key(101),
                fee_payer: key(102),
                rebalance_fee_pool: key(103),
            }
            .to_account_metas(None),
            data: marginfi::instruction::MarginfiAccountClose {}.data(),
        };
        let actual = marginfi_type_crate::ix_builders::account::marginfi_account_close(
            &marginfi_type_crate::ix_builders::account::MarginfiAccountClose {
                marginfi_account: key(100),
                authority: key(101),
                fee_payer: key(102),
                rebalance_fee_pool: key(103),
            },
        );
        assert_eq!(
            expected, actual,
            "marginfi_account_close optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::CloseLiquidationRecord {
                marginfi_account: key(100),
                liquidation_record: key(101),
                record_payer: key(102),
            }
            .to_account_metas(None),
            data: marginfi::instruction::MarginfiAccountCloseLiqRecord {}.data(),
        };
        let actual = marginfi_type_crate::ix_builders::account::marginfi_account_close_liq_record(
            &marginfi_type_crate::ix_builders::account::MarginfiAccountCloseLiqRecord {
                marginfi_account: key(100),
                liquidation_record: key(101),
                record_payer: key(102),
            },
        );
        assert_eq!(
            expected, actual,
            "marginfi_account_close_liq_record optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::CloseOrder {
                group: key(100),
                marginfi_account: key(101),
                authority: key(102),
                order: key(103),
                fee_recipient: key(104),
                system_program: key(105),
            }
            .to_account_metas(None),
            data: marginfi::instruction::MarginfiAccountCloseOrder {}.data(),
        };
        let actual = marginfi_type_crate::ix_builders::order::marginfi_account_close_order(
            &marginfi_type_crate::ix_builders::order::MarginfiAccountCloseOrder {
                group: key(100),
                marginfi_account: key(101),
                authority: key(102),
                order: key(103),
                fee_recipient: key(104),
                system_program: key(105),
            },
        );
        assert_eq!(
            expected, actual,
            "marginfi_account_close_order optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::CloseRebalanceOrder {
                marginfi_account: key(100),
                authority: Some(key(101)),
                fee_recipient: key(102),
                rebalance_order: key(103),
            }
            .to_account_metas(None),
            data: marginfi::instruction::MarginfiAccountCloseRebalanceOrder {}.data(),
        };
        let actual =
            marginfi_type_crate::ix_builders::rebalance::marginfi_account_close_rebalance_order(
                &marginfi_type_crate::ix_builders::rebalance::MarginfiAccountCloseRebalanceOrder {
                    marginfi_account: key(100),
                    authority: Some(key(101)),
                    fee_recipient: key(102),
                    rebalance_order: key(103),
                },
            );
        assert_eq!(
            expected, actual,
            "marginfi_account_close_rebalance_order optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::CloseRebalanceOrder {
                marginfi_account: key(100),
                authority: None,
                fee_recipient: key(102),
                rebalance_order: key(103),
            }
            .to_account_metas(None),
            data: marginfi::instruction::MarginfiAccountCloseRebalanceOrder {}.data(),
        };
        let actual =
            marginfi_type_crate::ix_builders::rebalance::marginfi_account_close_rebalance_order(
                &marginfi_type_crate::ix_builders::rebalance::MarginfiAccountCloseRebalanceOrder {
                    marginfi_account: key(100),
                    authority: None,
                    fee_recipient: key(102),
                    rebalance_order: key(103),
                },
            );
        assert_eq!(
            expected, actual,
            "marginfi_account_close_rebalance_order optional_none=True"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::EndExecuteOrder {
                group: key(100),
                marginfi_account: key(101),
                executor: key(102),
                fee_recipient: key(103),
                order: key(104),
                execute_record: key(105),
                fee_state: key(106),
            }
            .to_account_metas(None),
            data: marginfi::instruction::MarginfiAccountEndExecuteOrder {}.data(),
        };
        let actual = marginfi_type_crate::ix_builders::order::marginfi_account_end_execute_order(
            &marginfi_type_crate::ix_builders::order::MarginfiAccountEndExecuteOrder {
                group: key(100),
                marginfi_account: key(101),
                executor: key(102),
                fee_recipient: key(103),
                order: key(104),
                execute_record: key(105),
                fee_state: key(106),
            },
        );
        assert_eq!(
            expected, actual,
            "marginfi_account_end_execute_order optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::EndRebalance {
                group: key(100),
                marginfi_account: key(101),
                rebalance_order: key(102),
                rebalance_record: key(103),
                executor: key(104),
                fee_pool: key(105),
                system_program: key(106),
            }
            .to_account_metas(None),
            data: marginfi::instruction::MarginfiAccountEndRebalance {}.data(),
        };
        let actual = marginfi_type_crate::ix_builders::rebalance::marginfi_account_end_rebalance(
            &marginfi_type_crate::ix_builders::rebalance::MarginfiAccountEndRebalance {
                group: key(100),
                marginfi_account: key(101),
                rebalance_order: key(102),
                rebalance_record: key(103),
                executor: key(104),
                fee_pool: key(105),
                system_program: key(106),
            },
        );
        assert_eq!(
            expected, actual,
            "marginfi_account_end_rebalance optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::InitLiquidationRecord {
                marginfi_account: key(100),
                fee_payer: key(101),
                liquidation_record: key(102),
                system_program: key(103),
            }
            .to_account_metas(None),
            data: marginfi::instruction::MarginfiAccountInitLiqRecord {}.data(),
        };
        let actual = marginfi_type_crate::ix_builders::account::marginfi_account_init_liq_record(
            &marginfi_type_crate::ix_builders::account::MarginfiAccountInitLiqRecord {
                marginfi_account: key(100),
                fee_payer: key(101),
                liquidation_record: key(102),
                system_program: key(103),
            },
        );
        assert_eq!(
            expected, actual,
            "marginfi_account_init_liq_record optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::MarginfiAccountInitialize {
                marginfi_group: key(100),
                marginfi_account: key(101),
                authority: key(102),
                fee_payer: key(103),
                system_program: key(104),
            }
            .to_account_metas(None),
            data: marginfi::instruction::MarginfiAccountInitialize {}.data(),
        };
        let actual = marginfi_type_crate::ix_builders::account::marginfi_account_initialize(
            &marginfi_type_crate::ix_builders::account::MarginfiAccountInitialize {
                marginfi_group: key(100),
                marginfi_account: key(101),
                authority: key(102),
                fee_payer: key(103),
                system_program: key(104),
            },
        );
        assert_eq!(
            expected, actual,
            "marginfi_account_initialize optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::MarginfiAccountInitializePda {
                marginfi_group: key(100),
                marginfi_account: key(101),
                authority: key(102),
                fee_payer: key(103),
                instructions_sysvar: key(104),
                system_program: key(105),
            }
            .to_account_metas(None),
            data: marginfi::instruction::MarginfiAccountInitializePda {
                account_index: 20u16,
                third_party_id: Some(21u16),
            }
            .data(),
        };
        let actual = marginfi_type_crate::ix_builders::account::marginfi_account_initialize_pda(
            &marginfi_type_crate::ix_builders::account::MarginfiAccountInitializePda {
                marginfi_group: key(100),
                marginfi_account: key(101),
                authority: key(102),
                fee_payer: key(103),
                instructions_sysvar: key(104),
                system_program: key(105),
            },
            20u16,
            Some(21u16),
        );
        assert_eq!(
            expected, actual,
            "marginfi_account_initialize_pda optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::KeeperCloseOrder {
                marginfi_account: key(100),
                fee_recipient: key(101),
                order: key(102),
            }
            .to_account_metas(None),
            data: marginfi::instruction::MarginfiAccountKeeperCloseOrder {}.data(),
        };
        let actual = marginfi_type_crate::ix_builders::order::marginfi_account_keeper_close_order(
            &marginfi_type_crate::ix_builders::order::MarginfiAccountKeeperCloseOrder {
                marginfi_account: key(100),
                fee_recipient: key(101),
                order: key(102),
            },
        );
        assert_eq!(
            expected, actual,
            "marginfi_account_keeper_close_order optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::PlaceOrder {
                group: key(100),
                marginfi_account: key(101),
                fee_payer: key(102),
                authority: key(103),
                order: key(104),
                fee_state: key(105),
                global_fee_wallet: key(106),
                system_program: key(107),
            }
            .to_account_metas(None),
            data: marginfi::instruction::MarginfiAccountPlaceOrder {
                bank_keys: vec![key(20)],
                trigger: OrderTrigger::Both {
                    stop_loss: wi(1),
                    take_profit: wi(2),
                    max_slippage: 23,
                },
            }
            .data(),
        };
        let actual = marginfi_type_crate::ix_builders::order::marginfi_account_place_order(
            &marginfi_type_crate::ix_builders::order::MarginfiAccountPlaceOrder {
                group: key(100),
                marginfi_account: key(101),
                fee_payer: key(102),
                authority: key(103),
                order: key(104),
                fee_state: key(105),
                global_fee_wallet: key(106),
                system_program: key(107),
            },
            vec![key(20)],
            OrderTrigger::Both {
                stop_loss: wi(1),
                take_profit: wi(2),
                max_slippage: 23,
            },
        );
        assert_eq!(
            expected, actual,
            "marginfi_account_place_order optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::PlaceRebalanceOrder {
                group: key(100),
                marginfi_account: key(101),
                authority: key(102),
                mint: key(103),
                rebalance_order: key(104),
                fee_payer: key(105),
                system_program: key(106),
            }
            .to_account_metas(None),
            data: marginfi::instruction::MarginfiAccountPlaceRebalanceOrder {
                allowed_banks: vec![key(20)],
                min_improvement: Some(wi(21)),
                cooldown_seconds: Some(22u64),
                amount: Some(23u64),
                keeper_tip: Some(24u64),
            }
            .data(),
        };
        let actual =
            marginfi_type_crate::ix_builders::rebalance::marginfi_account_place_rebalance_order(
                &marginfi_type_crate::ix_builders::rebalance::MarginfiAccountPlaceRebalanceOrder {
                    group: key(100),
                    marginfi_account: key(101),
                    authority: key(102),
                    mint: key(103),
                    rebalance_order: key(104),
                    fee_payer: key(105),
                    system_program: key(106),
                },
                vec![key(20)],
                Some(wi(21)),
                Some(22u64),
                Some(23u64),
                Some(24u64),
            );
        assert_eq!(
            expected, actual,
            "marginfi_account_place_rebalance_order optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::SetAccountFreeze {
                group: key(100),
                marginfi_account: key(101),
                admin: key(102),
                instruction_sysvar: key(103),
            }
            .to_account_metas(None),
            data: marginfi::instruction::MarginfiAccountSetFreeze { frozen: true }.data(),
        };
        let actual = marginfi_type_crate::ix_builders::account::marginfi_account_set_freeze(
            &marginfi_type_crate::ix_builders::account::MarginfiAccountSetFreeze {
                group: key(100),
                marginfi_account: key(101),
                admin: key(102),
                instruction_sysvar: key(103),
            },
            true,
        );
        assert_eq!(
            expected, actual,
            "marginfi_account_set_freeze optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::SetKeeperCloseFlags {
                group: key(100),
                marginfi_account: key(101),
                authority: key(102),
            }
            .to_account_metas(None),
            data: marginfi::instruction::MarginfiAccountSetKeeperCloseFlags {
                bank_keys_opt: Some(vec![key(20)]),
            }
            .data(),
        };
        let actual =
            marginfi_type_crate::ix_builders::order::marginfi_account_set_keeper_close_flags(
                &marginfi_type_crate::ix_builders::order::MarginfiAccountSetKeeperCloseFlags {
                    group: key(100),
                    marginfi_account: key(101),
                    authority: key(102),
                },
                Some(vec![key(20)]),
            );
        assert_eq!(
            expected, actual,
            "marginfi_account_set_keeper_close_flags optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::SettleRebalanceTip {
                group: key(100),
                marginfi_account: key(101),
                rebalance_record: key(102),
                executor: key(103),
                fee_pool: key(104),
                caller: key(105),
            }
            .to_account_metas(None),
            data: marginfi::instruction::MarginfiAccountSettleRebalanceTip {}.data(),
        };
        let actual =
            marginfi_type_crate::ix_builders::rebalance::marginfi_account_settle_rebalance_tip(
                &marginfi_type_crate::ix_builders::rebalance::MarginfiAccountSettleRebalanceTip {
                    group: key(100),
                    marginfi_account: key(101),
                    rebalance_record: key(102),
                    executor: key(103),
                    fee_pool: key(104),
                    caller: key(105),
                },
            );
        assert_eq!(
            expected, actual,
            "marginfi_account_settle_rebalance_tip optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::StartExecuteOrder {
                group: key(100),
                marginfi_account: key(101),
                fee_payer: key(102),
                executor: key(103),
                order: key(104),
                execute_record: key(105),
                instruction_sysvar: key(106),
                system_program: key(107),
            }
            .to_account_metas(None),
            data: marginfi::instruction::MarginfiAccountStartExecuteOrder {}.data(),
        };
        let actual = marginfi_type_crate::ix_builders::order::marginfi_account_start_execute_order(
            &marginfi_type_crate::ix_builders::order::MarginfiAccountStartExecuteOrder {
                group: key(100),
                marginfi_account: key(101),
                fee_payer: key(102),
                executor: key(103),
                order: key(104),
                execute_record: key(105),
                instruction_sysvar: key(106),
                system_program: key(107),
            },
        );
        assert_eq!(
            expected, actual,
            "marginfi_account_start_execute_order optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::StartRebalance {
                group: key(100),
                marginfi_account: key(101),
                rebalance_order: key(102),
                executor: key(103),
                rebalance_record: key(104),
                fee_payer: key(105),
                instruction_sysvar: key(106),
                system_program: key(107),
            }
            .to_account_metas(None),
            data: marginfi::instruction::MarginfiAccountStartRebalance {
                moves: vec![RebalanceMove {
                    src_index: 1,
                    dst_index: 2,
                    amount: wi(13),
                    ..Default::default()
                }],
                execution_seq: 21u64,
            }
            .data(),
        };
        let actual = marginfi_type_crate::ix_builders::rebalance::marginfi_account_start_rebalance(
            &marginfi_type_crate::ix_builders::rebalance::MarginfiAccountStartRebalance {
                group: key(100),
                marginfi_account: key(101),
                rebalance_order: key(102),
                executor: key(103),
                rebalance_record: key(104),
                fee_payer: key(105),
                instruction_sysvar: key(106),
                system_program: key(107),
            },
            vec![RebalanceMove {
                src_index: 1,
                dst_index: 2,
                amount: wi(13),
                ..Default::default()
            }],
            21u64,
        );
        assert_eq!(
            expected, actual,
            "marginfi_account_start_rebalance optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::TagLiquidationRecord {
                marginfi_account: key(100),
                group: key(101),
            }
            .to_account_metas(None),
            data: marginfi::instruction::MarginfiAccountTagLiqRecord {}.data(),
        };
        let actual = marginfi_type_crate::ix_builders::account::marginfi_account_tag_liq_record(
            &marginfi_type_crate::ix_builders::account::MarginfiAccountTagLiqRecord {
                marginfi_account: key(100),
                group: key(101),
            },
        );
        assert_eq!(
            expected, actual,
            "marginfi_account_tag_liq_record optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::TopUpRebalanceFeePool {
                marginfi_account: key(100),
                fee_pool: key(101),
                payer: key(102),
                system_program: key(103),
            }
            .to_account_metas(None),
            data: marginfi::instruction::MarginfiAccountTopUpRebalanceFeePool { amount: 20u64 }
                .data(),
        };
        let actual = marginfi_type_crate::ix_builders::rebalance::marginfi_account_top_up_rebalance_fee_pool(&marginfi_type_crate::ix_builders::rebalance::MarginfiAccountTopUpRebalanceFeePool { marginfi_account: key(100), fee_pool: key(101), payer: key(102), system_program: key(103) }, 20u64);
        assert_eq!(
            expected, actual,
            "marginfi_account_top_up_rebalance_fee_pool optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::MarginfiAccountUpdateEmissionsDestinationAccount {
                marginfi_account: key(100),
                authority: key(101),
                destination_account: key(102),
            }
            .to_account_metas(None),
            data: marginfi::instruction::MarginfiAccountUpdateEmissionsDestinationAccount {}.data(),
        };
        let actual = marginfi_type_crate::ix_builders::account::marginfi_account_update_emissions_destination_account(&marginfi_type_crate::ix_builders::account::MarginfiAccountUpdateEmissionsDestinationAccount { marginfi_account: key(100), authority: key(101), destination_account: key(102) });
        assert_eq!(
            expected, actual,
            "marginfi_account_update_emissions_destination_account optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::UpdateRebalanceOrder {
                marginfi_account: key(100),
                authority: key(101),
                rebalance_order: key(102),
            }
            .to_account_metas(None),
            data: marginfi::instruction::MarginfiAccountUpdateRebalanceOrder {
                allowed_banks: Some(vec![key(20)]),
                min_improvement: Some(wi(21)),
                cooldown_seconds: Some(22u64),
                amount: Some(23u64),
                keeper_tip: Some(24u64),
            }
            .data(),
        };
        let actual =
            marginfi_type_crate::ix_builders::rebalance::marginfi_account_update_rebalance_order(
                &marginfi_type_crate::ix_builders::rebalance::MarginfiAccountUpdateRebalanceOrder {
                    marginfi_account: key(100),
                    authority: key(101),
                    rebalance_order: key(102),
                },
                Some(vec![key(20)]),
                Some(wi(21)),
                Some(22u64),
                Some(23u64),
                Some(24u64),
            );
        assert_eq!(
            expected, actual,
            "marginfi_account_update_rebalance_order optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::WithdrawRebalanceFeePool {
                marginfi_account: key(100),
                authority: key(101),
                fee_pool: key(102),
                destination: key(103),
                system_program: key(104),
            }
            .to_account_metas(None),
            data: marginfi::instruction::MarginfiAccountWithdrawRebalanceFeePool { amount: 20u64 }
                .data(),
        };
        let actual = marginfi_type_crate::ix_builders::rebalance::marginfi_account_withdraw_rebalance_fee_pool(&marginfi_type_crate::ix_builders::rebalance::MarginfiAccountWithdrawRebalanceFeePool { marginfi_account: key(100), authority: key(101), fee_pool: key(102), destination: key(103), system_program: key(104) }, 20u64);
        assert_eq!(
            expected, actual,
            "marginfi_account_withdraw_rebalance_fee_pool optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::MarginfiGroupConfigure {
                marginfi_group: key(100),
                admin: key(101),
                instruction_sysvar: key(102),
            }
            .to_account_metas(None),
            data: marginfi::instruction::MarginfiGroupConfigure {
                new_admin: Some(key(20)),
                new_curve_admin: Some(key(21)),
                new_limit_admin: Some(key(22)),
                new_flow_admin: Some(key(23)),
                new_emissions_admin: Some(key(24)),
                new_metadata_admin: Some(key(25)),
            }
            .data(),
        };
        let actual = marginfi_type_crate::ix_builders::pool::marginfi_group_configure(
            &marginfi_type_crate::ix_builders::pool::MarginfiGroupConfigure {
                marginfi_group: key(100),
                admin: key(101),
                instruction_sysvar: key(102),
            },
            Some(key(20)),
            Some(key(21)),
            Some(key(22)),
            Some(key(23)),
            Some(key(24)),
            Some(key(25)),
        );
        assert_eq!(
            expected, actual,
            "marginfi_group_configure optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::MarginfiGroupConfigureGov {
                marginfi_group: key(100),
                governance_admin: key(101),
                instruction_sysvar: key(102),
            }
            .to_account_metas(None),
            data: marginfi::instruction::MarginfiGroupConfigureGov {
                new_admin: Some(key(20)),
                new_emode_admin: Some(key(21)),
                new_risk_admin: Some(key(22)),
                emode_max_init_leverage: Some(wi(23)),
                emode_max_maint_leverage: Some(wi(24)),
                same_asset_emode_init_leverage: Some(wi(25)),
                same_asset_emode_maint_leverage: Some(wi(26)),
            }
            .data(),
        };
        let actual = marginfi_type_crate::ix_builders::pool::marginfi_group_configure_gov(
            &marginfi_type_crate::ix_builders::pool::MarginfiGroupConfigureGov {
                marginfi_group: key(100),
                governance_admin: key(101),
                instruction_sysvar: key(102),
            },
            Some(key(20)),
            Some(key(21)),
            Some(key(22)),
            Some(wi(23)),
            Some(wi(24)),
            Some(wi(25)),
            Some(wi(26)),
        );
        assert_eq!(
            expected, actual,
            "marginfi_group_configure_gov optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::MarginfiGroupInitialize {
                marginfi_group: key(100),
                admin: key(101),
                fee_state: key(102),
                system_program: key(103),
                instruction_sysvar: key(104),
            }
            .to_account_metas(None),
            data: marginfi::instruction::MarginfiGroupInitialize {}.data(),
        };
        let actual = marginfi_type_crate::ix_builders::pool::marginfi_group_initialize(
            &marginfi_type_crate::ix_builders::pool::MarginfiGroupInitialize {
                marginfi_group: key(100),
                admin: key(101),
                fee_state: key(102),
                system_program: key(103),
                instruction_sysvar: key(104),
            },
        );
        assert_eq!(
            expected, actual,
            "marginfi_group_initialize optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::SetGovernanceAdmin {
                marginfi_group: key(100),
                signer: key(101),
                instruction_sysvar: key(102),
            }
            .to_account_metas(None),
            data: marginfi::instruction::MarginfiGroupSetGovernanceAdmin {
                new_governance_admin: key(20),
            }
            .data(),
        };
        let actual = marginfi_type_crate::ix_builders::pool::marginfi_group_set_governance_admin(
            &marginfi_type_crate::ix_builders::pool::MarginfiGroupSetGovernanceAdmin {
                marginfi_group: key(100),
                signer: key(101),
                instruction_sysvar: key(102),
            },
            key(20),
        );
        assert_eq!(
            expected, actual,
            "marginfi_group_set_governance_admin optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::MonitorArchiveInitialize {
                payer: key(100),
                archive: key(101),
            }
            .to_account_metas(None),
            data: marginfi::instruction::MonitorArchiveInitialize {
                snapshot_manager: key(20),
            }
            .data(),
        };
        let actual = marginfi_type_crate::ix_builders::archive::monitor_archive_initialize(
            &marginfi_type_crate::ix_builders::archive::MonitorArchiveInitialize {
                payer: key(100),
                archive: key(101),
            },
            key(20),
        );
        assert_eq!(
            expected, actual,
            "monitor_archive_initialize optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::MonitorArchiveUpsertBatch {
                snapshot_manager: key(100),
                archive: key(101),
            }
            .to_account_metas(None),
            data: marginfi::instruction::MonitorArchiveUpsertBatch {
                updates: vec![SnapshotUpdateInput {
                    snapshot_hour: 11,
                    price: 12,
                    native_apy: 13,
                }],
            }
            .data(),
        };
        let actual = marginfi_type_crate::ix_builders::archive::monitor_archive_upsert_batch(
            &marginfi_type_crate::ix_builders::archive::MonitorArchiveUpsertBatch {
                snapshot_manager: key(100),
                archive: key(101),
            },
            vec![SnapshotUpdateInput {
                snapshot_hour: 11,
                price: 12,
                native_apy: 13,
            }],
        );
        assert_eq!(
            expected, actual,
            "monitor_archive_upsert_batch optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::PanicPause {
                pause_authority: key(100),
                fee_state: key(101),
                instruction_sysvar: key(102),
            }
            .to_account_metas(None),
            data: marginfi::instruction::PanicPause {}.data(),
        };
        let actual = marginfi_type_crate::ix_builders::admin::panic_pause(
            &marginfi_type_crate::ix_builders::admin::PanicPause {
                pause_authority: key(100),
                fee_state: key(101),
                instruction_sysvar: key(102),
            },
        );
        assert_eq!(expected, actual, "panic_pause optional_none=False");
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::PanicUnpause {
                global_fee_admin: key(100),
                fee_state: key(101),
                instruction_sysvar: key(102),
            }
            .to_account_metas(None),
            data: marginfi::instruction::PanicUnpause {}.data(),
        };
        let actual = marginfi_type_crate::ix_builders::admin::panic_unpause(
            &marginfi_type_crate::ix_builders::admin::PanicUnpause {
                global_fee_admin: key(100),
                fee_state: key(101),
                instruction_sysvar: key(102),
            },
        );
        assert_eq!(expected, actual, "panic_unpause optional_none=False");
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::PanicUnpausePermissionless {
                fee_state: key(100),
            }
            .to_account_metas(None),
            data: marginfi::instruction::PanicUnpausePermissionless {}.data(),
        };
        let actual = marginfi_type_crate::ix_builders::admin::panic_unpause_permissionless(
            &marginfi_type_crate::ix_builders::admin::PanicUnpausePermissionless {
                fee_state: key(100),
            },
        );
        assert_eq!(
            expected, actual,
            "panic_unpause_permissionless optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::PropagateFee {
                fee_state: key(100),
                marginfi_group: key(101),
            }
            .to_account_metas(None),
            data: marginfi::instruction::PropagateFeeState {}.data(),
        };
        let actual = marginfi_type_crate::ix_builders::admin::propagate_fee_state(
            &marginfi_type_crate::ix_builders::admin::PropagateFeeState {
                fee_state: key(100),
                marginfi_group: key(101),
            },
        );
        assert_eq!(expected, actual, "propagate_fee_state optional_none=False");
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::PropagateKaminoMarketEmergency {
                reserve: key(100),
                lending_market: key(101),
                bank: key(102),
            }
            .to_account_metas(None),
            data: marginfi::instruction::PropagateKaminoMarketEmergency {}.data(),
        };
        let actual = marginfi_type_crate::ix_builders::kamino::propagate_kamino_market_emergency(
            &marginfi_type_crate::ix_builders::kamino::PropagateKaminoMarketEmergency {
                reserve: key(100),
                lending_market: key(101),
                bank: key(102),
            },
        );
        assert_eq!(
            expected, actual,
            "propagate_kamino_market_emergency optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::PropagateStakedSettings {
                marginfi_group: key(100),
                staked_settings: key(101),
                bank: key(102),
            }
            .to_account_metas(None),
            data: marginfi::instruction::PropagateStakedSettings {}.data(),
        };
        let actual = marginfi_type_crate::ix_builders::admin::propagate_staked_settings(
            &marginfi_type_crate::ix_builders::admin::PropagateStakedSettings {
                marginfi_group: key(100),
                staked_settings: key(101),
                bank: key(102),
            },
        );
        assert_eq!(
            expected, actual,
            "propagate_staked_settings optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::LendingAccountPurgeDelevBalance {
                group: key(100),
                marginfi_account: key(101),
                risk_admin: key(102),
                bank: key(103),
                instruction_sysvar: key(104),
            }
            .to_account_metas(None),
            data: marginfi::instruction::PurgeDeleverageBalance {}.data(),
        };
        let actual = marginfi_type_crate::ix_builders::admin::purge_deleverage_balance(
            &marginfi_type_crate::ix_builders::admin::PurgeDeleverageBalance {
                group: key(100),
                marginfi_account: key(101),
                risk_admin: key(102),
                bank: key(103),
                instruction_sysvar: key(104),
            },
        );
        assert_eq!(
            expected, actual,
            "purge_deleverage_balance optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::ResizeGlobalFeeState {
                fee_state: key(100),
                payer: key(101),
                system_program: key(102),
            }
            .to_account_metas(None),
            data: marginfi::instruction::ResizeGlobalFeeState {}.data(),
        };
        let actual = marginfi_type_crate::ix_builders::admin::resize_global_fee_state(
            &marginfi_type_crate::ix_builders::admin::ResizeGlobalFeeState {
                fee_state: key(100),
                payer: key(101),
                system_program: key(102),
            },
        );
        assert_eq!(
            expected, actual,
            "resize_global_fee_state optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::SolendDeposit {
                group: key(100),
                marginfi_account: key(101),
                authority: key(102),
                bank: key(103),
                signer_token_account: key(104),
                liquidity_vault_authority: key(105),
                liquidity_vault: key(106),
                integration_acc_2: key(107),
                lending_market: key(108),
                lending_market_authority: key(109),
                integration_acc_1: key(110),
                mint: key(111),
                reserve_liquidity_supply: key(112),
                reserve_collateral_mint: key(113),
                reserve_collateral_supply: key(114),
                user_collateral: key(115),
                pyth_price: key(116),
                switchboard_feed: key(117),
                solend_program: key(118),
                token_program: key(119),
            }
            .to_account_metas(None),
            data: marginfi::instruction::SolendDeposit { amount: 20u64 }.data(),
        };
        let actual = marginfi_type_crate::ix_builders::solend::solend_deposit(
            &marginfi_type_crate::ix_builders::solend::SolendDeposit {
                group: key(100),
                marginfi_account: key(101),
                authority: key(102),
                bank: key(103),
                signer_token_account: key(104),
                liquidity_vault_authority: key(105),
                liquidity_vault: key(106),
                integration_acc_2: key(107),
                lending_market: key(108),
                lending_market_authority: key(109),
                integration_acc_1: key(110),
                mint: key(111),
                reserve_liquidity_supply: key(112),
                reserve_collateral_mint: key(113),
                reserve_collateral_supply: key(114),
                user_collateral: key(115),
                pyth_price: key(116),
                switchboard_feed: key(117),
                solend_program: key(118),
                token_program: key(119),
            },
            20u64,
        );
        assert_eq!(expected, actual, "solend_deposit optional_none=False");
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::SolendInitObligation {
                fee_payer: key(100),
                bank: key(101),
                signer_token_account: key(102),
                liquidity_vault_authority: key(103),
                liquidity_vault: key(104),
                integration_acc_2: key(105),
                lending_market: key(106),
                lending_market_authority: key(107),
                integration_acc_1: key(108),
                mint: key(109),
                reserve_liquidity_supply: key(110),
                reserve_collateral_mint: key(111),
                reserve_collateral_supply: key(112),
                user_collateral: key(113),
                pyth_price: key(114),
                switchboard_feed: key(115),
                solend_program: key(116),
                token_program: key(117),
                rent: key(118),
                system_program: key(119),
            }
            .to_account_metas(None),
            data: marginfi::instruction::SolendInitObligation { amount: 20u64 }.data(),
        };
        let actual = marginfi_type_crate::ix_builders::solend::solend_init_obligation(
            &marginfi_type_crate::ix_builders::solend::SolendInitObligation {
                fee_payer: key(100),
                bank: key(101),
                signer_token_account: key(102),
                liquidity_vault_authority: key(103),
                liquidity_vault: key(104),
                integration_acc_2: key(105),
                lending_market: key(106),
                lending_market_authority: key(107),
                integration_acc_1: key(108),
                mint: key(109),
                reserve_liquidity_supply: key(110),
                reserve_collateral_mint: key(111),
                reserve_collateral_supply: key(112),
                user_collateral: key(113),
                pyth_price: key(114),
                switchboard_feed: key(115),
                solend_program: key(116),
                token_program: key(117),
                rent: key(118),
                system_program: key(119),
            },
            20u64,
        );
        assert_eq!(
            expected, actual,
            "solend_init_obligation optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::SolendWithdraw {
                group: key(100),
                marginfi_account: key(101),
                authority: key(102),
                bank: key(103),
                destination_token_account: key(104),
                liquidity_vault_authority: key(105),
                liquidity_vault: key(106),
                integration_acc_2: key(107),
                lending_market: key(108),
                lending_market_authority: key(109),
                integration_acc_1: key(110),
                mint: key(111),
                reserve_liquidity_supply: key(112),
                reserve_collateral_mint: key(113),
                reserve_collateral_supply: key(114),
                user_collateral: key(115),
                solend_program: key(116),
                token_program: key(117),
            }
            .to_account_metas(None),
            data: marginfi::instruction::SolendWithdraw {
                amount: 20u64,
                withdraw_all: Some(true),
            }
            .data(),
        };
        let actual = marginfi_type_crate::ix_builders::solend::solend_withdraw(
            &marginfi_type_crate::ix_builders::solend::SolendWithdraw {
                group: key(100),
                marginfi_account: key(101),
                authority: key(102),
                bank: key(103),
                destination_token_account: key(104),
                liquidity_vault_authority: key(105),
                liquidity_vault: key(106),
                integration_acc_2: key(107),
                lending_market: key(108),
                lending_market_authority: key(109),
                integration_acc_1: key(110),
                mint: key(111),
                reserve_liquidity_supply: key(112),
                reserve_collateral_mint: key(113),
                reserve_collateral_supply: key(114),
                user_collateral: key(115),
                solend_program: key(116),
                token_program: key(117),
            },
            20u64,
            Some(true),
        );
        assert_eq!(expected, actual, "solend_withdraw optional_none=False");
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::StartDeleverage {
                marginfi_account: key(100),
                liquidation_record: key(101),
                group: key(102),
                risk_admin: key(103),
                instruction_sysvar: key(104),
            }
            .to_account_metas(None),
            data: marginfi::instruction::StartDeleverage {}.data(),
        };
        let actual = marginfi_type_crate::ix_builders::liquidation::start_deleverage(
            &marginfi_type_crate::ix_builders::liquidation::StartDeleverage {
                marginfi_account: key(100),
                liquidation_record: key(101),
                group: key(102),
                risk_admin: key(103),
                instruction_sysvar: key(104),
            },
        );
        assert_eq!(expected, actual, "start_deleverage optional_none=False");
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::StartLiquidation {
                marginfi_account: key(100),
                liquidation_record: key(101),
                group: key(102),
                liquidation_receiver: key(103),
                instruction_sysvar: key(104),
            }
            .to_account_metas(None),
            data: marginfi::instruction::StartLiquidation {}.data(),
        };
        let actual = marginfi_type_crate::ix_builders::liquidation::start_liquidation(
            &marginfi_type_crate::ix_builders::liquidation::StartLiquidation {
                marginfi_account: key(100),
                liquidation_record: key(101),
                group: key(102),
                liquidation_receiver: key(103),
                instruction_sysvar: key(104),
            },
        );
        assert_eq!(expected, actual, "start_liquidation optional_none=False");
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::SuperAdminDeposit {
                group: key(100),
                admin: key(101),
                bank: key(102),
                admin_token_account: key(103),
                liquidity_vault: key(104),
                token_program: key(105),
                instruction_sysvar: key(106),
            }
            .to_account_metas(None),
            data: marginfi::instruction::SuperAdminDeposit { amount: 20u64 }.data(),
        };
        let actual = marginfi_type_crate::ix_builders::admin::super_admin_deposit(
            &marginfi_type_crate::ix_builders::admin::SuperAdminDeposit {
                group: key(100),
                admin: key(101),
                bank: key(102),
                admin_token_account: key(103),
                liquidity_vault: key(104),
                token_program: key(105),
                instruction_sysvar: key(106),
            },
            20u64,
        );
        assert_eq!(expected, actual, "super_admin_deposit optional_none=False");
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::SuperAdminWithdraw {
                group: key(100),
                admin: key(101),
                bank: key(102),
                destination_token_account: key(103),
                liquidity_vault_authority: key(104),
                liquidity_vault: key(105),
                token_program: key(106),
                instruction_sysvar: key(107),
            }
            .to_account_metas(None),
            data: marginfi::instruction::SuperAdminWithdraw { amount: 20u64 }.data(),
        };
        let actual = marginfi_type_crate::ix_builders::admin::super_admin_withdraw(
            &marginfi_type_crate::ix_builders::admin::SuperAdminWithdraw {
                group: key(100),
                admin: key(101),
                bank: key(102),
                destination_token_account: key(103),
                liquidity_vault_authority: key(104),
                liquidity_vault: key(105),
                token_program: key(106),
                instruction_sysvar: key(107),
            },
            20u64,
        );
        assert_eq!(expected, actual, "super_admin_withdraw optional_none=False");
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::SyncIndexerFlags { payer: key(100) }
                .to_account_metas(None),
            data: marginfi::instruction::SyncIndexerFlags {}.data(),
        };
        let actual = marginfi_type_crate::ix_builders::pool::sync_indexer_flags(
            &marginfi_type_crate::ix_builders::pool::SyncIndexerFlags { payer: key(100) },
        );
        assert_eq!(expected, actual, "sync_indexer_flags optional_none=False");
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::TransferToNewAccount {
                group: key(100),
                old_marginfi_account: key(101),
                new_marginfi_account: key(102),
                authority: key(103),
                fee_payer: key(104),
                new_authority: key(105),
                global_fee_wallet: key(106),
                fee_state: key(107),
                system_program: key(108),
            }
            .to_account_metas(None),
            data: marginfi::instruction::TransferToNewAccount {}.data(),
        };
        let actual = marginfi_type_crate::ix_builders::account::transfer_to_new_account(
            &marginfi_type_crate::ix_builders::account::TransferToNewAccount {
                group: key(100),
                old_marginfi_account: key(101),
                new_marginfi_account: key(102),
                authority: key(103),
                fee_payer: key(104),
                new_authority: key(105),
                global_fee_wallet: key(106),
                fee_state: key(107),
                system_program: key(108),
            },
        );
        assert_eq!(
            expected, actual,
            "transfer_to_new_account optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::TransferToNewAccountPda {
                group: key(100),
                old_marginfi_account: key(101),
                new_marginfi_account: key(102),
                authority: key(103),
                fee_payer: key(104),
                new_authority: key(105),
                global_fee_wallet: key(106),
                fee_state: key(107),
                instructions_sysvar: key(108),
                system_program: key(109),
            }
            .to_account_metas(None),
            data: marginfi::instruction::TransferToNewAccountPda {
                account_index: 20u16,
                third_party_id: Some(21u16),
            }
            .data(),
        };
        let actual = marginfi_type_crate::ix_builders::account::transfer_to_new_account_pda(
            &marginfi_type_crate::ix_builders::account::TransferToNewAccountPda {
                group: key(100),
                old_marginfi_account: key(101),
                new_marginfi_account: key(102),
                authority: key(103),
                fee_payer: key(104),
                new_authority: key(105),
                global_fee_wallet: key(106),
                fee_state: key(107),
                instructions_sysvar: key(108),
                system_program: key(109),
            },
            20u16,
            Some(21u16),
        );
        assert_eq!(
            expected, actual,
            "transfer_to_new_account_pda optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::UpdateDeleverageWithdrawals {
                marginfi_group: key(100),
                delegate_flow_admin: key(101),
                instruction_sysvar: key(102),
            }
            .to_account_metas(None),
            data: marginfi::instruction::UpdateDeleverageWithdrawals {
                outflow_usd: 20u32,
                update_seq: 21u64,
                event_start_slot: 22u64,
                event_end_slot: 23u64,
            }
            .data(),
        };
        let actual = marginfi_type_crate::ix_builders::admin::update_deleverage_withdrawals(
            &marginfi_type_crate::ix_builders::admin::UpdateDeleverageWithdrawals {
                marginfi_group: key(100),
                delegate_flow_admin: key(101),
                instruction_sysvar: key(102),
            },
            20u32,
            21u64,
            22u64,
            23u64,
        );
        assert_eq!(
            expected, actual,
            "update_deleverage_withdrawals optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::UpdateGroupRateLimiter {
                marginfi_group: key(100),
                delegate_flow_admin: key(101),
                instruction_sysvar: key(102),
            }
            .to_account_metas(None),
            data: marginfi::instruction::UpdateGroupRateLimiter {
                outflow_usd: Some(20u64),
                inflow_usd: Some(21u64),
                update_seq: 22u64,
                event_start_slot: 23u64,
                event_end_slot: 24u64,
            }
            .data(),
        };
        let actual = marginfi_type_crate::ix_builders::admin::update_group_rate_limiter(
            &marginfi_type_crate::ix_builders::admin::UpdateGroupRateLimiter {
                marginfi_group: key(100),
                delegate_flow_admin: key(101),
                instruction_sysvar: key(102),
            },
            Some(20u64),
            Some(21u64),
            22u64,
            23u64,
            24u64,
        );
        assert_eq!(
            expected, actual,
            "update_group_rate_limiter optional_none=False"
        );
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::WriteBankMetadata {
                group: key(100),
                bank: key(101),
                metadata_admin: key(102),
                metadata: key(103),
                instruction_sysvar: key(104),
            }
            .to_account_metas(None),
            data: marginfi::instruction::WriteBankMetadata {
                ticker: Some(vec![3, 4, 5]),
                description: Some(vec![3, 4, 5]),
            }
            .data(),
        };
        let actual = marginfi_type_crate::ix_builders::pool::write_bank_metadata(
            &marginfi_type_crate::ix_builders::pool::WriteBankMetadata {
                group: key(100),
                bank: key(101),
                metadata_admin: key(102),
                metadata: key(103),
                instruction_sysvar: key(104),
            },
            Some(vec![3, 4, 5]),
            Some(vec![3, 4, 5]),
        );
        assert_eq!(expected, actual, "write_bank_metadata optional_none=False");
    }
    {
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::WriteBankMetadataPreInit {
                group: key(100),
                bank_mint: key(101),
                bank: key(102),
                metadata_admin: key(103),
                metadata: key(104),
                instruction_sysvar: key(105),
            }
            .to_account_metas(None),
            data: marginfi::instruction::WriteBankMetadataPreInit {
                bank_seed: 20u64,
                ticker: Some(vec![3, 4, 5]),
                description: Some(vec![3, 4, 5]),
            }
            .data(),
        };
        let actual = marginfi_type_crate::ix_builders::pool::write_bank_metadata_pre_init(
            &marginfi_type_crate::ix_builders::pool::WriteBankMetadataPreInit {
                group: key(100),
                bank_mint: key(101),
                bank: key(102),
                metadata_admin: key(103),
                metadata: key(104),
                instruction_sysvar: key(105),
            },
            20u64,
            Some(vec![3, 4, 5]),
            Some(vec![3, 4, 5]),
        );
        assert_eq!(
            expected, actual,
            "write_bank_metadata_pre_init optional_none=False"
        );
    }
}

#[test]
fn all_idl_instructions_have_builders() {
    let idl_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../target/idl/marginfi.json"
    );
    let Ok(data) = std::fs::read_to_string(idl_path) else {
        assert!(
            std::env::var_os("CI").is_none(),
            "fresh marginfi IDL required in CI: {idl_path}"
        );
        return;
    };
    let idl: serde_json::Value = serde_json::from_str(&data).unwrap();
    let names: std::collections::BTreeSet<_> = idl["instructions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["name"].as_str().unwrap())
        .collect();
    let builders: std::collections::BTreeSet<_> = marginfi_type_crate::ix_builders::BUILDERS
        .iter()
        .copied()
        .collect();
    assert_eq!(names, builders);
    let tested: std::collections::BTreeSet<_> = [
        "admin_close_account",
        "config_group_fee",
        "configure_bank_rate_limits",
        "configure_deleverage_withdrawal_limit",
        "configure_group_rate_limits",
        "disable_staked_oracles",
        "drift_claim_bad_debt",
        "drift_deposit",
        "drift_harvest_reward",
        "drift_init_user",
        "drift_withdraw",
        "edit_fee_state_premium",
        "edit_global_fee_state",
        "edit_staked_settings",
        "enable_staked_oracle_onramp",
        "end_deleverage",
        "end_liquidation",
        "init_bank_metadata",
        "init_global_fee_state",
        "init_staked_settings",
        "juplend_deposit",
        "juplend_init_position",
        "juplend_withdraw",
        "kamino_deposit",
        "kamino_harvest_reward",
        "kamino_init_obligation",
        "kamino_withdraw",
        "lending_account_borrow",
        "lending_account_close_balance",
        "lending_account_deposit",
        "lending_account_end_flashloan",
        "lending_account_liquidate",
        "lending_account_pulse_health",
        "lending_account_repay",
        "lending_account_start_flashloan",
        "lending_account_withdraw",
        "lending_pool_accrue_bank_interest",
        "lending_pool_add_bank",
        "lending_pool_add_bank_drift",
        "lending_pool_add_bank_juplend",
        "lending_pool_add_bank_kamino",
        "lending_pool_add_bank_permissionless",
        "lending_pool_add_bank_solend",
        "lending_pool_add_bank_with_seed",
        "lending_pool_backfill_bank_is_t22_flag",
        "lending_pool_backfill_staked_bank_validator_vote_account",
        "lending_pool_clear_circuit_breaker",
        "lending_pool_clone_bank",
        "lending_pool_clone_emode",
        "lending_pool_close_bank",
        "lending_pool_collect_bank_fees",
        "lending_pool_collect_bank_premium_fees",
        "lending_pool_configure_bank",
        "lending_pool_configure_bank_emode",
        "lending_pool_configure_bank_gov",
        "lending_pool_configure_bank_interest_only",
        "lending_pool_configure_bank_limits_only",
        "lending_pool_configure_bank_oracle",
        "lending_pool_configure_bank_oracle_scope",
        "lending_pool_configure_bank_premium",
        "lending_pool_configure_group_premium",
        "lending_pool_emissions_deposit",
        "lending_pool_force_tokenless_repay_complete",
        "lending_pool_handle_bankruptcy",
        "lending_pool_init_same_asset_emode_registry",
        "lending_pool_pulse_bank_price_cache",
        "lending_pool_resize_bank_account",
        "lending_pool_resize_group_account",
        "lending_pool_set_bank_same_asset_emode_eligibility",
        "lending_pool_set_oracle_price",
        "lending_pool_update_fees_destination_account",
        "lending_pool_withdraw_fees",
        "lending_pool_withdraw_fees_permissionless",
        "lending_pool_withdraw_insurance",
        "marginfi_account_close",
        "marginfi_account_close_liq_record",
        "marginfi_account_close_order",
        "marginfi_account_close_rebalance_order",
        "marginfi_account_end_execute_order",
        "marginfi_account_end_rebalance",
        "marginfi_account_init_liq_record",
        "marginfi_account_initialize",
        "marginfi_account_initialize_pda",
        "marginfi_account_keeper_close_order",
        "marginfi_account_place_order",
        "marginfi_account_place_rebalance_order",
        "marginfi_account_set_freeze",
        "marginfi_account_set_keeper_close_flags",
        "marginfi_account_settle_rebalance_tip",
        "marginfi_account_start_execute_order",
        "marginfi_account_start_rebalance",
        "marginfi_account_tag_liq_record",
        "marginfi_account_top_up_rebalance_fee_pool",
        "marginfi_account_update_emissions_destination_account",
        "marginfi_account_update_rebalance_order",
        "marginfi_account_withdraw_rebalance_fee_pool",
        "marginfi_group_configure",
        "marginfi_group_configure_gov",
        "marginfi_group_initialize",
        "marginfi_group_set_governance_admin",
        "monitor_archive_initialize",
        "monitor_archive_upsert_batch",
        "panic_pause",
        "panic_unpause",
        "panic_unpause_permissionless",
        "propagate_fee_state",
        "propagate_kamino_market_emergency",
        "propagate_staked_settings",
        "purge_deleverage_balance",
        "resize_global_fee_state",
        "solend_deposit",
        "solend_init_obligation",
        "solend_withdraw",
        "start_deleverage",
        "start_liquidation",
        "super_admin_deposit",
        "super_admin_withdraw",
        "sync_indexer_flags",
        "transfer_to_new_account",
        "transfer_to_new_account_pda",
        "update_deleverage_withdrawals",
        "update_group_rate_limiter",
        "write_bank_metadata",
        "write_bank_metadata_pre_init",
    ]
    .into_iter()
    .collect();
    assert_eq!(tested, builders, "each builder must have a parity case");
}

#[test]
fn runtime_program_id_only_changes_optional_sentinels() {
    use marginfi_type_crate::ix_builders::liquidation::*;
    let old = marginfi_type_crate::ID;
    let new = key(9999);
    let accounts = EndLiquidation {
        marginfi_account: old,
        liquidation_record: key(2),
        group: key(3),
        liquidation_receiver: key(4),
        fee_state: key(5),
        global_fee_wallet: key(6),
        system_program: key(7),
        fee_payer: None,
    };
    let ix = end_liquidation_with_program_id(new, &accounts);
    assert_eq!(ix.program_id, new);
    assert_eq!(ix.accounts[0].pubkey, old);
    assert_eq!(ix.accounts.last().unwrap().pubkey, new);
}
