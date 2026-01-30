use crate::utils::wrapped_i80f48_to_f64;
use anchor_lang::prelude::*;
use marginfi_type_crate::types::{u32_to_centi, u32_to_milli, Bank};

/// Echo the information used to create banks to the log output. Useful for at-a-glance debugging
/// bank creation txes in explorer. Note: costs a lot of CU
pub fn log_pool_info(bank: &Bank) {
    let conf = bank.config;
    msg!(
        "Asset weight init: {:?} maint: {:?}",
        wrapped_i80f48_to_f64(conf.asset_weight_init),
        wrapped_i80f48_to_f64(conf.asset_weight_maint)
    );
    msg!(
        "Liab weight init: {:?} maint: {:?}",
        wrapped_i80f48_to_f64(conf.liability_weight_init),
        wrapped_i80f48_to_f64(conf.liability_weight_maint)
    );
    msg!(
        "deposit limit: {:?} borrow limit: {:?} init val limit: {:?}",
        conf.deposit_limit,
        conf.borrow_limit,
        conf.total_asset_value_init_limit
    );
    msg!(
        "op state: {:?} risk tier: {:?} asset tag: {:?}",
        conf.operational_state as u8,
        conf.risk_tier as u8,
        conf.asset_tag
    );
    msg!(
        "oracle conf {:?} age: {:?} flags: {:?}",
        conf.oracle_max_confidence,
        conf.oracle_max_age,
        bank.flags as u8
    );
    let interest = conf.interest_rate_config;
    msg!(
        "Insurance fixed: {:?} ir: {:?}",
        wrapped_i80f48_to_f64(interest.insurance_fee_fixed_apr),
        wrapped_i80f48_to_f64(interest.insurance_ir_fee)
    );
    msg!(
        "Group fixed: {:?} ir: {:?} origination: {:?}",
        wrapped_i80f48_to_f64(interest.protocol_fixed_fee_apr),
        wrapped_i80f48_to_f64(interest.protocol_ir_fee),
        wrapped_i80f48_to_f64(interest.protocol_origination_fee)
    );
    msg!(
        "Rate at 0: {:.4} rate at 100: {:.4}",
        u32_to_milli(interest.zero_util_rate).to_num::<f64>(),
        u32_to_milli(interest.hundred_util_rate).to_num::<f64>(),
    );
    for (i, p) in interest.points.iter().enumerate() {
        if p.util != 0 {
            msg!(
                "  Point {}: util={:.4} rate={:.4}",
                i,
                u32_to_centi(p.util).to_num::<f64>(),
                u32_to_milli(p.rate).to_num::<f64>(),
            );
        }
    }
}
