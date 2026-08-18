use anchor_lang::{InstructionData, ToAccountMetas};
use bytemuck::Zeroable;
use fixed_macro::types::I80F48;
use solana_sdk::{instruction::Instruction, pubkey::Pubkey};

fn key(n: u32) -> Pubkey {
    let mut bytes = [0u8; 32];
    bytes[..4].copy_from_slice(&n.to_le_bytes());
    Pubkey::new_from_array(bytes)
}

/// Builds the same instruction through anchor's generated client structs and through the
/// type-crate builder, then asserts the two are identical.
macro_rules! assert_parity {
    ($anchor:ident, $ours:ident, $builder:ident, { $($f:ident: $v:expr),* $(,)? },
     { $($a:ident: $av:expr),* $(,)? }) => {{
        let expected = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::$anchor { $($f: $v),* }.to_account_metas(None),
            data: marginfi::instruction::$ours { $($a: $av),* }.data(),
        };
        assert_eq!(expected, $builder(&$ours { $($f: $v),* } $(, $av)*));
    }};
}

#[test]
fn lending_ix_builders_match_anchor() {
    use marginfi_type_crate::ix_builders::lending::*;

    assert_parity!(LendingAccountDeposit, LendingAccountDeposit, lending_account_deposit, {
        group: key(1),
        marginfi_account: key(2),
        authority: key(3),
        bank: key(4),
        signer_token_account: key(5),
        liquidity_vault: key(6),
        token_program: key(7),
    }, {
        amount: 1u64,
        deposit_up_to_limit: Some(true),
    });

    assert_parity!(LendingAccountRepay, LendingAccountRepay, lending_account_repay, {
        group: key(8),
        marginfi_account: key(9),
        authority: key(10),
        bank: key(11),
        signer_token_account: key(12),
        liquidity_vault: key(13),
        token_program: key(14),
    }, {
        amount: 4u64,
        repay_all: Some(false),
    });

    assert_parity!(LendingAccountWithdraw, LendingAccountWithdraw, lending_account_withdraw, {
        group: key(15),
        marginfi_account: key(16),
        authority: key(17),
        bank: key(18),
        destination_token_account: key(19),
        bank_liquidity_vault_authority: key(20),
        liquidity_vault: key(21),
        token_program: key(22),
    }, {
        amount: 7u64,
        withdraw_all: Some(true),
    });

    assert_parity!(LendingAccountBorrow, LendingAccountBorrow, lending_account_borrow, {
        group: key(23),
        marginfi_account: key(24),
        authority: key(25),
        bank: key(26),
        destination_token_account: key(27),
        bank_liquidity_vault_authority: key(28),
        liquidity_vault: key(29),
        token_program: key(30),
    }, {
        amount: 10u64,
    });

    assert_parity!(LendingAccountCloseBalance, LendingAccountCloseBalance, lending_account_close_balance, {
        group: key(31),
        marginfi_account: key(32),
        authority: key(33),
        bank: key(34),
    }, {
    });

    assert_parity!(LendingAccountLiquidate, LendingAccountLiquidate, lending_account_liquidate, {
        group: key(35),
        asset_bank: key(36),
        liab_bank: key(37),
        liquidator_marginfi_account: key(38),
        authority: key(39),
        liquidatee_marginfi_account: key(40),
        bank_liquidity_vault_authority: key(41),
        bank_liquidity_vault: key(42),
        bank_insurance_vault: key(43),
        token_program: key(44),
    }, {
        asset_amount: 11u64,
        liquidatee_accounts: 12u8,
        liquidator_accounts: 13u8,
    });

    assert_parity!(LendingAccountStartFlashloan, LendingAccountStartFlashloan, lending_account_start_flashloan, {
        marginfi_account: key(45),
        authority: key(46),
        ixs_sysvar: key(47),
    }, {
        end_index: 14u64,
    });

    assert_parity!(LendingAccountEndFlashloan, LendingAccountEndFlashloan, lending_account_end_flashloan, {
        marginfi_account: key(48),
        group: key(49),
        authority: key(50),
    }, {
    });

    assert_parity!(PulseHealth, LendingAccountPulseHealth, lending_account_pulse_health, {
        marginfi_account: key(51),
        group: key(52),
    }, {
    });

    assert_parity!(LendingAccountDeposit, LendingAccountDeposit, lending_account_deposit, {
        group: key(938),
        marginfi_account: key(939),
        authority: key(940),
        bank: key(941),
        signer_token_account: key(942),
        liquidity_vault: key(943),
        token_program: key(944),
    }, {
        amount: 87u64,
        deposit_up_to_limit: None::<bool>,
    });

    assert_parity!(LendingAccountRepay, LendingAccountRepay, lending_account_repay, {
        group: key(945),
        marginfi_account: key(946),
        authority: key(947),
        bank: key(948),
        signer_token_account: key(949),
        liquidity_vault: key(950),
        token_program: key(951),
    }, {
        amount: 88u64,
        repay_all: None::<bool>,
    });

    assert_parity!(LendingAccountWithdraw, LendingAccountWithdraw, lending_account_withdraw, {
        group: key(952),
        marginfi_account: key(953),
        authority: key(954),
        bank: key(955),
        destination_token_account: key(956),
        bank_liquidity_vault_authority: key(957),
        liquidity_vault: key(958),
        token_program: key(959),
    }, {
        amount: 89u64,
        withdraw_all: None::<bool>,
    });
}

#[test]
fn account_ix_builders_match_anchor() {
    use marginfi_type_crate::ix_builders::account::*;

    assert_parity!(MarginfiAccountInitialize, MarginfiAccountInitialize, marginfi_account_initialize, {
        marginfi_group: key(53),
        marginfi_account: key(54),
        authority: key(55),
        fee_payer: key(56),
        system_program: key(57),
    }, {
    });

    assert_parity!(MarginfiAccountInitializePda, MarginfiAccountInitializePda, marginfi_account_initialize_pda, {
        marginfi_group: key(58),
        marginfi_account: key(59),
        authority: key(60),
        fee_payer: key(61),
        instructions_sysvar: key(62),
        system_program: key(63),
    }, {
        account_index: 15u16,
        third_party_id: Some(17u16),
    });

    assert_parity!(MarginfiAccountClose, MarginfiAccountClose, marginfi_account_close, {
        marginfi_account: key(64),
        authority: key(65),
        fee_payer: key(66),
    }, {
    });

    assert_parity!(SetAccountFreeze, MarginfiAccountSetFreeze, marginfi_account_set_freeze, {
        group: key(67),
        marginfi_account: key(68),
        admin: key(69),
    }, {
        frozen: false,
    });

    assert_parity!(MarginfiAccountUpdateEmissionsDestinationAccount, MarginfiAccountUpdateEmissionsDestinationAccount, marginfi_account_update_emissions_destination_account, {
        marginfi_account: key(70),
        authority: key(71),
        destination_account: key(72),
    }, {
    });

    assert_parity!(TransferToNewAccount, TransferToNewAccount, transfer_to_new_account, {
        group: key(73),
        old_marginfi_account: key(74),
        new_marginfi_account: key(75),
        authority: key(76),
        fee_payer: key(77),
        new_authority: key(78),
        global_fee_wallet: key(79),
        fee_state: key(80),
        system_program: key(81),
    }, {
    });

    assert_parity!(TransferToNewAccountPda, TransferToNewAccountPda, transfer_to_new_account_pda, {
        group: key(82),
        old_marginfi_account: key(83),
        new_marginfi_account: key(84),
        authority: key(85),
        fee_payer: key(86),
        new_authority: key(87),
        global_fee_wallet: key(88),
        fee_state: key(89),
        instructions_sysvar: key(90),
        system_program: key(91),
    }, {
        account_index: 19u16,
        third_party_id: Some(21u16),
    });

    assert_parity!(InitLiquidationRecord, MarginfiAccountInitLiqRecord, marginfi_account_init_liq_record, {
        marginfi_account: key(92),
        fee_payer: key(93),
        liquidation_record: key(94),
        system_program: key(95),
    }, {
    });

    assert_parity!(CloseLiquidationRecord, MarginfiAccountCloseLiqRecord, marginfi_account_close_liq_record, {
        marginfi_account: key(96),
        liquidation_record: key(97),
        record_payer: key(98),
    }, {
    });

    assert_parity!(AdminCloseAccount, AdminCloseAccount, admin_close_account, {
        group: key(727),
        marginfi_account: key(728),
        global_fee_wallet: key(729),
    }, {
    });

    assert_parity!(MarginfiAccountInitializePda, MarginfiAccountInitializePda, marginfi_account_initialize_pda, {
        marginfi_group: key(828),
        marginfi_account: key(829),
        authority: key(830),
        fee_payer: key(831),
        instructions_sysvar: key(832),
        system_program: key(833),
    }, {
        account_index: 81u16,
        third_party_id: None::<u16>,
    });

    assert_parity!(TransferToNewAccountPda, TransferToNewAccountPda, transfer_to_new_account_pda, {
        group: key(834),
        old_marginfi_account: key(835),
        new_marginfi_account: key(836),
        authority: key(837),
        fee_payer: key(838),
        new_authority: key(839),
        global_fee_wallet: key(840),
        fee_state: key(841),
        instructions_sysvar: key(842),
        system_program: key(843),
    }, {
        account_index: 82u16,
        third_party_id: None::<u16>,
    });
}

