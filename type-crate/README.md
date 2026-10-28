# marginfi Rust types and instruction builders

`marginfi-type-crate` provides account types, PDA helpers and instruction builders for the
marginfi 0.1.13 program interface. The instruction builders and PDA helpers can be used without
Anchor or the marginfi program crate. They return Solana `Instruction` values that can be submitted
by a client or invoked from a program.

The default features retain the existing Anchor integration. For an Anchor-free dependency, disable
those defaults and select a network explicitly:

```toml
marginfi-type-crate = { version = "0.2.0", default-features = false, features = ["ix_builders", "pdas", "mainnet-beta"] }
```

The network options are `mainnet-beta`, `devnet`, `staging`, `stagingalt`, and `localnet`. Exactly
one is required with `ix_builders`. `pdas` is independent of `ix_builders`; `anchor` enables
Anchor derives and `pdas`. With all three features disabled, the crate's normal dependency graph
contains no Solana or Anchor crates, and `types::Pubkey` is a lightweight stub. In this types-only
configuration, the program `ID` is not exported, including when a network feature is selected.
With any of the three features enabled, `types::Pubkey` is the Solana pubkey and `ID` is available.
The stub remains available at `types::pubkey::Pubkey` in every configuration. Cargo unifies features
across dependents, so another dependency can enable a Solana-facing feature for the shared crate.

## Building an instruction

Each instruction has an accounts struct and a function named after the program instruction. Fixed
accounts, signer and writable flags, the discriminator, and arguments come from the program IDL.
For example:

```rust,ignore
use marginfi_type_crate::ix_builders::lending::{
    lending_account_deposit, LendingAccountDeposit,
};

let mut ix = lending_account_deposit(
    &LendingAccountDeposit {
        group, marginfi_account, authority, bank, signer_token_account, liquidity_vault,
        token_program,
    },
    amount,
    None,
);
// Append the mint for Token-2022 and any transfer-hook accounts required by the token.
```

The `*_with_program_id` variant accepts a runtime program ID. It constructs omitted optional
accounts using that ID without changing any other account. PDA derivations also accept a program ID
where applicable.

**Remaining accounts are not inferred by the builders.** Append them in the order required by the
instruction and its token, oracle and integration configuration. For a health check, each active
bank is followed by its oracle and integration accounts; `pdas::bank_observation_keys` derives those
keys for the bank's configured oracle setup. Additional accounts may precede or follow those groups,
for example the Token-2022 mint, transfer-hook accounts, or multiple health partitions. The
instruction's implementation is the authority for the full layout. In particular, rebalance has
its own positional layout and is not a generic health-account list.

For CPI, pass `AccountInfo`s in the corresponding order and include all dynamically required
accounts. See `examples/native_cpi.rs` and `programs/native-cpi-example` in the repository for
Anchor-free examples. An instruction may impose additional restrictions on CPI or transaction
ordering.

## Reading account state

Account structs support zero-copy access with `bytemuck`. Check the account owner and discriminator,
and check the data length before casting. For example:

```rust
use marginfi_type_crate::{constants::discriminators, types::Bank};

fn load_bank(data: &[u8]) -> Option<&Bank> {
    let bytes = data.get(..8 + core::mem::size_of::<Bank>())?;
    if bytes[..8] != discriminators::BANK {
        return None;
    }
    bytemuck::try_from_bytes(&bytes[8..]).ok()
}
```

The caller must verify the account owner against the intended marginfi program ID. The cast can
also fail if the supplied buffer is not properly aligned.

## Maintenance

Run `anchor build -p marginfi -- --no-default-features --features custom-heap` to refresh the IDL,
then `python3 scripts/generate-ix-builders.py` to regenerate the fixed instruction interfaces.
The Anchor parity tests and the separate non-Anchor serialization tests must both pass before
publishing an updated SDK. Review changes to remaining-account layouts separately: those layouts
are not described completely by the IDL.
