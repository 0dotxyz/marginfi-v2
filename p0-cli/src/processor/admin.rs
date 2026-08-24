use crate::{
    config::Config,
    utils::{find_fee_state_pda, send_tx},
};
use anchor_client::anchor_lang::prelude::*;
use anyhow::Result;
use marginfi_type_crate::ix_builders;
use marginfi_type_crate::types::Bank;
use marginfi_type_crate::{bank_authority_seed, types::BankVaultType};
use solana_sdk::pubkey::Pubkey;

pub fn process_collect_fees(config: Config, bank_pk: Pubkey) -> Result<()> {
    let rpc_client = config.mfi_program.rpc();
    let bank = config.mfi_program.account::<Bank>(bank_pk)?;
    let fee_state = config
        .mfi_program
        .account::<marginfi_type_crate::types::FeeState>(
            find_fee_state_pda(&config.program_id).0,
        )?;

    let bank_mint_account = rpc_client.get_account(&bank.mint)?;
    let token_program = bank_mint_account.owner;
    let fee_ata = anchor_spl::associated_token::get_associated_token_address_with_program_id(
        &fee_state.global_fee_wallet,
        &bank.mint,
        &token_program,
    );

    let (liquidity_vault_authority, _) = Pubkey::find_program_address(
        bank_authority_seed!(BankVaultType::Liquidity, bank_pk),
        &config.program_id,
    );

    let create_fee_ata_ix =
        spl_associated_token_account::instruction::create_associated_token_account_idempotent(
            &config.explicit_fee_payer(),
            &fee_state.global_fee_wallet,
            &bank.mint,
            &token_program,
        );

    let mut ix = ix_builders::with_program_id(
        ix_builders::pool::lending_pool_collect_bank_fees(
            &ix_builders::pool::LendingPoolCollectBankFees {
                group: bank.group,
                bank: bank_pk,
                fee_vault: bank.fee_vault,
                token_program,
                liquidity_vault_authority,
                liquidity_vault: bank.liquidity_vault,
                insurance_vault: bank.insurance_vault,
                fee_state: find_fee_state_pda(&config.program_id).0,
                fee_ata,
            },
        ),
        config.program_id,
    );
    ix.accounts
        .push(AccountMeta::new_readonly(bank.mint, false));

    let signing_keypairs = config.get_signers(false);

    let sig = send_tx(&config, vec![create_fee_ata_ix, ix], &signing_keypairs)?;
    println!("Collect fees successful (sig: {})", sig);

    Ok(())
}