#[test]
fn order_ix_builders_match_anchor() {
    use marginfi_type_crate::ix_builders::order::*;
    use marginfi_type_crate::types::OrderTrigger;

    assert_parity!(PlaceOrder, MarginfiAccountPlaceOrder, marginfi_account_place_order, {
        group: key(99),
        marginfi_account: key(100),
        fee_payer: key(101),
        authority: key(102),
        order: key(103),
        fee_state: key(104),
        global_fee_wallet: key(105),
        system_program: key(106),
    }, {
        bank_keys: vec![key(107), key(108)],
        trigger: OrderTrigger::Both { stop_loss: I80F48!(1).into(), take_profit: I80F48!(2).into(), max_slippage: 25u32 },
    });

    assert_parity!(CloseOrder, MarginfiAccountCloseOrder, marginfi_account_close_order, {
        group: key(109),
        marginfi_account: key(110),
        authority: key(111),
        order: key(112),
        fee_recipient: key(113),
        system_program: key(114),
    }, {
    });

    assert_parity!(KeeperCloseOrder, MarginfiAccountKeeperCloseOrder, marginfi_account_keeper_close_order, {
        marginfi_account: key(115),
        fee_recipient: key(116),
        order: key(117),
    }, {
    });

    assert_parity!(SetKeeperCloseFlags, MarginfiAccountSetKeeperCloseFlags, marginfi_account_set_keeper_close_flags, {
        group: key(118),
        marginfi_account: key(119),
        authority: key(120),
    }, {
        bank_keys_opt: Some(vec![key(121), key(122)]),
    });

    assert_parity!(StartExecuteOrder, MarginfiAccountStartExecuteOrder, marginfi_account_start_execute_order, {
        group: key(123),
        marginfi_account: key(124),
        fee_payer: key(125),
        executor: key(126),
        order: key(127),
        execute_record: key(128),
        instruction_sysvar: key(129),
        system_program: key(130),
    }, {
    });

    assert_parity!(EndExecuteOrder, MarginfiAccountEndExecuteOrder, marginfi_account_end_execute_order, {
        group: key(131),
        marginfi_account: key(132),
        executor: key(133),
        fee_recipient: key(134),
        order: key(135),
        execute_record: key(136),
        fee_state: key(137),
    }, {
    });

    assert_parity!(SetKeeperCloseFlags, MarginfiAccountSetKeeperCloseFlags, marginfi_account_set_keeper_close_flags, {
        group: key(960),
        marginfi_account: key(961),
        authority: key(962),
    }, {
        bank_keys_opt: None::<Vec<Pubkey>>,
    });
}

#[test]
fn liquidation_ix_builders_match_anchor() {
    use marginfi_type_crate::ix_builders::liquidation::*;

    assert_parity!(StartLiquidation, StartLiquidation, start_liquidation, {
        marginfi_account: key(138),
        liquidation_record: key(139),
        group: key(140),
        liquidation_receiver: key(141),
        instruction_sysvar: key(142),
    }, {
    });

    assert_parity!(EndLiquidation, EndLiquidation, end_liquidation, {
        marginfi_account: key(143),
        liquidation_record: key(144),
        group: key(145),
        liquidation_receiver: key(146),
        fee_state: key(147),
        global_fee_wallet: key(148),
        system_program: key(149),
        fee_payer: Some(key(150)),
    }, {
    });

    assert_parity!(EndLiquidation, EndLiquidation, end_liquidation, {
        marginfi_account: key(151),
        liquidation_record: key(152),
        group: key(153),
        liquidation_receiver: key(154),
        fee_state: key(155),
        global_fee_wallet: key(156),
        system_program: key(157),
        fee_payer: None,
    }, {
    });

    assert_parity!(StartDeleverage, StartDeleverage, start_deleverage, {
        marginfi_account: key(158),
        liquidation_record: key(159),
        group: key(160),
        risk_admin: key(161),
        instruction_sysvar: key(162),
    }, {
    });

    assert_parity!(EndDeleverage, EndDeleverage, end_deleverage, {
        marginfi_account: key(163),
        liquidation_record: key(164),
        group: key(165),
        risk_admin: key(166),
    }, {
    });
}

