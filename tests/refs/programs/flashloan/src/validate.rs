use anchor_lang::prelude::*;
use anchor_lang::solana_program::instruction::Instruction;
use anchor_lang::solana_program::sysvar::instructions::{
    load_current_index_checked, load_instruction_at_checked,
};
use solana_program::{instruction::get_stack_height, serialize_utils::read_u16};

use crate::constants;
use crate::errors::ErrorCodes;
use crate::state::context::Flashloan;

pub fn validate_flashloan(ctx: &Context<Flashloan>, amount: u64) -> Result<()> {
    if amount < constants::MIN_FLASHLOAN_AMOUNT {
        return Err(ErrorCodes::FlashloanInvalidParams.into());
    }

    let ix_sysvar_account = &ctx.accounts.instruction_sysvar.to_account_info();
    let current_index = load_current_index_checked(ix_sysvar_account)?;
    let current_ixs = load_instruction_at_checked(current_index as usize, ix_sysvar_account)?;

    // verify that the current instruction is from flashloan program
    if current_ixs.program_id != *ctx.program_id {
        return Err(ErrorCodes::FlashloanInvalidParams.into());
    }

    // No cpi calls are allowed to flashloan program
    if get_stack_height() > constants::FLASHLOAN_STACK_HEIGHT {
        return Err(ErrorCodes::FlashloanCPICallNotAllowed.into());
    }

    // Validate that there's a corresponding payback instruction in the same transaction
    validate_payback_instruction_exists(ctx, amount, current_index, ix_sysvar_account)?;

    Ok(())
}

fn validate_payback_instruction_exists(
    ctx: &Context<Flashloan>,
    borrow_amount: u64,
    current_index: u16,
    ix_sysvar_account: &AccountInfo,
) -> Result<()> {
    let total_instructions = get_instruction_count(ix_sysvar_account)?;

    let search_start = current_index + 1;

    let mut payback_found = false;

    for i in (search_start..total_instructions).rev() {
        match load_instruction_at_checked(i as usize, ix_sysvar_account) {
            Ok(instruction) => {
                if instruction.program_id != *ctx.program_id {
                    continue;
                }

                match is_flashloan_payback_instruction(&instruction, borrow_amount, ctx) {
                    Ok(true) => {
                        payback_found = if payback_found {
                            return Err(ErrorCodes::FlashloanMultiplePaybacksFound.into());
                        } else {
                            true
                        };

                        continue;
                    }

                    Ok(false) => {
                        // throw error if invalid instruction is found
                        return Err(ErrorCodes::FlashloanInvalidInstruction.into());
                    }

                    Err(_) => {
                        return Err(ErrorCodes::FlashloanPaybackNotFound.into());
                    }
                }
            }

            Err(_) => {
                return Err(ErrorCodes::FlashloanPaybackNotFound.into());
            }
        }
    }

    if !payback_found {
        return Err(ErrorCodes::FlashloanPaybackNotFound.into());
    }

    Ok(())
}

fn get_instruction_count(ix_sysvar_account: &AccountInfo) -> Result<u16> {
    let data = ix_sysvar_account.try_borrow_data()?;

    let mut current: usize = 0;
    let count =
        read_u16(&mut current, &data).map_err(|_| ErrorCodes::FlashloanInvalidInstructionSysvar)?;

    Ok(count)
}

fn is_flashloan_payback_instruction(
    instruction: &Instruction,
    expected_amount: u64,
    ctx: &Context<Flashloan>,
) -> Result<bool> {
    // 8 bytes discriminator + 8 bytes amount
    if instruction.data.len() != 16 {
        return Ok(false);
    }

    let account_infos = ctx.accounts.to_account_infos();

    if instruction.accounts.len() != account_infos.len() {
        return Ok(false);
    }

    for (i, account) in account_infos.iter().enumerate() {
        if instruction.accounts[i].pubkey == account.key() {
            continue;
        } else {
            return Ok(false);
        }
    }

    let discriminator = &instruction.data[0..8];

    if discriminator != constants::FLASHLOAN_PAYBACK_DISCRIMINATOR {
        return Ok(false);
    }

    let amount_bytes: [u8; 8] = instruction.data[8..16]
        .try_into()
        .map_err(|_| ErrorCodes::FlashloanInvalidInstructionData)?;

    let instruction_amount = u64::from_le_bytes(amount_bytes);

    if instruction_amount != expected_amount {
        return Ok(false);
    }

    Ok(true)
}

pub fn validate_flashloan_payback(active_flashloan_amount: u64, amount: u64) -> Result<()> {
    if amount < constants::MIN_FLASHLOAN_AMOUNT {
        return Err(ErrorCodes::FlashloanInvalidParams.into());
    }

    if amount != active_flashloan_amount {
        return Err(ErrorCodes::FlashloanInvalidParams.into());
    }

    // No cpi calls are allowed to flashloan program
    if get_stack_height() > constants::FLASHLOAN_STACK_HEIGHT {
        return Err(ErrorCodes::FlashloanCPICallNotAllowed.into());
    }

    Ok(())
}
