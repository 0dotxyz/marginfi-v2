# Rust Types for Marginfi-v2

Want to use the Mrgnlend types without importing the entire program? Look no further. This is the
on-chain types, a discriminator const per account, instruction builders, and PDA derivations, with
the bare minimum dependencies beyond that.

Notes:
* `types::Pubkey` is a 32-byte stub by default, to avoid pulling any crypto dependencies. Enabling
  `ix_builders`, `pdas`, or `anchor` makes it the real `solana_pubkey::Pubkey`.
* `types::pubkey::Pubkey` is always the stub, under every feature set. Reach for `types::Pubkey`
  unless you specifically want the dependency-free one.
* Discriminators are available as a const for each on-chain struct.
* `Default`, `PartialEq`, and `Eq` on the zero-copy account structs come from the `anchor` feature.
  Without it, construct with `<Bank as bytemuck::Zeroable>::zeroed()`.

## Features

Every configuration pulls `id-crate`, `bytemuck`, `fixed`, `fixed-macro`, `bs58`,
`static_assertions`, and `solana-pubkey`. The table lists what each feature adds on top.

| Feature | Adds | Pulls |
| --- | --- | --- |
| _(none)_ (default) | Types only, stub Pubkey | nothing |
| `ix_builders` | Instruction builders | `borsh`, `solana-instruction`, `solana-pubkey/borsh` |
| `pdas` | PDA derivations | `solana-instruction`, `solana-pubkey` curve25519/sha2 (off-chain only; syscalls on-chain) |
| `anchor` | Anchor account/serde derives | `anchor-lang`, `pdas` |
| `mainnet-beta` / `devnet` / `staging` / `stagingalt` / `localnet` | Selects which program `ID` is | nothing |

`ix_builders` and `pdas` never pull anchor, so a non-anchor on-chain program can depend on this
crate without inheriting an anchor version. Builders return a plain `Instruction`, leaving the
caller to submit or CPI it however they prefer.

## Reading account state

The account structs are `bytemuck::Pod` without any feature enabled, so a client or a non-anchor
on-chain program can borrow them straight out of the account buffer. Skip the 8-byte discriminator
and cast the rest:

```rust
use marginfi_type_crate::{constants::discriminators, types::Bank};

fn load_bank(data: &[u8]) -> Option<&Bank> {
    if data.len() < 8 || data[..8] != discriminators::BANK {
        return None;
    }
    bytemuck::try_from_bytes::<Bank>(&data[8..8 + core::mem::size_of::<Bank>()]).ok()
}
```

No allocation and no deserialization pass; this is the same mechanism Anchor's `AccountLoader`
uses. Account data is 8-byte aligned and the account structs are `align(8)`, so the `+8` offset
stays aligned.

## Building instructions

Enable `ix_builders`, which has an accounts struct and a builder per instruction:

```rust
let mut ix = ix_builders::lending::lending_account_deposit(
    &ix_builders::lending::LendingAccountDeposit { group, marginfi_account, authority, bank, signer_token_account, liquidity_vault, token_program },
    amount,
    None,
);
ix.accounts.push(remaining_account);
```

Builders emit only the fixed accounts. Where a builder's doc comment names `remaining_accounts`,
push those onto `Instruction::accounts` yourself, in the order the doc gives.

Struct names follow the instruction, not the program's `Accounts` struct, so the IDL's
`PulseHealth` is `lending::LendingAccountPulseHealth` here.

### Picking a cluster

Builders address every instruction to `ID`, which the network feature selects, so `ix_builders`
requires one of `mainnet-beta`, `devnet`, `staging`, `stagingalt`, or `localnet`. Building without
one is a compile error rather than a silent default.

```toml
marginfi-type-crate = { version = "0.2.0", features = ["ix_builders", "mainnet-beta"] }
```

Clients that choose their cluster at runtime can retarget a built instruction instead:

```rust
let ix = ix_builders::with_program_id(
    ix_builders::lending::lending_account_deposit(&accounts, amount, None),
    program_id,
);
```
