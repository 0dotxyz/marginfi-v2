use {
    super::load_all_banks,
    crate::{
        config::Config,
        output,
        profile::Profile,
        utils::{
            build_wsol_wrap_ixs, find_bank_vault_authority_pda, find_execute_order_pda,
            find_fee_state_pda, find_liquidation_record_pda, find_order_pda,
            load_bank_oracle_account_metas, load_observation_account_metas,
            load_observation_account_metas_close_last,
            load_observation_account_metas_with_bank_writable, load_observation_bank_only_metas,
            send_tx, EXP_10_I80F48,
        },
    },
    anchor_client::anchor_lang::{InstructionData, ToAccountMetas},
    anyhow::{anyhow, bail, Context, Result},
    base64::Engine as _,
    fixed::types::I80F48,
    marginfi::state::bank::BankVaultType,
    marginfi_type_crate::{
        constants::MARGINFI_ACCOUNT_SEED,
        types::{Bank, FeeState, MarginfiAccount, Order, OrderTrigger},
    },
    serde::Deserialize,
    solana_client::rpc_filter::{Memcmp, RpcFilterType},
    solana_sdk::{
        instruction::{AccountMeta, Instruction},
        pubkey::Pubkey,
        signature::Keypair,
        signer::Signer,
        system_program, sysvar,
    },
    spl_associated_token_account::instruction::create_associated_token_account_idempotent,
    std::{collections::HashMap, fs, path::PathBuf, str::FromStr},
};

#[derive(Debug, Deserialize)]
struct JsonAccountMeta {
    pub pubkey: String,
    #[serde(default)]
    pub is_signer: bool,
    #[serde(default)]
    pub is_writable: bool,
}

#[derive(Debug, Deserialize)]
struct JsonInstruction {
    pub program_id: String,
    pub accounts: Vec<JsonAccountMeta>,
    #[serde(default)]
    pub data_base64: Option<String>,
    #[serde(default)]
    pub data_base58: Option<String>,
}

fn build_authority_ata_ix(
    config: &Config,
    owner: &Pubkey,
    mint: &Pubkey,
    token_program: &Pubkey,
) -> Instruction {
    create_associated_token_account_idempotent(
        &config.explicit_fee_payer(),
        owner,
        mint,
        token_program,
    )
}

fn load_extra_instructions(extra_ixs_file: Option<PathBuf>) -> Result<Vec<Instruction>> {
    let Some(path) = extra_ixs_file else {
        return Ok(vec![]);
    };

    let raw = fs::read_to_string(&path)
        .with_context(|| format!("failed to read extra instructions file {}", path.display()))?;
    let parsed: Vec<JsonInstruction> = serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse JSON in {}", path.display()))?;

    parsed
        .into_iter()
        .map(|ix| {
            let program_id = Pubkey::from_str(&ix.program_id)
                .with_context(|| format!("invalid program_id {}", ix.program_id))?;

            let accounts = ix
                .accounts
                .into_iter()
                .map(|meta| {
                    Ok(AccountMeta {
                        pubkey: Pubkey::from_str(&meta.pubkey)
                            .with_context(|| format!("invalid account pubkey {}", meta.pubkey))?,
                        is_signer: meta.is_signer,
                        is_writable: meta.is_writable,
                    })
                })
                .collect::<Result<Vec<_>>>()?;

            let data = match (ix.data_base64, ix.data_base58) {
                (Some(data), None) => base64::engine::general_purpose::STANDARD.decode(data)?,
                (None, Some(data)) => bs58::decode(data).into_vec()?,
                (Some(_), Some(_)) => {
                    bail!("extra instruction must specify only one of data_base64 or data_base58")
                }
                (None, None) => vec![],
            };

            Ok(Instruction {
                program_id,
                accounts,
                data,
            })
        })
        .collect()
}

pub fn marginfi_account_list(profile: Profile, config: &Config) -> Result<()> {
    let group = profile
        .marginfi_group
        .context("marginfi group not set in profile")?;
    let authority = config.authority();
    let json = config.json_output;

    let banks = HashMap::from_iter(load_all_banks(config, Some(group))?);

    let accounts = config.mfi_program.accounts::<MarginfiAccount>(vec![
        RpcFilterType::Memcmp(Memcmp::new_raw_bytes(8, group.to_bytes().to_vec())),
        RpcFilterType::Memcmp(Memcmp::new_raw_bytes(8 + 32, authority.to_bytes().to_vec())),
    ])?;

    if json {
        let vals = accounts
            .iter()
            .map(|(address, marginfi_account)| {
                let is_default = profile
                    .marginfi_account
                    .is_some_and(|default_account| default_account == *address);
                output::account_detail_json(*address, marginfi_account, &banks, is_default)
            })
            .collect::<Vec<_>>();
        println!("{}", serde_json::to_string_pretty(&vals)?);
        return Ok(());
    }

    if accounts.is_empty() {
        println!("No marginfi accounts found");
    }

    for (address, marginfi_account) in &accounts {
        let is_default = profile
            .marginfi_account
            .map_or(false, |default_account| default_account == *address);
        output::print_account_detail(*address, marginfi_account, &banks, is_default, json);
    }

    Ok(())
}

