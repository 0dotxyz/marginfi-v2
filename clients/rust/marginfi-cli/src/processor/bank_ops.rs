use {
    super::{group_get_all, load_all_banks},
    crate::output,
    crate::{
        config::Config,
        profile::Profile,
        utils::{
            find_bank_emssions_token_account_pda, find_bank_vault_authority_pda,
            find_bank_vault_pda, send_tx,
        },
    },
    anchor_client::anchor_lang::{AnchorDeserialize, InstructionData, ToAccountMetas},
    anyhow::{bail, Context, Result},
    fixed::types::I80F48,
    marginfi::state::{
        bank::{BankImpl, BankVaultType},
        price::{
            parse_swb_ignore_alignment, LitePullFeedAccountData, OraclePriceFeedAdapter,
            PriceAdapter,
        },
    },
    marginfi_type_crate::{
        constants::METADATA_SEED,
        types::{
            Bank, BankConfigOpt, InterestRateConfigOpt, MarginfiGroup, OracleSetup, WrappedI80F48,
        },
    },
    pyth_solana_receiver_sdk::price_update::PriceUpdateV2,
    solana_client::rpc_filter::{Memcmp, RpcFilterType},
    solana_sdk::{
        account::{ReadableAccount, WritableAccount},
        account_info::IntoAccountInfo,
        clock::Clock,
        instruction::{AccountMeta, Instruction},
        pubkey::Pubkey,
        system_program,
    },
    std::{
        cell::RefCell,
        mem::size_of,
        time::{SystemTime, UNIX_EPOCH},
    },
    switchboard_on_demand::PullFeedAccountData,
};

pub fn bank_get(config: Config, bank_pk: Option<Pubkey>) -> Result<()> {
    let rpc_client = config.mfi_program.rpc();
    let json = config.json_output;

    if let Some(address) = bank_pk {
        let mut bank: Bank = config.mfi_program.account(address)?;
        let group: MarginfiGroup = config.mfi_program.account(bank.group)?;

        let current_timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let current_timestamp = current_timestamp.as_secs() as i64;

        bank.accrue_interest(current_timestamp, &group)?;
        bank.update_bank_cache(&group)?;

        output::print_bank_detail(&address, &bank, json);

        // Vault balances (table mode only for now)
        if !json {
            let liquidity_vault_balance =
                rpc_client.get_token_account_balance(&bank.liquidity_vault)?;
            let fee_vault_balance = rpc_client.get_token_account_balance(&bank.fee_vault)?;
            let insurance_vault_balance =
                rpc_client.get_token_account_balance(&bank.insurance_vault)?;

            println!("Token balances:");
            println!(
                "\tliquidity vault: {} (native: {})",
                liquidity_vault_balance.ui_amount.unwrap_or(0.0),
                liquidity_vault_balance.amount
            );
            println!(
                "\tfee vault: {} (native: {})",
                fee_vault_balance.ui_amount.unwrap_or(0.0),
                fee_vault_balance.amount
            );
            println!(
                "\tinsurance vault: {} (native: {})",
                insurance_vault_balance.ui_amount.unwrap_or(0.0),
                insurance_vault_balance.amount
            );
            if bank.emissions_mint != Pubkey::default() {
                let emissions_token_account = find_bank_emssions_token_account_pda(
                    address,
                    bank.emissions_mint,
                    config.program_id,
                )
                .0;
                let emissions_vault_balance =
                    rpc_client.get_token_account_balance(&emissions_token_account)?;
                println!(
                    "\temissions vault: {} (native: {} - TA: {})",
                    emissions_vault_balance.ui_amount.unwrap_or(0.0),
                    emissions_vault_balance.amount,
                    emissions_token_account
                );
            }
        }
    } else {
        group_get_all(config)?;
    }
    Ok(())
}

pub fn bank_get_all(config: Config, marginfi_group: Option<Pubkey>) -> Result<()> {
    let json = config.json_output;
    let accounts = load_all_banks(&config, marginfi_group)?;
    output::print_banks_table(&accounts, json);
    Ok(())
}

