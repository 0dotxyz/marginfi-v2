#[cfg(feature = "anchor")]
use anchor_lang::prelude::*;

use super::bank::JuplendLayout;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "anchor", derive(AnchorSerialize, AnchorDeserialize))]
#[cfg_attr(feature = "anchor", borsh(use_discriminant = true))]
#[repr(u8)]
pub enum JuplendOracleSetup {
    FixedJuplend = 0,

    JuplendPythPull = 1,

    JuplendSwitchboardPull = 2,

    ScopeJuplend = 3,

    JuplendMSOL = 4,

    JuplendLST = 5,
}

pub fn juplend_setup_layout(setup: JuplendOracleSetup) -> JuplendLayout {
    match setup {
        JuplendOracleSetup::FixedJuplend => JuplendLayout {
            lending: 0,
            liquidity: 1,
            len: 2,
        },
        JuplendOracleSetup::JuplendPythPull
        | JuplendOracleSetup::JuplendSwitchboardPull
        | JuplendOracleSetup::ScopeJuplend => JuplendLayout {
            lending: 1,
            liquidity: 2,
            len: 3,
        },
        JuplendOracleSetup::JuplendMSOL | JuplendOracleSetup::JuplendLST => JuplendLayout {
            lending: 1,
            liquidity: 3,
            len: 4,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn juplend_layout_definitions_match_oracle_setup() {
        let test_cases = vec![
            (
                JuplendOracleSetup::FixedJuplend,
                JuplendLayout {
                    lending: 0,
                    liquidity: 1,
                    len: 2,
                },
            ),
            (
                JuplendOracleSetup::JuplendPythPull,
                JuplendLayout {
                    lending: 1,
                    liquidity: 2,
                    len: 3,
                },
            ),
            (
                JuplendOracleSetup::JuplendSwitchboardPull,
                JuplendLayout {
                    lending: 1,
                    liquidity: 2,
                    len: 3,
                },
            ),
            (
                JuplendOracleSetup::ScopeJuplend,
                JuplendLayout {
                    lending: 1,
                    liquidity: 2,
                    len: 3,
                },
            ),
            (
                JuplendOracleSetup::JuplendMSOL,
                JuplendLayout {
                    lending: 1,
                    liquidity: 3,
                    len: 4,
                },
            ),
            (
                JuplendOracleSetup::JuplendLST,
                JuplendLayout {
                    lending: 1,
                    liquidity: 3,
                    len: 4,
                },
            ),
        ];

        for (setup, expected) in test_cases {
            assert_eq!(
                juplend_setup_layout(setup),
                expected,
                "Layout mismatch for {:?}",
                setup
            );
        }
    }
}