pub fn marginfi_account_use(
    mut profile: Profile,
    config: &Config,
    marginfi_account_pk: Pubkey,
) -> Result<()> {
    let authority = config.authority();

    let marginfi_account = config
        .mfi_program
        .account::<MarginfiAccount>(marginfi_account_pk)?;

    if marginfi_account.authority != authority {
        return Err(anyhow!("Marginfi account does not belong to authority"));
    }

    profile.config(
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        Some(marginfi_account.group),
        Some(marginfi_account_pk),
    )?;

    println!("Default marginfi account set to: {marginfi_account_pk}");

    Ok(())
}

/// Print the marginfi account for the provided address or the default marginfi account if none is provided
///
/// If marginfi account address is provided use the group in the marginfi account data, otherwise use the profile defaults
pub fn marginfi_account_get(
    profile: Profile,
    config: &Config,
    marginfi_account_pk: Option<Pubkey>,
) -> Result<()> {
    let json = config.json_output;
    let marginfi_account_pk = match marginfi_account_pk {
        Some(pk) => pk,
        None => profile
            .marginfi_account
            .context("marginfi account not set in profile")?,
    };

    let marginfi_account = config
        .mfi_program
        .account::<MarginfiAccount>(marginfi_account_pk)?;

    let group = marginfi_account.group;

    let banks = HashMap::from_iter(load_all_banks(config, Some(group))?);

    output::print_account_detail(marginfi_account_pk, &marginfi_account, &banks, false, json);

    Ok(())
}

pub fn marginfi_account_deposit(
    profile: &Profile,
    config: &Config,
    bank_pk: Pubkey,
    ui_amount: f64,
    deposit_up_to_limit: Option<bool>,
) -> Result<()> {
    let rpc_client = config.mfi_program.rpc();
    let authority = config.authority();
    let marginfi_account_pk = profile.get_marginfi_account()?;
    let marginfi_account = config
        .mfi_program
        .account::<MarginfiAccount>(marginfi_account_pk)?;
    let group = marginfi_account.group;

    let bank = config.mfi_program.account::<Bank>(bank_pk)?;

    let amount = (I80F48::from_num(ui_amount) * EXP_10_I80F48[bank.mint_decimals as usize])
        .floor()
        .to_num::<u64>();

    // Check that bank belongs to the correct group
    if bank.group != group {
        bail!("Bank does not belong to group")
    }

    let bank_mint_account = rpc_client.get_account(&bank.mint)?;
    let token_program = bank_mint_account.owner;

    let deposit_ata = anchor_spl::associated_token::get_associated_token_address_with_program_id(
        &authority,
        &bank.mint,
        &token_program,
    );

    let mut ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::LendingAccountDeposit {
            group,
            marginfi_account: marginfi_account_pk,
            authority,
            bank: bank_pk,
            signer_token_account: deposit_ata,
            liquidity_vault: bank.liquidity_vault,
            token_program,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::LendingAccountDeposit {
            amount,
            deposit_up_to_limit,
        }
        .data(),
    };
    if token_program == anchor_spl::token_2022::ID {
        ix.accounts
            .push(AccountMeta::new_readonly(bank.mint, false));
    }

    // If depositing native SOL, wrap it into WSOL first
    let mut ixs = Vec::new();
    if bank.mint == spl_token::native_mint::id() {
        ixs.extend(build_wsol_wrap_ixs(&authority, amount));
    } else {
        ixs.push(build_authority_ata_ix(
            config,
            &authority,
            &bank.mint,
            &token_program,
        ));
    }
    ixs.push(ix);

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(config, ixs, &signing_keypairs)?;
    println!("Deposit successful: {sig}");

    Ok(())
}

