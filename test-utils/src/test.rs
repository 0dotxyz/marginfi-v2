use super::marginfi_account::MarginfiAccountFixture;
#[cfg(feature = "transfer-hook")]
use crate::transfer_hook::TEST_HOOK_ID;
use crate::{
    bank::BankFixture, kamino::KaminoFixture, marginfi_group::*, native, spl::*, utils::*,
};

use anchor_lang::{prelude::*, InstructionData, ToAccountMetas};
use anchor_spl::associated_token::get_associated_token_address_with_program_id;
use anchor_spl::token::spl_token;
use bincode::deserialize;
use drift_mocks::drift::client as drift;
use drift_mocks::state::{MinimalSpotMarket, MinimalUser};
use fixed::types::I80F48;
use juplend_mocks::juplend_earn::client as juplend_lending;
use juplend_mocks::lending_reward_rate_model::client as juplend_rewards;
use juplend_mocks::liquidity::client as juplend_liquidity;
use juplend_mocks::state::Lending as JuplendLending;
use kamino_mocks::mock_kamino_lending_processor;
use kamino_mocks::state::{MinimalObligation, MinimalReserve};
use marginfi::{
    constants::JUPLEND_F_TOKEN_VAULT_SEED,
    state::{
        bank::{BankImpl, BankVaultType},
        drift::DriftConfigCompact,
        juplend::JuplendConfigCompact,
        kamino::KaminoConfigCompact,
    },
    utils::{find_bank_vault_authority_pda, find_bank_vault_pda},
};
use marginfi_type_crate::{
    constants::{MAX_ORACLE_KEYS, PYTH_PUSH_MIGRATED_DEPRECATED},
    types::{
        centi_to_u32, make_points, milli_to_u32, BankConfig, BankOperationalState,
        InterestRateConfig, OracleSetup, RatePoint, RiskTier, INTEREST_CURVE_SEVEN_POINT,
    },
};
use pyth_solana_receiver_sdk::price_update::{PriceUpdateV2, VerificationLevel};
use solana_sdk::{account::AccountSharedData, entrypoint::ProgramResult};

use fixed_macro::types::I80F48;
use lazy_static::lazy_static;
use solana_program::{hash::Hash, sysvar};
use solana_program_test::*;
use solana_sdk::{
    account::Account,
    compute_budget::ComputeBudgetInstruction,
    instruction::{AccountMeta, Instruction},
    pubkey,
    signature::Keypair,
    signer::Signer,
    transaction::Transaction,
};

use anchor_lang::system_program;
use std::{cell::RefCell, collections::HashMap, path::PathBuf, rc::Rc};

#[derive(Default, Debug, Clone)]
pub struct TestSettings {
    pub banks: Vec<TestBankSetting>,
    pub protocol_fees: bool,
}

impl TestSettings {
    pub fn all_banks_payer_not_admin() -> Self {
        let banks = vec![
            TestBankSetting {
                mint: BankMint::Usdc,
                ..TestBankSetting::default()
            },
            TestBankSetting {
                mint: BankMint::Fixed,
                ..TestBankSetting::default()
            },
            TestBankSetting {
                mint: BankMint::FixedLow,
                ..TestBankSetting::default()
            },
            TestBankSetting {
                mint: BankMint::Sol,
                ..TestBankSetting::default()
            },
            TestBankSetting {
                mint: BankMint::SolSwbPull,
                ..TestBankSetting::default()
            },
            TestBankSetting {
                mint: BankMint::SolSwbOrigFee,
                ..TestBankSetting::default()
            },
            TestBankSetting {
                mint: BankMint::SolEquivalent,
                ..TestBankSetting::default()
            },
            TestBankSetting {
                mint: BankMint::PyUSD,
                ..TestBankSetting::default()
            },
            TestBankSetting {
                mint: BankMint::T22WithFee,
                ..TestBankSetting::default()
            },
            TestBankSetting {
                mint: BankMint::SolEqIsolated,
                ..TestBankSetting::default()
            },
            TestBankSetting {
                mint: BankMint::KaminoUsdc,
                ..TestBankSetting::default()
            },
        ];

        Self {
            banks,
            protocol_fees: false,
        }
    }

    pub fn all_banks_one_isolated() -> Self {
        Self {
            banks: vec![
                TestBankSetting {
                    mint: BankMint::Usdc,
                    ..TestBankSetting::default()
                },
                TestBankSetting {
                    mint: BankMint::Sol,
                    ..TestBankSetting::default()
                },
                TestBankSetting {
                    mint: BankMint::SolEquivalent,
                    config: Some(BankConfig {
                        risk_tier: RiskTier::Isolated,
                        asset_weight_maint: I80F48!(0).into(),
                        asset_weight_init: I80F48!(0).into(),
                        ..*DEFAULT_SOL_EQUIVALENT_TEST_BANK_CONFIG
                    }),
                },
            ],
            protocol_fees: false,
        }
    }

    pub fn many_banks_10() -> Self {
        Self {
            banks: vec![
                TestBankSetting {
                    mint: BankMint::Usdc,
                    ..TestBankSetting::default()
                },
                TestBankSetting {
                    mint: BankMint::Sol,
                    ..TestBankSetting::default()
                },
                TestBankSetting {
                    mint: BankMint::SolEquivalent,
                    ..TestBankSetting::default()
                },
                TestBankSetting {
                    mint: BankMint::SolEquivalent1,
                    ..TestBankSetting::default()
                },
                TestBankSetting {
                    mint: BankMint::SolEquivalent2,
                    ..TestBankSetting::default()
                },
                TestBankSetting {
                    mint: BankMint::SolEquivalent3,
                    ..TestBankSetting::default()
                },
                TestBankSetting {
                    mint: BankMint::SolEquivalent4,
                    ..TestBankSetting::default()
                },
                TestBankSetting {
                    mint: BankMint::SolEquivalent5,
                    ..TestBankSetting::default()
                },
                TestBankSetting {
                    mint: BankMint::SolEquivalent6,
                    ..TestBankSetting::default()
                },
                TestBankSetting {
                    mint: BankMint::SolEquivalent7,
                    ..TestBankSetting::default()
                },
            ],
            protocol_fees: false,
        }
    }
}

#[derive(Default, Debug, Clone)]
pub struct TestBankSetting {
    pub mint: BankMint,
    pub config: Option<BankConfig>,
}

#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub enum BankMint {
    /// $1
    Usdc,
    /// $2
    Fixed,
    /// 0.0001
    FixedLow,
    /// $10
    Sol,
    /// $10
    SolSwbPull,
    /// $10
    SolSwbOrigFee,
    /// $10
    SolEquivalent,
    /// $10
    SolEquivalent1,
    /// $10
    SolEquivalent2,
    /// $10
    SolEquivalent3,
    /// $10
    SolEquivalent4,
    /// $10
    SolEquivalent5,
    /// $10
    SolEquivalent6,
    /// $10
    SolEquivalent7,
    /// $10
    SolEquivalent8,
    /// $10
    SolEquivalent9,
    /// $1
    UsdcT22,
    /// $0.50 (50 cents)
    T22WithFee,
    /// $1
    PyUSD,
    /// $10
    SolEqIsolated,
    /// $1
    KaminoUsdc,
}

impl Default for BankMint {
    fn default() -> Self {
        Self::Usdc
    }
}

impl BankMint {
    pub fn is_integration_bank(&self) -> bool {
        matches!(self, Self::KaminoUsdc)
    }
}

pub struct TestFixture {
    pub context: Rc<RefCell<ProgramTestContext>>,
    pub marginfi_group: MarginfiGroupFixture,
    pub banks: HashMap<BankMint, BankFixture>,
    pub usdc_mint: MintFixture,
    pub fixed_mint: MintFixture,
    pub fixed_low_mint: MintFixture,
    pub sol_mint: MintFixture,
    pub sol_equivalent_mint: MintFixture,
    pub mnde_mint: MintFixture,
    pub usdc_t22_mint: MintFixture,
    pub pyusd_mint: MintFixture,
}

pub struct KaminoBankSetup {
    pub test_f: TestFixture,
    pub bank_f: BankFixture,
    pub obligation: Pubkey,
    pub reserve_liquidity_supply: Pubkey,
}

impl KaminoBankSetup {
    pub async fn create_user_with_liquidity(
        &self,
        ui_amount: f64,
    ) -> (MarginfiAccountFixture, TokenAccountFixture) {
        let user = self.test_f.create_marginfi_account().await;
        let user_token = self
            .bank_f
            .mint
            .create_token_account_and_mint_to(ui_amount)
            .await;
        (user, user_token)
    }

    pub async fn load_state(&self, user_token: &TokenAccountFixture) -> KaminoStateSnapshot {
        self.test_f
            .load_kamino_state(self.obligation, self.reserve_liquidity_supply, user_token)
            .await
    }

    pub async fn load_reserve(&self) -> MinimalReserve {
        let bank = self.bank_f.load().await;
        self.test_f
            .load_and_deserialize(&bank.integration_acc_1)
            .await
    }

