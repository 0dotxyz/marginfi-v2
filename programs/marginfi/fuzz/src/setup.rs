use anchor_lang::prelude::*;
use anchor_lang::prelude::{AccountInfo, AccountLoader, Context, Program, Rent, Signer};
use fixed_macro::types::I80F48;
use marginfi_type_crate::types::{
    FeeState, MarginfiGroup,
};
use std::mem::size_of;

use crate::{
    account_state::AccountsState,
    utils::account_info_ref_lifetime_shortener as airls,
};
use marginfi_type_crate::types::MarginfiAccount;

pub fn sort_balances<'a>(marginfi_account_ai: &'a AccountInfo<'a>) {
    let marginfi_account_loader =
        AccountLoader::<MarginfiAccount>::try_from(marginfi_account_ai).unwrap();
    let mut marginfi_account = marginfi_account_loader.load_mut().unwrap();
    marginfi_account
        .lending_account
        .balances
        .sort_by_key(|a| !a.active);
}

pub fn set_discriminator<T: Discriminator>(ai: AccountInfo) {
    let mut data = ai.try_borrow_mut_data().unwrap();

    if data[..8].ne(&[0u8; 8]) {
        panic!("Account discriminator is already set");
    }

    data[..8].copy_from_slice(&T::DISCRIMINATOR);
}

pub fn initialize_marginfi_group<'a>(
    state: &'a AccountsState,
    admin: AccountInfo<'a>,
    fee_state: AccountInfo<'a>,
    system_program: AccountInfo<'a>,
) -> AccountInfo<'a> {
    let program_id = marginfi::ID;
    let marginfi_group =
        state.new_owned_account(size_of::<MarginfiGroup>(), program_id, Rent::free());

    marginfi::instructions::marginfi_group::initialize_group(Context::new(
        &marginfi::ID,
        &mut marginfi::instructions::MarginfiGroupInitialize {
            marginfi_group: AccountLoader::try_from_unchecked(&program_id, airls(&marginfi_group))
                .unwrap(),
            admin: Signer::try_from(airls(&admin)).unwrap(),
            fee_state: AccountLoader::try_from_unchecked(&program_id, airls(&fee_state)).unwrap(),
            system_program: Program::try_from(airls(&system_program)).unwrap(),
        },
        &[],
        Default::default(),
    ))
    .unwrap();

    set_discriminator::<MarginfiGroup>(marginfi_group.clone());

    marginfi::instructions::marginfi_group::configure(
        Context::new(
            &marginfi::ID,
            &mut marginfi::instructions::MarginfiGroupConfigure {
                marginfi_group: AccountLoader::try_from_unchecked(
                    &program_id,
                    airls(&marginfi_group),
                )
                .unwrap(),
                admin: Signer::try_from(airls(&admin)).unwrap(),
            },
            &[],
            Default::default(),
        ),
        Some(admin.key()),
        Some(admin.key()),
        Some(admin.key()),
        Some(admin.key()),
        Some(admin.key()),
        Some(admin.key()),
        Some(admin.key()),
        Some(admin.key()),
        None,
        None,
        None,
        None,
    )
    .unwrap();

    marginfi_group
}

pub fn initialize_fee_state<'a>(
    state: &'a AccountsState,
    admin: AccountInfo<'a>,
    wallet: AccountInfo<'a>,
    system_program: AccountInfo<'a>,
) -> AccountInfo<'a> {
    let program_id = marginfi::ID;
    let (fee_state, _fee_state_bump) = state.new_fee_state(program_id);

    marginfi::instructions::marginfi_group::initialize_fee_state(
        Context::new(
            &marginfi::ID,
            &mut marginfi::instructions::InitFeeState {
                payer: Signer::try_from(airls(&admin)).unwrap(),
                fee_state: AccountLoader::try_from_unchecked(&program_id, airls(&fee_state))
                    .unwrap(),
                system_program: Program::try_from(airls(&system_program)).unwrap(),
            },
            &[],
            Default::default(),
        ),
        admin.key(),
        wallet.key(),
        0,
        0,
        0,
        I80F48!(0).into(),
        I80F48!(0).into(),
        I80F48!(0.05).into(),
        I80F48!(0.05).into(),
    )
    .unwrap();

    set_discriminator::<FeeState>(fee_state.clone());

    fee_state
}