pub fn marginfi_account_withdraw(
    profile: &Profile,
    config: &Config,
    bank_pk: Pubkey,
    ui_amount: f64,
    withdraw_all: bool,
) -> Result<()> {
    let authority = config.authority();
    let rpc_client = config.mfi_program.rpc();

    let marginfi_account_pk = profile.get_marginfi_account()?;

    let marginfi_account = config
        .mfi_program
        .account::<MarginfiAccount>(marginfi_account_pk)?;
    let group = marginfi_account.group;

    let banks = HashMap::from_iter(load_all_banks(config, Some(group))?);
    let bank = banks.get(&bank_pk).context("Bank not found")?;

    let amount = (I80F48::from_num(ui_amount) * EXP_10_I80F48[bank.mint_decimals as usize])
        .floor()
        .to_num::<u64>();

    // Check that bank belongs to the correct group
    if bank.group != group {
        bail!("Bank does not belong to group")
    }

    let bank_mint_account = rpc_client.get_account(&bank.mint)?;
    let token_program = bank_mint_account.owner;

    let withdraw_ata = anchor_spl::associated_token::get_associated_token_address_with_program_id(
        &authority,
        &bank.mint,
        &token_program,
    );

    let mut ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::LendingAccountWithdraw {
            group,
            marginfi_account: marginfi_account_pk,
            authority,
            bank: bank_pk,
            liquidity_vault: bank.liquidity_vault,
            token_program,
            destination_token_account: withdraw_ata,
            bank_liquidity_vault_authority: find_bank_vault_authority_pda(
                &bank_pk,
                BankVaultType::Liquidity,
                &config.program_id,
            )
            .0,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::LendingAccountWithdraw {
            amount,
            withdraw_all: if withdraw_all { Some(true) } else { None },
        }
        .data(),
    };

    if token_program == anchor_spl::token_2022::ID {
        ix.accounts
            .push(AccountMeta::new_readonly(bank.mint, false));
    }
    let observation_metas = if withdraw_all {
        load_observation_account_metas_close_last(
            &marginfi_account,
            &banks,
            vec![],
            vec![],
            bank_pk,
        )
    } else {
        load_observation_account_metas(&marginfi_account, &banks, vec![], vec![])
    };
    ix.accounts.extend(observation_metas);

    let create_ide_ata_ix = build_authority_ata_ix(config, &authority, &bank.mint, &token_program);
    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(config, vec![create_ide_ata_ix, ix], &signing_keypairs)?;
    println!("Withdraw successful: {sig}");

    Ok(())
}

pub fn marginfi_account_borrow(
    profile: &Profile,
    config: &Config,
    bank_pk: Pubkey,
    ui_amount: f64,
) -> Result<()> {
    let authority = config.authority();
    let rpc_client = config.mfi_program.rpc();

    let marginfi_account_pk = profile.get_marginfi_account()?;

    let marginfi_account = config
        .mfi_program
        .account::<MarginfiAccount>(marginfi_account_pk)?;
    let group = marginfi_account.group;

    let banks = HashMap::from_iter(load_all_banks(config, Some(group))?);
    let bank = banks.get(&bank_pk).context("Bank not found")?;

    let amount = (I80F48::from_num(ui_amount) * EXP_10_I80F48[bank.mint_decimals as usize])
        .floor()
        .to_num::<u64>();

    // Check that bank belongs to the correct group
    if bank.group != group {
        bail!("Bank does not belong to group")
    }

    let bank_mint_account = rpc_client.get_account(&bank.mint)?;
    let token_program = bank_mint_account.owner;

    let borrow_ata = anchor_spl::associated_token::get_associated_token_address_with_program_id(
        &authority,
        &bank.mint,
        &token_program,
    );

    let mut ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::LendingAccountBorrow {
            group,
            marginfi_account: marginfi_account_pk,
            authority,
            bank: bank_pk,
            liquidity_vault: bank.liquidity_vault,
            token_program,
            destination_token_account: borrow_ata,
            bank_liquidity_vault_authority: find_bank_vault_authority_pda(
                &bank_pk,
                BankVaultType::Liquidity,
                &config.program_id,
            )
            .0,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::LendingAccountBorrow { amount }.data(),
    };

    if token_program == anchor_spl::token_2022::ID {
        ix.accounts
            .push(AccountMeta::new_readonly(bank.mint, false));
    }
    ix.accounts.extend(load_observation_account_metas(
        &marginfi_account,
        &banks,
        vec![bank_pk],
        vec![],
    ));

    let create_ide_ata_ix = build_authority_ata_ix(config, &authority, &bank.mint, &token_program);
    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(config, vec![create_ide_ata_ix, ix], &signing_keypairs)?;
    println!("Borrow successful: {sig}");

    Ok(())
}

