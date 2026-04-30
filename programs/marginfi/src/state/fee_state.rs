use marginfi_type_crate::types::FeeState;

use anchor_lang::prelude::*;

pub trait FeeStateImpl {
    fn is_pause_authority(&self, signer: Pubkey) -> bool;
    fn set_pause_delegate_admin(&mut self, new_pause_delegate_admin: Option<Pubkey>);
}

impl FeeStateImpl for FeeState {
    fn is_pause_authority(&self, signer: Pubkey) -> bool {
        signer == self.global_fee_admin
            || (self.pause_delegate_admin != Pubkey::default()
                && signer == self.pause_delegate_admin)
    }

    fn set_pause_delegate_admin(&mut self, new_pause_delegate_admin: Option<Pubkey>) {
        let new_pause_delegate_admin = new_pause_delegate_admin.unwrap_or_default();
        if self.pause_delegate_admin == new_pause_delegate_admin {
            msg!(
                "No change to pause delegate admin: {:?}",
                new_pause_delegate_admin
            );
        } else {
            msg!(
                "Set pause delegate admin from {:?} to {:?}",
                self.pause_delegate_admin,
                new_pause_delegate_admin
            );
            self.pause_delegate_admin = new_pause_delegate_admin;
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::utils::hex_to_bytes;

    use super::*;
    use anchor_lang::{
        pubkey,
        solana_program::{account_info::AccountInfo, pubkey::Pubkey},
    };
    use fixed::types::I80F48;
    use fixed_macro::types::I80F48;
    use marginfi_type_crate::constants::discriminators;
    use std::{cell::RefCell, rc::Rc};

    #[test]
    fn fee_state_regression() {
        // HoMNdUF3RDZDPKAARYK1mxcPFfUnPjLmpKYibZzAijev Mainnet August 1, 2025
        let mut bytes = hex_to_bytes("3fe01055c124ebdcf99abe673005fd029ed4e061d83daab0145fa9c01140e8ab5e05b1aaaeef4317ab83bfce0e61a1a233632ba4cfbabf812b8023caf2be7df80972bfc9ff327540ab83bfce0e61a1a233632ba4cfbabf812b8023caf2be7df80972bfc9ff327540000000000000000080d1f008ff000000000000000000000000000000000000000000000000000000000000000000000033333333331300000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000");

        let expected_len = 8 + std::mem::size_of::<FeeState>();
        assert_eq!(
            bytes.len(),
            expected_len,
            "Buffer length mismatch: {} vs {}",
            bytes.len(),
            expected_len
        );

        // Split out the discriminator and check it
        let (disc, payload) = bytes.split_at_mut(8);
        assert_eq!(disc, discriminators::FEE_STATE, "Discriminator mismatch");

        let (expected_key, expected_bump) =
            Pubkey::find_program_address(&[b"feestate"], &crate::ID);

        // Wrap into AccountInfo to test zero-copy deserialization
        let mut lamports = 1u64;
        let account_info = AccountInfo {
            key: &expected_key,
            lamports: Rc::new(RefCell::new(&mut lamports)),
            data: Rc::new(RefCell::new(payload)),
            owner: &crate::ID,
            rent_epoch: 0,
            is_signer: false,
            is_writable: true,
            executable: false,
        };

        let binding = account_info.data.borrow();
        let fee_state: &FeeState = bytemuck::from_bytes(&binding);

        // 1) PDA and bump seed
        assert_eq!(fee_state.key, expected_key);
        assert_eq!(fee_state.bump_seed, expected_bump);

        // 2) Flat, alignment, and reserved fields
        let admin = pubkey!("CYXEgwbPHu2f9cY3mcUkinzDoDcsSan7myh1uBvYRbEw");
        assert_eq!(fee_state.global_fee_admin, admin); // the MS
        assert_eq!(fee_state.global_fee_wallet, admin); // MS was also wallet at this time
        assert_eq!(fee_state.placeholder0, 0);
        assert_eq!(fee_state.bank_init_flat_sol_fee, 150000000);

        // 3) Percentage-based fees: with tolerance
        let tol: I80F48 = I80F48!(0.001);
        let expected_liquidation_max_fee: I80F48 = I80F48!(0);
        let actual_max_fee: I80F48 = fee_state.liquidation_max_fee.into();
        assert!(
            (actual_max_fee - expected_liquidation_max_fee).abs()
                <= expected_liquidation_max_fee * tol
        );

        // program_fee_fixed
        let expected_program_fee_fixed: I80F48 = I80F48!(0);
        let actual_program_fee_fixed: I80F48 = fee_state.program_fee_fixed.into();
        assert!(
            (actual_program_fee_fixed - expected_program_fee_fixed).abs()
                <= expected_program_fee_fixed * tol
        );

        // program_fee_rate
        let expected_program_fee_rate: I80F48 = I80F48!(0.075);
        let actual_program_fee_rate: I80F48 = fee_state.program_fee_rate.into();
        assert!(
            (actual_program_fee_rate - expected_program_fee_rate).abs()
                <= expected_program_fee_rate * tol
        );

        // 4) Remaining reserved and flat fields
        assert_eq!(fee_state.placeholder1, 0);
        assert_eq!(fee_state.liquidation_flat_sol_fee, 0);
    }
}
