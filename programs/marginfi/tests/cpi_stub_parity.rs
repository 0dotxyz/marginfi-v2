use anchor_lang::InstructionData;
use marginfi_type_crate::marginfi_cpi_stubs;

#[test]
fn no_argument_cpi_data_matches_anchor() {
    assert_eq!(
        marginfi_cpi_stubs::start_liquidation_data(),
        marginfi::instruction::StartLiquidation {}.data()
    );
    assert_eq!(
        marginfi_cpi_stubs::lending_pool_handle_bankruptcy_data(),
        marginfi::instruction::LendingPoolHandleBankruptcy {}.data()
    );
}

#[test]
fn initialize_pda_cpi_data_matches_anchor() {
    for (account_index, third_party_id) in [
        (0, None),
        (u16::MAX, None),
        (0, Some(0)),
        (0x1234, Some(0xabcd)),
        (u16::MAX, Some(u16::MAX)),
    ] {
        assert_eq!(
            marginfi_cpi_stubs::marginfi_account_initialize_pda_data(account_index, third_party_id),
            marginfi::instruction::MarginfiAccountInitializePda {
                account_index,
                third_party_id,
            }
            .data()
        );
    }
}