pub fn marginfi_account_liquidate(
    profile: &Profile,
    config: &Config,
    liquidatee_marginfi_account_pk: Pubkey,
    asset_bank_pk: Pubkey,
    liability_bank_pk: Pubkey,
    ui_asset_amount: f64,
) -> Result<()> {
    let authority = config.authority();
    let rpc_client = config.mfi_program.rpc();

    let marginfi_account_pk = profile.get_marginfi_account()?;

    let marginfi_account = config
        .mfi_program
        .account::<MarginfiAccount>(marginfi_account_pk)?;
    let group = marginfi_account.group;

    let banks = HashMap::from_iter(load_all_banks(config, Some(group))?);
    let asset_bank = banks.get(&asset_bank_pk).context("Asset bank not found")?;
    let liability_bank = banks
        .get(&liability_bank_pk)
        .context("Liability bank not found")?;

    let liquidatee_marginfi_account = config
        .mfi_program
        .account::<MarginfiAccount>(liquidatee_marginfi_account_pk)?;

    if liquidatee_marginfi_account.group != group {
        bail!("Liquidatee marginfi account does not belong to group")
    }

    let asset_amount = (I80F48::from_num(ui_asset_amount)
        * EXP_10_I80F48[asset_bank.mint_decimals as usize])
        .floor()
        .to_num::<u64>();

    // Check that banks belong to the correct group
    if asset_bank.group != group {
        bail!("Asset bank does not belong to group")
    }
    if liability_bank.group != group {
        bail!("Liability bank does not belong to group")
    }

    let liability_mint_account = rpc_client.get_account(&liability_bank.mint)?;
    let token_program = liability_mint_account.owner;

    let liquidator_accounts = load_observation_account_metas(
        &marginfi_account,
        &banks,
        vec![liability_bank_pk, asset_bank_pk],
        vec![],
    );
    let liquidatee_accounts =
        load_observation_account_metas(&liquidatee_marginfi_account, &banks, vec![], vec![]);

    let mut ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::LendingAccountLiquidate {
            group,
            asset_bank: asset_bank_pk,
            liab_bank: liability_bank_pk,
            liquidator_marginfi_account: marginfi_account_pk,
            authority,
            liquidatee_marginfi_account: liquidatee_marginfi_account_pk,
            bank_liquidity_vault_authority: find_bank_vault_authority_pda(
                &liability_bank_pk,
                BankVaultType::Liquidity,
                &config.program_id,
            )
            .0,
            bank_liquidity_vault: liability_bank.liquidity_vault,
            bank_insurance_vault: liability_bank.insurance_vault,
            token_program,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::LendingAccountLiquidate {
            asset_amount,
            liquidatee_accounts: liquidatee_accounts.len() as u8,
            liquidator_accounts: liquidator_accounts.len() as u8,
        }
        .data(),
    };

    if token_program == anchor_spl::token_2022::ID {
        ix.accounts
            .push(AccountMeta::new_readonly(liability_bank.mint, false));
    }
    ix.accounts
        .extend(load_bank_oracle_account_metas(asset_bank));
    ix.accounts
        .extend(load_bank_oracle_account_metas(liability_bank));
    ix.accounts.extend(liquidator_accounts);
    ix.accounts.extend(liquidatee_accounts);

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(config, vec![ix], &signing_keypairs)?;
    println!("Liquidation successful: {sig}");

    Ok(())
}

pub fn marginfi_account_create(profile: &Profile, config: &Config) -> Result<()> {
    let authority = config.authority();

    let marginfi_account_key = Keypair::new();

    let ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::MarginfiAccountInitialize {
            marginfi_group: profile
                .marginfi_group
                .context("marginfi group not set in profile")?,
            marginfi_account: marginfi_account_key.pubkey(),
            system_program: system_program::ID,
            authority,
            fee_payer: config.explicit_fee_payer(),
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::MarginfiAccountInitialize.data(),
    };

    let marginfi_account_pk = marginfi_account_key.pubkey();

    let mut signing_keypairs = config.get_signers(false);
    signing_keypairs.push(&marginfi_account_key);
    let _sig = send_tx(config, vec![ix], &signing_keypairs)?;
    println!("{marginfi_account_pk}");

    if config.send_tx {
        let mut profile = profile.clone();

        profile.set_marginfi_account(Some(marginfi_account_key.pubkey()))?;
    }

    Ok(())
}

