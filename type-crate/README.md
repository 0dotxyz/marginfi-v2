# Rust types and instruction builders for marginfi v2

This crate exposes marginfi's on-chain account types, account discriminators, instruction builders,
and PDA derivations without requiring consumers to import the marginfi program crate. Anchor support
is optional; native Solana programs can use the builders and PDA helpers without an Anchor dependency.

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

Builders emit the discriminator, serialized arguments, and fixed accounts. Where a builder's doc
comment names `remaining_accounts`, append those metas in the documented order before submitting
or invoking the instruction. The corresponding `AccountInfo` values passed to CPI must use the
same order as the final `Instruction::accounts` list.

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

## Calling from a non-Anchor program

Use the modular Solana crates directly; neither `ix_builders` nor `pdas` enables Anchor:

```toml
[dependencies]
marginfi-type-crate = { version = "0.2.0", default-features = false, features = ["ix_builders", "pdas", "mainnet-beta"] }
solana-account-info = "3.1"
solana-cpi = "3.1"
solana-instruction = "3.4"
solana-program-error = "3.0"
```

Build the instruction, append any documented dynamic metas, and pass matching account infos to
`invoke_signed`. The invoked marginfi program account is included in the account-info slice but is
not an instruction meta. This example is also compiled from `examples/native_cpi.rs`:

```rust,ignore
use marginfi_type_crate::ix_builders::{self, lending::LendingAccountDeposit};
use solana_account_info::AccountInfo;
use solana_cpi::invoke_signed;
use solana_instruction::AccountMeta;

fn cpi_deposit<'a>(
    accounts: &LendingAccountDeposit,
    amount: u64,
    fixed_account_infos: &[AccountInfo<'a>],
    marginfi_program: AccountInfo<'a>,
    token_2022_mint: Option<(AccountMeta, AccountInfo<'a>)>,
    signer_seeds: &[&[&[u8]]],
) -> Result<(), solana_program_error::ProgramError> {
    let mut ix = ix_builders::lending::lending_account_deposit(accounts, amount, None);
    let mut account_infos = fixed_account_infos.to_vec();

    // A Token-2022 deposit requires its mint immediately after the fixed accounts.
    if let Some((mint_meta, mint_info)) = token_2022_mint {
        ix.accounts.push(mint_meta);
        account_infos.push(mint_info);
    }

    // Transfer-hook-enabled mints may require additional metas and AccountInfos after the mint;
    // resolve and append both lists in the order required by the hook program.
    account_infos.push(marginfi_program);
    invoke_signed(&ix, &account_infos, signer_seeds)
}
```

For health-sensitive instructions, append each active bank followed by the keys returned by
`pdas::bank_observation_keys(&bank)`. Follow the individual builder documentation when an
instruction has a Token-2022 mint prefix, transfer-hook accounts, multiple health partitions, or
integration-specific state.
