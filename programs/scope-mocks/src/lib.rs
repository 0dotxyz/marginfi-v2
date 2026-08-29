#![allow(unexpected_cfgs)]

pub mod state;

use anchor_lang::prelude::*;

declare_id!(marginfi_type_crate::pdas::SCOPE_PROGRAM_ID);

/// Scope is read-only for marginfi (no CPIs), so this program has no instructions: the crate
/// exists to share the mirrored account layout (`state`) with the marginfi program and to
/// produce an IDL for TS tests.
#[program]
pub mod scope_mocks {}
