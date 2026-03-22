//! Program state processor
use {
    anchor_lang::solana_program::{
        account_info::{next_account_info, AccountInfo},
        entrypoint::ProgramResult,
        msg,
        program::invoke_signed,
        program_error::ProgramError,
        pubkey,
        pubkey::Pubkey,
        system_instruction,
    },
    spl_tlv_account_resolution::{account::ExtraAccountMeta, state::ExtraAccountMetaList},
    spl_token_2022::{
        extension::{
            transfer_hook::TransferHookAccount, BaseStateWithExtensions, StateWithExtensions,
        },
        state::{Account, Mint},
    },
    spl_transfer_hook_interface::{
        collect_extra_account_metas_signer_seeds,
        error::TransferHookError,
        get_extra_account_metas_address, get_extra_account_metas_address_and_bump_seed,
        instruction::{ExecuteInstruction, TransferHookInstruction},
    },
};

pub static TEST_HOOK_ID: Pubkey = pubkey!("TRANSFERHKTRANSFERHKTRANSFERHKTRANSFERHKTRA");

#[cfg(not(feature = "no-entrypoint"))]
anchor_lang::solana_program::entrypoint!(process);

#[allow(unused)]
fn check_token_account_is_transferring(account_info: &AccountInfo) -> Result<(), ProgramError> {
    let account_data = account_info.try_borrow_data()?;
    let token_account = StateWithExtensions::<Account>::unpack(&account_data)?;
    let extension = token_account.get_extension::<TransferHookAccount>()?;
    if bool::from(extension.transferring) {
        Ok(())
    } else {
        Err(TransferHookError::ProgramCalledOutsideOfTransfer.into())
    }
}

/// Processes an [Execute](enum.TransferHookInstruction.html) instruction.
pub fn process_execute(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    _amount: u64,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();

    let _source_account_info = next_account_info(account_info_iter)?;
    let _mint_info = next_account_info(account_info_iter)?;
    let _destination_account_info = next_account_info(account_info_iter)?;
    let _authority_info = next_account_info(account_info_iter)?;
    let _extra_account_metas_info = next_account_info(account_info_iter)?;

    Ok(())
}

/// Processes a
/// [InitializeExtraAccountMetaList](enum.TransferHookInstruction.html)
/// instruction.
pub fn process_initialize_extra_account_meta_list(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    extra_account_metas: &[ExtraAccountMeta],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();

    let extra_account_metas_info = next_account_info(account_info_iter)?;
    let mint_info = next_account_info(account_info_iter)?;
    let authority_info = next_account_info(account_info_iter)?;
    let _system_program_info = next_account_info(account_info_iter)?;

    // check that the one mint we want to target is trying to create extra
    // account metas
    #[cfg(feature = "forbid-additional-mints")]
    if *mint_info.key != crate::mint::id() {
        return Err(ProgramError::InvalidArgument);
    }

    // check that the mint authority is valid without fully deserializing
    let mint_data = mint_info.try_borrow_data()?;
    let mint = StateWithExtensions::<Mint>::unpack(&mint_data)?;
    let mint_authority = mint
        .base
        .mint_authority
        .ok_or(TransferHookError::MintHasNoMintAuthority)?;

    // Check signers
    if !authority_info.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    if *authority_info.key != mint_authority {
        return Err(TransferHookError::IncorrectMintAuthority.into());
    }

    // Check validation account
    let (expected_validation_address, bump_seed) =
        get_extra_account_metas_address_and_bump_seed(mint_info.key, program_id);
    if expected_validation_address != *extra_account_metas_info.key {
        return Err(ProgramError::InvalidSeeds);
    }

    // Create the account
    let bump_seed = [bump_seed];
    let signer_seeds = collect_extra_account_metas_signer_seeds(mint_info.key, &bump_seed);
    let length = extra_account_metas.len();
    let account_size = ExtraAccountMetaList::size_of(length)?;
    invoke_signed(
        &system_instruction::allocate(extra_account_metas_info.key, account_size as u64),
        &[extra_account_metas_info.clone()],
        &[&signer_seeds],
    )?;
    invoke_signed(
        &system_instruction::assign(extra_account_metas_info.key, program_id),
        &[extra_account_metas_info.clone()],
        &[&signer_seeds],
    )?;

    // Write the data
    let mut data = extra_account_metas_info.try_borrow_mut_data()?;
    ExtraAccountMetaList::init::<ExecuteInstruction>(&mut data, extra_account_metas)?;

    Ok(())
}