    pub async fn load_user_accounted_collateral(
        &self,
        user: &MarginfiAccountFixture,
    ) -> Option<u64> {
        let user_state = user.load().await;
        let user_balance = user_state.lending_account.get_balance(&self.bank_f.key)?;
        let bank_state = self.bank_f.load().await;
        Some(
            bank_state
                .get_asset_amount(user_balance.asset_shares.into())
                .unwrap()
                .to_num::<u64>(),
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KaminoStateSnapshot {
    pub user_balance: u64,
    pub reserve_supply_balance: u64,
    pub obligation_collateral: u64,
}

pub struct DriftBankSetup {
    pub test_f: TestFixture,
    pub bank_f: BankFixture,
    pub spot_market_vault: Pubkey,
    pub market_index: u16,
}

impl DriftBankSetup {
    pub async fn create_user_with_liquidity(
        &self,
        ui_amount: f64,
    ) -> (MarginfiAccountFixture, TokenAccountFixture) {
        let user = self.test_f.create_marginfi_account().await;
        let user_token = self
            .bank_f
            .mint
            .create_token_account_and_mint_to(ui_amount)
            .await;
        (user, user_token)
    }

    pub async fn load_state(&self, user_token: &TokenAccountFixture) -> DriftStateSnapshot {
        self.test_f
            .load_drift_state(
                &self.bank_f,
                self.spot_market_vault,
                self.market_index,
                user_token,
            )
            .await
    }

    pub async fn load_spot_market(&self) -> MinimalSpotMarket {
        let bank = self.bank_f.load().await;
        self.test_f
            .load_and_deserialize(&bank.integration_acc_1)
            .await
    }

    pub async fn load_user_accounted_scaled_balance(
        &self,
        user: &MarginfiAccountFixture,
    ) -> Option<u64> {
        let user_state = user.load().await;
        let user_balance = user_state.lending_account.get_balance(&self.bank_f.key)?;
        let bank_state = self.bank_f.load().await;
        Some(
            bank_state
                .get_asset_amount(user_balance.asset_shares.into())
                .unwrap()
                .to_num::<u64>(),
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DriftStateSnapshot {
    pub user_balance: u64,
    pub spot_market_vault_balance: u64,
    pub user_scaled_balance: u64,
}

pub struct JuplendBankSetup {
    pub test_f: TestFixture,
    pub bank_f: BankFixture,
    pub lending: Pubkey,
    pub reserve_vault: Pubkey,
}

impl JuplendBankSetup {
    pub async fn create_user_with_liquidity(
        &self,
        ui_amount: f64,
    ) -> (MarginfiAccountFixture, TokenAccountFixture) {
        let user = self.test_f.create_marginfi_account().await;
        let user_token = self
            .bank_f
            .mint
            .create_token_account_and_mint_to(ui_amount)
            .await;
        (user, user_token)
    }

    pub async fn load_state(&self, user_token: &TokenAccountFixture) -> JuplendStateSnapshot {
        self.test_f
            .load_juplend_state(&self.bank_f, self.reserve_vault, user_token)
            .await
    }

    pub async fn load_lending(&self) -> JuplendLending {
        self.test_f.load_and_deserialize(&self.lending).await
    }

    pub async fn load_user_accounted_shares(&self, user: &MarginfiAccountFixture) -> Option<u64> {
        let user_state = user.load().await;
        let user_balance = user_state.lending_account.get_balance(&self.bank_f.key)?;
        let bank_state = self.bank_f.load().await;
        Some(
            bank_state
                .get_asset_amount(user_balance.asset_shares.into())
                .unwrap()
                .to_num::<u64>(),
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JuplendStateSnapshot {
    pub user_balance: u64,
    pub reserve_vault_balance: u64,
    pub f_token_vault_balance: u64,
}

pub const PYTH_USDC_FEED: Pubkey = pubkey!("PythUsdcPrice111111111111111111111111111111");
pub const SWITCHBOARD_USDC_FEED: Pubkey = pubkey!("SwchUsdcPrice111111111111111111111111111111");
pub const PYTH_SOL_FEED: Pubkey = pubkey!("PythSo1Price1111111111111111111111111111111");
pub const SWITCHBOARD_SOL_FEED: Pubkey = pubkey!("SwchSo1Price1111111111111111111111111111111");
pub const PYTH_SOL_EQUIVALENT_FEED: Pubkey = pubkey!("PythSo1Equiva1entPrice111111111111111111111");
pub const PYTH_MNDE_FEED: Pubkey = pubkey!("PythMndePrice111111111111111111111111111111");
pub const FAKE_PYTH_USDC_FEED: Pubkey = pubkey!("FakePythUsdcPrice11111111111111111111111111");
pub const PYTH_PUSH_SOL_FULLV_FEED: Pubkey = pubkey!("PythPushFu11So1Price11111111111111111111111");
pub const PYTH_PUSH_SOL_PARTV_FEED: Pubkey = pubkey!("PythPushHa1fSo1Price11111111111111111111111");
pub const PYTH_PUSH_FULLV_FEED_ID: [u8; 32] = [17; 32];
pub const PYTH_PUSH_PARTV_FEED_ID: [u8; 32] = [18; 32];
pub const PYTH_PUSH_REAL_SOL_FEED_ID: [u8; 32] = [
    239, 13, 139, 111, 218, 44, 235, 164, 29, 161, 93, 64, 149, 209, 218, 57, 42, 13, 47, 142, 208,
    198, 199, 188, 15, 76, 250, 200, 194, 128, 181, 109,
];
pub const PYTH_PUSH_REAL_USDC_FEED_ID: [u8; 32] = [
    234, 160, 32, 198, 28, 196, 121, 113, 40, 19, 70, 28, 225, 83, 137, 74, 150, 166, 192, 11, 33,
    237, 12, 252, 39, 152, 209, 249, 169, 233, 201, 74,
];
pub const INEXISTENT_PYTH_USDC_FEED: Pubkey =
    pubkey!("FakePythUsdcPrice11111111111111111111111111");
pub const PYTH_T22_WITH_FEE_FEED: Pubkey = pubkey!("PythT22WithFeePrice111111111111111111111111");
pub const PYTH_PYUSD_FEED: Pubkey = pubkey!("PythPyusdPrice11111111111111111111111111111");
pub const PYTH_PUSH_USDC_REAL_FEED: Pubkey = pubkey!("PythPushUsdcRea1Price1111111111111111111111");
pub const PYTH_PUSH_SOL_REAL_FEED: Pubkey = pubkey!("PythPushSo1Rea1Price11111111111111111111111");

pub const SWITCH_PULL_SOL_REAL_FEED: Pubkey =
    pubkey!("BSzfJs4d1tAkSDqkepnfzEVcx2WtDVnwwXa2giy9PLeP");

pub fn get_oracle_id_from_feed_id(feed_id: Pubkey) -> Option<Pubkey> {
    match feed_id.to_bytes() {
        PYTH_PUSH_FULLV_FEED_ID => Some(PYTH_PUSH_SOL_FULLV_FEED),
        PYTH_PUSH_PARTV_FEED_ID => Some(PYTH_PUSH_SOL_PARTV_FEED),
        PYTH_PUSH_REAL_SOL_FEED_ID => Some(PYTH_PUSH_SOL_REAL_FEED),
        PYTH_PUSH_REAL_USDC_FEED_ID => Some(PYTH_PUSH_USDC_REAL_FEED),
        _ => None,
    }
}

pub fn create_oracle_key_array(pyth_oracle: Pubkey) -> [Pubkey; MAX_ORACLE_KEYS] {
    let mut keys = [Pubkey::default(); MAX_ORACLE_KEYS];
    keys[0] = pyth_oracle;

    keys
}

lazy_static! {
    pub static ref DEFAULT_TEST_BANK_INTEREST_RATE_CONFIG: InterestRateConfig =
        InterestRateConfig {
            // TODO deprecate in 1.7
            optimal_utilization_rate: I80F48!(0.5).into(),
            plateau_interest_rate: I80F48!(0.6).into(),
            max_interest_rate: I80F48!(3).into(),

            insurance_fee_fixed_apr: I80F48!(0).into(),
            insurance_ir_fee: I80F48!(0).into(),
            protocol_ir_fee: I80F48!(0).into(),
            protocol_fixed_fee_apr: I80F48!(0).into(),
            protocol_origination_fee: I80F48!(0).into(),

            zero_util_rate: milli_to_u32(I80F48!(0)),
            hundred_util_rate: milli_to_u32(I80F48!(3)),
            points: make_points(&[
                RatePoint::new(centi_to_u32(I80F48!(0.5)), milli_to_u32(I80F48!(0.6))),
            ]),
            curve_type: INTEREST_CURVE_SEVEN_POINT,
            ..Default::default()
        };
    pub static ref DEFAULT_TEST_BANK_CONFIG: BankConfig = BankConfig {
        oracle_setup: OracleSetup::PythPushOracle,
        asset_weight_maint: I80F48!(1).into(),
        asset_weight_init: I80F48!(1).into(),
        liability_weight_init: I80F48!(1).into(),
        liability_weight_maint: I80F48!(1).into(),

        operational_state: BankOperationalState::Operational,
        risk_tier: RiskTier::Collateral,
        config_flags: PYTH_PUSH_MIGRATED_DEPRECATED,

        interest_rate_config: InterestRateConfig {
            // TODO deprecate in 1.7
            optimal_utilization_rate: I80F48!(0).into(),
            plateau_interest_rate: I80F48!(0).into(),
            max_interest_rate: I80F48!(0).into(),

            insurance_fee_fixed_apr: I80F48!(0).into(),
            insurance_ir_fee: I80F48!(0).into(),
            protocol_ir_fee: I80F48!(0).into(),
            protocol_fixed_fee_apr: I80F48!(0).into(),
            protocol_origination_fee: I80F48!(0).into(),

            zero_util_rate: milli_to_u32(I80F48!(0)),
            hundred_util_rate: milli_to_u32(I80F48!(3)),
            points: make_points(&[
                RatePoint::new(centi_to_u32(I80F48!(0.5)), milli_to_u32(I80F48!(0.6))),
            ]),
            curve_type: INTEREST_CURVE_SEVEN_POINT,
            ..Default::default()
        },
        oracle_max_age: 100,
        ..Default::default()
    };
    pub static ref DEFAULT_USDC_TEST_BANK_CONFIG: BankConfig = BankConfig {
        deposit_limit: native!(1_000_000_000, "USDC"),
        borrow_limit: native!(1_000_000_000, "USDC"),
        oracle_keys: create_oracle_key_array(PYTH_USDC_FEED),
        ..*DEFAULT_TEST_BANK_CONFIG
    };
    pub static ref DEFAULT_FIXED_TEST_BANK_CONFIG: BankConfig = BankConfig {
        oracle_setup: OracleSetup::Fixed,
        deposit_limit: native!(1_000_000_000, "FIXED"),
        borrow_limit: native!(1_000_000_000, "FIXED"),
        fixed_price: I80F48!(2.0).into(),
        ..*DEFAULT_TEST_BANK_CONFIG
    };
    pub static ref DEFAULT_FIXED_LOW_TEST_BANK_CONFIG: BankConfig = BankConfig {
        oracle_setup: OracleSetup::Fixed,
        deposit_limit: native!(1_000_000_000, "FIXED_LOW"),
        borrow_limit: native!(1_000_000_000, "FIXED_LOW"),
        fixed_price: I80F48!(0.0001).into(),
        ..*DEFAULT_TEST_BANK_CONFIG
    };
    pub static ref DEFAULT_PYUSD_TEST_BANK_CONFIG: BankConfig = BankConfig {
        deposit_limit: native!(1_000_000_000, "PYUSD"),
        borrow_limit: native!(1_000_000_000, "PYUSD"),
        oracle_keys: create_oracle_key_array(PYTH_PYUSD_FEED),
        ..*DEFAULT_TEST_BANK_CONFIG
    };
    pub static ref DEFAULT_SOL_EQ_ISO_TEST_BANK_CONFIG: BankConfig = BankConfig {
        deposit_limit: native!(1_000_000, "SOL_EQ_ISO"),
        borrow_limit: native!(1_000_000, "SOL_EQ_ISO"),
        oracle_keys: create_oracle_key_array(PYTH_SOL_EQUIVALENT_FEED),
        risk_tier: RiskTier::Isolated,
        asset_weight_maint: I80F48!(0).into(),
        asset_weight_init: I80F48!(0).into(),
        ..*DEFAULT_TEST_BANK_CONFIG
    };
    pub static ref DEFAULT_T22_WITH_FEE_TEST_BANK_CONFIG: BankConfig = BankConfig {
        deposit_limit: native!(1_000_000_000, "T22_WITH_FEE"),
        borrow_limit: native!(1_000_000_000, "T22_WITH_FEE"),
        oracle_keys: create_oracle_key_array(PYTH_T22_WITH_FEE_FEED),
        ..*DEFAULT_TEST_BANK_CONFIG
    };
    pub static ref DEFAULT_SOL_TEST_BANK_CONFIG: BankConfig = BankConfig {
        deposit_limit: native!(1_000_000, "SOL"),
        borrow_limit: native!(1_000_000, "SOL"),
        oracle_keys: create_oracle_key_array(PYTH_SOL_FEED),
        ..*DEFAULT_TEST_BANK_CONFIG
    };
    pub static ref DEFAULT_SOL_EQUIVALENT_TEST_BANK_CONFIG: BankConfig = BankConfig {
        deposit_limit: native!(1_000_000, "SOL_EQ"),
        borrow_limit: native!(1_000_000, "SOL_EQ"),
        oracle_keys: create_oracle_key_array(PYTH_SOL_EQUIVALENT_FEED),
        ..*DEFAULT_TEST_BANK_CONFIG
    };
    pub static ref DEFAULT_MNDE_TEST_BANK_CONFIG: BankConfig = BankConfig {
        deposit_limit: native!(1_000_000, "MNDE"),
        borrow_limit: native!(1_000_000, "MNDE"),
        oracle_keys: create_oracle_key_array(PYTH_MNDE_FEED),
        ..*DEFAULT_TEST_BANK_CONFIG
    };
    pub static ref DEFAULT_SOL_TEST_PYTH_PUSH_FULLV_BANK_CONFIG: BankConfig = BankConfig {
        deposit_limit: native!(1_000_000, "SOL"),
        borrow_limit: native!(1_000_000, "SOL"),
        oracle_keys: create_oracle_key_array(PYTH_PUSH_SOL_FULLV_FEED),
        ..*DEFAULT_TEST_BANK_CONFIG
    };
    /// This banks orale always has an insufficient verification level.
    pub static ref DEFAULT_SOL_TEST_PYTH_PUSH_PARTV_BANK_CONFIG: BankConfig = BankConfig {
        deposit_limit: native!(1_000_000, "SOL"),
        borrow_limit: native!(1_000_000, "SOL"),
        oracle_keys: create_oracle_key_array(PYTH_PUSH_SOL_PARTV_FEED),
        ..*DEFAULT_TEST_BANK_CONFIG
    };
    pub static ref DEFAULT_USDC_TEST_REAL_BANK_CONFIG: BankConfig = BankConfig {
        deposit_limit: native!(1_000_000_000, "USDC"),
        borrow_limit: native!(1_000_000_000, "USDC"),
        oracle_keys: create_oracle_key_array(PYTH_PUSH_USDC_REAL_FEED),
        ..*DEFAULT_TEST_BANK_CONFIG
    };
    pub static ref DEFAULT_PYTH_PUSH_SOL_TEST_REAL_BANK_CONFIG: BankConfig = BankConfig {
        deposit_limit: native!(1_000_000, "SOL"),
        borrow_limit: native!(1_000_000, "SOL"),
        oracle_keys: create_oracle_key_array(PYTH_PUSH_SOL_REAL_FEED),
        oracle_max_age: 100,
        ..*DEFAULT_TEST_BANK_CONFIG
    };
    pub static ref DEFAULT_SB_PULL_SOL_TEST_REAL_BANK_CONFIG: BankConfig = BankConfig {
        oracle_setup: OracleSetup::SwitchboardPull,
        deposit_limit: native!(1_000_000, "SOL"),
        borrow_limit: native!(1_000_000, "SOL"),
        oracle_keys: create_oracle_key_array(SWITCH_PULL_SOL_REAL_FEED),
        ..*DEFAULT_TEST_BANK_CONFIG
    };
    pub static ref DEFAULT_SB_PULL_WITH_ORIGINATION_FEE_BANK_CONFIG: BankConfig = BankConfig {
        oracle_setup: OracleSetup::SwitchboardPull,
        deposit_limit: native!(1_000_000, "SOL"),
        borrow_limit: native!(1_000_000, "SOL"),
        oracle_keys: create_oracle_key_array(SWITCH_PULL_SOL_REAL_FEED),
        interest_rate_config: InterestRateConfig {
            protocol_origination_fee: I80F48!(0.018).into(),
            ..*DEFAULT_TEST_BANK_INTEREST_RATE_CONFIG
        },
        ..*DEFAULT_TEST_BANK_CONFIG
    };
}

pub const USDC_MINT_DECIMALS: u8 = 6;
pub const FIXED_MINT_DECIMALS: u8 = 6;
pub const PYUSD_MINT_DECIMALS: u8 = 6;
pub const T22_WITH_FEE_MINT_DECIMALS: u8 = 6;
pub const SOL_MINT_DECIMALS: u8 = 9;
pub const MNDE_MINT_DECIMALS: u8 = 9;

pub fn marginfi_entry<'info>(
    program_id: &Pubkey,
    accounts: &'info [AccountInfo<'info>],
    data: &[u8],
) -> ProgramResult {
    marginfi::entry(program_id, accounts, data)
}

pub const FAKE_KAMINO_PROGRAM_ID: Pubkey = pubkey!("KFake2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD");
pub const TOKEN_METADATA_PROGRAM_ID: Pubkey =
    pubkey!("metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s");
const KAMINO_TEST_BANK_SEED: u64 = 555;
const KAMINO_INIT_OBLIGATION_NOMINAL_AMOUNT: u64 = 500;
const DRIFT_TEST_BANK_SEED: u64 = 777;
const DRIFT_USDC_MARKET_INDEX: u16 = 0;
const DRIFT_INIT_USER_NOMINAL_AMOUNT: u64 = 100;
const JUPLEND_TEST_BANK_SEED: u64 = 888;
const JUPLEND_INIT_POSITION_NOMINAL_AMOUNT: u64 = 1_000_000;

impl TestFixture {
    // Full serialized LendingMarket account payload size (without 8-byte discriminator).
    // Keep this aligned with Kamino's on-chain account layout used by integration tests.
    const KAMINO_LENDING_MARKET_ACCOUNT_LEN: usize = 4656;
    const KAMINO_LENDING_MARKET_BUMP_OFFSET: usize = 16;
    const KAMINO_LENDING_MARKET_SCOPE_CHAIN_OFFSET: usize = 125;
    // `LendingMarket` discriminator from complete Kamino IDL.
    const KAMINO_LENDING_MARKET_DISCRIMINATOR: [u8; 8] = [246, 114, 50, 98, 72, 157, 28, 120];

    pub async fn new(test_settings: Option<TestSettings>) -> TestFixture {
        TestFixture::new_with_t22_extension(test_settings, &[]).await
    }

    pub async fn new_with_t22_extension(
        test_settings: Option<TestSettings>,
        extensions: &[SupportedExtension],
    ) -> TestFixture {
        Self::new_with_t22_extension_inner(test_settings, extensions, false).await
    }

    async fn new_with_t22_extension_inner(
        test_settings: Option<TestSettings>,
        extensions: &[SupportedExtension],
        add_integration_programs: bool,
    ) -> TestFixture {
        let settings = test_settings.clone().unwrap_or_default();
        let mut program = ProgramTest::default();

        let mem_map_not_copy_feature_gate = pubkey!("EenyoWx9UMXYKpR8mW5Jmfmy2fRjzUtM7NduYMY8bx33");
        program.deactivate_feature(mem_map_not_copy_feature_gate);

        program.prefer_bpf(true);
        program.add_program("marginfi", marginfi::ID, None);
        #[cfg(feature = "transfer-hook")]
        program.add_program("test_transfer_hook", TEST_HOOK_ID, None);
        program.add_program("mocks", mocks::ID, None);

        program.prefer_bpf(false);
        let requires_real_kamino_program = add_integration_programs;
        let mut original_sbf_out_dir = None;

        if requires_real_kamino_program {
            original_sbf_out_dir = Some(std::env::var_os("SBF_OUT_DIR"));
            let fixtures_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../tests/fixtures")
                .to_string_lossy()
                .to_string();
            std::env::set_var("SBF_OUT_DIR", fixtures_dir);

            program.prefer_bpf(true);
            program.add_program("kamino_lending", kamino_mocks::kamino_lending::ID, None);
            program.add_program("kamino_farms", kamino_mocks::kamino_farms::ID, None);
            program.add_program("drift", drift_mocks::drift::ID, None);
            program.add_program("juplend_lending", juplend_mocks::ID, None);
            program.add_program("juplend_liquidity", juplend_mocks::liquidity::ID, None);
            program.add_program(
                "juplend_rewards_rate_model",
                juplend_mocks::lending_reward_rate_model::ID,
                None,
            );
            program.add_program("token_metadata", TOKEN_METADATA_PROGRAM_ID, None);
            program.prefer_bpf(false);
        } else {
            program.add_program(
                "kamino_lending",
                kamino_mocks::kamino_lending::ID,
                processor!(mock_kamino_lending_processor),
            );
            program.add_program(
                "kamino_farms",
                kamino_mocks::kamino_farms::ID,
                processor!(mock_kamino_lending_processor),
            );
        }
        program.add_program(
            "fake_kamino_lending",
            FAKE_KAMINO_PROGRAM_ID,
            processor!(mock_kamino_lending_processor),
        );

        let usdc_keypair = Keypair::new();
        let fixed_keypair = Keypair::new();
        let fixed_low_keypair = Keypair::new();
        let sol_keypair = Keypair::new();
        let sol_equivalent_keypair = Keypair::new();
        let mnde_keypair = Keypair::new();
        let usdc_t22_keypair = Keypair::new();
        let t22_with_fee_keypair = Keypair::new();

        program.add_account(
            PYTH_USDC_FEED,
            create_pyth_push_oracle_account(
                PYTH_USDC_FEED.to_bytes(),
                1.0,
                USDC_MINT_DECIMALS.into(),
                None,
                VerificationLevel::Full,
            ),
        );
        program.add_account(
            PYTH_PYUSD_FEED,
            create_pyth_push_oracle_account(
                PYTH_PYUSD_FEED.to_bytes(),
                1.0,
                PYUSD_MINT_DECIMALS.into(),
                None,
                VerificationLevel::Full,
            ),
        );
        program.add_account(
            PYTH_T22_WITH_FEE_FEED,
            create_pyth_push_oracle_account(
                PYTH_T22_WITH_FEE_FEED.to_bytes(),
                0.5,
                T22_WITH_FEE_MINT_DECIMALS.into(),
                None,
                VerificationLevel::Full,
            ),
        );
        program.add_account(
            PYTH_SOL_FEED,
            create_pyth_push_oracle_account(
                PYTH_SOL_FEED.to_bytes(),
                10.0,
                SOL_MINT_DECIMALS.into(),
                None,
                VerificationLevel::Full,
            ),
        );
        program.add_account(
            PYTH_SOL_EQUIVALENT_FEED,
            create_pyth_push_oracle_account(
                PYTH_SOL_EQUIVALENT_FEED.to_bytes(),
                10.0,
                SOL_MINT_DECIMALS.into(),
                None,
                VerificationLevel::Full,
            ),
        );
        program.add_account(
            PYTH_MNDE_FEED,
            create_pyth_push_oracle_account(
                PYTH_MNDE_FEED.to_bytes(),
                10.0,
                MNDE_MINT_DECIMALS.into(),
                None,
                VerificationLevel::Full,
            ),
        );
        program.add_account(
            PYTH_PUSH_SOL_FULLV_FEED,
            create_pyth_push_oracle_account(
                PYTH_PUSH_FULLV_FEED_ID,
                10.0,
                SOL_MINT_DECIMALS.into(),
                None,
                VerificationLevel::Full,
            ),
        );
        program.add_account(
            PYTH_PUSH_SOL_PARTV_FEED,
            create_pyth_push_oracle_account(
                PYTH_PUSH_PARTV_FEED_ID,
                10.0,
                SOL_MINT_DECIMALS.into(),
                None,
                VerificationLevel::Partial { num_signatures: 5 },
            ),
        );
        // From mainnet: https://solana.fm/address/Dpw1EAVrSB1ibxiDQyTAW6Zip3J4Btk2x4SgApQCeFbX
        program.add_account(
            PYTH_PUSH_USDC_REAL_FEED,
            create_pyth_push_oracle_account_from_bytes(
                include_bytes!("../data/pyth_push_usdc_price.bin").to_vec(),
            ),
        );
        program.add_account(
            PYTH_PUSH_SOL_REAL_FEED,
            create_pyth_push_oracle_account_from_bytes(
                include_bytes!("../data/pyth_push_sol_price.bin").to_vec(),
            ),
        );

        // From mainnet: https://solana.fm/address/BSzfJs4d1tAkSDqkepnfzEVcx2WtDVnwwXa2giy9PLeP
        // Sol @ ~ $153
        program.add_account(
            SWITCH_PULL_SOL_REAL_FEED,
            create_switch_pull_oracle_account_from_bytes(
                include_bytes!("../data/swb_pull_sol_price.bin").to_vec(),
            ),
        );

        let context = Rc::new(RefCell::new(program.start_with_context().await));

        if let Some(original_sbf_out_dir) = original_sbf_out_dir {
            if let Some(val) = original_sbf_out_dir {
                std::env::set_var("SBF_OUT_DIR", val);
            } else {
                std::env::remove_var("SBF_OUT_DIR");
            }
        }

        {
            let ctx = context.borrow_mut();
            let mut clock: Clock = ctx.banks_client.get_sysvar().await.unwrap();
            clock.unix_timestamp = 0;
            ctx.set_sysvar(&clock);
        }

        solana_logger::setup_with_default(RUST_LOG_DEFAULT);

        let usdc_mint_f = MintFixture::new(
            Rc::clone(&context),
            Some(usdc_keypair),
            Some(USDC_MINT_DECIMALS),
        )
        .await;

        let fixed_mint_f = MintFixture::new(
            Rc::clone(&context),
            Some(fixed_keypair),
            Some(FIXED_MINT_DECIMALS),
        )
        .await;

        let fixed_low_mint_f = MintFixture::new(
            Rc::clone(&context),
            Some(fixed_low_keypair),
            Some(FIXED_MINT_DECIMALS),
        )
        .await;

        let sol_mint_f = MintFixture::new(
            Rc::clone(&context),
            Some(sol_keypair),
            Some(SOL_MINT_DECIMALS),
        )
        .await;
        let sol_equivalent_mint_f = MintFixture::new(
            Rc::clone(&context),
            Some(sol_equivalent_keypair),
            Some(SOL_MINT_DECIMALS),
        )
        .await;
        let mnde_mint_f = MintFixture::new(
            Rc::clone(&context),
            Some(mnde_keypair),
            Some(MNDE_MINT_DECIMALS),
        )
        .await;
        let usdc_t22_mint_f = MintFixture::new_token_22(
            Rc::clone(&context),
            Some(usdc_t22_keypair),
            Some(USDC_MINT_DECIMALS),
            extensions,
        )
        .await;
        let pyusd_mint_f = MintFixture::new_from_file(&context, "src/fixtures/pyUSD.json");
        let t22_with_fee_mint_f = MintFixture::new_token_22(
            Rc::clone(&context),
            Some(t22_with_fee_keypair),
            Some(T22_WITH_FEE_MINT_DECIMALS),
            &[SupportedExtension::TransferFee],
        )
        .await;

        let tester_group = MarginfiGroupFixture::new(Rc::clone(&context)).await;

        tester_group
            .set_protocol_fees_flag(settings.protocol_fees)
            .await;

        let mut banks = HashMap::new();
        for bank in settings.banks.iter() {
            let (bank_mint, default_config) = match bank.mint {
                BankMint::Usdc => (&usdc_mint_f, *DEFAULT_USDC_TEST_BANK_CONFIG),
                BankMint::Fixed => (&fixed_mint_f, *DEFAULT_FIXED_TEST_BANK_CONFIG),
                BankMint::FixedLow => (&fixed_low_mint_f, *DEFAULT_FIXED_LOW_TEST_BANK_CONFIG),
                BankMint::Sol => (&sol_mint_f, *DEFAULT_SOL_TEST_BANK_CONFIG),
                BankMint::SolSwbPull => (&sol_mint_f, *DEFAULT_SB_PULL_SOL_TEST_REAL_BANK_CONFIG),
                BankMint::SolSwbOrigFee => (
                    &sol_mint_f,
                    *DEFAULT_SB_PULL_WITH_ORIGINATION_FEE_BANK_CONFIG,
                ),
                BankMint::SolEquivalent => (
                    &sol_equivalent_mint_f,
                    *DEFAULT_SOL_EQUIVALENT_TEST_BANK_CONFIG,
                ),
                BankMint::SolEquivalent1 => (
                    &sol_equivalent_mint_f,
                    *DEFAULT_SOL_EQUIVALENT_TEST_BANK_CONFIG,
                ),
                BankMint::SolEquivalent2 => (
                    &sol_equivalent_mint_f,
                    *DEFAULT_SOL_EQUIVALENT_TEST_BANK_CONFIG,
                ),
                BankMint::SolEquivalent3 => (
                    &sol_equivalent_mint_f,
                    *DEFAULT_SOL_EQUIVALENT_TEST_BANK_CONFIG,
                ),
                BankMint::SolEquivalent4 => (
                    &sol_equivalent_mint_f,
                    *DEFAULT_SOL_EQUIVALENT_TEST_BANK_CONFIG,
                ),
                BankMint::SolEquivalent5 => (
                    &sol_equivalent_mint_f,
                    *DEFAULT_SOL_EQUIVALENT_TEST_BANK_CONFIG,
                ),
                BankMint::SolEquivalent6 => (
                    &sol_equivalent_mint_f,
                    *DEFAULT_SOL_EQUIVALENT_TEST_BANK_CONFIG,
                ),
                BankMint::SolEquivalent7 => (
                    &sol_equivalent_mint_f,
                    *DEFAULT_SOL_EQUIVALENT_TEST_BANK_CONFIG,
                ),
                BankMint::SolEquivalent8 => (
                    &sol_equivalent_mint_f,
                    *DEFAULT_SOL_EQUIVALENT_TEST_BANK_CONFIG,
                ),
                BankMint::SolEquivalent9 => (
                    &sol_equivalent_mint_f,
                    *DEFAULT_SOL_EQUIVALENT_TEST_BANK_CONFIG,
                ),
                BankMint::T22WithFee => {
                    (&t22_with_fee_mint_f, *DEFAULT_T22_WITH_FEE_TEST_BANK_CONFIG)
                }
                BankMint::UsdcT22 => (&usdc_t22_mint_f, *DEFAULT_USDC_TEST_BANK_CONFIG),
                BankMint::PyUSD => (&pyusd_mint_f, *DEFAULT_PYUSD_TEST_BANK_CONFIG),
                BankMint::SolEqIsolated => {
                    (&sol_equivalent_mint_f, *DEFAULT_SOL_EQ_ISO_TEST_BANK_CONFIG)
                }
                BankMint::KaminoUsdc => (&usdc_mint_f, *DEFAULT_USDC_TEST_BANK_CONFIG),
            };

            let kamino_f = if matches!(bank.mint, BankMint::KaminoUsdc) {
                let kamino_usdc_f = KaminoFixture::new_from_files(
                    Rc::clone(&context),
                    "src/fixtures/kamino_usdc_reserve.json",
                    "src/fixtures/kamino_usdc_obligation.json",
                );
                Some(kamino_usdc_f)
            } else {
                None
            };
            let price: I80F48 = I80F48::from_num(get_mint_price(bank.mint.clone()));

            banks.insert(
                bank.mint.clone(),
                tester_group
                    .try_lending_pool_add_bank(
                        bank_mint,
                        kamino_f,
                        bank.config.unwrap_or(default_config),
                        Some(price),
                    )
                    .await
                    .unwrap(),
            );
        }

        TestFixture {
            context: Rc::clone(&context),
            marginfi_group: tester_group,
            banks,
            usdc_mint: usdc_mint_f,
            fixed_mint: fixed_mint_f,
            fixed_low_mint: fixed_low_mint_f,
            sol_mint: sol_mint_f,
            sol_equivalent_mint: sol_equivalent_mint_f,
            mnde_mint: mnde_mint_f,
            usdc_t22_mint: usdc_t22_mint_f,
            pyusd_mint: pyusd_mint_f,
        }
    }

    pub async fn create_marginfi_account(&self) -> MarginfiAccountFixture {
        MarginfiAccountFixture::new(Rc::clone(&self.context), &self.marginfi_group.key).await
    }

    pub async fn try_load(
        &self,
        address: &Pubkey,
    ) -> anyhow::Result<Option<Account>, BanksClientError> {
        self.context
            .borrow_mut()
            .banks_client
            .get_account(*address)
            .await
    }

    pub async fn load_and_deserialize<T: anchor_lang::AccountDeserialize>(
        &self,
        address: &Pubkey,
    ) -> T {
        let ai = self
            .context
            .borrow_mut()
            .banks_client
            .get_account(*address)
            .await
            .unwrap()
            .unwrap();

        T::try_deserialize(&mut ai.data.as_slice()).unwrap()
    }

    pub fn payer(&self) -> Pubkey {
        self.context.borrow().payer.pubkey()
    }

    pub fn payer_keypair(&self) -> Keypair {
        clone_keypair(&self.context.borrow().payer)
    }

    pub fn get_bank(&self, bank_mint: &BankMint) -> &BankFixture {
        self.banks.get(bank_mint).unwrap()
    }

    pub fn get_bank_mut(&mut self, bank_mint: &BankMint) -> &mut BankFixture {
        self.banks.get_mut(bank_mint).unwrap()
    }

    pub fn set_time(&self, timestamp: i64) {
        let clock = Clock {
            unix_timestamp: timestamp,
            ..Default::default()
        };
        self.context.borrow_mut().set_sysvar(&clock);
    }

    pub async fn set_pyth_oracle_timestamp(&self, address: Pubkey, timestamp: i64) {
        let mut ctx = self.context.borrow_mut();

        let mut account = ctx
            .banks_client
            .get_account(address)
            .await
            .unwrap()
            .unwrap();

        let data = account.data.as_mut_slice();
        let mut price_update = PriceUpdateV2::deserialize(&mut &data[8..]).unwrap();

        price_update.price_message.publish_time = timestamp;
        price_update.price_message.prev_publish_time = timestamp;

        let mut data = vec![];
        let mut account_data = vec![];

        data.extend_from_slice(PriceUpdateV2::DISCRIMINATOR);

        price_update.serialize(&mut account_data).unwrap();

        data.extend_from_slice(&account_data);

        let mut aso = AccountSharedData::from(account);

        aso.set_data_from_slice(data.as_slice());

        ctx.set_account(&address, &aso);
    }

    pub async fn advance_time(&self, seconds: i64) {
        let mut clock: Clock = self
            .context
            .borrow_mut()
            .banks_client
            .get_sysvar()
            .await
            .unwrap();
        clock.unix_timestamp += seconds;
        self.context.borrow_mut().set_sysvar(&clock);
        self.context
            .borrow_mut()
            .warp_forward_force_reward_interval_end()
            .unwrap();
    }

    pub async fn get_minimum_rent_for_size(&self, size: usize) -> u64 {
        self.context
            .borrow_mut()
            .banks_client
            .get_rent()
            .await
            .unwrap()
            .minimum_balance(size)
    }

    pub async fn get_latest_blockhash(&self) -> Hash {
        self.context
            .borrow_mut()
            .banks_client
            .get_latest_blockhash()
            .await
            .unwrap()
    }

    /// Refresh the cached blockhash in the test context.
    /// Call this in long-running tests to prevent BlockhashNotFound errors.
    pub async fn refresh_blockhash(&self) {
        let blockhash = self
            .context
            .borrow_mut()
            .banks_client
            .get_latest_blockhash()
            .await
            .unwrap();
        self.context.borrow_mut().last_blockhash = blockhash;
    }

    fn derive_kamino_lending_market_authority(lending_market: Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[b"lma", lending_market.as_ref()],
            &kamino_mocks::kamino_lending::ID,
        )
    }

    fn derive_kamino_user_metadata(owner: Pubkey) -> Pubkey {
        Pubkey::find_program_address(
            &[b"user_meta", owner.as_ref()],
            &kamino_mocks::kamino_lending::ID,
        )
        .0
    }

    fn derive_kamino_base_obligation(owner: Pubkey, lending_market: Pubkey) -> Pubkey {
        Pubkey::find_program_address(
            &[
                &[0u8],
                &[0u8],
                owner.as_ref(),
                lending_market.as_ref(),
                system_program::ID.as_ref(),
                system_program::ID.as_ref(),
            ],
            &kamino_mocks::kamino_lending::ID,
        )
        .0
    }

    fn derive_drift_state() -> Pubkey {
        Pubkey::find_program_address(&[b"drift_state"], &drift_mocks::drift::ID).0
    }

    fn derive_drift_signer() -> Pubkey {
        Pubkey::find_program_address(&[b"drift_signer"], &drift_mocks::drift::ID).0
    }

    fn derive_drift_spot_market(market_index: u16) -> Pubkey {
        Pubkey::find_program_address(
            &[b"spot_market", &market_index.to_le_bytes()],
            &drift_mocks::drift::ID,
        )
        .0
    }

    fn derive_drift_spot_market_vault(market_index: u16) -> Pubkey {
        Pubkey::find_program_address(
            &[b"spot_market_vault", &market_index.to_le_bytes()],
            &drift_mocks::drift::ID,
        )
        .0
    }

    fn derive_drift_insurance_fund_vault(market_index: u16) -> Pubkey {
        Pubkey::find_program_address(
            &[b"insurance_fund_vault", &market_index.to_le_bytes()],
            &drift_mocks::drift::ID,
        )
        .0
    }

    fn derive_drift_user(authority: Pubkey, user_index: u16) -> Pubkey {
        Pubkey::find_program_address(
            &[b"user", authority.as_ref(), &user_index.to_le_bytes()],
            &drift_mocks::drift::ID,
        )
        .0
    }

    fn derive_drift_user_stats(authority: Pubkey) -> Pubkey {
        Pubkey::find_program_address(
            &[b"user_stats", authority.as_ref()],
            &drift_mocks::drift::ID,
        )
        .0
    }

    async fn process_ixs(
        ctx: Rc<RefCell<ProgramTestContext>>,
        ixs: &[Instruction],
    ) -> std::result::Result<(), BanksClientError> {
        let tx = {
            let c = ctx.borrow();
            Transaction::new_signed_with_payer(
                ixs,
                Some(&c.payer.pubkey()),
                &[&c.payer],
                c.last_blockhash,
            )
        };
        ctx.borrow_mut()
            .banks_client
            .process_transaction_with_preflight(tx)
            .await
    }

    pub async fn setup_kamino_bank(test_settings: Option<TestSettings>) -> KaminoBankSetup {
        let settings = test_settings.unwrap_or(TestSettings {
            banks: vec![],
            protocol_fees: false,
        });
        let test_f = TestFixture::new_with_t22_extension_inner(Some(settings), &[], true).await;

        let kamino_fixture = KaminoFixture::new_from_files(
            test_f.context.clone(),
            "src/fixtures/kamino_usdc_reserve.json",
            "src/fixtures/kamino_usdc_obligation.json",
        );
        let (reserve_key, _) = load_account_from_file("src/fixtures/kamino_usdc_reserve.json");

        let mut reserve: MinimalReserve = test_f.load_and_deserialize(&reserve_key).await;
        let mut clock = test_f.get_clock().await;
        clock.slot = reserve.slot;
        clock.unix_timestamp = reserve.market_price_last_updated_ts as i64;
        test_f.context.borrow_mut().set_sysvar(&clock);

        // Keep fixture setup simple by disabling reserve farms for these local ix tests.
        if reserve.farm_collateral != Pubkey::default() || reserve.farm_debt != Pubkey::default() {
            let mut reserve_account = test_f.try_load(&reserve_key).await.unwrap().unwrap();
            let reserve_data =
                bytemuck::from_bytes_mut::<MinimalReserve>(&mut reserve_account.data[8..]);
            reserve_data.farm_collateral = Pubkey::default();
            reserve_data.farm_debt = Pubkey::default();
            reserve.farm_collateral = Pubkey::default();
            reserve.farm_debt = Pubkey::default();

            test_f
                .context
                .borrow_mut()
                .set_account(&reserve_key, &AccountSharedData::from(reserve_account));
        }

        let lending_market = reserve.lending_market;
        let (lending_market_authority, lending_market_authority_bump) =
            Self::derive_kamino_lending_market_authority(lending_market);
        let reserve_liquidity_supply = reserve.supply_vault;
        let reserve_collateral_mint = reserve.collateral_mint_pubkey;
        let reserve_collateral_supply = reserve.collateral_supply_vault;
        create_spl_mint_account_if_missing(
            test_f.context.clone(),
            reserve.mint_pubkey,
            test_f.payer(),
            0,
            reserve.mint_decimals as u8,
        )
        .await;
        let reserve_mint =
            MintFixture::from_existing(test_f.context.clone(), reserve.mint_pubkey).await;

        let lending_market_account = test_f.try_load(&lending_market).await.unwrap();
        if lending_market_account.is_none() {
            let mut data = vec![0u8; 8 + Self::KAMINO_LENDING_MARKET_ACCOUNT_LEN];
            data[..8].copy_from_slice(&Self::KAMINO_LENDING_MARKET_DISCRIMINATOR);
            // `lending_market.bump_seed` is used in PDA seed constraints for `lending_market_authority`.
            data[Self::KAMINO_LENDING_MARKET_BUMP_OFFSET
                ..Self::KAMINO_LENDING_MARKET_BUMP_OFFSET + 8]
                .copy_from_slice(&(u64::from(lending_market_authority_bump)).to_le_bytes());
            // Keep refresh behavior aligned with TS tests where `scopePrices` is null.
            // This prevents unconditional price refresh during local fixture setup.
            data[Self::KAMINO_LENDING_MARKET_SCOPE_CHAIN_OFFSET] = 100;
            test_f.context.borrow_mut().set_account(
                &lending_market,
                &Account {
                    lamports: 1_000_000,
                    data,
                    owner: kamino_mocks::kamino_lending::ID,
                    executable: false,
                    rent_epoch: 0,
                }
                .into(),
            );
        }
        create_system_account_if_missing(test_f.context.clone(), lending_market_authority).await;
        create_spl_mint_account_if_missing(
            test_f.context.clone(),
            reserve_collateral_mint,
            lending_market_authority,
            reserve.mint_total_supply,
            reserve.mint_decimals as u8,
        )
        .await;
        create_spl_token_account_if_missing(
            test_f.context.clone(),
            reserve_liquidity_supply,
            reserve.mint_pubkey,
            lending_market_authority,
            reserve.available_amount,
        )
        .await;
        create_spl_token_account_if_missing(
            test_f.context.clone(),
            reserve_collateral_supply,
            reserve_collateral_mint,
            lending_market_authority,
            reserve.mint_total_supply,
        )
        .await;
        let (bank_key, _) = Pubkey::find_program_address(
            &[
                test_f.marginfi_group.key.as_ref(),
                reserve_mint.key.as_ref(),
                &KAMINO_TEST_BANK_SEED.to_le_bytes(),
            ],
            &marginfi::ID,
        );
        let bank_f = BankFixture::new(
            test_f.context.clone(),
            bank_key,
            &reserve_mint,
            Some(kamino_fixture),
        );

        let liquidity_vault_authority =
            find_bank_vault_authority_pda(&bank_key, BankVaultType::Liquidity).0;
        let obligation =
            Self::derive_kamino_base_obligation(liquidity_vault_authority, lending_market);
        create_system_account_if_missing(test_f.context.clone(), obligation).await;

        let bank_config = KaminoConfigCompact::new(
            PYTH_USDC_FEED,
            I80F48!(1).into(),
            I80F48!(1).into(),
            native!(1_000_000, "USDC"),
            OracleSetup::KaminoPythPush,
            BankOperationalState::Operational,
            RiskTier::Collateral,
            PYTH_PUSH_MIGRATED_DEPRECATED,
            1_000_000_000_000,
            100,
            0,
        );

        let add_bank_accounts = marginfi::accounts::LendingPoolAddBankKamino {
            group: test_f.marginfi_group.key,
            admin: test_f.payer(),
            fee_payer: test_f.payer(),
            bank_mint: reserve_mint.key,
            bank: bank_key,
            integration_acc_1: reserve_key,
            integration_acc_2: obligation,
            liquidity_vault_authority,
            liquidity_vault: find_bank_vault_pda(&bank_key, BankVaultType::Liquidity).0,
            insurance_vault_authority: find_bank_vault_authority_pda(
                &bank_key,
                BankVaultType::Insurance,
            )
            .0,
            insurance_vault: find_bank_vault_pda(&bank_key, BankVaultType::Insurance).0,
            fee_vault_authority: find_bank_vault_authority_pda(&bank_key, BankVaultType::Fee).0,
            fee_vault: find_bank_vault_pda(&bank_key, BankVaultType::Fee).0,
            token_program: reserve_mint.token_program,
            system_program: system_program::ID,
        };
        let mut add_bank_ix = Instruction {
            program_id: marginfi::ID,
            accounts: add_bank_accounts.to_account_metas(Some(true)),
            data: marginfi::instruction::LendingPoolAddBankKamino {
                bank_config,
                bank_seed: KAMINO_TEST_BANK_SEED,
            }
            .data(),
        };
        add_bank_ix
            .accounts
            .push(AccountMeta::new_readonly(PYTH_USDC_FEED, false));
        add_bank_ix
            .accounts
            .push(AccountMeta::new_readonly(reserve_key, false));
        Self::process_ixs(test_f.context.clone(), &[add_bank_ix])
            .await
            .unwrap();

        let init_source = reserve_mint.create_token_account_and_mint_to(1.0).await;
        let user_metadata = Self::derive_kamino_user_metadata(liquidity_vault_authority);
        create_system_account_if_missing(test_f.context.clone(), user_metadata).await;

        let init_ix = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::KaminoInitObligation {
                fee_payer: test_f.payer(),
                bank: bank_key,
                signer_token_account: init_source.key,
                liquidity_vault_authority,
                liquidity_vault: find_bank_vault_pda(&bank_key, BankVaultType::Liquidity).0,
                integration_acc_2: obligation,
                user_metadata,
                lending_market,
                lending_market_authority,
                integration_acc_1: reserve_key,
                mint: reserve_mint.key,
                reserve_liquidity_supply,
                reserve_collateral_mint,
                reserve_destination_deposit_collateral: reserve_collateral_supply,
                obligation_farm_user_state: None,
                reserve_farm_state: None,
                kamino_program: kamino_mocks::kamino_lending::ID,
                farms_program: kamino_mocks::kamino_farms::ID,
                collateral_token_program: spl_token::ID,
                liquidity_token_program: reserve_mint.token_program,
                instruction_sysvar_account: sysvar::instructions::ID,
                rent: sysvar::rent::ID,
                system_program: system_program::ID,
            }
            .to_account_metas(Some(true)),
            data: marginfi::instruction::KaminoInitObligation {
                amount: KAMINO_INIT_OBLIGATION_NOMINAL_AMOUNT,
            }
            .data(),
        };
        let cu_ix = ComputeBudgetInstruction::set_compute_unit_limit(2_000_000);
        Self::process_ixs(test_f.context.clone(), &[cu_ix, init_ix])
            .await
            .unwrap();

        KaminoBankSetup {
            test_f,
            bank_f,
            obligation,
            reserve_liquidity_supply,
        }
    }

    pub async fn setup_drift_bank(test_settings: Option<TestSettings>) -> DriftBankSetup {
        let settings = test_settings.unwrap_or(TestSettings {
            banks: vec![],
            protocol_fees: false,
        });
        let test_f = TestFixture::new_with_t22_extension_inner(Some(settings), &[], true).await;

        let drift_state = Self::derive_drift_state();
        let drift_signer = Self::derive_drift_signer();
        let spot_market = Self::derive_drift_spot_market(DRIFT_USDC_MARKET_INDEX);
        let spot_market_vault = Self::derive_drift_spot_market_vault(DRIFT_USDC_MARKET_INDEX);
        let insurance_fund_vault = Self::derive_drift_insurance_fund_vault(DRIFT_USDC_MARKET_INDEX);

        let init_drift_state_ix = Instruction {
            program_id: drift_mocks::drift::ID,
            accounts: drift::accounts::Initialize {
                admin: test_f.payer(),
                state: drift_state,
                quote_asset_mint: test_f.usdc_mint.key,
                drift_signer,
                rent: sysvar::rent::ID,
                system_program: system_program::ID,
                token_program: spl_token::ID,
            }
            .to_account_metas(Some(true)),
            data: drift::args::Initialize {}.data(),
        };

        let init_spot_market_ix = Instruction {
            program_id: drift_mocks::drift::ID,
            accounts: drift::accounts::InitializeSpotMarket {
                spot_market,
                spot_market_mint: test_f.usdc_mint.key,
                spot_market_vault,
                insurance_fund_vault,
                drift_signer,
                state: drift_state,
                oracle: system_program::ID,
                admin: test_f.payer(),
                rent: sysvar::rent::ID,
                system_program: system_program::ID,
                token_program: spl_token::ID,
            }
            .to_account_metas(Some(true)),
            data: drift::args::InitializeSpotMarket {
                optimal_utilization: 500_000,
                optimal_borrow_rate: 50_000,
                max_borrow_rate: 1_000_000,
                oracle_source: drift_mocks::drift::types::OracleSource::QuoteAsset,
                initial_asset_weight: 10_000,
                maintenance_asset_weight: 10_000,
                initial_liability_weight: 10_000,
                maintenance_liability_weight: 10_000,
                imf_factor: 0,
                liquidator_fee: 0,
                if_liquidation_fee: 0,
                active_status: true,
                asset_tier: drift_mocks::drift::types::AssetTier::Collateral,
                scale_initial_asset_weight_start: 0,
                withdraw_guard_threshold: 10_000_000_000_000,
                order_tick_size: 1,
                order_step_size: 1,
                if_total_factor: 0,
                name: [0; 32],
            }
            .data(),
        };

        let cu_ix = ComputeBudgetInstruction::set_compute_unit_limit(2_000_000);
        Self::process_ixs(
            test_f.context.clone(),
            &[cu_ix, init_drift_state_ix, init_spot_market_ix],
        )
        .await
        .unwrap();

        let (bank_key, _) = Pubkey::find_program_address(
            &[
                test_f.marginfi_group.key.as_ref(),
                test_f.usdc_mint.key.as_ref(),
                &DRIFT_TEST_BANK_SEED.to_le_bytes(),
            ],
            &marginfi::ID,
        );
        let bank_f = BankFixture::new(test_f.context.clone(), bank_key, &test_f.usdc_mint, None);

        let liquidity_vault_authority =
            find_bank_vault_authority_pda(&bank_key, BankVaultType::Liquidity).0;
        let drift_user = Self::derive_drift_user(liquidity_vault_authority, 0);
        let drift_user_stats = Self::derive_drift_user_stats(liquidity_vault_authority);
        create_system_account_if_missing(test_f.context.clone(), drift_user).await;
        create_system_account_if_missing(test_f.context.clone(), drift_user_stats).await;

        let bank_config = DriftConfigCompact::new(
            PYTH_USDC_FEED,
            I80F48!(1).into(),
            I80F48!(1).into(),
            native!(1_000_000, "USDC"),
            OracleSetup::DriftPythPull,
            BankOperationalState::Operational,
            RiskTier::Collateral,
            PYTH_PUSH_MIGRATED_DEPRECATED,
            1_000_000_000_000,
            100,
            0,
        );

        let add_bank_accounts = marginfi::accounts::LendingPoolAddBankDrift {
            group: test_f.marginfi_group.key,
            admin: test_f.payer(),
            fee_payer: test_f.payer(),
            bank_mint: test_f.usdc_mint.key,
            bank: bank_key,
            integration_acc_1: spot_market,
            integration_acc_2: drift_user,
            integration_acc_3: drift_user_stats,
            liquidity_vault_authority,
            liquidity_vault: find_bank_vault_pda(&bank_key, BankVaultType::Liquidity).0,
            insurance_vault_authority: find_bank_vault_authority_pda(
                &bank_key,
                BankVaultType::Insurance,
            )
            .0,
            insurance_vault: find_bank_vault_pda(&bank_key, BankVaultType::Insurance).0,
            fee_vault_authority: find_bank_vault_authority_pda(&bank_key, BankVaultType::Fee).0,
            fee_vault: find_bank_vault_pda(&bank_key, BankVaultType::Fee).0,
            token_program: spl_token::ID,
            system_program: system_program::ID,
        };
        let mut add_bank_ix = Instruction {
            program_id: marginfi::ID,
            accounts: add_bank_accounts.to_account_metas(Some(true)),
            data: marginfi::instruction::LendingPoolAddBankDrift {
                bank_config,
                bank_seed: DRIFT_TEST_BANK_SEED,
            }
            .data(),
        };
        add_bank_ix
            .accounts
            .push(AccountMeta::new_readonly(PYTH_USDC_FEED, false));
        add_bank_ix
            .accounts
            .push(AccountMeta::new_readonly(spot_market, false));
        Self::process_ixs(test_f.context.clone(), &[add_bank_ix])
            .await
            .unwrap();

        let init_source = test_f.usdc_mint.create_token_account_and_mint_to(1.0).await;
        let init_user_ix = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::DriftInitUser {
                fee_payer: test_f.payer(),
                signer_token_account: init_source.key,
                bank: bank_key,
                liquidity_vault_authority,
                liquidity_vault: find_bank_vault_pda(&bank_key, BankVaultType::Liquidity).0,
                mint: test_f.usdc_mint.key,
                integration_acc_3: drift_user_stats,
                integration_acc_2: drift_user,
                drift_state,
                integration_acc_1: spot_market,
                drift_spot_market_vault: spot_market_vault,
                drift_oracle: None,
                drift_program: drift_mocks::drift::ID,
                token_program: spl_token::ID,
                rent: sysvar::rent::ID,
                system_program: system_program::ID,
            }
            .to_account_metas(Some(true)),
            data: marginfi::instruction::DriftInitUser {
                amount: DRIFT_INIT_USER_NOMINAL_AMOUNT,
            }
            .data(),
        };
        let cu_ix = ComputeBudgetInstruction::set_compute_unit_limit(2_000_000);
        Self::process_ixs(test_f.context.clone(), &[cu_ix, init_user_ix])
            .await
            .unwrap();

        DriftBankSetup {
            test_f,
            bank_f,
            spot_market_vault,
            market_index: DRIFT_USDC_MARKET_INDEX,
        }
    }

    pub async fn setup_juplend_bank(test_settings: Option<TestSettings>) -> JuplendBankSetup {
        let settings = test_settings.unwrap_or(TestSettings {
            banks: vec![],
            protocol_fees: false,
        });
        let test_f = TestFixture::new_with_t22_extension_inner(Some(settings), &[], true).await;

        let mint = test_f.usdc_mint.key;
        let token_program = spl_token::ID;

        let liquidity = derive_juplend_liquidity();
        let auth_list = derive_juplend_auth_list();
        let token_reserve = derive_juplend_token_reserve(mint);
        let rate_model = derive_juplend_rate_model(mint);
        let reserve_vault =
            get_associated_token_address_with_program_id(&liquidity, &mint, &token_program);

        let lending_admin = derive_juplend_lending_admin();
        let f_token_mint = derive_juplend_f_token_mint(mint);
        let lending = derive_juplend_lending(mint, f_token_mint);
        let metadata_account = derive_token_metadata(f_token_mint);

        let rewards_admin = derive_juplend_rewards_admin();
        let rewards_rate_model = derive_juplend_rewards_rate_model(mint);

        let user_supply_position = derive_juplend_supply_position(mint, lending);
        let user_borrow_position = derive_juplend_borrow_position(mint, lending);

        // Seed a small mint supply so Juplend borrow-config admin checks accept
        // non-zero debt ceiling values.
        let _borrow_limit_warmup_source =
            test_f.usdc_mint.create_token_account_and_mint_to(1.0).await;

        let init_liquidity_ix = Instruction {
            program_id: juplend_mocks::liquidity::ID,
            accounts: juplend_liquidity::accounts::InitLiquidity {
                signer: test_f.payer(),
                liquidity,
                auth_list,
                system_program: system_program::ID,
            }
            .to_account_metas(Some(true)),
            data: juplend_liquidity::args::InitLiquidity {
                authority: test_f.payer(),
                revenue_collector: test_f.payer(),
            }
            .data(),
        };

        let init_rewards_admin_ix = Instruction {
            program_id: juplend_mocks::lending_reward_rate_model::ID,
            accounts: juplend_rewards::accounts::InitLendingRewardsAdmin {
                signer: test_f.payer(),
                lending_rewards_admin: rewards_admin,
                system_program: system_program::ID,
            }
            .to_account_metas(Some(true)),
            data: juplend_rewards::args::InitLendingRewardsAdmin {
                authority: test_f.payer(),
                lending_program: juplend_mocks::ID,
            }
            .data(),
        };

        let init_lending_admin_ix = Instruction {
            program_id: juplend_mocks::ID,
            accounts: juplend_lending::accounts::InitLendingAdmin {
                authority: test_f.payer(),
                lending_admin,
                system_program: system_program::ID,
            }
            .to_account_metas(Some(true)),
            data: juplend_lending::args::InitLendingAdmin {
                liquidity_program: juplend_mocks::liquidity::ID,
                rebalancer: test_f.payer(),
                authority: test_f.payer(),
            }
            .data(),
        };

        let init_token_reserve_ix = Instruction {
            program_id: juplend_mocks::liquidity::ID,
            accounts: juplend_liquidity::accounts::InitTokenReserve {
                authority: test_f.payer(),
                liquidity,
                auth_list,
                mint,
                vault: reserve_vault,
                rate_model,
                token_reserve,
                token_program,
                associated_token_program: anchor_spl::associated_token::ID,
                system_program: system_program::ID,
            }
            .to_account_metas(Some(true)),
            data: juplend_liquidity::args::InitTokenReserve {}.data(),
        };

        let update_rate_data_ix = Instruction {
            program_id: juplend_mocks::liquidity::ID,
            accounts: juplend_liquidity::accounts::UpdateRateDataV1 {
                authority: test_f.payer(),
                auth_list,
                rate_model,
                mint,
                token_reserve,
            }
            .to_account_metas(Some(true)),
            data: juplend_liquidity::args::UpdateRateDataV1 {
                rate_data: juplend_mocks::liquidity::types::RateDataV1Params {
                    kink: 8_000,
                    rate_at_utilization_zero: 400,
                    rate_at_utilization_kink: 1_000,
                    rate_at_utilization_max: 15_000,
                },
            }
            .data(),
        };

        let update_token_config_ix = Instruction {
            program_id: juplend_mocks::liquidity::ID,
            accounts: juplend_liquidity::accounts::UpdateTokenConfig {
                authority: test_f.payer(),
                auth_list,
                rate_model,
                mint,
                token_reserve,
            }
            .to_account_metas(Some(true)),
            data: juplend_liquidity::args::UpdateTokenConfig {
                token_config: juplend_mocks::liquidity::types::TokenConfig {
                    token: mint,
                    fee: 0,
                    max_utilization: 10_000,
                },
            }
            .data(),
        };

        let init_rewards_rate_model_ix = Instruction {
            program_id: juplend_mocks::lending_reward_rate_model::ID,
            accounts: juplend_rewards::accounts::InitLendingRewardsRateModel {
                authority: test_f.payer(),
                lending_rewards_admin: rewards_admin,
                mint,
                lending_rewards_rate_model: rewards_rate_model,
                system_program: system_program::ID,
            }
            .to_account_metas(Some(true)),
            data: juplend_rewards::args::InitLendingRewardsRateModel {}.data(),
        };

        let init_lending_ix = Instruction {
            program_id: juplend_mocks::ID,
            accounts: juplend_lending::accounts::InitLending {
                signer: test_f.payer(),
                lending_admin,
                mint,
                f_token_mint,
                metadata_account,
                lending,
                token_reserves_liquidity: token_reserve,
                token_program,
                system_program: system_program::ID,
                sysvar_instruction: sysvar::instructions::ID,
                metadata_program: TOKEN_METADATA_PROGRAM_ID,
                rent: sysvar::rent::ID,
            }
            .to_account_metas(Some(true)),
            data: juplend_lending::args::InitLending {
                symbol: "jlUSDC".to_string(),
                liquidity_program: juplend_mocks::liquidity::ID,
            }
            .data(),
        };

        let set_rewards_rate_model_ix = Instruction {
            program_id: juplend_mocks::ID,
            accounts: juplend_lending::accounts::SetRewardsRateModel {
                signer: test_f.payer(),
                lending_admin,
                lending,
                f_token_mint,
                new_rewards_rate_model: rewards_rate_model,
                supply_token_reserves_liquidity: token_reserve,
            }
            .to_account_metas(Some(true)),
            data: juplend_lending::args::SetRewardsRateModel { mint }.data(),
        };

        let init_protocol_positions_ix = Instruction {
            program_id: juplend_mocks::liquidity::ID,
            accounts: juplend_liquidity::accounts::InitNewProtocol {
                authority: test_f.payer(),
                auth_list,
                user_supply_position,
                user_borrow_position,
                system_program: system_program::ID,
            }
            .to_account_metas(Some(true)),
            data: juplend_liquidity::args::InitNewProtocol {
                supply_mint: mint,
                borrow_mint: mint,
                protocol: lending,
            }
            .data(),
        };

        let update_user_supply_config_ix = Instruction {
            program_id: juplend_mocks::liquidity::ID,
            accounts: juplend_liquidity::accounts::UpdateUserSupplyConfig {
                authority: test_f.payer(),
                protocol: lending,
                auth_list,
                rate_model,
                mint,
                token_reserve,
                user_supply_position,
            }
            .to_account_metas(Some(true)),
            data: juplend_liquidity::args::UpdateUserSupplyConfig {
                user_supply_config: juplend_mocks::liquidity::types::UserSupplyConfig {
                    mode: 1,
                    expand_percent: 2_000,
                    expand_duration: 172_800,
                    base_withdrawal_limit: u64::MAX as u128,
                },
            }
            .data(),
        };

        let update_user_borrow_config_ix = Instruction {
            program_id: juplend_mocks::liquidity::ID,
            accounts: juplend_liquidity::accounts::UpdateUserBorrowConfig {
                authority: test_f.payer(),
                protocol: lending,
                auth_list,
                rate_model,
                mint,
                token_reserve,
                user_borrow_position,
            }
            .to_account_metas(Some(true)),
            data: juplend_liquidity::args::UpdateUserBorrowConfig {
                user_borrow_config: juplend_mocks::liquidity::types::UserBorrowConfig {
                    mode: 1,
                    expand_percent: 2_000,
                    expand_duration: 172_800,
                    // Minimal non-zero limits are enough for tests that only cover
                    // deposit/withdraw; we don't exercise native borrow flows here.
                    base_debt_ceiling: 1,
                    max_debt_ceiling: 1,
                },
            }
            .data(),
        };

        let update_user_class_ix = Instruction {
            program_id: juplend_mocks::liquidity::ID,
            accounts: juplend_liquidity::accounts::UpdateUserClass {
                authority: test_f.payer(),
                auth_list,
            }
            .to_account_metas(Some(true)),
            data: juplend_liquidity::args::UpdateUserClass {
                user_class: vec![juplend_mocks::liquidity::types::AddressU8 {
                    addr: lending,
                    value: 1,
                }],
            }
            .data(),
        };

        let cu_ix = ComputeBudgetInstruction::set_compute_unit_limit(2_000_000);
        Self::process_ixs(
            test_f.context.clone(),
            &[
                cu_ix.clone(),
                init_liquidity_ix,
                init_rewards_admin_ix,
                init_lending_admin_ix,
            ],
        )
        .await
        .unwrap();
        Self::process_ixs(
            test_f.context.clone(),
            &[
                cu_ix.clone(),
                init_token_reserve_ix,
                update_rate_data_ix,
                update_token_config_ix,
            ],
        )
        .await
        .unwrap();
        Self::process_ixs(
            test_f.context.clone(),
            &[
                cu_ix.clone(),
                init_rewards_rate_model_ix,
                init_lending_ix,
                set_rewards_rate_model_ix,
            ],
        )
        .await
        .unwrap();
        Self::process_ixs(
            test_f.context.clone(),
            &[
                cu_ix.clone(),
                init_protocol_positions_ix,
                update_user_supply_config_ix,
                update_user_borrow_config_ix,
                update_user_class_ix,
            ],
        )
        .await
        .unwrap();

        let (bank_key, _) = Pubkey::find_program_address(
            &[
                test_f.marginfi_group.key.as_ref(),
                test_f.usdc_mint.key.as_ref(),
                &JUPLEND_TEST_BANK_SEED.to_le_bytes(),
            ],
            &marginfi::ID,
        );
        let bank_f = BankFixture::new(test_f.context.clone(), bank_key, &test_f.usdc_mint, None);

        let liquidity_vault_authority =
            find_bank_vault_authority_pda(&bank_key, BankVaultType::Liquidity).0;

        let bank_config = JuplendConfigCompact::new(
            PYTH_USDC_FEED,
            I80F48!(1).into(),
            I80F48!(1).into(),
            native!(1_000_000, "USDC"),
            OracleSetup::JuplendPythPull,
            RiskTier::Collateral,
            PYTH_PUSH_MIGRATED_DEPRECATED,
            1_000_000_000_000,
            100,
            0,
        );

        let mut add_bank_ix = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::LendingPoolAddBankJuplend {
                group: test_f.marginfi_group.key,
                admin: test_f.payer(),
                fee_payer: test_f.payer(),
                bank_mint: mint,
                bank: bank_key,
                integration_acc_1: lending,
                liquidity_vault_authority,
                liquidity_vault: find_bank_vault_pda(&bank_key, BankVaultType::Liquidity).0,
                insurance_vault_authority: find_bank_vault_authority_pda(
                    &bank_key,
                    BankVaultType::Insurance,
                )
                .0,
                insurance_vault: find_bank_vault_pda(&bank_key, BankVaultType::Insurance).0,
                fee_vault_authority: find_bank_vault_authority_pda(&bank_key, BankVaultType::Fee).0,
                fee_vault: find_bank_vault_pda(&bank_key, BankVaultType::Fee).0,
                f_token_mint,
                integration_acc_2: Pubkey::find_program_address(
                    &[JUPLEND_F_TOKEN_VAULT_SEED.as_bytes(), bank_key.as_ref()],
                    &marginfi::ID,
                )
                .0,
                token_program,
                system_program: system_program::ID,
            }
            .to_account_metas(Some(true)),
            data: marginfi::instruction::LendingPoolAddBankJuplend {
                bank_config,
                bank_seed: JUPLEND_TEST_BANK_SEED,
            }
            .data(),
        };
        add_bank_ix
            .accounts
            .push(AccountMeta::new_readonly(PYTH_USDC_FEED, false));
        add_bank_ix
            .accounts
            .push(AccountMeta::new_readonly(lending, false));
        Self::process_ixs(test_f.context.clone(), &[add_bank_ix])
            .await
            .unwrap();

        let bank_state = bank_f.load().await;
        create_spl_token_account_if_missing(
            test_f.context.clone(),
            bank_state.integration_acc_3,
            mint,
            liquidity_vault_authority,
            0,
        )
        .await;

        let claim_account = derive_juplend_claim_account(liquidity_vault_authority, mint);
        let init_claim_account_ix = Instruction {
            program_id: juplend_mocks::liquidity::ID,
            accounts: juplend_liquidity::accounts::InitClaimAccount {
                signer: test_f.payer(),
                claim_account,
                system_program: system_program::ID,
            }
            .to_account_metas(Some(true)),
            data: juplend_liquidity::args::InitClaimAccount {
                mint,
                user: liquidity_vault_authority,
            }
            .data(),
        };
        Self::process_ixs(
            test_f.context.clone(),
            &[cu_ix.clone(), init_claim_account_ix],
        )
        .await
        .unwrap();

        let init_source = test_f
            .usdc_mint
            .create_token_account_and_mint_to(10.0)
            .await;
        let init_position_ix = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::JuplendInitPosition {
                fee_payer: test_f.payer(),
                signer_token_account: init_source.key,
                bank: bank_key,
                liquidity_vault_authority,
                liquidity_vault: find_bank_vault_pda(&bank_key, BankVaultType::Liquidity).0,
                mint,
                integration_acc_1: lending,
                f_token_mint,
                integration_acc_2: bank_state.integration_acc_2,
                lending_admin,
                supply_token_reserves_liquidity: token_reserve,
                lending_supply_position_on_liquidity: user_supply_position,
                rate_model,
                vault: reserve_vault,
                liquidity,
                liquidity_program: juplend_mocks::liquidity::ID,
                rewards_rate_model,
                juplend_program: juplend_mocks::ID,
                token_program,
                associated_token_program: anchor_spl::associated_token::ID,
                system_program: system_program::ID,
            }
            .to_account_metas(Some(true)),
            data: marginfi::instruction::JuplendInitPosition {
                amount: JUPLEND_INIT_POSITION_NOMINAL_AMOUNT,
            }
            .data(),
        };
        Self::process_ixs(test_f.context.clone(), &[cu_ix, init_position_ix])
            .await
            .unwrap();

        JuplendBankSetup {
            test_f,
            bank_f,
            lending,
            reserve_vault,
        }
    }

    pub async fn run_kamino_deposit(
        &self,
        bank_f: &BankFixture,
        user: &MarginfiAccountFixture,
        signer_token_account: Pubkey,
        amount: u64,
    ) -> std::result::Result<(), BanksClientError> {
        let refresh_reserve_ix = user.make_kamino_refresh_reserve_ix(bank_f).await;
        let refresh_obligation_ix = user.make_kamino_refresh_obligation_ix(bank_f).await;
        let deposit_ix = user
            .make_kamino_deposit_ix(signer_token_account, bank_f, amount)
            .await;

        let ixs = vec![refresh_reserve_ix, refresh_obligation_ix, deposit_ix];
        Self::process_ixs(self.context.clone(), &ixs).await
    }

    pub async fn run_kamino_withdraw(
        &self,
        bank_f: &BankFixture,
        user: &MarginfiAccountFixture,
        destination_token_account: Pubkey,
        amount: u64,
        withdraw_all: Option<bool>,
    ) -> std::result::Result<(), BanksClientError> {
        let refresh_reserve_ix = user.make_kamino_refresh_reserve_ix(bank_f).await;
        let refresh_obligation_ix = user.make_kamino_refresh_obligation_ix(bank_f).await;
        let withdraw_ix = user
            .make_kamino_withdraw_ix(destination_token_account, bank_f, amount, withdraw_all)
            .await;

        let ixs = vec![refresh_reserve_ix, refresh_obligation_ix, withdraw_ix];
        Self::process_ixs(self.context.clone(), &ixs).await
    }

    pub async fn run_drift_deposit(
        &self,
        bank_f: &BankFixture,
        user: &MarginfiAccountFixture,
        signer_token_account: Pubkey,
        amount: u64,
    ) -> std::result::Result<(), BanksClientError> {
        let cu_ix = ComputeBudgetInstruction::set_compute_unit_limit(2_000_000);
        let deposit_ix = user
            .make_drift_deposit_ix(signer_token_account, bank_f, amount)
            .await;
        let ixs = vec![cu_ix, deposit_ix];
        Self::process_ixs(self.context.clone(), &ixs).await
    }

    pub async fn run_drift_withdraw(
        &self,
        bank_f: &BankFixture,
        user: &MarginfiAccountFixture,
        destination_token_account: Pubkey,
        amount: u64,
        withdraw_all: Option<bool>,
    ) -> std::result::Result<(), BanksClientError> {
        let cu_ix = ComputeBudgetInstruction::set_compute_unit_limit(2_000_000);
        let withdraw_ix = user
            .make_drift_withdraw_ix(destination_token_account, bank_f, amount, withdraw_all)
            .await;
        let ixs = vec![cu_ix, withdraw_ix];
        Self::process_ixs(self.context.clone(), &ixs).await
    }

    pub async fn run_juplend_deposit(
        &self,
        bank_f: &BankFixture,
        user: &MarginfiAccountFixture,
        signer_token_account: Pubkey,
        amount: u64,
    ) -> std::result::Result<(), BanksClientError> {
        let cu_ix = ComputeBudgetInstruction::set_compute_unit_limit(2_000_000);
        let deposit_ix = user
            .make_juplend_deposit_ix(signer_token_account, bank_f, amount)
            .await;
        let ixs = vec![cu_ix, deposit_ix];
        Self::process_ixs(self.context.clone(), &ixs).await
    }

    pub async fn run_juplend_withdraw(
        &self,
        bank_f: &BankFixture,
        user: &MarginfiAccountFixture,
        destination_token_account: Pubkey,
        amount: u64,
        withdraw_all: Option<bool>,
    ) -> std::result::Result<(), BanksClientError> {
        let cu_ix = ComputeBudgetInstruction::set_compute_unit_limit(2_000_000);
        let withdraw_ix = user
            .make_juplend_withdraw_ix(destination_token_account, bank_f, amount, withdraw_all)
            .await;
        let ixs = vec![cu_ix, withdraw_ix];
        Self::process_ixs(self.context.clone(), &ixs).await
    }

    pub async fn load_kamino_state(
        &self,
        obligation: Pubkey,
        reserve_liquidity_supply: Pubkey,
        user_token: &TokenAccountFixture,
    ) -> KaminoStateSnapshot {
        let obligation: MinimalObligation = self.load_and_deserialize(&obligation).await;

        KaminoStateSnapshot {
            user_balance: user_token.balance().await,
            reserve_supply_balance: TokenAccountFixture::fetch(
                self.context.clone(),
                reserve_liquidity_supply,
            )
            .await
            .balance()
            .await,
            obligation_collateral: obligation.deposits[0].deposited_amount,
        }
    }

    pub async fn load_drift_state(
        &self,
        bank_f: &BankFixture,
        spot_market_vault: Pubkey,
        market_index: u16,
        user_token: &TokenAccountFixture,
    ) -> DriftStateSnapshot {
        let bank = bank_f.load().await;
        let drift_user: MinimalUser = self.load_and_deserialize(&bank.integration_acc_2).await;

        DriftStateSnapshot {
            user_balance: user_token.balance().await,
            spot_market_vault_balance: TokenAccountFixture::fetch(
                self.context.clone(),
                spot_market_vault,
            )
            .await
            .balance()
            .await,
            user_scaled_balance: drift_user.get_scaled_balance(market_index),
        }
    }

    pub async fn load_juplend_state(
        &self,
        bank_f: &BankFixture,
        reserve_vault: Pubkey,
        user_token: &TokenAccountFixture,
    ) -> JuplendStateSnapshot {
        let bank = bank_f.load().await;

        JuplendStateSnapshot {
            user_balance: user_token.balance().await,
            reserve_vault_balance: TokenAccountFixture::fetch(self.context.clone(), reserve_vault)
                .await
                .balance()
                .await,
            f_token_vault_balance: TokenAccountFixture::fetch(
                self.context.clone(),
                bank.integration_acc_2,
            )
            .await
            .balance()
            .await,
        }
    }

    pub async fn get_slot(&self) -> u64 {
        self.context
            .borrow_mut()
            .banks_client
            .get_root_slot()
            .await
            .unwrap()
    }

    pub async fn get_clock(&self) -> Clock {
        deserialize::<Clock>(
            &self
                .context
                .borrow_mut()
                .banks_client
                .get_account(sysvar::clock::ID)
                .await
                .unwrap()
                .unwrap()
                .data,
        )
        .unwrap()
    }

    pub async fn get_sufficient_collateral_for_outflow(
        &self,
        outflow_amount: f64,
        outflow_mint: &BankMint,
        collateral_mint: &BankMint,
    ) -> f64 {
        let outflow_bank = self.get_bank(outflow_mint);
        let collateral_bank = self.get_bank(collateral_mint);

        let outflow_mint_price = outflow_bank.get_price().await;
        let collateral_mint_price = collateral_bank.get_price().await;

        let collateral_amount = get_sufficient_collateral_for_outflow(
            outflow_amount,
            outflow_mint_price,
            collateral_mint_price,
        );

        let decimal_scaling = 10.0_f64.powi(collateral_bank.mint.mint.decimals as i32);
        let collateral_amount =
            ((collateral_amount * decimal_scaling).round() + 1.) / decimal_scaling;

        get_max_deposit_amount_pre_fee(collateral_amount)
    }
}

pub fn get_mint_price(mint: BankMint) -> f64 {
    match mint {
        // For the T22 with fee variant, it's 50 cents
        BankMint::T22WithFee => 0.5,
        BankMint::Fixed => 2.0,
        BankMint::FixedLow => 0.0001,
        // For USDC-based and PYUSD mints, the price is roughly 1.0.
        BankMint::Usdc | BankMint::UsdcT22 | BankMint::PyUSD | BankMint::KaminoUsdc => 1.0,
        // For SOL and its equivalents, use the SOL price (here, roughly 10.0).
        BankMint::Sol
        | BankMint::SolSwbPull
        | BankMint::SolSwbOrigFee
        | BankMint::SolEquivalent
        | BankMint::SolEquivalent1
        | BankMint::SolEquivalent2
        | BankMint::SolEquivalent3
        | BankMint::SolEquivalent4
        | BankMint::SolEquivalent5
        | BankMint::SolEquivalent6
        | BankMint::SolEquivalent7
        | BankMint::SolEquivalent8
        | BankMint::SolEquivalent9
        | BankMint::SolEqIsolated => 10.0,
    }
}

fn derive_juplend_liquidity() -> Pubkey {
    Pubkey::find_program_address(&[b"liquidity"], &juplend_mocks::liquidity::ID).0
}

fn derive_juplend_auth_list() -> Pubkey {
    Pubkey::find_program_address(&[b"auth_list"], &juplend_mocks::liquidity::ID).0
}

fn derive_juplend_token_reserve(mint: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"reserve", mint.as_ref()], &juplend_mocks::liquidity::ID).0
}

