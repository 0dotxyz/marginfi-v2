use {
    crate::utils::EXP_10_I80F48,
    comfy_table::{presets::UTF8_FULL, Cell, Color, ContentArrangement, Table},
    fixed::types::I80F48,
    marginfi::state::{bank::BankImpl, bank_config::BankConfigImpl},
    marginfi_type_crate::types::{BalanceSide, Bank, MarginfiAccount, MarginfiGroup},
    serde_json::{json, Value},
    solana_sdk::pubkey::Pubkey,
    std::{
        collections::HashMap,
        ops::Not,
        time::{Duration, SystemTime, UNIX_EPOCH},
    },
};

/// Clamp mint_decimals to the supported range to prevent panics.
fn safe_decimals(bank: &Bank) -> usize {
    (bank.mint_decimals as usize).min(EXP_10_I80F48.len() - 1)
}

fn bank_summary_json(address: &Pubkey, bank: &Bank) -> Value {
    let decimals = safe_decimals(bank);
    let total_deposits = bank
        .get_asset_amount(bank.total_asset_shares.into())
        .unwrap_or(I80F48::ZERO)
        / EXP_10_I80F48[decimals];
    let total_liabilities = bank
        .get_liability_amount(bank.total_liability_shares.into())
        .unwrap_or(I80F48::ZERO)
        / EXP_10_I80F48[decimals];

    json!({
        "address": address.to_string(),
        "mint": bank.mint.to_string(),
        "deposits": total_deposits.to_string(),
        "borrows": total_liabilities.to_string(),
        "state": format!("{:?}", bank.config.operational_state),
        "risk_tier": format!("{:?}", bank.config.risk_tier),
    })
}

fn account_balances_json(
    marginfi_account: &MarginfiAccount,
    banks: &HashMap<Pubkey, Bank>,
) -> Vec<Value> {
    marginfi_account
        .lending_account
        .get_active_balances_iter()
        .filter_map(|balance| {
            let bank = banks.get(&balance.bank_pk)?;
            let decimals = safe_decimals(bank);

            let (side, amount) = if balance.is_empty(BalanceSide::Assets).not() {
                let v = bank
                    .get_asset_amount(balance.asset_shares.into())
                    .unwrap_or(I80F48::ZERO)
                    / EXP_10_I80F48[decimals];
                ("deposit", v)
            } else if balance.is_empty(BalanceSide::Liabilities).not() {
                let v = bank
                    .get_liability_amount(balance.liability_shares.into())
                    .unwrap_or(I80F48::ZERO)
                    / EXP_10_I80F48[decimals];
                ("borrow", v)
            } else {
                return None;
            };

            Some(json!({
                "bank": balance.bank_pk.to_string(),
                "mint": bank.mint.to_string(),
                "side": side,
                "amount": format!("{:.6}", amount),
            }))
        })
        .collect()
}

pub fn account_detail_json(
    address: Pubkey,
    marginfi_account: &MarginfiAccount,
    banks: &HashMap<Pubkey, Bank>,
    default: bool,
) -> Value {
    json!({
        "address": address.to_string(),
        "group": marginfi_account.group.to_string(),
        "authority": marginfi_account.authority.to_string(),
        "default": default,
        "balances": account_balances_json(marginfi_account, banks),
    })
}

pub fn banks_table_json(banks: &[(Pubkey, Bank)]) -> Value {
    Value::Array(
        banks
            .iter()
            .map(|(address, bank)| bank_summary_json(address, bank))
            .collect(),
    )
}

pub fn group_detail_json(address: &Pubkey, group: &MarginfiGroup) -> Value {
    json!({
        "address": address.to_string(),
        "admin": group.admin.to_string(),
    })
}