#[test]
fn kamino_ix_builders_match_anchor() {
    use marginfi_type_crate::ix_builders::kamino::*;
    use marginfi_type_crate::types::KaminoConfigCompact;

    assert_parity!(KaminoInitObligation, KaminoInitObligation, kamino_init_obligation, {
        fee_payer: key(167),
        bank: key(168),
        signer_token_account: key(169),
        liquidity_vault_authority: key(170),
        liquidity_vault: key(171),
        integration_acc_2: key(172),
        user_metadata: key(173),
        lending_market: key(174),
        lending_market_authority: key(175),
        integration_acc_1: key(176),
        mint: key(177),
        reserve_liquidity_supply: key(178),
        reserve_collateral_mint: key(179),
        reserve_destination_deposit_collateral: key(180),
        obligation_farm_user_state: Some(key(181)),
        reserve_farm_state: Some(key(182)),
        kamino_program: key(183),
        farms_program: key(184),
        collateral_token_program: key(185),
        liquidity_token_program: key(186),
        instruction_sysvar_account: key(187),
        rent: key(188),
        system_program: key(189),
    }, {
        amount: 30u64,
    });

    assert_parity!(KaminoInitObligation, KaminoInitObligation, kamino_init_obligation, {
        fee_payer: key(190),
        bank: key(191),
        signer_token_account: key(192),
        liquidity_vault_authority: key(193),
        liquidity_vault: key(194),
        integration_acc_2: key(195),
        user_metadata: key(196),
        lending_market: key(197),
        lending_market_authority: key(198),
        integration_acc_1: key(199),
        mint: key(200),
        reserve_liquidity_supply: key(201),
        reserve_collateral_mint: key(202),
        reserve_destination_deposit_collateral: key(203),
        obligation_farm_user_state: None,
        reserve_farm_state: None,
        kamino_program: key(204),
        farms_program: key(205),
        collateral_token_program: key(206),
        liquidity_token_program: key(207),
        instruction_sysvar_account: key(208),
        rent: key(209),
        system_program: key(210),
    }, {
        amount: 31u64,
    });

    assert_parity!(KaminoDeposit, KaminoDeposit, kamino_deposit, {
        group: key(211),
        marginfi_account: key(212),
        authority: key(213),
        bank: key(214),
        signer_token_account: key(215),
        liquidity_vault_authority: key(216),
        liquidity_vault: key(217),
        integration_acc_2: key(218),
        lending_market: key(219),
        lending_market_authority: key(220),
        integration_acc_1: key(221),
        mint: key(222),
        reserve_liquidity_supply: key(223),
        reserve_collateral_mint: key(224),
        reserve_destination_deposit_collateral: key(225),
        obligation_farm_user_state: Some(key(226)),
        reserve_farm_state: Some(key(227)),
        kamino_program: key(228),
        farms_program: key(229),
        collateral_token_program: key(230),
        liquidity_token_program: key(231),
        instruction_sysvar_account: key(232),
    }, {
        amount: 32u64,
        refresh_reserve: Some(false),
    });

    assert_parity!(KaminoDeposit, KaminoDeposit, kamino_deposit, {
        group: key(233),
        marginfi_account: key(234),
        authority: key(235),
        bank: key(236),
        signer_token_account: key(237),
        liquidity_vault_authority: key(238),
        liquidity_vault: key(239),
        integration_acc_2: key(240),
        lending_market: key(241),
        lending_market_authority: key(242),
        integration_acc_1: key(243),
        mint: key(244),
        reserve_liquidity_supply: key(245),
        reserve_collateral_mint: key(246),
        reserve_destination_deposit_collateral: key(247),
        obligation_farm_user_state: None,
        reserve_farm_state: None,
        kamino_program: key(248),
        farms_program: key(249),
        collateral_token_program: key(250),
        liquidity_token_program: key(251),
        instruction_sysvar_account: key(252),
    }, {
        amount: 35u64,
        refresh_reserve: Some(true),
    });

    assert_parity!(KaminoWithdraw, KaminoWithdraw, kamino_withdraw, {
        group: key(253),
        marginfi_account: key(254),
        authority: key(255),
        bank: key(256),
        destination_token_account: key(257),
        liquidity_vault_authority: key(258),
        liquidity_vault: key(259),
        integration_acc_2: key(260),
        lending_market: key(261),
        lending_market_authority: key(262),
        integration_acc_1: key(263),
        mint: key(264),
        reserve_liquidity_supply: key(265),
        reserve_collateral_mint: key(266),
        reserve_source_collateral: key(267),
        obligation_farm_user_state: Some(key(268)),
        reserve_farm_state: Some(key(269)),
        kamino_program: key(270),
        farms_program: key(271),
        collateral_token_program: key(272),
        liquidity_token_program: key(273),
        instruction_sysvar_account: key(274),
    }, {
        amount: 38u64,
        flags: Some(40u8),
    });

    assert_parity!(KaminoWithdraw, KaminoWithdraw, kamino_withdraw, {
        group: key(275),
        marginfi_account: key(276),
        authority: key(277),
        bank: key(278),
        destination_token_account: key(279),
        liquidity_vault_authority: key(280),
        liquidity_vault: key(281),
        integration_acc_2: key(282),
        lending_market: key(283),
        lending_market_authority: key(284),
        integration_acc_1: key(285),
        mint: key(286),
        reserve_liquidity_supply: key(287),
        reserve_collateral_mint: key(288),
        reserve_source_collateral: key(289),
        obligation_farm_user_state: None,
        reserve_farm_state: None,
        kamino_program: key(290),
        farms_program: key(291),
        collateral_token_program: key(292),
        liquidity_token_program: key(293),
        instruction_sysvar_account: key(294),
    }, {
        amount: 41u64,
        flags: Some(43u8),
    });

    assert_parity!(KaminoHarvestReward, KaminoHarvestReward, kamino_harvest_reward, {
        bank: key(295),
        fee_state: key(296),
        destination_token_account: key(297),
        liquidity_vault_authority: key(298),
        user_state: key(299),
        farm_state: key(300),
        global_config: key(301),
        reward_mint: key(302),
        user_reward_ata: key(303),
        rewards_vault: key(304),
        rewards_treasury_vault: key(305),
        farm_vaults_authority: key(306),
        scope_prices: Some(key(307)),
        farms_program: key(308),
        token_program: key(309),
    }, {
        reward_index: 44u64,
    });

    assert_parity!(KaminoHarvestReward, KaminoHarvestReward, kamino_harvest_reward, {
        bank: key(310),
        fee_state: key(311),
        destination_token_account: key(312),
        liquidity_vault_authority: key(313),
        user_state: key(314),
        farm_state: key(315),
        global_config: key(316),
        reward_mint: key(317),
        user_reward_ata: key(318),
        rewards_vault: key(319),
        rewards_treasury_vault: key(320),
        farm_vaults_authority: key(321),
        scope_prices: None,
        farms_program: key(322),
        token_program: key(323),
    }, {
        reward_index: 45u64,
    });

    assert_parity!(LendingPoolAddBankKamino, LendingPoolAddBankKamino, lending_pool_add_bank_kamino, {
        group: key(730),
        admin: key(731),
        fee_payer: key(732),
        bank_mint: key(733),
        bank: key(734),
        integration_acc_1: key(735),
        integration_acc_2: key(736),
        liquidity_vault_authority: key(737),
        liquidity_vault: key(738),
        insurance_vault_authority: key(739),
        insurance_vault: key(740),
        fee_vault_authority: key(741),
        fee_vault: key(742),
        token_program: key(743),
        system_program: key(744),
    }, {
        bank_config: KaminoConfigCompact::default(),
        bank_seed: 88u64,
    });

    assert_parity!(KaminoDeposit, KaminoDeposit, kamino_deposit, {
        group: key(894),
        marginfi_account: key(895),
        authority: key(896),
        bank: key(897),
        signer_token_account: key(898),
        liquidity_vault_authority: key(899),
        liquidity_vault: key(900),
        integration_acc_2: key(901),
        lending_market: key(902),
        lending_market_authority: key(903),
        integration_acc_1: key(904),
        mint: key(905),
        reserve_liquidity_supply: key(906),
        reserve_collateral_mint: key(907),
        reserve_destination_deposit_collateral: key(908),
        obligation_farm_user_state: Some(key(909)),
        reserve_farm_state: Some(key(910)),
        kamino_program: key(911),
        farms_program: key(912),
        collateral_token_program: key(913),
        liquidity_token_program: key(914),
        instruction_sysvar_account: key(915),
    }, {
        amount: 85u64,
        refresh_reserve: None::<bool>,
    });

    assert_parity!(KaminoWithdraw, KaminoWithdraw, kamino_withdraw, {
        group: key(916),
        marginfi_account: key(917),
        authority: key(918),
        bank: key(919),
        destination_token_account: key(920),
        liquidity_vault_authority: key(921),
        liquidity_vault: key(922),
        integration_acc_2: key(923),
        lending_market: key(924),
        lending_market_authority: key(925),
        integration_acc_1: key(926),
        mint: key(927),
        reserve_liquidity_supply: key(928),
        reserve_collateral_mint: key(929),
        reserve_source_collateral: key(930),
        obligation_farm_user_state: Some(key(931)),
        reserve_farm_state: Some(key(932)),
        kamino_program: key(933),
        farms_program: key(934),
        collateral_token_program: key(935),
        liquidity_token_program: key(936),
        instruction_sysvar_account: key(937),
    }, {
        amount: 86u64,
        flags: None::<u8>,
    });
}