pub fn marginfi_account_close(profile: &Profile, config: &Config) -> Result<()> {
    let authority = config.authority();

    let marginfi_account_pk = profile.get_marginfi_account()?;
    println!("Closing marginfi account {}", marginfi_account_pk);

    let ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::MarginfiAccountClose {
            marginfi_account: marginfi_account_pk,
            authority,
            fee_payer: config.explicit_fee_payer(),
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::MarginfiAccountClose.data(),
    };

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(config, vec![ix], &signing_keypairs)?;
    println!("Marginfi account closed successfully (sig: {})", sig);

    if config.send_tx {
        let mut profile = profile.clone();
        profile.set_marginfi_account(None)?;
    }

    Ok(())
}

pub fn marginfi_account_place_order(
    profile: &Profile,
    config: &Config,
    bank_1: Pubkey,
    bank_2: Pubkey,
    trigger: OrderTrigger,
) -> Result<()> {
    let authority = config.authority();
    let marginfi_account_pk = profile.get_marginfi_account()?;
    let marginfi_account = config
        .mfi_program
        .account::<MarginfiAccount>(marginfi_account_pk)?;
    let group_pk = marginfi_account.group;

    let bank_keys = vec![bank_1, bank_2];

    let (order_pda, _bump) = find_order_pda(&marginfi_account_pk, &bank_keys, &config.program_id);

    // Fee state PDA is single-instance; load it to get the global fee wallet required by the ix.
    let fee_state_pk = find_fee_state_pda(&config.program_id).0;
    let fee_state: FeeState = config.mfi_program.account(fee_state_pk)?;

    println!(
        "Placing order for marginfi account: {}",
        marginfi_account_pk
    );
    println!("Bank 1: {}", bank_1);
    println!("Bank 2: {}", bank_2);
    println!("Order PDA: {}", order_pda);

    let ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::PlaceOrder {
            group: group_pk,
            marginfi_account: marginfi_account_pk,
            fee_payer: config.explicit_fee_payer(),
            authority,
            order: order_pda,
            fee_state: fee_state_pk,
            global_fee_wallet: fee_state.global_fee_wallet,
            system_program: system_program::id(),
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::MarginfiAccountPlaceOrder { bank_keys, trigger }.data(),
    };

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(config, vec![ix], &signing_keypairs)?;
    println!("Order placed successfully (sig: {})", sig);

    Ok(())
}

pub fn marginfi_account_close_order(
    profile: &Profile,
    config: &Config,
    order_pk: Pubkey,
    fee_recipient: Option<Pubkey>,
) -> Result<()> {
    let authority = config.authority();
    let marginfi_account_pk = profile.get_marginfi_account()?;

    let fee_recipient = fee_recipient.unwrap_or(authority);

    println!("Closing order: {}", order_pk);
    println!("Fee recipient: {}", fee_recipient);

    let ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::CloseOrder {
            marginfi_account: marginfi_account_pk,
            authority,
            order: order_pk,
            fee_recipient,
            system_program: system_program::id(),
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::MarginfiAccountCloseOrder.data(),
    };

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(config, vec![ix], &signing_keypairs)?;
    println!("Order closed successfully (sig: {})", sig);

    Ok(())
}

pub fn marginfi_account_keeper_close_order(
    config: &Config,
    marginfi_account_pk: Pubkey,
    order_pk: Pubkey,
    fee_recipient: Option<Pubkey>,
) -> Result<()> {
    let fee_recipient = fee_recipient.unwrap_or(config.authority());

    let ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::KeeperCloseOrder {
            marginfi_account: marginfi_account_pk,
            fee_recipient,
            order: order_pk,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::MarginfiAccountKeeperCloseOrder.data(),
    };

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(config, vec![ix], &signing_keypairs)?;
    println!("Keeper close order successful (sig: {})", sig);

    Ok(())
}

pub fn marginfi_account_init_liquidation_record(
    profile: &Profile,
    config: &Config,
    marginfi_account_pk: Option<Pubkey>,
) -> Result<()> {
    let marginfi_account_pk = match marginfi_account_pk {
        Some(pubkey) => pubkey,
        None => profile.get_marginfi_account()?,
    };
    let liq_record_pk = find_liquidation_record_pda(&marginfi_account_pk, &config.program_id).0;

    // Avoid throwing if the record already exists.
    if config.mfi_program.rpc().get_account(&liq_record_pk).is_ok() {
        println!("Liquidation record already exists: {}", liq_record_pk);
        return Ok(());
    }

    let ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::InitLiquidationRecord {
            marginfi_account: marginfi_account_pk,
            fee_payer: config.explicit_fee_payer(),
            liquidation_record: liq_record_pk,
            system_program: system_program::id(),
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::MarginfiAccountInitLiqRecord.data(),
    };

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(config, vec![ix], &signing_keypairs)?;
    println!(
        "Liquidation record initialized (sig: {})\nRecord: {}",
        sig, liq_record_pk
    );

    Ok(())
}

