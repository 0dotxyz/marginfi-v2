#[cfg(test)]
mod tests {
    use anchor_lang::AnchorDeserialize;
    use fixed::types::I80F48;
    use marginfi::state::marginfi_account::{
        get_health_components, HealthPriceMode, RiskRequirementType,
    };
    use marginfi_type_crate::types::MarginfiGroup;
    use pyth_solana_receiver_sdk::price_update::PriceUpdateV2;

    use crate::utils::account_info_slice_lifetime_shortener as aisls;
    use crate::{
        arbitrary_helpers::{AccountIdx, AssetAmount, BankAndOracleConfig, BankIdx, PriceChange},
        MarginfiFuzzContext,
    };
    use anchor_lang::prelude::AccountLoader;
    use marginfi_type_crate::types::MarginfiAccount;

    #[test]
    fn deposit_test() {
        let account_state = crate::account_state::AccountsState::new();

        let a = MarginfiFuzzContext::setup(&account_state, &[BankAndOracleConfig::dummy(); 2], 2);

        let al =
            AccountLoader::<MarginfiGroup>::try_from_unchecked(&marginfi::ID, &a.marginfi_group)
                .unwrap();

        assert_eq!(al.load().unwrap().admin, a.admin.key());

        a.process_action_deposit(&AccountIdx(0), &BankIdx(0), &AssetAmount(1000), None)
            .unwrap();

        let marginfi_account_ai = AccountLoader::<MarginfiAccount>::try_from_unchecked(
            &marginfi::ID,
            &a.marginfi_accounts[0].margin_account,
        )
        .unwrap();
        let marginfi_account = marginfi_account_ai.load().unwrap();

        assert_eq!(
            I80F48::from(marginfi_account.lending_account.balances[0].asset_shares),
            I80F48!(1000)
        );
    }

    #[test]
    fn borrow_test() {
        let account_state = crate::account_state::AccountsState::new();
        let a = MarginfiFuzzContext::setup(&account_state, &[BankAndOracleConfig::dummy(); 2], 2);

        a.process_action_deposit(&AccountIdx(1), &BankIdx(1), &AssetAmount(1000), None)
            .unwrap();
        a.process_action_deposit(&AccountIdx(0), &BankIdx(0), &AssetAmount(1000), None)
            .unwrap();
        a.process_action_borrow(&AccountIdx(0), &BankIdx(1), &AssetAmount(100))
            .unwrap();

        let marginfi_account_ai = AccountLoader::<MarginfiAccount>::try_from_unchecked(
            &marginfi::ID,
            &a.marginfi_accounts[0].margin_account,
        )
        .unwrap();

        {
            let marginfi_account = marginfi_account_ai.load().unwrap();

            assert_eq!(
                I80F48::from(marginfi_account.lending_account.balances[0].asset_shares),
                I80F48!(1000)
            );
            assert_eq!(
                I80F48::from(marginfi_account.lending_account.balances[1].liability_shares),
                I80F48!(100)
            );
        }

        a.process_action_repay(&AccountIdx(0), &BankIdx(1), &AssetAmount(100), false)
            .unwrap();

        let marginfi_account = marginfi_account_ai.load().unwrap();

        assert_eq!(
            I80F48::from(marginfi_account.lending_account.balances[1].liability_shares),
            I80F48!(0)
        );
    }

    #[test]
    fn liquidation_test() {
        let account_state = crate::account_state::AccountsState::new();
        let a = MarginfiFuzzContext::setup(&account_state, &[BankAndOracleConfig::dummy(); 2], 3);

        a.process_action_deposit(&AccountIdx(1), &BankIdx(1), &AssetAmount(1000), None)
            .unwrap();
        a.process_action_deposit(&AccountIdx(0), &BankIdx(0), &AssetAmount(1000), None)
            .unwrap();
        a.process_action_borrow(&AccountIdx(0), &BankIdx(1), &AssetAmount(500))
            .unwrap();

        a.banks[1].log_oracle_price().unwrap();

        a.process_update_oracle(&BankIdx(1), &PriceChange(10000000000000))
            .unwrap();

        a.banks[1].log_oracle_price().unwrap();

        let marginfi_account_ai = AccountLoader::<MarginfiAccount>::try_from_unchecked(
            &marginfi::ID,
            &a.marginfi_accounts[0].margin_account,
        )
        .unwrap();

        {
            let marginfi_account = marginfi_account_ai.load().unwrap();
            let margin_account = &a.marginfi_accounts[0];
            let bank_map = a.get_bank_map();
            let remaining_accounts =
                margin_account.get_remaining_accounts(&bank_map, vec![], vec![], None);
            let group_ai = AccountLoader::<MarginfiGroup>::try_from_unchecked(
                &marginfi::ID,
                &a.marginfi_group,
            )
            .unwrap();
            let group = group_ai.load().unwrap();

            let (_assets, _liabs) = get_health_components(
                &marginfi_account,
                &group,
                aisls(&remaining_accounts),
                RiskRequirementType::Maintenance,
                &mut None,
                HealthPriceMode::Live { liq_cache: None },
            )
            .unwrap();
        }

        a.process_action_deposit(&AccountIdx(2), &BankIdx(1), &AssetAmount(1000), None)
            .unwrap();

        a.process_liquidate_account(&AccountIdx(2), &AccountIdx(0), &AssetAmount(50))
            .unwrap();

        let marginfi_account_ai = AccountLoader::<MarginfiAccount>::try_from_unchecked(
            &marginfi::ID,
            &a.marginfi_accounts[0].margin_account,
        )
        .unwrap();

        let marginfi_account = marginfi_account_ai.load().unwrap();

        assert_eq!(
            I80F48::from(marginfi_account.lending_account.balances[0].asset_shares),
            I80F48!(950)
        );
    }

