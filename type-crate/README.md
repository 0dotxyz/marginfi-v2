# marginfi Rust types and instruction builders

`marginfi-type-crate` provides account types, PDA helpers and instruction builders for the
marginfi 0.1.13 program interface. The instruction builders and PDA helpers can be used without
Anchor or the marginfi program crate. They return Solana `Instruction` values that can be submitted
by a client or invoked from a program.

The default features keep the Anchor integration. An Anchor-free dependency disables the defaults
and selects a network:

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

## Features

| Feature | Adds | Pulls in |
| --- | --- | --- |
| `anchor` (default) | Anchor derives on the account types, and `pdas` | `anchor-lang` |
| `client` (default) | Nothing | Nothing |
| `ix_builders` | Instruction builders | `borsh`, `solana-instruction`, `solana-pubkey` with `borsh` |
| `pdas` | PDA derivations | `solana-instruction`, `solana-pubkey` with `curve25519` and `sha2` |
| `mainnet-beta`, `devnet`, `staging`, `stagingalt`, `localnet` | Select the program `ID` | Nothing |

Every configuration depends on `bytemuck`, `fixed`, `fixed-macro`, `bs58` and `static_assertions`.

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
// Remaining accounts: the mint for Token-2022, then any transfer-hook accounts.
```

Accounts structs are named after the instruction, not the program's Anchor `Accounts` struct. For
example, the program's `PulseHealth` is `lending::LendingAccountPulseHealth` here.

The `*_with_program_id` variant accepts a runtime program ID. It constructs omitted optional
accounts using that ID without changing any other account. PDA derivations also accept a program ID
where applicable.

**Remaining accounts are not inferred by the builders.** They follow the fixed accounts, in the order
the instruction and its token, oracle and integration configuration require. Each builder's doc lists
its remaining accounts. For a health check, each active
bank is followed by its oracle and integration accounts; `pdas::bank_observation_keys` derives those
keys for the bank's configured oracle setup. Additional accounts may precede or follow those groups,
for example the Token-2022 mint, transfer-hook accounts, or multiple health partitions. The
instruction's implementation is the authority for the full layout. In particular, rebalance has
its own positional layout and is not a generic health-account list.

A CPI passes `AccountInfo`s in the same order, including every dynamically required account.
`examples/native_cpi.rs` and `programs/native-cpi-example` in the repository are Anchor-free examples.
An instruction may impose additional restrictions on CPI or transaction ordering.

## Calling from a non-Anchor program

`ix_builders` and `pdas` do not enable Anchor, so a non-Anchor program depends only on the modular
Solana crates:

```toml
[dependencies]
marginfi-type-crate = { version = "0.2.0", default-features = false, features = ["ix_builders", "pdas", "mainnet-beta"] }
solana-account-info = "3.1"
solana-cpi = "3.1"
solana-instruction = "3.4"
solana-program-error = "3.0"
```

The function below builds the instruction, appends the dynamic accounts, and passes matching
`AccountInfo`s to `invoke_signed`. The `AccountInfo` list includes the marginfi program account, which
is not an instruction account. A Token-2022 deposit puts the mint right after the fixed accounts,
followed by any transfer-hook accounts. The function is compiled from `examples/native_cpi.rs`:

```rust,ignore
use marginfi_type_crate::ix_builders::{self, lending::LendingAccountDeposit};
use solana_account_info::AccountInfo;
use solana_cpi::invoke_signed;
use solana_instruction::AccountMeta;
use solana_program_error::ProgramError;

pub fn cpi_deposit<'a>(
    accounts: &LendingAccountDeposit,
    amount: u64,
    fixed_account_infos: &[AccountInfo<'a>],
    marginfi_program: AccountInfo<'a>,
    token_2022_mint: Option<(AccountMeta, AccountInfo<'a>)>,
    signer_seeds: &[&[&[u8]]],
) -> Result<(), ProgramError> {
    let mut ix = ix_builders::lending::lending_account_deposit(accounts, amount, None);
    let mut account_infos = fixed_account_infos.to_vec();

    if let Some((mint_meta, mint_info)) = token_2022_mint {
        ix.accounts.push(mint_meta);
        account_infos.push(mint_info);
    }

    account_infos.push(marginfi_program);
    invoke_signed(&ix, &account_infos, signer_seeds)
}
```

## Reading account state

Account structs support zero-copy access with `bytemuck`. A safe read checks the account owner, the
discriminator and the data length before casting:

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

`load_bank` leaves the owner check, against the marginfi program ID, to its caller. The cast also
fails if the buffer is not properly aligned.

Derives on the account types differ with the `anchor` feature. For example, `Bank` derives
`Default`, `PartialEq` and `Eq` only with it. Every account type implements `bytemuck::Zeroable` in
both configurations, so `Zeroable::zeroed()` builds one either way.

## Maintenance

`anchor build -p marginfi -- --no-default-features --features custom-heap` refreshes the IDL, and
`python3 scripts/generate-ix-builders.py` regenerates the builders from it. A release of the SDK needs
both the Anchor parity tests and the separate non-Anchor serialization tests to pass. The IDL does not
describe remaining-account layouts: the generator's `remaining_notes` holds them, and the generator
fails when a handler that reads remaining accounts has no note.