/// Processes a
/// [UpdateExtraAccountMetaList](enum.TransferHookInstruction.html)
/// instruction.
pub fn process_update_extra_account_meta_list(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    extra_account_metas: &[ExtraAccountMeta],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();

    let extra_account_metas_info = next_account_info(account_info_iter)?;
    let mint_info = next_account_info(account_info_iter)?;
    let authority_info = next_account_info(account_info_iter)?;

    // check that the mint authority is valid without fully deserializing
    let mint_data = mint_info.try_borrow_data()?;
    let mint = StateWithExtensions::<Mint>::unpack(&mint_data)?;
    let mint_authority = mint
        .base
        .mint_authority
        .ok_or(TransferHookError::MintHasNoMintAuthority)?;

    // Check signers
    if !authority_info.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    if *authority_info.key != mint_authority {
        return Err(TransferHookError::IncorrectMintAuthority.into());
    }

    // Check validation account
    let expected_validation_address = get_extra_account_metas_address(mint_info.key, program_id);
    if expected_validation_address != *extra_account_metas_info.key {
        return Err(ProgramError::InvalidSeeds);
    }

    // Check if the extra metas have been initialized
    let min_account_size = ExtraAccountMetaList::size_of(0)?;
    let original_account_size = extra_account_metas_info.data_len();
    if program_id != extra_account_metas_info.owner || original_account_size < min_account_size {
        return Err(ProgramError::UninitializedAccount);
    }

    // If the new extra_account_metas length is different, resize the account and
    // update
    let length = extra_account_metas.len();
    let account_size = ExtraAccountMetaList::size_of(length)?;
    if account_size >= original_account_size {
        extra_account_metas_info.realloc(account_size, false)?;
        let mut data = extra_account_metas_info.try_borrow_mut_data()?;
        ExtraAccountMetaList::update::<ExecuteInstruction>(&mut data, extra_account_metas)?;
    } else {
        {
            let mut data = extra_account_metas_info.try_borrow_mut_data()?;
            ExtraAccountMetaList::update::<ExecuteInstruction>(&mut data, extra_account_metas)?;
        }
        extra_account_metas_info.realloc(account_size, false)?;
    }

    Ok(())
}

/// Processes an [Instruction](enum.Instruction.html).
pub fn process(program_id: &Pubkey, accounts: &[AccountInfo], input: &[u8]) -> ProgramResult {
    let instruction = TransferHookInstruction::unpack(input)?;

    match instruction {
        TransferHookInstruction::Execute { amount } => {
            msg!("Instruction: Execute");
            process_execute(program_id, accounts, amount)
        }
        TransferHookInstruction::InitializeExtraAccountMetaList {
            extra_account_metas,
        } => {
            msg!("Instruction: InitializeExtraAccountMetaList");
            process_initialize_extra_account_meta_list(program_id, accounts, &extra_account_metas)
        }
        TransferHookInstruction::UpdateExtraAccountMetaList {
            extra_account_metas,
        } => {
            msg!("Instruction: UpdateExtraAccountMetaList");
            process_update_extra_account_meta_list(program_id, accounts, &extra_account_metas)
        }
    }
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use solana_program_test::{processor, tokio, ProgramTest};
//     use solana_sdk::{
//         instruction::{AccountMeta, Instruction},
//         signer::Signer,
//         transaction::Transaction,
//     };
//     use spl_discriminator::SplDiscriminate;
//     use spl_transfer_hook_interface::instruction::ExecuteInstruction;

//     #[tokio::test]
//     async fn invoke_hook() {
//         let ctx = ProgramTest::new("transfer_hook", TEST_HOOK_ID, processor!(super::process))
//             .start_with_context()
//             .await;

//         let mut data = ExecuteInstruction::SPL_DISCRIMINATOR_SLICE.to_vec();
//         data.extend_from_slice(&0_u64.to_le_bytes());

//         let ix = Instruction {
//             program_id: TEST_HOOK_ID,
//             accounts: vec![
//                 AccountMeta::new_readonly(Pubkey::new_unique(), false),
//                 AccountMeta::new_readonly(Pubkey::new_unique(), false),
//                 AccountMeta::new_readonly(Pubkey::new_unique(), false),
//                 AccountMeta::new_readonly(Pubkey::new_unique(), false),
//                 AccountMeta::new_readonly(Pubkey::new_unique(), false),
//             ],
//             data,
//         };
//         let transaction = Transaction::new_signed_with_payer(
//             &[ix],
//             Some(&ctx.payer.pubkey()),
//             &[&ctx.payer],
//             ctx.banks_client.get_latest_blockhash().await.unwrap(),
//         );

//         ctx.banks_client
//             .process_transaction(transaction)
//             .await
//             .unwrap();
//     }
// }