    #[test]
    fn liquidation_and_bankruptcy() {
        let account_state = crate::account_state::AccountsState::new();

        let a = MarginfiFuzzContext::setup(&account_state, &[BankAndOracleConfig::dummy(); 2], 3);

        a.process_action_deposit(&AccountIdx(1), &BankIdx(1), &AssetAmount(1000), None)
            .unwrap();
        a.process_action_deposit(&AccountIdx(0), &BankIdx(0), &AssetAmount(1000), None)
            .unwrap();
        a.process_action_borrow(&AccountIdx(0), &BankIdx(1), &AssetAmount(500))
            .unwrap();

        a.process_update_oracle(&BankIdx(1), &PriceChange(1000000000000))
            .unwrap();

        let marginfi_account_ai = AccountLoader::<MarginfiAccount>::try_from_unchecked(
            &marginfi::ID,
            &a.marginfi_accounts[0].margin_account,
        )
        .unwrap();

        {
            let marginfi_account = marginfi_account_ai.load().unwrap();
            let margin_account = &a.marginfi_accounts[0];
            let bank_map = a.get_bank_map();
            let remaining_accounts =
                margin_account.get_remaining_accounts(&bank_map, vec![], vec![], None);
            let group_ai = AccountLoader::<MarginfiGroup>::try_from_unchecked(
                &marginfi::ID,
                &a.marginfi_group,
            )
            .unwrap();
            let group = group_ai.load().unwrap();

            let (_assets, _liabs) = get_health_components(
                &marginfi_account,
                &group,
                aisls(&remaining_accounts),
                RiskRequirementType::Maintenance,
                &mut None,
                HealthPriceMode::Live { liq_cache: None },
            )
            .unwrap();
        }

        a.process_action_deposit(&AccountIdx(2), &BankIdx(1), &AssetAmount(1000), None)
            .unwrap();

        a.process_liquidate_account(&AccountIdx(2), &AccountIdx(0), &AssetAmount(1000))
            .unwrap();

        let marginfi_account_ai = AccountLoader::<MarginfiAccount>::try_from_unchecked(
            &marginfi::ID,
            &a.marginfi_accounts[0].margin_account,
        )
        .unwrap();

        let marginfi_account = marginfi_account_ai.load().unwrap();

        assert_eq!(
            I80F48::from(marginfi_account.lending_account.balances[0].asset_shares),
            I80F48!(0)
        );
        assert_eq!(
            I80F48::from(marginfi_account.lending_account.balances[0].liability_shares),
            I80F48!(0)
        );
    }

    #[test]
    fn price_update() {
        let account_state = crate::account_state::AccountsState::new();

        let a = MarginfiFuzzContext::setup(&account_state, &[BankAndOracleConfig::dummy(); 2], 3);

        a.process_update_oracle(&BankIdx(0), &PriceChange(1100))
            .unwrap();

        let new_price = {
            let data = a.banks[0].oracle.try_borrow_data().unwrap();
            let price_update = PriceUpdateV2::deserialize(&mut &data[8..]).unwrap();
            price_update.price_message.ema_price
        };

        assert_eq!(new_price, 1100);
    }

    #[test]
    fn pyth_timestamp_update() {
        let account_state = crate::account_state::AccountsState::new();

        let a = MarginfiFuzzContext::setup(&account_state, &[BankAndOracleConfig::dummy(); 2], 3);

        let initial_timestamp = {
            let data = a.banks[0].oracle.try_borrow_data().unwrap();
            let price_update = PriceUpdateV2::deserialize(&mut &data[8..]).unwrap();
            price_update.price_message.publish_time
        };
        assert_eq!(initial_timestamp, 0);

        a.banks[0].refresh_oracle(123_456).unwrap();

        let updated_timestamp_via_0_10 = {
            let data = a.banks[0].oracle.try_borrow_data().unwrap();
            let price_update = PriceUpdateV2::deserialize(&mut &data[8..]).unwrap();
            price_update.price_message.publish_time
        };
        assert_eq!(updated_timestamp_via_0_10, 123_456);
    }
}