pub fn marginfi_account_keeper_execute_order(
    config: &Config,
    order_pk: Pubkey,
    fee_recipient: Option<Pubkey>,
    extra_ixs_file: Option<PathBuf>,
) -> Result<()> {
    let authority = config.authority();
    let fee_recipient = fee_recipient.unwrap_or(authority);

    let order: Order = config.mfi_program.account(order_pk)?;
    let marginfi_account_pk = order.marginfi_account;
    let marginfi_account: MarginfiAccount = config.mfi_program.account(marginfi_account_pk)?;
    let group_pk = marginfi_account.group;
    let banks = HashMap::from_iter(load_all_banks(config, Some(group_pk))?);

    let observation_metas =
        load_observation_account_metas(&marginfi_account, &banks, vec![], vec![]);
    let execute_record_pk = find_execute_order_pda(&order_pk, &config.program_id).0;
    let fee_state_pk = find_fee_state_pda(&config.program_id).0;

    let mut start_ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::StartExecuteOrder {
            group: group_pk,
            marginfi_account: marginfi_account_pk,
            fee_payer: config.explicit_fee_payer(),
            executor: authority,
            order: order_pk,
            execute_record: execute_record_pk,
            instruction_sysvar: sysvar::instructions::id(),
            system_program: system_program::id(),
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::MarginfiAccountStartExecuteOrder.data(),
    };
    start_ix.accounts.extend(observation_metas.clone());

    let mut end_ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::EndExecuteOrder {
            group: group_pk,
            marginfi_account: marginfi_account_pk,
            executor: authority,
            fee_recipient,
            order: order_pk,
            execute_record: execute_record_pk,
            fee_state: fee_state_pk,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::MarginfiAccountEndExecuteOrder.data(),
    };
    end_ix.accounts.extend(observation_metas);

    let mut ixs = vec![start_ix];
    ixs.extend(load_extra_instructions(extra_ixs_file)?);
    ixs.push(end_ix);

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(config, ixs, &signing_keypairs)?;
    println!("Keeper execute order successful (sig: {})", sig);

    Ok(())
}

pub fn marginfi_account_liquidate_receivership(
    config: &Config,
    liquidatee_marginfi_account_pk: Pubkey,
    init_liq_record_if_missing: bool,
    extra_ixs_file: Option<PathBuf>,
) -> Result<()> {
    let authority = config.authority();
    let liquidatee_marginfi_account: MarginfiAccount =
        config.mfi_program.account(liquidatee_marginfi_account_pk)?;

    let group_pk = liquidatee_marginfi_account.group;
    let banks = HashMap::from_iter(load_all_banks(config, Some(group_pk))?);
    let start_metas = load_observation_account_metas_with_bank_writable(
        &liquidatee_marginfi_account,
        &banks,
        vec![],
        vec![],
        true,
    );
    let end_metas = load_observation_bank_only_metas(
        &liquidatee_marginfi_account,
        &banks,
        vec![],
        vec![],
        true,
    );

    let liq_record_pk =
        find_liquidation_record_pda(&liquidatee_marginfi_account_pk, &config.program_id).0;
    let fee_state_pk = find_fee_state_pda(&config.program_id).0;
    let fee_state: FeeState = config.mfi_program.account(fee_state_pk)?;

    let mut ixs = Vec::new();

    let liq_record_exists = config.mfi_program.rpc().get_account(&liq_record_pk).is_ok();
    if !liq_record_exists && init_liq_record_if_missing {
        ixs.push(Instruction {
            program_id: config.program_id,
            accounts: marginfi::accounts::InitLiquidationRecord {
                marginfi_account: liquidatee_marginfi_account_pk,
                fee_payer: config.explicit_fee_payer(),
                liquidation_record: liq_record_pk,
                system_program: system_program::id(),
            }
            .to_account_metas(Some(true)),
            data: marginfi::instruction::MarginfiAccountInitLiqRecord.data(),
        });
    } else if !liq_record_exists {
        bail!(
            "Liquidation record does not exist for account {}. Run `account init-liq-record` first or pass --init-liq-record-if-missing.",
            liquidatee_marginfi_account_pk
        );
    }

    let mut start_ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::StartLiquidation {
            marginfi_account: liquidatee_marginfi_account_pk,
            liquidation_record: liq_record_pk,
            group: group_pk,
            liquidation_receiver: authority,
            instruction_sysvar: sysvar::instructions::id(),
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::StartLiquidation.data(),
    };
    start_ix.accounts.extend(start_metas);
    ixs.push(start_ix);

    ixs.extend(load_extra_instructions(extra_ixs_file)?);

    let mut end_ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::EndLiquidation {
            marginfi_account: liquidatee_marginfi_account_pk,
            liquidation_record: liq_record_pk,
            group: group_pk,
            liquidation_receiver: authority,
            fee_state: fee_state_pk,
            global_fee_wallet: fee_state.global_fee_wallet,
            system_program: system_program::id(),
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::EndLiquidation.data(),
    };
    end_ix.accounts.extend(end_metas);
    ixs.push(end_ix);

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(config, ixs, &signing_keypairs)?;
    println!("Receivership liquidation bundle sent (sig: {})", sig);

    Ok(())
}