fn derive_juplend_rate_model(mint: Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[b"rate_model", mint.as_ref()],
        &juplend_mocks::liquidity::ID,
    )
    .0
}

fn derive_juplend_lending_admin() -> Pubkey {
    Pubkey::find_program_address(&[b"lending_admin"], &juplend_mocks::ID).0
}

fn derive_juplend_f_token_mint(mint: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"f_token_mint", mint.as_ref()], &juplend_mocks::ID).0
}

fn derive_juplend_lending(mint: Pubkey, f_token_mint: Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[b"lending", mint.as_ref(), f_token_mint.as_ref()],
        &juplend_mocks::ID,
    )
    .0
}

fn derive_juplend_supply_position(mint: Pubkey, protocol: Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[b"user_supply_position", mint.as_ref(), protocol.as_ref()],
        &juplend_mocks::liquidity::ID,
    )
    .0
}

fn derive_juplend_borrow_position(mint: Pubkey, protocol: Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[b"user_borrow_position", mint.as_ref(), protocol.as_ref()],
        &juplend_mocks::liquidity::ID,
    )
    .0
}

fn derive_juplend_rewards_admin() -> Pubkey {
    Pubkey::find_program_address(
        &[b"lending_rewards_admin"],
        &juplend_mocks::lending_reward_rate_model::ID,
    )
    .0
}

fn derive_juplend_rewards_rate_model(mint: Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[b"lending_rewards_rate_model", mint.as_ref()],
        &juplend_mocks::lending_reward_rate_model::ID,
    )
    .0
}

fn derive_juplend_claim_account(user: Pubkey, mint: Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[b"user_claim", user.as_ref(), mint.as_ref()],
        &juplend_mocks::liquidity::ID,
    )
    .0
}

fn derive_token_metadata(mint: Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[
            b"metadata",
            TOKEN_METADATA_PROGRAM_ID.as_ref(),
            mint.as_ref(),
        ],
        &TOKEN_METADATA_PROGRAM_ID,
    )
    .0
}