/// Print a single bank in detail (verbose view).
pub fn print_bank_detail(address: &Pubkey, bank: &Bank, json: bool) {
    let decimals = safe_decimals(bank);
    let total_deposits = bank
        .get_asset_amount(bank.total_asset_shares.into())
        .unwrap_or(I80F48::ZERO)
        / EXP_10_I80F48[decimals];
    let total_liabilities = bank
        .get_liability_amount(bank.total_liability_shares.into())
        .unwrap_or(I80F48::ZERO)
        / EXP_10_I80F48[decimals];
    let last_update_hours = SystemTime::now()
        .duration_since(UNIX_EPOCH + Duration::from_secs(bank.last_update as u64))
        .unwrap_or_default()
        .as_secs_f32()
        / 3600_f32;

    if json {
        let val = json!({
            "address": address.to_string(),
            "group": bank.group.to_string(),
            "mint": bank.mint.to_string(),
            "mint_decimals": bank.mint_decimals,
            "total_deposits": total_deposits.to_string(),
            "total_liabilities": total_liabilities.to_string(),
            "config": {
                "operational_state": format!("{:?}", bank.config.operational_state),
                "risk_tier": format!("{:?}", bank.config.risk_tier),
                "total_asset_value_init_limit": bank.config.total_asset_value_init_limit,
                "asset_weight_init": format!("{:?}", bank.config.asset_weight_init),
                "asset_weight_maint": format!("{:?}", bank.config.asset_weight_maint),
                "deposit_limit": bank.config.deposit_limit,
                "liability_weight_init": format!("{:?}", bank.config.liability_weight_init),
                "liability_weight_maint": format!("{:?}", bank.config.liability_weight_maint),
                "borrow_limit": bank.config.borrow_limit,
                "interest_rate": {
                    "optimal_utilization_rate": format!("{:?}", bank.config.interest_rate_config.optimal_utilization_rate),
                    "plateau_interest_rate": format!("{:?}", bank.config.interest_rate_config.plateau_interest_rate),
                    "max_interest_rate": format!("{:?}", bank.config.interest_rate_config.max_interest_rate),
                    "insurance_fee_fixed_apr": format!("{:?}", bank.config.interest_rate_config.insurance_fee_fixed_apr),
                    "insurance_ir_fee": format!("{:?}", bank.config.interest_rate_config.insurance_ir_fee),
                    "protocol_fixed_fee_apr": format!("{:?}", bank.config.interest_rate_config.protocol_fixed_fee_apr),
                    "protocol_ir_fee": format!("{:?}", bank.config.interest_rate_config.protocol_ir_fee),
                },
                "oracle_setup": format!("{:?}", bank.config.oracle_setup),
                "oracle_keys": bank.config.oracle_keys.iter().map(|k| k.to_string()).collect::<Vec<_>>(),
                "oracle_max_age": bank.config.get_oracle_max_age(),
            },
            "emissions": {
                "flags": bank.flags,
                "rate": format!("{:?}", I80F48::from(bank.emissions_rate)),
                "mint": bank.emissions_mint.to_string(),
                "remaining": format!("{:?}", I80F48::from(bank.emissions_remaining)),
            },
            "unclaimed_fees": format!("{:?}", I80F48::from(bank.collected_group_fees_outstanding) / EXP_10_I80F48[decimals]),
            "unclaimed_insurance": format!("{:?}", I80F48::from(bank.collected_insurance_fees_outstanding) / EXP_10_I80F48[decimals]),
            "last_update": bank.last_update,
            "last_update_hours_ago": last_update_hours,
        });
        println!("{}", serde_json::to_string_pretty(&val).unwrap());
    } else {
        let mut table = Table::new();
        table
            .load_preset(UTF8_FULL)
            .set_content_arrangement(ContentArrangement::Dynamic)
            .set_width(100);

        table.set_header(vec![
            Cell::new(format!("Bank: {}", address)),
            Cell::new("Value"),
        ]);

        table.add_row(vec!["Group", &bank.group.to_string()]);
        table.add_row(vec!["Mint", &bank.mint.to_string()]);
        table.add_row(vec!["Total Deposits", &format!("{:.6}", total_deposits)]);
        table.add_row(vec![
            "Total Liabilities",
            &format!("{:.6}", total_liabilities),
        ]);
        table.add_row(vec![
            "Operational State",
            &format!("{:?}", bank.config.operational_state),
        ]);
        table.add_row(vec!["Risk Tier", &format!("{:?}", bank.config.risk_tier)]);
        table.add_row(vec![
            "Asset Weight (Init/Maint)",
            &format!(
                "{:?} / {:?}",
                bank.config.asset_weight_init, bank.config.asset_weight_maint
            ),
        ]);
        table.add_row(vec![
            "Deposit Limit",
            &format!(
                "{:.2}",
                I80F48::from_num(bank.config.deposit_limit) / EXP_10_I80F48[decimals]
            ),
        ]);
        table.add_row(vec![
            "Liability Weight (Init/Maint)",
            &format!(
                "{:?} / {:?}",
                bank.config.liability_weight_init, bank.config.liability_weight_maint
            ),
        ]);
        table.add_row(vec![
            "Borrow Limit",
            &format!(
                "{:.2}",
                I80F48::from_num(bank.config.borrow_limit) / EXP_10_I80F48[decimals]
            ),
        ]);
        table.add_row(vec![
            "Oracle",
            &format!(
                "{:?} (max age: {}s)",
                bank.config.oracle_setup,
                bank.config.get_oracle_max_age()
            ),
        ]);
        table.add_row(vec!["Emissions Flags", &format!("0b{:b}", bank.flags)]);
        table.add_row(vec![
            "Last Update",
            &format!("{:.2}h ago (ts: {})", last_update_hours, bank.last_update),
        ]);

        println!("{table}");
    }
}

