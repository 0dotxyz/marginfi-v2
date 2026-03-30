This folder contains minimal IDLs used to build mock programs for integrations, which is in turn
used for e.g. CPI calls.

For the declare_program! macro, anchor searches for a folder named "idls" exactly. We ensure that
the idl here uses the canonical name when possible.

For the complete or "actual" IDL that is more typically used on mainnet, in the TS suite, etc, see
idls-complete.

To see how these minimal IDLs are generated, see the `distill` scripts in /scripts/