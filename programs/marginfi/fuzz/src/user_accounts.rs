use std::collections::HashMap;

use anchor_lang::{
    prelude::{AccountInfo, AccountLoader, Pubkey},
    Key,
};
use fixed::types::I80F48;
use marginfi_type_crate::types::{user_account::MarginfiAccount, BalanceSide, Bank, OracleSetup};

use crate::{arbitrary_helpers::BankIdx, bank_accounts::BankAccounts};

pub struct UserAccount<'info> {
    pub margin_account: AccountInfo<'info>,
    pub token_accounts: Vec<AccountInfo<'info>>,
}

impl<'info> UserAccount<'info> {
    pub fn new(
        margin_account: AccountInfo<'info>,
        token_accounts: Vec<AccountInfo<'info>>,
    ) -> Self {
        Self {
            margin_account,
            token_accounts,
        }
    }

    pub fn get_liquidation_banks(
        &'info self,
        banks: &[BankAccounts],
    ) -> Option<(BankIdx, BankIdx)> {
        let marginfi_account_al =
            AccountLoader::<MarginfiAccount>::try_from(&self.margin_account).ok()?;
        let marginfi_account = marginfi_account_al.load().ok()?;

        // let bank_map = get_bank_map(banks);

        let mut asset_balances = marginfi_account
            .lending_account
            .balances
            .iter()
            .filter(|blc| !blc.is_empty(BalanceSide::Assets))
            .collect::<Vec<_>>();
        let mut liab_balances = marginfi_account
            .lending_account
            .balances
            .iter()
            .filter(|blc| !blc.is_empty(BalanceSide::Liabilities))
            .collect::<Vec<_>>();

        asset_balances
            .sort_by(|a, b| I80F48::from(a.asset_shares).cmp(&I80F48::from(b.asset_shares)));
        liab_balances.sort_by(|a, b| {
            I80F48::from(a.liability_shares).cmp(&I80F48::from(b.liability_shares))
        });

        let best_asset_bank = asset_balances.first()?.bank_pk;
        let best_liab_bank = liab_balances.first()?.bank_pk;

        let best_asset_pos = banks
            .iter()
            .position(|bank| bank.bank.key.eq(&best_asset_bank))?;
        let best_liab_pos = banks
            .iter()
            .position(|bank| bank.bank.key.eq(&best_liab_bank))?;

        Some((BankIdx(best_asset_pos as u8), BankIdx(best_liab_pos as u8)))
    }

    pub fn get_remaining_accounts(
        &'info self,
        bank_map: &HashMap<Pubkey, &'info BankAccounts<'info>>,
        include_banks: Vec<Pubkey>,
        exclude_banks: Vec<Pubkey>,
        close_bank_last: Option<Pubkey>,
    ) -> Vec<AccountInfo<'info>> {
        let marginfi_account_al =
            AccountLoader::<MarginfiAccount>::try_from(&self.margin_account).unwrap();
        let marginfi_account = marginfi_account_al.load().unwrap();

        let mut bank_pks = marginfi_account
            .lending_account
            .balances
            .iter()
            .filter_map(|balance| balance.is_active().then_some(balance.bank_pk))
            .collect::<Vec<_>>();

        for bank_pk in include_banks {
            if !bank_pks.contains(&bank_pk) {
                bank_pks.push(bank_pk);
            }
        }

        bank_pks.retain(|bank_pk| !exclude_banks.contains(bank_pk));

        // Sort all bank_pks in descending order to mirror client-side account composition.
        bank_pks.sort_by(|a, b| b.cmp(a));

        // For close-balance flows, force the closing bank's group to be last.
        if let Some(close_bank) = close_bank_last {
            if let Some(close_bank_idx) = bank_pks.iter().position(|bank_pk| *bank_pk == close_bank)
            {
                let close_bank = bank_pks.remove(close_bank_idx);
                bank_pks.push(close_bank);
            }
        }

        let ais = bank_pks
            .into_iter()
            .flat_map(|bank_pk| {
                let bank_accounts = bank_map.get(&bank_pk).unwrap();
                assert_eq!(bank_pk, bank_accounts.bank.key());

                let bank: Bank = {
                    *AccountLoader::<Bank>::try_from(&bank_accounts.bank)
                        .unwrap()
                        .load()
                        .unwrap()
                };

                // For Fixed-price banks: only bank account, otherwise: include bank and oracle
                if bank.config.oracle_setup == OracleSetup::Fixed {
                    vec![bank_accounts.bank.clone()]
                } else {
                    vec![bank_accounts.bank.clone(), bank_accounts.oracle.clone()]
                }
            })
            .collect::<Vec<_>>();

        ais
    }
}