#[test]
fn drift_ix_builders_match_anchor() {
    use marginfi_type_crate::ix_builders::drift::*;
    use marginfi_type_crate::types::DriftConfigCompact;

    assert_parity!(DriftInitUser, DriftInitUser, drift_init_user, {
        fee_payer: key(324),
        signer_token_account: key(325),
        bank: key(326),
        liquidity_vault_authority: key(327),
        liquidity_vault: key(328),
        mint: key(329),
        integration_acc_3: key(330),
        integration_acc_2: key(331),
        drift_state: key(332),
        integration_acc_1: key(333),
        drift_spot_market_vault: key(334),
        drift_oracle: Some(key(335)),
        drift_program: key(336),
        token_program: key(337),
        rent: key(338),
        system_program: key(339),
    }, {
        amount: 46u64,
    });

    assert_parity!(DriftInitUser, DriftInitUser, drift_init_user, {
        fee_payer: key(340),
        signer_token_account: key(341),
        bank: key(342),
        liquidity_vault_authority: key(343),
        liquidity_vault: key(344),
        mint: key(345),
        integration_acc_3: key(346),
        integration_acc_2: key(347),
        drift_state: key(348),
        integration_acc_1: key(349),
        drift_spot_market_vault: key(350),
        drift_oracle: None,
        drift_program: key(351),
        token_program: key(352),
        rent: key(353),
        system_program: key(354),
    }, {
        amount: 47u64,
    });

    assert_parity!(DriftDeposit, DriftDeposit, drift_deposit, {
        group: key(355),
        marginfi_account: key(356),
        authority: key(357),
        bank: key(358),
        drift_oracle: Some(key(359)),
        liquidity_vault_authority: key(360),
        liquidity_vault: key(361),
        signer_token_account: key(362),
        drift_state: key(363),
        integration_acc_2: key(364),
        integration_acc_3: key(365),
        integration_acc_1: key(366),
        drift_spot_market_vault: key(367),
        mint: key(368),
        drift_program: key(369),
        token_program: key(370),
        system_program: key(371),
    }, {
        amount: 48u64,
    });

    assert_parity!(DriftDeposit, DriftDeposit, drift_deposit, {
        group: key(372),
        marginfi_account: key(373),
        authority: key(374),
        bank: key(375),
        drift_oracle: None,
        liquidity_vault_authority: key(376),
        liquidity_vault: key(377),
        signer_token_account: key(378),
        drift_state: key(379),
        integration_acc_2: key(380),
        integration_acc_3: key(381),
        integration_acc_1: key(382),
        drift_spot_market_vault: key(383),
        mint: key(384),
        drift_program: key(385),
        token_program: key(386),
        system_program: key(387),
    }, {
        amount: 49u64,
    });

    assert_parity!(DriftWithdraw, DriftWithdraw, drift_withdraw, {
        group: key(388),
        marginfi_account: key(389),
        authority: key(390),
        bank: key(391),
        drift_oracle: Some(key(392)),
        liquidity_vault_authority: key(393),
        liquidity_vault: key(394),
        destination_token_account: key(395),
        drift_state: key(396),
        integration_acc_2: key(397),
        integration_acc_3: key(398),
        integration_acc_1: key(399),
        drift_spot_market_vault: key(400),
        drift_reward_oracle: Some(key(401)),
        drift_reward_spot_market: Some(key(402)),
        drift_reward_mint: Some(key(403)),
        drift_reward_oracle_2: Some(key(404)),
        drift_reward_spot_market_2: Some(key(405)),
        drift_reward_mint_2: Some(key(406)),
        drift_signer: key(407),
        mint: key(408),
        drift_program: key(409),
        token_program: key(410),
        system_program: key(411),
    }, {
        amount: 50u64,
        withdraw_all: Some(false),
    });

    assert_parity!(DriftWithdraw, DriftWithdraw, drift_withdraw, {
        group: key(412),
        marginfi_account: key(413),
        authority: key(414),
        bank: key(415),
        drift_oracle: None,
        liquidity_vault_authority: key(416),
        liquidity_vault: key(417),
        destination_token_account: key(418),
        drift_state: key(419),
        integration_acc_2: key(420),
        integration_acc_3: key(421),
        integration_acc_1: key(422),
        drift_spot_market_vault: key(423),
        drift_reward_oracle: None,
        drift_reward_spot_market: None,
        drift_reward_mint: None,
        drift_reward_oracle_2: None,
        drift_reward_spot_market_2: None,
        drift_reward_mint_2: None,
        drift_signer: key(424),
        mint: key(425),
        drift_program: key(426),
        token_program: key(427),
        system_program: key(428),
    }, {
        amount: 53u64,
        withdraw_all: Some(true),
    });

    assert_parity!(DriftHarvestReward, DriftHarvestReward, drift_harvest_reward, {
        bank: key(429),
        fee_state: key(430),
        liquidity_vault_authority: key(431),
        intermediary_token_account: key(432),
        destination_token_account: key(433),
        drift_state: key(434),
        integration_acc_2: key(435),
        integration_acc_3: key(436),
        harvest_drift_spot_market: key(437),
        harvest_drift_spot_market_vault: key(438),
        drift_signer: key(439),
        reward_mint: key(440),
        drift_program: key(441),
        token_program: key(442),
    }, {
    });

    assert_parity!(DriftClaimBadDebt, DriftClaimBadDebt, drift_claim_bad_debt, {
        payer: key(443),
        bank: key(444),
        fee_state: key(445),
        liquidity_vault_authority: key(446),
        integration_acc_2: key(447),
        integration_acc_3: key(448),
        distributor: key(449),
        claim_status: key(450),
        from: key(451),
        claim_mint: key(452),
        global_fee_wallet: key(453),
        claimant_token_account: key(454),
        destination_token_account: key(455),
        merkle_distributor_program: key(456),
        associated_token_program: key(457),
        token_program: key(458),
        system_program: key(459),
    }, {
        amount: 56u64,
        proof: vec![[58u8; 32], [59u8; 32]],
    });

    assert_parity!(LendingPoolAddBankDrift, LendingPoolAddBankDrift, lending_pool_add_bank_drift, {
        group: key(745),
        admin: key(746),
        fee_payer: key(747),
        bank_mint: key(748),
        bank: key(749),
        integration_acc_1: key(750),
        integration_acc_2: key(751),
        integration_acc_3: key(752),
        liquidity_vault_authority: key(753),
        liquidity_vault: key(754),
        insurance_vault_authority: key(755),
        insurance_vault: key(756),
        fee_vault_authority: key(757),
        fee_vault: key(758),
        token_program: key(759),
        system_program: key(760),
    }, {
        bank_config: DriftConfigCompact::default(),
        bank_seed: 89u64,
    });

    assert_parity!(DriftWithdraw, DriftWithdraw, drift_withdraw, {
        group: key(846),
        marginfi_account: key(847),
        authority: key(848),
        bank: key(849),
        drift_oracle: Some(key(850)),
        liquidity_vault_authority: key(851),
        liquidity_vault: key(852),
        destination_token_account: key(853),
        drift_state: key(854),
        integration_acc_2: key(855),
        integration_acc_3: key(856),
        integration_acc_1: key(857),
        drift_spot_market_vault: key(858),
        drift_reward_oracle: Some(key(859)),
        drift_reward_spot_market: Some(key(860)),
        drift_reward_mint: Some(key(861)),
        drift_reward_oracle_2: Some(key(862)),
        drift_reward_spot_market_2: Some(key(863)),
        drift_reward_mint_2: Some(key(864)),
        drift_signer: key(865),
        mint: key(866),
        drift_program: key(867),
        token_program: key(868),
        system_program: key(869),
    }, {
        amount: 83u64,
        withdraw_all: None::<bool>,
    });
}

