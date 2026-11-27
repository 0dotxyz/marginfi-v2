use anchor_lang::prelude::{AccountInfo, Pubkey};
use anchor_lang::solana_program::{
    program::invoke_signed, program_option::COption, program_pack::Pack,
};
use marginfi_fuzz::stubs::TestSyscallStubs;
use solana_sdk::program_stubs;
use spl_token::state::{Account, AccountState};

#[test]
fn anchor_invoke_runs_fuzz_token_transfer_stub() {
    let previous = program_stubs::set_syscall_stubs(Box::new(TestSyscallStubs {
        unix_timestamp: None,
    }));
    let mint = Pubkey::new_unique();
    let authority = Pubkey::new_unique();
    let source = Pubkey::new_unique();
    let destination = Pubkey::new_unique();
    let token_program = spl_token::ID;
    let mut source_lamports = 0;
    let mut destination_lamports = 0;
    let mut authority_lamports = 0;
    let mut source_data = vec![0; Account::LEN];
    let mut destination_data = vec![0; Account::LEN];
    let mut authority_data = [];
    let token_account = |amount| Account {
        mint,
        owner: authority,
        amount,
        delegate: COption::None,
        state: AccountState::Initialized,
        is_native: COption::None,
        delegated_amount: 0,
        close_authority: COption::None,
    };
    Account::pack(token_account(10), &mut source_data).unwrap();
    Account::pack(token_account(0), &mut destination_data).unwrap();
    let source_info = AccountInfo::new(
        &source,
        false,
        true,
        &mut source_lamports,
        &mut source_data,
        &token_program,
        false,
    );
    let destination_info = AccountInfo::new(
        &destination,
        false,
        true,
        &mut destination_lamports,
        &mut destination_data,
        &token_program,
        false,
    );
    let authority_info = AccountInfo::new(
        &authority,
        true,
        false,
        &mut authority_lamports,
        &mut authority_data,
        &token_program,
        false,
    );
    let ix =
        spl_token::instruction::transfer(&token_program, &source, &destination, &authority, &[], 7)
            .unwrap();

    let result = invoke_signed(
        &ix,
        &[
            source_info.clone(),
            destination_info.clone(),
            authority_info,
        ],
        &[],
    );
    program_stubs::set_syscall_stubs(previous);
    result.unwrap();
    assert_eq!(
        Account::unpack(&source_info.data.borrow()).unwrap().amount,
        3
    );
    assert_eq!(
        Account::unpack(&destination_info.data.borrow())
            .unwrap()
            .amount,
        7
    );
}