pub fn bank_inspect_price_oracle(config: Config, bank_pk: Pubkey) -> Result<()> {
    use marginfi::state::price::{OraclePriceType, PriceBias};

    let bank: Bank = config.mfi_program.account(bank_pk)?;
    let opfa = match bank.config.oracle_setup {
        OracleSetup::Fixed => OraclePriceFeedAdapter::try_from_bank_with_max_age(
            &bank,
            &[],
            &Clock::default(),
            u64::MAX,
        )
        .map_err(|e| anyhow::anyhow!("failed to create oracle price feed adapter: {:?}", e))?,
        _ => {
            let oracle_keys = crate::utils::bank_observation_keys(&bank);
            let rpc = config.mfi_program.rpc();
            let mut oracle_accounts: Vec<_> = oracle_keys
                .iter()
                .map(|pk| rpc.get_account(pk))
                .collect::<std::result::Result<Vec<_>, _>>()?;
            let oracle_ais: Vec<_> = oracle_keys
                .iter()
                .zip(oracle_accounts.iter_mut())
                .map(|(pk, acc)| (pk, acc).into_account_info())
                .collect();

            OraclePriceFeedAdapter::try_from_bank_with_max_age(
                &bank,
                &oracle_ais,
                &Clock::default(),
                u64::MAX,
            )
            .map_err(|e| anyhow::anyhow!("failed to create oracle price feed adapter: {:?}", e))?
        }
    };

    let (real_price, maint_asset_price, maint_liab_price, init_asset_price, init_liab_price) = (
        opfa.get_price_of_type_ignore_conf(OraclePriceType::RealTime, None)?,
        opfa.get_price_of_type_ignore_conf(OraclePriceType::RealTime, Some(PriceBias::Low))?,
        opfa.get_price_of_type_ignore_conf(OraclePriceType::RealTime, Some(PriceBias::High))?,
        opfa.get_price_of_type_ignore_conf(OraclePriceType::TimeWeighted, Some(PriceBias::Low))?,
        opfa.get_price_of_type_ignore_conf(OraclePriceType::TimeWeighted, Some(PriceBias::High))?,
    );

    let keys = bank
        .config
        .oracle_keys
        .iter()
        .filter(|k| k != &&Pubkey::default())
        .collect::<Vec<_>>();

    println!(
        r##"
Oracle Setup: {setup:?}
Oracle Keys: {keys:#?}
Prince:
    Realtime: {real_price}
    Maint: {maint_asset_price} (asset) {maint_liab_price} (liab)
    Init: {init_asset_price} (asset) {init_liab_price} (liab)
    "##,
        setup = bank.config.oracle_setup,
        keys = keys,
        real_price = real_price,
        maint_asset_price = maint_asset_price,
        maint_liab_price = maint_liab_price,
        init_asset_price = init_asset_price,
        init_liab_price = init_liab_price,
    );

    Ok(())
}

pub fn show_oracle_ages(
    config: Config,
    marginfi_group: Option<Pubkey>,
    only_stale: bool,
) -> Result<()> {
    let default_group = solana_sdk::pubkey!("4qp6Fx6tnZkY5Wropq9wUYgtFxXKwE6viZxFHg3rdAG8");
    let group = marginfi_group.unwrap_or(default_group);

    let banks = config
        .mfi_program
        .accounts::<Bank>(vec![RpcFilterType::Memcmp(Memcmp::new_raw_bytes(
            8 + size_of::<Pubkey>() + size_of::<u8>(),
            group.to_bytes().to_vec(),
        ))])?;

    if banks.is_empty() {
        println!("No banks found for group {}", group);
        return Ok(());
    }

    let mut pyth_feeds: Vec<(u16, Pubkey, Pubkey)> = Vec::new();
    let mut swb_feeds: Vec<(u16, Pubkey, Pubkey)> = Vec::new();

    for (_, bank) in banks {
        let Some(first_oracle) = bank
            .config
            .oracle_keys
            .iter()
            .copied()
            .find(|key| *key != Pubkey::default())
        else {
            continue;
        };

        match bank.config.oracle_setup {
            OracleSetup::PythPushOracle
            | OracleSetup::KaminoPythPush
            | OracleSetup::StakedWithPythPush => {
                pyth_feeds.push((bank.config.oracle_max_age, bank.mint, first_oracle));
            }
            OracleSetup::SwitchboardPull => {
                swb_feeds.push((bank.config.oracle_max_age, bank.mint, first_oracle));
            }
            _ => {}
        }
    }

    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;

    let mut pyth_rows: Vec<(f64, f64, Pubkey)> = Vec::new();
    if !pyth_feeds.is_empty() {
        let keys = pyth_feeds
            .iter()
            .map(|(_, _, key)| *key)
            .collect::<Vec<_>>();
        let accounts = config
            .mfi_program
            .rpc()
            .get_multiple_accounts(keys.as_slice())?;

        for (maybe_account, (max_age, mint, _)) in accounts.into_iter().zip(pyth_feeds.iter()) {
            let Some(account) = maybe_account else {
                continue;
            };

            let Ok(price_update) = PriceUpdateV2::deserialize(&mut &account.data()[8..]) else {
                continue;
            };

            let age_min = (now - price_update.price_message.publish_time) as f64 / 60.0;
            let allowed_min = if *max_age == 0 {
                1.0
            } else {
                *max_age as f64 / 60.0
            };
            pyth_rows.push((age_min, allowed_min, *mint));
        }
    }

    let mut swb_rows: Vec<(f64, f64, Pubkey)> = Vec::new();
    if !swb_feeds.is_empty() {
        let keys = swb_feeds.iter().map(|(_, _, key)| *key).collect::<Vec<_>>();
        let mut accounts = config
            .mfi_program
            .rpc()
            .get_multiple_accounts(keys.as_slice())?;

        for (maybe_account, (max_age, mint, _)) in accounts.iter_mut().zip(swb_feeds.iter()) {
            let Some(account) = maybe_account else {
                continue;
            };

            let data = account.data_as_mut_slice();
            let cell = RefCell::new(data);
            let Ok(feed): Result<PullFeedAccountData, _> =
                parse_swb_ignore_alignment(cell.borrow())
            else {
                continue;
            };
            let lite_feed = LitePullFeedAccountData::from(&feed);

            let age_min = (now - lite_feed.last_update_timestamp) as f64 / 60.0;
            let allowed_min = if *max_age == 0 {
                1.0
            } else {
                *max_age as f64 / 60.0
            };
            swb_rows.push((age_min, allowed_min, *mint));
        }
    }

    pyth_rows.sort_by(|(a, _, _), (b, _, _)| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    swb_rows.sort_by(|(a, _, _), (b, _, _)| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));

    println!("Group: {}", group);
    println!("Pyth");
    for (age, allowed, mint) in pyth_rows {
        if only_stale && age < allowed {
            continue;
        }
        println!(
            "- {:?}: {:.2}min (allowed: {:.2}min){}",
            mint,
            age,
            allowed,
            if age >= allowed { " [STALE]" } else { "" }
        );
    }

    println!("Switchboard");
    for (age, allowed, mint) in swb_rows {
        if only_stale && age < allowed {
            continue;
        }
        println!(
            "- {:?}: {:.2}min (allowed: {:.2}min){}",
            mint,
            age,
            allowed,
            if age >= allowed { " [STALE]" } else { "" }
        );
    }

    Ok(())
}

pub fn bank_configure(
    config: Config,
    profile: Profile,
    bank_pk: Pubkey,
    bank_config_opt: BankConfigOpt,
) -> Result<()> {
    let configure_bank_ixs_builder = config.mfi_program.request();
    let signing_keypairs = config.get_signers(false);

    let configure_bank_ixs = configure_bank_ixs_builder
        .accounts(marginfi::accounts::LendingPoolConfigureBank {
            group: profile
                .marginfi_group
                .context("marginfi group not set in profile")?,
            admin: config.authority(),
            bank: bank_pk,
        })
        .args(marginfi::instruction::LendingPoolConfigureBank {
            bank_config_opt: bank_config_opt.clone(),
        })
        .instructions()?;

    let sig = send_tx(&config, configure_bank_ixs, &signing_keypairs)?;

    println!("Transaction signature: {}", sig);

    Ok(())
}

pub fn bank_configure_interest_only(
    config: Config,
    bank_pk: Pubkey,
    interest_rate_config: InterestRateConfigOpt,
) -> Result<()> {
    let bank: Bank = config.mfi_program.account(bank_pk)?;

    let ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::LendingPoolConfigureBankInterestOnly {
            group: bank.group,
            delegate_curve_admin: config.authority(),
            bank: bank_pk,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::LendingPoolConfigureBankInterestOnly {
            interest_rate_config,
        }
        .data(),
    };

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(&config, vec![ix], &signing_keypairs)?;
    println!("Bank interest config updated (sig: {})", sig);

    Ok(())
}

pub fn bank_configure_limits_only(
    config: Config,
    bank_pk: Pubkey,
    deposit_limit_ui: Option<f64>,
    borrow_limit_ui: Option<f64>,
    total_asset_value_init_limit: Option<u64>,
) -> Result<()> {
    let bank: Bank = config.mfi_program.account(bank_pk)?;

    let deposit_limit = deposit_limit_ui
        .map(|ui_amount| spl_token::ui_amount_to_amount(ui_amount, bank.mint_decimals));
    let borrow_limit = borrow_limit_ui
        .map(|ui_amount| spl_token::ui_amount_to_amount(ui_amount, bank.mint_decimals));

    let ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::LendingPoolConfigureBankLimitsOnly {
            group: bank.group,
            delegate_limit_admin: config.authority(),
            bank: bank_pk,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::LendingPoolConfigureBankLimitsOnly {
            deposit_limit,
            borrow_limit,
            total_asset_value_init_limit,
        }
        .data(),
    };

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(&config, vec![ix], &signing_keypairs)?;
    println!("Bank limits updated (sig: {})", sig);

    Ok(())
}

pub fn bank_configure_oracle(
    config: Config,
    profile: Profile,
    bank_pk: Pubkey,
    setup: u8,
    oracle: Pubkey,
) -> Result<()> {
    let configure_bank_ixs_builder = config.mfi_program.request();
    let signing_keypairs = config.get_signers(false);

    let extra_accounts = vec![AccountMeta::new_readonly(oracle, false)];

    let mut configure_bank_ixs = configure_bank_ixs_builder
        .accounts(marginfi::accounts::LendingPoolConfigureBankOracle {
            group: profile
                .marginfi_group
                .context("marginfi group not set in profile")?,
            admin: config.authority(),
            bank: bank_pk,
        })
        .args(marginfi::instruction::LendingPoolConfigureBankOracle { setup, oracle })
        .instructions()?;

    configure_bank_ixs[0].accounts.extend(extra_accounts);

    let sig = send_tx(&config, configure_bank_ixs, &signing_keypairs)?;

    println!("Transaction signature: {}", sig);

    Ok(())
}

pub fn bank_force_tokenless_repay_complete(config: Config, bank_pk: Pubkey) -> Result<()> {
    let bank: Bank = config.mfi_program.account(bank_pk)?;

    let ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::LendingPoolForceTokenlessRepayComplete {
            group: bank.group,
            risk_admin: config.authority(),
            bank: bank_pk,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::LendingPoolForceTokenlessRepayComplete {}.data(),
    };

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(&config, vec![ix], &signing_keypairs)?;
    println!("Tokenless repay complete set (sig: {})", sig);

    Ok(())
}

// --------------------------------------------------------------------------
// New bank commands
// --------------------------------------------------------------------------

pub fn bank_close(config: Config, _profile: Profile, bank_pk: Pubkey) -> Result<()> {
    let bank: Bank = config.mfi_program.account(bank_pk)?;

    let ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::LendingPoolCloseBank {
            group: bank.group,
            bank: bank_pk,
            admin: config.authority(),
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::LendingPoolCloseBank {}.data(),
    };

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(&config, vec![ix], &signing_keypairs)?;
    println!("Bank closed (sig: {})", sig);

    Ok(())
}

pub fn bank_accrue_interest(config: Config, bank_pk: Pubkey) -> Result<()> {
    let bank: Bank = config.mfi_program.account(bank_pk)?;

    let ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::LendingPoolAccrueBankInterest {
            group: bank.group,
            bank: bank_pk,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::LendingPoolAccrueBankInterest {}.data(),
    };

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(&config, vec![ix], &signing_keypairs)?;
    println!("Interest accrued (sig: {})", sig);

    Ok(())
}

pub fn bank_set_fixed_price(
    config: Config,
    _profile: Profile,
    bank_pk: Pubkey,
    price: f64,
) -> Result<()> {
    let bank: Bank = config.mfi_program.account(bank_pk)?;

    let price_wrapped: WrappedI80F48 = I80F48::from_num(price).into();

    let ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::LendingPoolSetFixedOraclePrice {
            group: bank.group,
            admin: config.authority(),
            bank: bank_pk,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::LendingPoolSetFixedOraclePrice {
            price: price_wrapped,
        }
        .data(),
    };

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(&config, vec![ix], &signing_keypairs)?;
    println!("Fixed price set (sig: {})", sig);

    Ok(())
}

pub fn bank_configure_emode(
    config: Config,
    _profile: Profile,
    bank_pk: Pubkey,
    emode_tag: u16,
) -> Result<()> {
    let bank: Bank = config.mfi_program.account(bank_pk)?;
    let entries = bank.emode.emode_config.entries;

    let ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::LendingPoolConfigureBankEmode {
            group: bank.group,
            emode_admin: config.authority(),
            bank: bank_pk,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::LendingPoolConfigureBankEmode { emode_tag, entries }.data(),
    };

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(&config, vec![ix], &signing_keypairs)?;
    println!("Emode configured (sig: {})", sig);

    Ok(())
}

pub fn bank_clone_emode(
    config: Config,
    copy_from_bank: Pubkey,
    copy_to_bank: Pubkey,
) -> Result<()> {
    let source: Bank = config.mfi_program.account(copy_from_bank)?;
    let destination: Bank = config.mfi_program.account(copy_to_bank)?;

    if source.group != destination.group {
        bail!("source and destination banks belong to different groups");
    }

    let ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::LendingPoolCloneEmode {
            group: source.group,
            signer: config.authority(),
            copy_from_bank,
            copy_to_bank,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::LendingPoolCloneEmode {}.data(),
    };

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(&config, vec![ix], &signing_keypairs)?;
    println!("emode cloned (sig: {})", sig);

    Ok(())
}

pub fn bank_migrate_curve(config: Config, bank_pk: Pubkey) -> Result<()> {
    let ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::MigrateCurve { bank: bank_pk }.to_account_metas(Some(true)),
        data: marginfi::instruction::MigrateCurve {}.data(),
    };

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(&config, vec![ix], &signing_keypairs)?;
    println!("curve migrated (sig: {})", sig);

    Ok(())
}