#[test]
fn juplend_ix_builders_match_anchor() {
    use marginfi_type_crate::ix_builders::juplend::*;
    use marginfi_type_crate::types::JuplendConfigCompact;

    assert_parity!(JuplendInitPosition, JuplendInitPosition, juplend_init_position, {
        fee_payer: key(460),
        signer_token_account: key(461),
        bank: key(462),
        liquidity_vault_authority: key(463),
        liquidity_vault: key(464),
        mint: key(465),
        integration_acc_1: key(466),
        f_token_mint: key(467),
        integration_acc_2: key(468),
        lending_admin: key(469),
        supply_token_reserves_liquidity: key(470),
        lending_supply_position_on_liquidity: key(471),
        rate_model: key(472),
        vault: key(473),
        liquidity: key(474),
        liquidity_program: key(475),
        rewards_rate_model: key(476),
        juplend_program: key(477),
        token_program: key(478),
        associated_token_program: key(479),
        system_program: key(480),
    }, {
        amount: 60u64,
    });

    assert_parity!(JuplendDeposit, JuplendDeposit, juplend_deposit, {
        group: key(481),
        marginfi_account: key(482),
        authority: key(483),
        bank: key(484),
        signer_token_account: key(485),
        liquidity_vault_authority: key(486),
        liquidity_vault: key(487),
        mint: key(488),
        integration_acc_1: key(489),
        f_token_mint: key(490),
        integration_acc_2: key(491),
        lending_admin: key(492),
        supply_token_reserves_liquidity: key(493),
        lending_supply_position_on_liquidity: key(494),
        rate_model: key(495),
        vault: key(496),
        liquidity: key(497),
        liquidity_program: key(498),
        rewards_rate_model: key(499),
        juplend_program: key(500),
        token_program: key(501),
        associated_token_program: key(502),
        system_program: key(503),
    }, {
        amount: 61u64,
    });

    assert_parity!(JuplendWithdraw, JuplendWithdraw, juplend_withdraw, {
        group: key(504),
        marginfi_account: key(505),
        authority: key(506),
        bank: key(507),
        destination_token_account: key(508),
        liquidity_vault_authority: key(509),
        mint: key(510),
        integration_acc_1: key(511),
        f_token_mint: key(512),
        integration_acc_2: key(513),
        integration_acc_3: key(514),
        lending_admin: key(515),
        supply_token_reserves_liquidity: key(516),
        lending_supply_position_on_liquidity: key(517),
        rate_model: key(518),
        vault: key(519),
        claim_account: key(520),
        liquidity: key(521),
        liquidity_program: key(522),
        rewards_rate_model: key(523),
        juplend_program: key(524),
        token_program: key(525),
        associated_token_program: key(526),
        system_program: key(527),
    }, {
        amount: 62u64,
        withdraw_all: Some(false),
    });

    assert_parity!(LendingPoolAddBankJuplend, LendingPoolAddBankJuplend, lending_pool_add_bank_juplend, {
        group: key(761),
        admin: key(762),
        fee_payer: key(763),
        bank_mint: key(764),
        bank: key(765),
        integration_acc_1: key(766),
        liquidity_vault_authority: key(767),
        liquidity_vault: key(768),
        insurance_vault_authority: key(769),
        insurance_vault: key(770),
        fee_vault_authority: key(771),
        fee_vault: key(772),
        f_token_mint: key(773),
        integration_acc_2: key(774),
        token_program: key(775),
        system_program: key(776),
    }, {
        bank_config: JuplendConfigCompact::default(),
        bank_seed: 90u64,
    });

    assert_parity!(JuplendWithdraw, JuplendWithdraw, juplend_withdraw, {
        group: key(870),
        marginfi_account: key(871),
        authority: key(872),
        bank: key(873),
        destination_token_account: key(874),
        liquidity_vault_authority: key(875),
        mint: key(876),
        integration_acc_1: key(877),
        f_token_mint: key(878),
        integration_acc_2: key(879),
        integration_acc_3: key(880),
        lending_admin: key(881),
        supply_token_reserves_liquidity: key(882),
        lending_supply_position_on_liquidity: key(883),
        rate_model: key(884),
        vault: key(885),
        claim_account: key(886),
        liquidity: key(887),
        liquidity_program: key(888),
        rewards_rate_model: key(889),
        juplend_program: key(890),
        token_program: key(891),
        associated_token_program: key(892),
        system_program: key(893),
    }, {
        amount: 84u64,
        withdraw_all: None::<bool>,
    });
}

#[test]
fn solend_ix_builders_match_anchor() {
    use marginfi_type_crate::ix_builders::solend::*;
    use marginfi_type_crate::types::SolendConfigCompact;

    assert_parity!(SolendInitObligation, SolendInitObligation, solend_init_obligation, {
        fee_payer: key(528),
        bank: key(529),
        signer_token_account: key(530),
        liquidity_vault_authority: key(531),
        liquidity_vault: key(532),
        integration_acc_2: key(533),
        lending_market: key(534),
        lending_market_authority: key(535),
        integration_acc_1: key(536),
        mint: key(537),
        reserve_liquidity_supply: key(538),
        reserve_collateral_mint: key(539),
        reserve_collateral_supply: key(540),
        user_collateral: key(541),
        pyth_price: key(542),
        switchboard_feed: key(543),
        solend_program: key(544),
        token_program: key(545),
        rent: key(546),
        system_program: key(547),
    }, {
        amount: 65u64,
    });

    assert_parity!(SolendDeposit, SolendDeposit, solend_deposit, {
        group: key(548),
        marginfi_account: key(549),
        authority: key(550),
        bank: key(551),
        signer_token_account: key(552),
        liquidity_vault_authority: key(553),
        liquidity_vault: key(554),
        integration_acc_2: key(555),
        lending_market: key(556),
        lending_market_authority: key(557),
        integration_acc_1: key(558),
        mint: key(559),
        reserve_liquidity_supply: key(560),
        reserve_collateral_mint: key(561),
        reserve_collateral_supply: key(562),
        user_collateral: key(563),
        pyth_price: key(564),
        switchboard_feed: key(565),
        solend_program: key(566),
        token_program: key(567),
    }, {
        amount: 66u64,
    });

    assert_parity!(SolendWithdraw, SolendWithdraw, solend_withdraw, {
        group: key(568),
        marginfi_account: key(569),
        authority: key(570),
        bank: key(571),
        destination_token_account: key(572),
        liquidity_vault_authority: key(573),
        liquidity_vault: key(574),
        integration_acc_2: key(575),
        lending_market: key(576),
        lending_market_authority: key(577),
        integration_acc_1: key(578),
        mint: key(579),
        reserve_liquidity_supply: key(580),
        reserve_collateral_mint: key(581),
        reserve_collateral_supply: key(582),
        user_collateral: key(583),
        solend_program: key(584),
        token_program: key(585),
    }, {
        amount: 67u64,
        withdraw_all: Some(true),
    });

    assert_parity!(SolendWithdraw, SolendWithdraw, solend_withdraw, {
        group: key(971),
        marginfi_account: key(972),
        authority: key(973),
        bank: key(974),
        destination_token_account: key(975),
        liquidity_vault_authority: key(976),
        liquidity_vault: key(977),
        integration_acc_2: key(978),
        lending_market: key(979),
        lending_market_authority: key(980),
        integration_acc_1: key(981),
        mint: key(982),
        reserve_liquidity_supply: key(983),
        reserve_collateral_mint: key(984),
        reserve_collateral_supply: key(985),
        user_collateral: key(986),
        solend_program: key(987),
        token_program: key(988),
    }, {
        amount: 90u64,
        withdraw_all: None::<bool>,
    });

    assert_parity!(LendingPoolAddBankSolend, LendingPoolAddBankSolend, lending_pool_add_bank_solend, {
        group: key(1020),
        admin: key(1021),
        fee_payer: key(1022),
        bank_mint: key(1023),
        bank: key(1024),
        integration_acc_1: key(1025),
        integration_acc_2: key(1026),
        liquidity_vault_authority: key(1027),
        liquidity_vault: key(1028),
        insurance_vault_authority: key(1029),
        insurance_vault: key(1030),
        fee_vault_authority: key(1031),
        fee_vault: key(1032),
        token_program: key(1033),
        system_program: key(1034),
    }, {
        bank_config: SolendConfigCompact::default(),
        bank_seed: 27u64,
    });
}

