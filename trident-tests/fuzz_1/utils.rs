use trident_fuzz::fuzzing::*;

use crate::types;
use crate::FuzzTest;

use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;

impl FuzzTest {
    pub fn liquidation_record_pda(&self, marginfi_account: Pubkey) -> Pubkey {
        Pubkey::find_program_address(
            &[
                crate::constants::LIQUIDATION_RECORD_SEED.as_bytes(),
                marginfi_account.as_ref(),
            ],
            &types::marginfi::program_id(),
        )
        .0
    }

    pub fn initialize_mint_2022(
        &mut self,
        payer: Pubkey,
        mint: Pubkey,
        decimals: u8,
        mint_authority: Pubkey,
    ) {
        let ixs =
            self.trident
                .initialize_mint_2022(&payer, &mint, decimals, &mint_authority, None, &[]);
        let res = self.trident.process_transaction(&ixs, None);
        invariant!(res.is_success());
    }

    pub fn initialize_mint(
        &mut self,
        payer: Pubkey,
        mint: Pubkey,
        decimals: u8,
        mint_authority: Pubkey,
    ) {
        let ixs = self
            .trident
            .initialize_mint(&payer, &mint, decimals, &mint_authority, None);
        let res = self.trident.process_transaction(&ixs, None);
        invariant!(res.is_success());
    }

    pub fn initialize_token_account(
        &mut self,
        payer: Pubkey,
        token_account: Pubkey,
        mint: Pubkey,
        owner: Pubkey,
    ) {
        let ixs = self
            .trident
            .initialize_token_account(&payer, &token_account, &mint, &owner);
        let res = self.trident.process_transaction(&ixs, None);
        invariant!(res.is_success());
    }
    pub fn initialize_token_account_2022(
        &mut self,
        payer: Pubkey,
        token_account: Pubkey,
        mint: Pubkey,
        owner: Pubkey,
        extensions: &[AccountExtension],
    ) {
        let ixs = self.trident.initialize_token_account_2022(
            &payer,
            &token_account,
            &mint,
            &owner,
            extensions,
        );
        let res = self.trident.process_transaction(&ixs, None);
        invariant!(res.is_success());
    }

    pub fn mint_to_2022(
        &mut self,
        token_account: Pubkey,
        mint: Pubkey,
        mint_authority: Pubkey,
        amount: u64,
    ) {
        let ix: Instruction =
            self.trident
                .mint_to_2022(&token_account, &mint, &mint_authority, amount);
        let res = self.trident.process_transaction(&[ix], None);

        invariant!(res.is_success());
    }
    pub fn mint_to(
        &mut self,
        token_account: Pubkey,
        mint: Pubkey,
        mint_authority: Pubkey,
        amount: u64,
    ) {
        let ix: Instruction = self
            .trident
            .mint_to(&token_account, &mint, &mint_authority, amount);
        let res = self.trident.process_transaction(&[ix], None);

        invariant!(res.is_success());
    }

    pub fn get_marginfi_account_banks(
        &mut self,
        marginfi_account: Pubkey,
        interacting_with_bank: Option<Pubkey>,
    ) -> Vec<Pubkey> {
        let marginfi_account_data = self
            .trident
            .get_account_with_type::<types::marginfi::MarginfiAccount>(&marginfi_account, None)
            .expect("This needs to exist");

        let lending = &marginfi_account_data.lending_account;

        let mut slots: Vec<(usize, Pubkey)> = lending
            .balances
            .iter()
            .enumerate()
            .filter(|(_, b)| b.active != 0)
            .map(|(i, b)| (i, b.bank_pk))
            .collect();

        if let Some(bank_pk) = interacting_with_bank {
            let already = lending
                .balances
                .iter()
                .any(|b| b.active != 0 && b.bank_pk == bank_pk);
            if !already {
                if let Some(empty_index) = lending.balances.iter().position(|b| b.active == 0) {
                    slots.push((empty_index, bank_pk));
                }
            }
        }

        slots.sort_by_key(|(i, _)| *i);
        slots.into_iter().map(|(_, pk)| pk).collect()
    }
}

#[tokio::main]
pub async fn get_slot() -> u64 {
    let client = RpcClient::new_with_commitment(
        String::from("https://api.mainnet-beta.solana.com"),
        CommitmentConfig::confirmed(),
    );

    let slot = client.get_slot().await.expect("Failed to get slot");

    slot
}