pub fn bank_pulse_price_cache(config: Config, bank_pk: Pubkey) -> Result<()> {
    let bank: Bank = config.mfi_program.account(bank_pk)?;

    let mut accounts = marginfi::accounts::LendingPoolPulseBankPriceCache {
        group: bank.group,
        bank: bank_pk,
    }
    .to_account_metas(Some(true));

    // Append all oracle accounts needed for this bank's oracle setup
    for oracle_pk in crate::utils::bank_observation_keys(&bank) {
        accounts.push(AccountMeta::new_readonly(oracle_pk, false));
    }

    let ix = Instruction {
        program_id: config.program_id,
        accounts,
        data: marginfi::instruction::LendingPoolPulseBankPriceCache {}.data(),
    };

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(&config, vec![ix], &signing_keypairs)?;
    println!("Price cache pulsed (sig: {})", sig);

    Ok(())
}

pub fn bank_configure_rate_limits(
    config: Config,
    _profile: Profile,
    bank_pk: Pubkey,
    hourly_max_outflow: Option<u64>,
    daily_max_outflow: Option<u64>,
) -> Result<()> {
    let bank: Bank = config.mfi_program.account(bank_pk)?;

    let ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::ConfigureBankRateLimits {
            group: bank.group,
            admin: config.authority(),
            bank: bank_pk,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::ConfigureBankRateLimits {
            hourly_max_outflow,
            daily_max_outflow,
        }
        .data(),
    };

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(&config, vec![ix], &signing_keypairs)?;
    println!("Rate limits configured (sig: {})", sig);

    Ok(())
}