#[test]
fn pool_ix_builders_match_anchor() {
    use marginfi_type_crate::ix_builders::pool::*;
    use marginfi_type_crate::types::{
        BankConfigCompact, BankConfigOpt, EmodeEntry, InterestRateConfigOpt, WrappedI80F48,
    };

    assert_parity!(MarginfiGroupInitialize, MarginfiGroupInitialize, marginfi_group_initialize, {
        marginfi_group: key(586),
        admin: key(587),
        fee_state: key(588),
        system_program: key(589),
    }, {
    });

    assert_parity!(LendingPoolAccrueBankInterest, LendingPoolAccrueBankInterest, lending_pool_accrue_bank_interest, {
        group: key(590),
        bank: key(591),
    }, {
    });

    assert_parity!(LendingPoolPulseBankPriceCache, LendingPoolPulseBankPriceCache, lending_pool_pulse_bank_price_cache, {
        group: key(592),
        bank: key(593),
    }, {
    });

    assert_parity!(LendingPoolCollectBankFees, LendingPoolCollectBankFees, lending_pool_collect_bank_fees, {
        group: key(594),
        bank: key(595),
        liquidity_vault_authority: key(596),
        liquidity_vault: key(597),
        insurance_vault: key(598),
        fee_vault: key(599),
        fee_state: key(600),
        fee_ata: key(601),
        token_program: key(602),
    }, {
    });

    assert_parity!(LendingPoolWithdrawFees, LendingPoolWithdrawFees, lending_pool_withdraw_fees, {
        group: key(603),
        bank: key(604),
        admin: key(605),
        fee_vault: key(606),
        fee_vault_authority: key(607),
        dst_token_account: key(608),
        token_program: key(609),
    }, {
        amount: 70u64,
    });

    assert_parity!(LendingPoolWithdrawFeesPermissionless, LendingPoolWithdrawFeesPermissionless, lending_pool_withdraw_fees_permissionless, {
        group: key(610),
        bank: key(611),
        fee_vault: key(612),
        fee_vault_authority: key(613),
        fees_destination_account: key(614),
        token_program: key(615),
    }, {
        amount: 71u64,
    });

    assert_parity!(LendingPoolWithdrawInsurance, LendingPoolWithdrawInsurance, lending_pool_withdraw_insurance, {
        group: key(616),
        bank: key(617),
        admin: key(618),
        insurance_vault: key(619),
        insurance_vault_authority: key(620),
        dst_token_account: key(621),
        token_program: key(622),
    }, {
        amount: 72u64,
    });

    assert_parity!(LendingPoolUpdateFeesDestinationAccount, LendingPoolUpdateFeesDestinationAccount, lending_pool_update_fees_destination_account, {
        group: key(623),
        bank: key(624),
        admin: key(625),
        destination_account: key(626),
    }, {
    });

    assert_parity!(LendingPoolEmissionsDeposit, LendingPoolEmissionsDeposit, lending_pool_emissions_deposit, {
        group: key(627),
        bank: key(628),
        mint: key(629),
        emissions_funding_account: key(630),
        depositor: key(631),
        liquidity_vault: key(632),
        token_program: key(633),
    }, {
        amount: 73u64,
    });

    assert_parity!(LendingPoolConfigureBank, LendingPoolConfigureBank, lending_pool_configure_bank, {
        group: key(634),
        admin: key(635),
        bank: key(636),
    }, {
        bank_config_opt: BankConfigOpt { deposit_limit: Some(74u64), ..Default::default() },
    });

    assert_parity!(LendingPoolConfigureBankOracle, LendingPoolConfigureBankOracle, lending_pool_configure_bank_oracle, {
        group: key(637),
        admin: key(638),
        bank: key(639),
    }, {
        setup: 75u8,
        oracle: key(640),
    });

    assert_parity!(LendingPoolClearCircuitBreaker, LendingPoolClearCircuitBreaker, lending_pool_clear_circuit_breaker, {
        group: key(641),
        authority: key(642),
        bank: key(643),
    }, {
        reseed_reference: true,
    });

    assert_parity!(LendingPoolHandleBankruptcy, LendingPoolHandleBankruptcy, lending_pool_handle_bankruptcy, {
        group: key(644),
        signer: key(645),
        bank: key(646),
        marginfi_account: key(647),
        liquidity_vault: key(648),
        insurance_vault: key(649),
        insurance_vault_authority: key(650),
        token_program: key(651),
    }, {
    });

    assert_parity!(SyncIndexerFlags, SyncIndexerFlags, sync_indexer_flags, {
        payer: key(652),
    }, {
    });

    assert_parity!(MarginfiGroupConfigure, MarginfiGroupConfigure, marginfi_group_configure, {
        marginfi_group: key(653),
        admin: key(654),
    }, {
        new_admin: Some(key(655)),
        new_emode_admin: Some(key(656)),
        new_curve_admin: Some(key(657)),
        new_limit_admin: Some(key(658)),
        new_flow_admin: Some(key(659)),
        new_emissions_admin: Some(key(660)),
        new_metadata_admin: Some(key(661)),
        new_risk_admin: Some(key(662)),
        emode_max_init_leverage: Some(WrappedI80F48::default()),
        emode_max_maint_leverage: Some(WrappedI80F48::default()),
        same_asset_emode_init_leverage: Some(WrappedI80F48::default()),
        same_asset_emode_maint_leverage: Some(WrappedI80F48::default()),
    });

    assert_parity!(LendingPoolAddBank, LendingPoolAddBank, lending_pool_add_bank, {
        marginfi_group: key(663),
        admin: key(664),
        fee_payer: key(665),
        fee_state: key(666),
        global_fee_wallet: key(667),
        bank_mint: key(668),
        bank: key(669),
        liquidity_vault_authority: key(670),
        liquidity_vault: key(671),
        insurance_vault_authority: key(672),
        insurance_vault: key(673),
        fee_vault_authority: key(674),
        fee_vault: key(675),
        token_program: key(676),
        system_program: key(677),
    }, {
        bank_config: BankConfigCompact::default(),
    });

    assert_parity!(LendingPoolAddBankWithSeed, LendingPoolAddBankWithSeed, lending_pool_add_bank_with_seed, {
        marginfi_group: key(678),
        admin: key(679),
        fee_payer: key(680),
        fee_state: key(681),
        global_fee_wallet: key(682),
        bank_mint: key(683),
        bank: key(684),
        liquidity_vault_authority: key(685),
        liquidity_vault: key(686),
        insurance_vault_authority: key(687),
        insurance_vault: key(688),
        fee_vault_authority: key(689),
        fee_vault: key(690),
        token_program: key(691),
        system_program: key(692),
    }, {
        bank_config: BankConfigCompact::default(),
        bank_seed: 81u64,
    });

    assert_parity!(LendingPoolBackfillBankIsT22Flag, LendingPoolBackfillBankIsT22Flag, lending_pool_backfill_bank_is_t22_flag, {
        bank: key(693),
        group: key(694),
        mint: key(695),
    }, {
        bank_seed: Some(82u64),
    });

    assert_parity!(LendingPoolCloneEmode, LendingPoolCloneEmode, lending_pool_clone_emode, {
        group: key(696),
        signer: key(697),
        copy_from_bank: key(698),
        copy_to_bank: key(699),
    }, {
    });

    assert_parity!(LendingPoolConfigureBankEmode, LendingPoolConfigureBankEmode, lending_pool_configure_bank_emode, {
        group: key(700),
        emode_admin: key(701),
        bank: key(702),
    }, {
        emode_tag: 83u16,
        entries: [EmodeEntry::zeroed(); 10],
    });

    assert_parity!(LendingPoolConfigureBankInterestOnly, LendingPoolConfigureBankInterestOnly, lending_pool_configure_bank_interest_only, {
        group: key(703),
        delegate_curve_admin: key(704),
        bank: key(705),
    }, {
        interest_rate_config: InterestRateConfigOpt::default(),
    });

    assert_parity!(LendingPoolConfigureBankLimitsOnly, LendingPoolConfigureBankLimitsOnly, lending_pool_configure_bank_limits_only, {
        group: key(706),
        delegate_limit_admin: key(707),
        bank: key(708),
    }, {
        deposit_limit: Some(84u64),
        borrow_limit: Some(85u64),
        total_asset_value_init_limit: Some(86u64),
    });

    assert_parity!(LendingPoolInitSameAssetEmodeRegistry, LendingPoolInitSameAssetEmodeRegistry, lending_pool_init_same_asset_emode_registry, {
        group: key(709),
        signer: key(710),
        same_asset_emode_registry: key(711),
        system_program: key(712),
    }, {
    });

    assert_parity!(LendingPoolResizeGroupAccount, LendingPoolResizeGroupAccount, lending_pool_resize_group_account, {
        group: key(713),
        payer: key(714),
        system_program: key(715),
    }, {
    });

    assert_parity!(LendingPoolSetBankSameAssetEmodeEligibility, LendingPoolSetBankSameAssetEmodeEligibility, lending_pool_set_bank_same_asset_emode_eligibility, {
        group: key(716),
        signer: key(717),
        bank: key(718),
        same_asset_emode_registry: key(719),
    }, {
        enabled: true,
    });

    assert_parity!(LendingPoolSetFixedOraclePrice, LendingPoolSetFixedOraclePrice, lending_pool_set_fixed_oracle_price, {
        group: key(720),
        admin: key(721),
        bank: key(722),
    }, {
        price: WrappedI80F48::default(),
    });

    assert_parity!(InitBankMetadata, InitBankMetadata, init_bank_metadata, {
        bank: key(723),
        fee_payer: key(724),
        metadata: key(725),
        system_program: key(726),
    }, {
    });

    assert_parity!(LendingPoolBackfillBankIsT22Flag, LendingPoolBackfillBankIsT22Flag, lending_pool_backfill_bank_is_t22_flag, {
        bank: key(963),
        group: key(964),
        mint: key(965),
    }, {
        bank_seed: None::<u64>,
    });

    assert_parity!(LendingPoolConfigureBankLimitsOnly, LendingPoolConfigureBankLimitsOnly, lending_pool_configure_bank_limits_only, {
        group: key(966),
        delegate_limit_admin: key(967),
        bank: key(968),
    }, {
        deposit_limit: None::<u64>,
        borrow_limit: None::<u64>,
        total_asset_value_init_limit: None::<u64>,
    });

    assert_parity!(MarginfiGroupConfigure, MarginfiGroupConfigure, marginfi_group_configure, {
        marginfi_group: key(969),
        admin: key(970),
    }, {
        new_admin: None::<Pubkey>,
        new_emode_admin: None::<Pubkey>,
        new_curve_admin: None::<Pubkey>,
        new_limit_admin: None::<Pubkey>,
        new_flow_admin: None::<Pubkey>,
        new_emissions_admin: None::<Pubkey>,
        new_metadata_admin: None::<Pubkey>,
        new_risk_admin: None::<Pubkey>,
        emode_max_init_leverage: None::<WrappedI80F48>,
        emode_max_maint_leverage: None::<WrappedI80F48>,
        same_asset_emode_init_leverage: None::<WrappedI80F48>,
        same_asset_emode_maint_leverage: None::<WrappedI80F48>,
    });

    assert_parity!(WriteBankMetadata, WriteBankMetadata, write_bank_metadata, {
        group: key(1011),
        bank: key(1012),
        metadata_admin: key(1013),
        metadata: key(1014),
    }, {
        ticker: Some(vec![18u8, 19u8]),
        description: Some(vec![20u8, 21u8]),
    });

    assert_parity!(WriteBankMetadataPreInit, WriteBankMetadataPreInit, write_bank_metadata_pre_init, {
        group: key(1015),
        bank_mint: key(1016),
        bank: key(1017),
        metadata_admin: key(1018),
        metadata: key(1019),
    }, {
        bank_seed: 21u64,
        ticker: Some(vec![23u8, 24u8]),
        description: Some(vec![25u8, 26u8]),
    });
}

