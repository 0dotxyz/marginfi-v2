use anchor_lang::prelude::*;
use bytemuck::Zeroable;
use marginfi_type_crate::types::{
    reconcile_emode_configs, reconcile_emode_configs_classic, EmodeConfig,
};
use solana_program::log::sol_log_compute_units;

#[derive(Accounts)]
pub struct BenchReconcileEmode<'info> {
    pub payer: Signer<'info>,
}

/// Benchmark harness for the emode reconcile functions (see `marginfi_type_crate::types::emode`).
///
/// Builds `num_configs` worst-case configs — each packed with the maximum MAX_EMODE_ENTRIES entries,
/// all sharing the same tags so none drop out of the intersection — then brackets each reconcile
/// variant with `sol_log_compute_units`. The CU cost of each is the delta between consecutive
/// "units remaining" log lines: lines [0]-[1] bracket the fixed-buffer `reconcile_emode_configs`,
/// lines [2]-[3] the heap-backed `reconcile_emode_configs_classic`.
///
/// Run via `anchor run bench-emode` (tests/bench/emodeReconcile.bench.ts).
pub fn bench_reconcile_emode(_ctx: Context<BenchReconcileEmode>, num_configs: u8) -> Result<()> {
    // One worst-case config: every entry slot filled with a distinct, non-empty tag.
    let mut full = EmodeConfig::zeroed();
    for (i, entry) in full.entries.iter_mut().enumerate() {
        entry.collateral_bank_emode_tag = (i as u16) + 1;
    }
    // N identical copies, so every tag survives the "seen in every config" intersection. Build the
    // input for each variant up front so neither bracketed region below includes the Vec copy cost —
    // the brackets must measure only the reconcile call itself.
    let configs_fixed: Vec<EmodeConfig> = (0..num_configs).map(|_| full).collect();
    let configs_classic = configs_fixed.clone();

    sol_log_compute_units();
    let fixed = reconcile_emode_configs(configs_fixed);
    sol_log_compute_units();

    sol_log_compute_units();
    let classic = reconcile_emode_configs_classic(configs_classic);
    sol_log_compute_units();

    // Consume the results so the calls aren't optimized away.
    core::hint::black_box((fixed, classic));
    Ok(())
}