/// Print banks in a summary table (for bank get-all / list).
pub fn print_banks_table(banks: &[(Pubkey, Bank)], json: bool) {
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&banks_table_json(banks)).unwrap()
        );
        return;
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic);

    table.set_header(vec![
        "Bank Address",
        "Mint",
        "Deposits",
        "Borrows",
        "State",
        "Risk",
    ]);

    for (address, bank) in banks {
        let decimals = safe_decimals(bank);
        let total_deposits = bank
            .get_asset_amount(bank.total_asset_shares.into())
            .unwrap_or(I80F48::ZERO)
            / EXP_10_I80F48[decimals];
        let total_liabilities = bank
            .get_liability_amount(bank.total_liability_shares.into())
            .unwrap_or(I80F48::ZERO)
            / EXP_10_I80F48[decimals];

        let addr_short = {
            let s = address.to_string();
            format!("{}...{}", &s[..4], &s[s.len() - 4..])
        };

        table.add_row(vec![
            Cell::new(&addr_short),
            Cell::new(&bank.mint.to_string()),
            Cell::new(&format!("{:.2}", total_deposits)),
            Cell::new(&format!("{:.2}", total_liabilities)),
            Cell::new(&format!("{:?}", bank.config.operational_state)),
            Cell::new(&format!("{:?}", bank.config.risk_tier)),
        ]);
    }

    println!("{table}");
    println!("Total banks: {}", banks.len());
}

/// Print a marginfi account with balances.
pub fn print_account_detail(
    address: Pubkey,
    marginfi_account: &MarginfiAccount,
    banks: &HashMap<Pubkey, Bank>,
    default: bool,
    json: bool,
) {
    let balances = account_balances_json(marginfi_account, banks);

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&account_detail_json(
                address,
                marginfi_account,
                banks,
                default,
            ))
            .unwrap()
        );
    } else {
        let label = if default { " (default)" } else { "" };
        println!("Account: {}{}", address, label);
        println!(
            "  Group: {}  Authority: {}",
            marginfi_account.group, marginfi_account.authority
        );

        if balances.is_empty() {
            println!("  No active balances");
            return;
        }

        let mut table = Table::new();
        table
            .load_preset(UTF8_FULL)
            .set_content_arrangement(ContentArrangement::Dynamic);

        table.set_header(vec!["Side", "Amount", "Bank", "Mint"]);

        for b in &balances {
            let side = b["side"].as_str().unwrap_or("");
            let amount = b["amount"].as_str().unwrap_or("0");
            let bank_addr = b["bank"].as_str().unwrap_or("");
            let mint = b["mint"].as_str().unwrap_or("");

            let side_cell = if side == "borrow" {
                Cell::new(side).fg(Color::Red)
            } else {
                Cell::new(side).fg(Color::Green)
            };

            let bank_short = if bank_addr.len() > 8 {
                format!(
                    "{}...{}",
                    &bank_addr[..4],
                    &bank_addr[bank_addr.len() - 4..]
                )
            } else {
                bank_addr.to_string()
            };

            table.add_row(vec![
                side_cell,
                Cell::new(amount),
                Cell::new(&bank_short),
                Cell::new(mint),
            ]);
        }

        println!("{table}");
    }
}

/// Print group info.
pub fn print_group_detail(address: &Pubkey, group: &MarginfiGroup, json: bool) {
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&group_detail_json(address, group)).unwrap()
        );
    } else {
        println!("Group: {}", address);
        println!("  Admin: {}", group.admin);
    }
}
