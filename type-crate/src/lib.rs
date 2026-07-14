use bytemuck::Zeroable;
use fixed::types::I80F48;
use types::FeeState;

pub mod constants;
pub mod macros;
pub mod types;

#[cfg(feature = "anchor")]
pub mod pdas;

#[cfg(feature = "anchor")]
pub use id_crate::ID;

/// Just a sample function demonstrating usage.
pub fn generic_fee_state() -> FeeState {
    let mut fee_state = FeeState::zeroed();
    fee_state.program_fee_fixed = I80F48::from_num(0.01).into();
    fee_state.program_fee_rate = I80F48::from_num(0.05).into();
    fee_state
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generic_fee_state_sample() {
        let state = generic_fee_state();
        let fee: I80F48 = state.program_fee_fixed.into();
        assert_eq!(fee, I80F48::from_num(0.01));
    }
}
