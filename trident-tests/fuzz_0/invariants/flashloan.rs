#![allow(clippy::too_many_arguments)]

use trident_fuzz::fuzzing::*;

pub fn assert_flashloan_closed_loop_user_unchanged(before: u64, after: u64) {
    invariant!(
        before == after,
        "flashloan: user token balance should be unchanged when borrow_amount == repay_amount and tx succeeds. before: {}, after: {}, delta: {}",
        before,
        after,
        after as i128 - before as i128
    );
}