pub fn marginfi_account_set_keeper_close_flags(
    profile: &Profile,
    config: &Config,
    bank_keys_opt: Option<Vec<Pubkey>>,
) -> Result<()> {
    let marginfi_account_pk = profile.get_marginfi_account()?;

    match &bank_keys_opt {
        Some(keys) => {
            println!("Setting liquidator close flags for specific banks:");
            for key in keys {
                println!("  - {}", key);
            }
        }
        None => {
            println!("Clearing all balance tags (liquidator will close all orders)");
        }
    }

    let ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::SetKeeperCloseFlags {
            marginfi_account: marginfi_account_pk,
            authority: config.authority(),
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::MarginfiAccountSetKeeperCloseFlags { bank_keys_opt }.data(),
    };

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(config, vec![ix], &signing_keypairs)?;
    println!("Liquidator close flags set successfully (sig: {})", sig);

    Ok(())
}

pub fn marginfi_account_repay(
    profile: &Profile,
    config: &Config,
    bank_pk: Pubkey,
    ui_amount: f64,
    repay_all: bool,
) -> Result<()> {
    let rpc_client = config.mfi_program.rpc();
    let authority = config.authority();
    let marginfi_account_pk = profile.get_marginfi_account()?;
    let marginfi_account = config
        .mfi_program
        .account::<MarginfiAccount>(marginfi_account_pk)?;
    let group = marginfi_account.group;

    let bank = config.mfi_program.account::<Bank>(bank_pk)?;

    let amount = (I80F48::from_num(ui_amount) * EXP_10_I80F48[bank.mint_decimals as usize])
        .floor()
        .to_num::<u64>();

    if bank.group != group {
        bail!("Bank does not belong to group")
    }

    let bank_mint_account = rpc_client.get_account(&bank.mint)?;
    let token_program = bank_mint_account.owner;

    let signer_token_account =
        anchor_spl::associated_token::get_associated_token_address_with_program_id(
            &authority,
            &bank.mint,
            &token_program,
        );

    let mut ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::LendingAccountRepay {
            group,
            marginfi_account: marginfi_account_pk,
            authority,
            bank: bank_pk,
            signer_token_account,
            liquidity_vault: bank.liquidity_vault,
            token_program,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::LendingAccountRepay {
            amount,
            repay_all: if repay_all { Some(true) } else { None },
        }
        .data(),
    };

    if token_program == anchor_spl::token_2022::ID {
        ix.accounts
            .push(AccountMeta::new_readonly(bank.mint, false));
    }
    if repay_all {
        let banks = HashMap::from_iter(load_all_banks(config, Some(group))?);
        ix.accounts.extend(load_observation_account_metas(
            &marginfi_account,
            &banks,
            vec![],
            vec![],
        ));
    }

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(config, vec![ix], &signing_keypairs)?;
    println!("Repay successful: {sig}");

    Ok(())
}

pub fn marginfi_account_close_balance(
    profile: &Profile,
    config: &Config,
    bank_pk: Pubkey,
) -> Result<()> {
    let marginfi_account_pk = profile.get_marginfi_account()?;
    let marginfi_account = config
        .mfi_program
        .account::<MarginfiAccount>(marginfi_account_pk)?;

    let ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::LendingAccountCloseBalance {
            group: marginfi_account.group,
            marginfi_account: marginfi_account_pk,
            authority: config.authority(),
            bank: bank_pk,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::LendingAccountCloseBalance.data(),
    };

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(config, vec![ix], &signing_keypairs)?;
    println!("Close balance successful: {sig}");

    Ok(())
}

