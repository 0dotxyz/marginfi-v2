use crate::{
    config::Config,
    utils::{find_fee_state_pda, send_tx, ui_to_native},
};
use anchor_client::anchor_lang::{prelude::*, InstructionData};
use anyhow::Result;
use marginfi::{bank_authority_seed, state::bank::BankVaultType};
use marginfi_type_crate::types::Bank;
use solana_sdk::{instruction::Instruction, pubkey::Pubkey};

pub fn process_collect_fees(config: Config, bank_pk: Pubkey, fee_ata: Pubkey) -> Result<()> {
    let rpc_client = config.mfi_program.rpc();
    let bank = config.mfi_program.account::<Bank>(bank_pk)?;

    let bank_mint_account = rpc_client.get_account(&bank.mint)?;
    let token_program = bank_mint_account.owner;

    let (liquidity_vault_authority, _) = Pubkey::find_program_address(
        bank_authority_seed!(BankVaultType::Liquidity, bank_pk),
        &config.program_id,
    );

    let mut ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::LendingPoolCollectBankFees {
            group: bank.group,
            bank: bank_pk,
            fee_vault: bank.fee_vault,
            token_program,
            liquidity_vault_authority,
            liquidity_vault: bank.liquidity_vault,
            insurance_vault: bank.insurance_vault,
            fee_state: find_fee_state_pda(&config.program_id).0,
            fee_ata,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::LendingPoolCollectBankFees {}.data(),
    };
    ix.accounts
        .push(AccountMeta::new_readonly(bank.mint, false));

    let signing_keypairs = config.get_signers(false);

    let sig = send_tx(&config, vec![ix], &signing_keypairs)?;
    println!("Collect fees successful (sig: {})", sig);

    Ok(())
}

pub fn process_withdraw_fees(
    config: Config,
    bank_pk: Pubkey,
    amount_ui: f64,
    dst_address: Option<Pubkey>,
) -> Result<()> {
    let rpc_client = config.mfi_program.rpc();
    let bank = config.mfi_program.account::<Bank>(bank_pk)?;
    let amount = ui_to_native(amount_ui, bank.mint_decimals);
    let dst_address = dst_address.unwrap_or(config.authority());

    let bank_mint_account = rpc_client.get_account(&bank.mint)?;
    let token_program = bank_mint_account.owner;

    let ata = anchor_spl::associated_token::get_associated_token_address_with_program_id(
        &dst_address,
        &bank.mint,
        &token_program,
    );

    let (fee_vault_authority, _) = Pubkey::find_program_address(
        bank_authority_seed!(BankVaultType::Fee, bank_pk),
        &config.program_id,
    );

    let create_ata_ix =
        spl_associated_token_account::instruction::create_associated_token_account_idempotent(
            &config.explicit_fee_payer(),
            &dst_address,
            &bank.mint,
            &token_program,
        );

    let mut ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::LendingPoolWithdrawFees {
            group: bank.group,
            bank: bank_pk,
            admin: config.authority(),
            fee_vault: bank.fee_vault,
            fee_vault_authority,
            dst_token_account: ata,
            token_program,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::LendingPoolWithdrawFees { amount }.data(),
    };
    ix.accounts
        .push(AccountMeta::new_readonly(bank.mint, false));

    let signing_keypairs = config.get_signers(false);

    let sig = send_tx(&config, vec![create_ata_ix, ix], &signing_keypairs)?;
    println!("Withdraw fees successful (sig: {})", sig);

    Ok(())
}

pub fn process_withdraw_insurance(
    config: Config,
    bank_pk: Pubkey,
    amount_ui: f64,
    dst_address: Option<Pubkey>,
) -> Result<()> {
    let rpc_client = config.mfi_program.rpc();
    let bank = config.mfi_program.account::<Bank>(bank_pk)?;
    let amount = ui_to_native(amount_ui, bank.mint_decimals);
    let dst_address = dst_address.unwrap_or(config.authority());

    let bank_mint_account = rpc_client.get_account(&bank.mint)?;
    let token_program = bank_mint_account.owner;

    let ata = anchor_spl::associated_token::get_associated_token_address_with_program_id(
        &dst_address,
        &bank.mint,
        &token_program,
    );

    let (insurance_vault_authority, _) = Pubkey::find_program_address(
        bank_authority_seed!(BankVaultType::Insurance, bank_pk),
        &config.program_id,
    );

    let create_ata_ix =
        spl_associated_token_account::instruction::create_associated_token_account_idempotent(
            &config.explicit_fee_payer(),
            &dst_address,
            &bank.mint,
            &token_program,
        );

    let mut ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::LendingPoolWithdrawInsurance {
            group: bank.group,
            bank: bank_pk,
            admin: config.authority(),
            insurance_vault: bank.insurance_vault,
            insurance_vault_authority,
            dst_token_account: ata,
            token_program,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::LendingPoolWithdrawInsurance { amount }.data(),
    };
    ix.accounts
        .push(AccountMeta::new_readonly(bank.mint, false));

    let signing_keypairs = config.get_signers(false);

    let sig = send_tx(&config, vec![create_ata_ix, ix], &signing_keypairs)?;
    println!("Withdraw insurance successful (sig: {})", sig);

    Ok(())
}
