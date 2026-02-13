# JupLend integration patch order

This file tracks the **intended apply order** of the incremental JupLend integration patches we generated.

> Apply patches from the repo root.

## Apply order

1. `juplend_mocks_cargo_error.patch`
2. `juplend_lending_idl_declare_program.patch`
3. `juplend_add_bank.patch`
4. `juplend_init_position.patch`
5. `juplend_deposit_withdraw.patch`
6. `juplend_liquidation.patch`
7. `juplend_liquidation_test_and_ts_plan.patch`
8. `juplend_ts_utils.patch`
9. `juplend_bankrun_builder.patch`
10. `juplend_anchor_genesis_and_dump_script.patch`
11. `juplend_bankrun_basic_test.patch`
12. `juplend_interest_test.patch`
13. `juplend_tests_fix.patch`
14. `juplend_ts_test_env_refactor.patch`
15. `juplend_ts_test_env_and_stale_health_test.patch`
16. `juplend_oracle_conversion_test.patch`
17. `juplend_deposit_withdraw_matrix_tests.patch`
18. `juplend_liquidation_and_limits_tests.patch`
19. `juplend_activation_and_health_pulse_tests.patch`
20. `juplend_receivership_allowlist_test.patch`
## How to apply

Use `git apply` (preferred) or `patch`.

### Using git

```bash
git apply /path/to/the.patch
```

If you hit whitespace issues:

```bash
git apply --reject --whitespace=fix /path/to/the.patch
```

### Using patch

```bash
patch -p1 < /path/to/the.patch
```