pub fn marginfi_account_transfer(
    profile: &Profile,
    config: &Config,
    new_authority: Pubkey,
) -> Result<()> {
    let authority = config.authority();
    let marginfi_account_pk = profile.get_marginfi_account()?;
    let marginfi_account = config
        .mfi_program
        .account::<MarginfiAccount>(marginfi_account_pk)?;

    let new_marginfi_account_key = Keypair::new();

    let fee_state_pk = find_fee_state_pda(&config.program_id).0;
    let fee_state: FeeState = config.mfi_program.account(fee_state_pk)?;

    let ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::TransferToNewAccount {
            group: marginfi_account.group,
            old_marginfi_account: marginfi_account_pk,
            new_marginfi_account: new_marginfi_account_key.pubkey(),
            authority,
            fee_payer: config.explicit_fee_payer(),
            new_authority,
            global_fee_wallet: fee_state.global_fee_wallet,
            system_program: system_program::ID,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::TransferToNewAccount.data(),
    };

    let new_account_pk = new_marginfi_account_key.pubkey();

    let mut signing_keypairs = config.get_signers(false);
    signing_keypairs.push(&new_marginfi_account_key);
    let sig = send_tx(config, vec![ix], &signing_keypairs)?;
    println!(
        "Transfer successful (sig: {})\nNew account: {}",
        sig, new_account_pk
    );

    if config.send_tx {
        let mut profile = profile.clone();
        profile.set_marginfi_account(Some(new_account_pk))?;
    }

    Ok(())
}

pub fn marginfi_account_create_pda(
    profile: &Profile,
    config: &Config,
    account_index: u16,
    third_party_id: Option<u16>,
) -> Result<()> {
    let group_pk = profile
        .marginfi_group
        .context("marginfi group not set in profile")?;
    let authority = config.authority();

    let third_party_id_val = third_party_id.unwrap_or(0);

    let (marginfi_account_pda, _bump) = Pubkey::find_program_address(
        &[
            MARGINFI_ACCOUNT_SEED.as_bytes(),
            group_pk.as_ref(),
            authority.as_ref(),
            &account_index.to_le_bytes(),
            &third_party_id_val.to_le_bytes(),
        ],
        &config.program_id,
    );

    let ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::MarginfiAccountInitializePda {
            marginfi_group: group_pk,
            marginfi_account: marginfi_account_pda,
            authority,
            fee_payer: config.explicit_fee_payer(),
            instructions_sysvar: sysvar::instructions::ID,
            system_program: system_program::ID,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::MarginfiAccountInitializePda {
            account_index,
            third_party_id,
        }
        .data(),
    };

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(config, vec![ix], &signing_keypairs)?;
    println!(
        "PDA account created successfully (sig: {})\nAccount: {}",
        sig, marginfi_account_pda
    );

    Ok(())
}

pub fn marginfi_account_set_freeze(
    _profile: &Profile,
    config: &Config,
    marginfi_account_pk: Pubkey,
    frozen: bool,
) -> Result<()> {
    let marginfi_account = config
        .mfi_program
        .account::<MarginfiAccount>(marginfi_account_pk)?;
    let ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::SetAccountFreeze {
            group: marginfi_account.group,
            marginfi_account: marginfi_account_pk,
            admin: config.authority(),
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::MarginfiAccountSetFreeze { frozen }.data(),
    };

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(config, vec![ix], &signing_keypairs)?;
    println!(
        "Account freeze {} successful: {sig}",
        if frozen { "enabled" } else { "disabled" }
    );

    Ok(())
}

pub fn marginfi_account_pulse_health(
    profile: &Profile,
    config: &Config,
    account: Option<Pubkey>,
) -> Result<()> {
    let marginfi_account_pk = match account {
        Some(pubkey) => pubkey,
        None => profile.get_marginfi_account()?,
    };

    let marginfi_account = config
        .mfi_program
        .account::<MarginfiAccount>(marginfi_account_pk)?;

    let banks = HashMap::from_iter(load_all_banks(config, Some(marginfi_account.group))?);

    let observation_metas =
        load_observation_account_metas(&marginfi_account, &banks, vec![], vec![]);

    let mut ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::PulseHealth {
            marginfi_account: marginfi_account_pk,
            group: marginfi_account.group,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::LendingAccountPulseHealth.data(),
    };

    ix.accounts.extend(observation_metas);

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(config, vec![ix], &signing_keypairs)?;
    println!("Pulse health successful: {sig}");

    Ok(())
}
