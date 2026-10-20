// Focused entry point: cargo build --release --features native_orders_test.
// Keep all fourteen native actions in the general fixture too, so they participate in the normal campaign.
#[derive(Clone)]
struct NativeOrdersFixture {
    ctx: TestContext,
    inner: Box<MarginfiFixture>,
}

#[fuzz_fixture]
impl NativeOrdersFixture {
    pub fn setup() -> Self {
        let mut inner = Box::new(MarginfiFixture::setup_base());
        assert!(inner.action_init_global_fee_state(0, 0));
        inner.native_ensure_for_campaign();
        Self { ctx: inner.ctx.clone(), inner }
    }

    // Crucible snapshots the outer context. Move it into the shared scenario implementation while
    // executing, then return it even on a panic so crash replay sees the correct SVM and dirty set.
    fn run(&mut self, action: impl FnOnce(&mut MarginfiFixture) -> bool) -> bool {
        std::mem::swap(&mut self.ctx, &mut self.inner.ctx);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| action(&mut self.inner)));
        std::mem::swap(&mut self.ctx, &mut self.inner.ctx);
        match result { Ok(ok) => ok, Err(panic) => std::panic::resume_unwind(panic) }
    }

    pub fn action_place_order(&mut self, trigger: u8, slippage_bps: u16) -> bool {
        self.run(|f| f.native_place_order(trigger, slippage_bps))
    }
    pub fn action_execute_order(&mut self, variant: u8, amount: u16) -> bool {
        self.run(|f| f.native_execute_order(variant, amount))
    }
    pub fn action_cancel_order(&mut self, unauthorized: bool) -> bool {
        self.run(|f| f.native_cancel_order(unauthorized))
    }
    pub fn action_place_rebalance(&mut self, amount: u32, tip: u32) -> bool {
        self.run(|f| f.native_place_rebalance(amount, tip))
    }
    pub fn action_update_rebalance(&mut self, amount: u32, tip: u32) -> bool {
        self.run(|f| f.native_update_rebalance(amount, tip))
    }
    pub fn action_execute_rebalance(&mut self, variant: u8, amount: u32) -> bool {
        self.run(|f| f.native_execute_rebalance(variant, amount))
    }
    pub fn action_settle_rebalance(&mut self, advance: bool, refund: bool) -> bool {
        self.run(|f| f.native_settle_rebalance(advance, refund))
    }
    pub fn action_cancel_rebalance(&mut self, unauthorized: bool) -> bool {
        self.run(|f| f.native_cancel_rebalance(unauthorized))
    }
    pub fn action_fee_pool(&mut self, withdraw: bool, amount: u32) -> bool {
        self.run(|f| f.native_rebalance_fee_pool(withdraw, amount))
    }
    pub fn action_move_price(&mut self, bank_index: u8, pct: u8, down: bool) -> bool {
        self.run(|f| f.native_move_price(bank_index, pct, down))
    }
    pub fn action_keeper_close_order(&mut self) -> bool {
        self.run(|f| f.native_keeper_close_order())
    }
    pub fn action_set_keeper_close_flags(&mut self, unauthorized: bool, all: bool) -> bool {
        self.run(|f| f.native_set_keeper_close_flags(unauthorized, all))
    }
    pub fn action_execute_rebalance_multi(&mut self, variant: u8, amount: u32) -> bool {
        self.run(|f| f.native_execute_rebalance_multi(variant, amount))
    }
    pub fn action_liquidate_order_user(&mut self, variant: u8, seize_pct: u8) -> bool {
        self.run(|f| f.native_liquidate_order_user(variant, seize_pct))
    }

    fn after_action(&mut self) {
        scout_log_action_outcome();
    }
}

#[cfg(feature = "native_orders_test")]
#[invariant_test]
fn native_orders_test(_f: &mut NativeOrdersFixture) {
    scout_check_session!();
    fixture.run(|f| { f.native_check_invariants(); true });
}