#[test]
fn admin_ix_builders_match_anchor() {
    use marginfi_type_crate::ix_builders::admin::*;
    use marginfi_type_crate::types::StakedSettingsEditConfig;
    use marginfi_type_crate::types::{StakedSettingsConfig, WrappedI80F48};

    assert_parity!(InitFeeState, InitGlobalFeeState, init_global_fee_state, {
        payer: key(777),
        fee_state: key(778),
        system_program: key(779),
    }, {
        admin: key(780),
        fee_wallet: key(781),
        bank_init_flat_sol_fee: 91u32,
        liquidation_flat_sol_fee: 92u32,
        order_init_flat_sol_fee: 93u32,
        program_fee_fixed: WrappedI80F48::default(),
        program_fee_rate: WrappedI80F48::default(),
        liquidation_max_fee: WrappedI80F48::default(),
        order_execution_max_fee: WrappedI80F48::default(),
    });

    assert_parity!(EditFeeState, EditGlobalFeeState, edit_global_fee_state, {
        global_fee_admin: key(782),
        fee_state: key(783),
    }, {
        admin: Some(key(784)),
        fee_wallet: Some(key(785)),
        bank_init_flat_sol_fee: Some(94u32),
        liquidation_flat_sol_fee: Some(95u32),
        order_init_flat_sol_fee: Some(96u32),
        program_fee_fixed: Some(WrappedI80F48::default()),
        program_fee_rate: Some(WrappedI80F48::default()),
        liquidation_max_fee: Some(WrappedI80F48::default()),
        order_execution_max_fee: Some(WrappedI80F48::default()),
        pause_delegate_admin: Some(key(786)),
        account_transfer_fee: Some(97u32),
    });

    assert_parity!(ResizeGlobalFeeState, ResizeGlobalFeeState, resize_global_fee_state, {
        fee_state: key(787),
        payer: key(788),
        system_program: key(789),
    }, {
    });

    assert_parity!(PropagateFee, PropagateFeeState, propagate_fee_state, {
        fee_state: key(790),
        marginfi_group: key(791),
    }, {
    });

    assert_parity!(InitStakedSettings, InitStakedSettings, init_staked_settings, {
        marginfi_group: key(792),
        admin: key(793),
        fee_payer: key(794),
        staked_settings: key(795),
        system_program: key(796),
    }, {
        settings: StakedSettingsConfig::default(),
    });

    assert_parity!(PropagateStakedSettings, PropagateStakedSettings, propagate_staked_settings, {
        marginfi_group: key(797),
        staked_settings: key(798),
        bank: key(799),
    }, {
    });

    assert_parity!(EnableStakedOracleOnramp, EnableStakedOracleOnramp, enable_staked_oracle_onramp, {
        group: key(800),
        admin: key(801),
        staked_settings: key(802),
    }, {
    });

    assert_parity!(DisableStakedOracles, DisableStakedOracles, disable_staked_oracles, {
        group: key(803),
        admin: key(804),
        staked_settings: key(805),
    }, {
    });

    assert_parity!(PanicPause, PanicPause, panic_pause, {
        pause_authority: key(806),
        fee_state: key(807),
    }, {
    });

    assert_parity!(PanicUnpause, PanicUnpause, panic_unpause, {
        global_fee_admin: key(808),
        fee_state: key(809),
    }, {
    });

    assert_parity!(PanicUnpausePermissionless, PanicUnpausePermissionless, panic_unpause_permissionless, {
        fee_state: key(810),
    }, {
    });

    assert_parity!(SuperAdminDeposit, SuperAdminDeposit, super_admin_deposit, {
        group: key(811),
        admin: key(812),
        bank: key(813),
        admin_token_account: key(814),
        liquidity_vault: key(815),
        token_program: key(816),
    }, {
        amount: 98u64,
    });

    assert_parity!(SuperAdminWithdraw, SuperAdminWithdraw, super_admin_withdraw, {
        group: key(817),
        admin: key(818),
        bank: key(819),
        destination_token_account: key(820),
        liquidity_vault_authority: key(821),
        liquidity_vault: key(822),
        token_program: key(823),
    }, {
        amount: 99u64,
    });

    assert_parity!(ConfigureDeleverageWithdrawalLimit, ConfigureDeleverageWithdrawalLimit, configure_deleverage_withdrawal_limit, {
        marginfi_group: key(824),
        admin: key(825),
    }, {
        limit: 100u32,
    });

    assert_parity!(UpdateDeleverageWithdrawals, UpdateDeleverageWithdrawals, update_deleverage_withdrawals, {
        marginfi_group: key(826),
        delegate_flow_admin: key(827),
    }, {
        outflow_usd: 101u32,
        update_seq: 102u64,
        event_start_slot: 103u64,
        event_end_slot: 104u64,
    });

    assert_parity!(EditFeeState, EditGlobalFeeState, edit_global_fee_state, {
        global_fee_admin: key(844),
        fee_state: key(845),
    }, {
        admin: None::<Pubkey>,
        fee_wallet: None::<Pubkey>,
        bank_init_flat_sol_fee: None::<u32>,
        liquidation_flat_sol_fee: None::<u32>,
        order_init_flat_sol_fee: None::<u32>,
        program_fee_fixed: None::<WrappedI80F48>,
        program_fee_rate: None::<WrappedI80F48>,
        liquidation_max_fee: None::<WrappedI80F48>,
        order_execution_max_fee: None::<WrappedI80F48>,
        pause_delegate_admin: None::<Pubkey>,
        account_transfer_fee: None::<u32>,
    });

    assert_parity!(EditStakedSettings, EditStakedSettings, edit_staked_settings, {
        marginfi_group: key(1001),
        admin: key(1002),
        staked_settings: key(1003),
    }, {
        settings: StakedSettingsEditConfig::default(),
    });

    assert_parity!(ConfigureBankRateLimits, ConfigureBankRateLimits, configure_bank_rate_limits, {
        group: key(1004),
        admin: key(1005),
        bank: key(1006),
    }, {
        hourly_max_outflow: Some(3u64),
        daily_max_outflow: Some(5u64),
    });

    assert_parity!(ConfigureGroupRateLimits, ConfigureGroupRateLimits, configure_group_rate_limits, {
        marginfi_group: key(1007),
        admin: key(1008),
    }, {
        hourly_max_outflow_usd: Some(7u64),
        daily_max_outflow_usd: Some(9u64),
    });

    assert_parity!(UpdateGroupRateLimiter, UpdateGroupRateLimiter, update_group_rate_limiter, {
        marginfi_group: key(1009),
        delegate_flow_admin: key(1010),
    }, {
        outflow_usd: Some(11u64),
        inflow_usd: Some(13u64),
        update_seq: 14u64,
        event_start_slot: 15u64,
        event_end_slot: 16u64,
    });
}