pub fn bank_withdraw_fees_permissionless(
    config: Config,
    bank_pk: Pubkey,
    amount: u64,
) -> Result<()> {
    let bank: Bank = config.mfi_program.account(bank_pk)?;

    let (fee_vault, _) = find_bank_vault_pda(&bank_pk, BankVaultType::Fee, &config.program_id);
    let (fee_vault_authority, _) =
        find_bank_vault_authority_pda(&bank_pk, BankVaultType::Fee, &config.program_id);

    let fees_destination_account = bank.fees_destination_account;

    let mut ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::LendingPoolWithdrawFeesPermissionless {
            group: bank.group,
            bank: bank_pk,
            fee_vault,
            fee_vault_authority,
            fees_destination_account,
            token_program: spl_token::id(),
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::LendingPoolWithdrawFeesPermissionless { amount }.data(),
    };
    // Append mint as remaining account for token-2022 compatibility
    ix.accounts
        .push(AccountMeta::new_readonly(bank.mint, false));

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(&config, vec![ix], &signing_keypairs)?;
    println!("Fees withdrawn permissionlessly (sig: {})", sig);

    Ok(())
}

pub fn bank_update_fees_destination(
    config: Config,
    _profile: Profile,
    bank_pk: Pubkey,
    destination: Pubkey,
) -> Result<()> {
    let bank: Bank = config.mfi_program.account(bank_pk)?;

    let ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::LendingPoolUpdateFeesDestinationAccount {
            group: bank.group,
            bank: bank_pk,
            admin: config.authority(),
            destination_account: destination,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::LendingPoolUpdateFeesDestinationAccount {}.data(),
    };

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(&config, vec![ix], &signing_keypairs)?;
    println!("Fees destination updated (sig: {})", sig);

    Ok(())
}

