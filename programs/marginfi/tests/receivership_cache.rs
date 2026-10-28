use anchor_lang::prelude::Clock;
use bytemuck::Zeroable;
use marginfi::{
    constants::PROGRAM_VERSION,
    instructions::marginfi_account::{end_receivership, start_receivership},
};
use marginfi_type_crate::types::{HealthCache, LiquidationRecord, MarginfiAccount, MarginfiGroup};
use solana_sdk::program_stubs::{set_syscall_stubs, SyscallStubs};

const STUB_UNIX_TIMESTAMP: i64 = 123;

struct FixedClockStub;

impl SyscallStubs for FixedClockStub {
    fn sol_get_clock_sysvar(&self, var_addr: *mut u8) -> u64 {
        // SAFETY: Clock::get supplies an aligned pointer to a Clock-sized buffer.
        unsafe {
            var_addr.cast::<Clock>().write(Clock {
                unix_timestamp: STUB_UNIX_TIMESTAMP,
                ..Clock::default()
            });
        }
        0
    }
}

// Keep this binary to one test: the syscall stub is global, so parallel tests would race.
#[test]
fn start_and_end_receivership_finalize_health_cache() -> anyhow::Result<()> {
    let mut liquidatee = MarginfiAccount::zeroed();
    let group = MarginfiGroup::default();
    let mut liquidation_record = LiquidationRecord::zeroed();
    set_syscall_stubs(Box::new(FixedClockStub));
    start_receivership(&mut liquidatee, &group, &mut liquidation_record, &[], true)?;
    let cache_after_start = liquidatee.health_cache;

    // Clear the start snapshot so it cannot mask a missing end-cache update.
    liquidatee.health_cache = HealthCache::zeroed();
    end_receivership(
        &mut liquidatee,
        &group,
        &mut liquidation_record,
        &[],
        true,
        false,
    )?;
    let cache_after_end = liquidatee.health_cache;

    for (ix, cache) in [("start", cache_after_start), ("end", cache_after_end)] {
        assert_eq!(cache.timestamp, STUB_UNIX_TIMESTAMP, "{ix}: timestamp");
        assert_eq!(
            cache.program_version, PROGRAM_VERSION,
            "{ix}: program_version"
        );
        assert!(cache.is_engine_ok(), "{ix}: engine_ok");
    }
    Ok(())
}