/// Instructions the SDK deliberately does not build. Governance and migration entrypoints that
/// are driven by an admin runbook rather than a client, so shipping a builder for them would
/// imply a support commitment we do not make.
const NO_BUILDER: &[&str] = &[
    "config_group_fee",
    "lending_pool_add_bank_permissionless",
    "lending_pool_backfill_staked_bank_validator_vote_account",
    "lending_pool_clone_bank",
    "lending_pool_close_bank",
    "lending_pool_force_tokenless_repay_complete",
    "purge_deleverage_balance",
];

/// A new instruction must either get a builder or be named in [`NO_BUILDER`]; landing one with
/// neither fails here rather than silently shipping an SDK that cannot reach it.
#[test]
fn every_idl_instruction_is_covered_or_allowlisted() {
    let idl_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../target/idl/marginfi.json"
    );
    // Build artifact of `anchor build -p marginfi`; skip where it was never generated, matching
    // `ix_utils::tests::check_discrims_match_idl`.
    let Ok(idl_str) = std::fs::read_to_string(idl_path) else {
        eprintln!("skipping every_idl_instruction_is_covered_or_allowlisted: {idl_path} not found");
        return;
    };
    let idl: serde_json::Value = serde_json::from_str(&idl_str).expect("marginfi.json is invalid");

    let idl_names: std::collections::BTreeSet<String> = idl["instructions"]
        .as_array()
        .expect("IDL has no `instructions` array")
        .iter()
        .map(|i| i["name"].as_str().unwrap().to_string())
        .collect();

    let built: std::collections::BTreeSet<String> = marginfi_type_crate::ix_builders::BUILDERS
        .iter()
        .map(|s| s.to_string())
        .collect();
    let allowlisted: std::collections::BTreeSet<String> =
        NO_BUILDER.iter().map(|s| s.to_string()).collect();

    let overlap: Vec<_> = built.intersection(&allowlisted).collect();
    assert!(
        overlap.is_empty(),
        "allowlisted but has a builder: {overlap:?}"
    );

    let covered: std::collections::BTreeSet<String> = built.union(&allowlisted).cloned().collect();
    let uncovered: Vec<_> = idl_names.difference(&covered).collect();
    assert!(
        uncovered.is_empty(),
        "instructions with no builder and not in NO_BUILDER: {uncovered:?}"
    );

    let stale: Vec<_> = covered.difference(&idl_names).collect();
    assert!(
        stale.is_empty(),
        "named here but absent from the IDL: {stale:?}"
    );
}