pub fn bank_init_metadata(config: Config, bank_pk: Pubkey) -> Result<()> {
    let (metadata, _) = Pubkey::find_program_address(
        &[METADATA_SEED.as_bytes(), bank_pk.as_ref()],
        &config.program_id,
    );

    let ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::InitBankMetadata {
            bank: bank_pk,
            fee_payer: config.authority(),
            metadata,
            system_program: system_program::id(),
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::InitBankMetadata {}.data(),
    };

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(&config, vec![ix], &signing_keypairs)?;
    println!("Bank metadata initialized (sig: {})", sig);

    Ok(())
}

pub fn bank_write_metadata(
    config: Config,
    _profile: Profile,
    bank_pk: Pubkey,
    ticker: String,
    description: String,
) -> Result<()> {
    let bank: Bank = config.mfi_program.account(bank_pk)?;

    let (metadata, _) = Pubkey::find_program_address(
        &[METADATA_SEED.as_bytes(), bank_pk.as_ref()],
        &config.program_id,
    );

    let ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::WriteBankMetadata {
            group: bank.group,
            bank: bank_pk,
            metadata_admin: config.authority(),
            metadata,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::WriteBankMetadata {
            ticker: Some(ticker.into_bytes()),
            description: Some(description.into_bytes()),
        }
        .data(),
    };

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(&config, vec![ix], &signing_keypairs)?;
    println!("Bank metadata written (sig: {})", sig);

    Ok(())
}
