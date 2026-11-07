#!/usr/bin/env python3
"""Generates the SDK instruction builders from target/idl/marginfi.json, which `anchor build -p marginfi` refreshes."""
import json, re, subprocess
from pathlib import Path
j=json.loads(Path('target/idl/marginfi.json').read_text())
dir=Path('type-crate/src/ix_builders')
oldmap = {}
for path in dir.glob('*.rs'):
 if path.name == 'mod.rs': continue
 for name in re.findall(r'^pub fn (\w+)\(', path.read_text(), re.M):
  if not name.endswith('_with_program_id'): oldmap[name] = path.stem

venues=('drift','juplend','kamino','solend')
def mod_for(n):
 if n in oldmap:return oldmap[n]
 if 'rebalance' in n:return 'rebalance'
 if n.startswith('monitor_archive'):return 'archive'
 for v in venues:
  if n.startswith((v+'_','propagate_'+v)) or n=='lending_pool_add_bank_'+v:return v
 if n.startswith('marginfi_account_'):return 'account'
 if n.startswith('marginfi_group_'):return 'pool'
 if n.startswith('lending_pool_'):return 'pool'
 return 'admin'

# marginfi's discriminators are in ix_discriminators::native and ::integration (builders in a venue module).
consts=Path('type-crate/src/constants.rs').read_text()
ixd_src=consts[consts.index('pub mod ix_discriminators {'):]
def sub_for(n):return 'integration' if mod_for(n) in venues else 'native'
ixd={}
for sub in ('native','integration'):
 body=ixd_src[ixd_src.index(f'pub mod {sub} {{'):]
 for k,v in re.findall(r'pub const (\w+): \[u8; 8\] =\s*\[([^\]]*)\]',body[:body.index('\n    }\n')]):ixd[tuple(map(int,v.split(',')))]=(sub,k)
missing=[i for i in j['instructions'] if tuple(i['discriminator']) not in ixd]
if missing:raise SystemExit('ix_discriminators in type-crate/src/constants.rs has no constant for:\n'+'\n'.join(f"    {sub_for(i['name'])}: pub const {i['name'].upper()}: [u8; 8] = {i['discriminator']};" for i in missing))
misplaced=[f"{i['name']} (in {ixd[tuple(i['discriminator'])][0]}, builder in {mod_for(i['name'])})" for i in j['instructions'] if ixd[tuple(i['discriminator'])][0]!=sub_for(i['name'])]
if misplaced:raise SystemExit('ix_discriminators has these in the wrong submodule: '+', '.join(misplaced))

# The IDL has no remaining-account layouts; these notes hold them per instruction.
health_note = 'Remaining accounts: each active bank followed by its oracle and integration accounts, for the health check.'
t22_transfer_note = 'Remaining accounts for Token-2022: the bank mint, then the transfer-hook accounts if the mint has a hook.'
new_setup_note = 'Remaining accounts: the oracle and integration accounts of the new setup, in the order a health check takes them.'
rebalance_blocks = 'the same bank blocks as `marginfi_account_start_rebalance`, in the same order'
remaining_notes = {
    'lending_account_borrow': 'Remaining accounts: the bank mint for Token-2022, then each active bank and its oracle accounts. Group rate limits can require the borrowed bank oracle group.',
    'lending_account_withdraw': 'Remaining accounts: the bank mint for Token-2022, then each active bank and its oracle accounts. Group rate limits can require the withdrawn bank oracle group.',
    'lending_account_pulse_health': 'Remaining accounts: each active bank followed by its oracle and integration accounts, for the health checks.',
    'lending_account_liquidate': "Remaining accounts: the liability mint for Token-2022, the asset bank's oracle and integration accounts, the liability bank's, any transfer-hook accounts, the liquidator's health accounts, then the liquidatee's.",
    'monitor_archive_upsert_batch': 'Remaining accounts: one mint account per update, in the order of the updates.',
    **dict.fromkeys([
        'lending_account_end_flashloan', 'start_liquidation', 'end_liquidation', 'start_deleverage',
        'end_deleverage', 'marginfi_account_start_execute_order', 'marginfi_account_end_execute_order',
        'kamino_withdraw', 'drift_withdraw', 'solend_withdraw', 'juplend_withdraw',
        'marginfi_account_tag_liq_record',
    ], health_note),
    **dict.fromkeys([
        'lending_account_deposit', 'lending_account_repay', 'lending_pool_collect_bank_fees',
        'lending_pool_withdraw_fees', 'lending_pool_withdraw_fees_permissionless',
        'lending_pool_withdraw_insurance', 'super_admin_deposit', 'super_admin_withdraw',
        'lending_pool_collect_bank_premium_fees',
    ], t22_transfer_note),
    **dict.fromkeys(['lending_pool_configure_bank_oracle', 'lending_pool_set_oracle_price'], new_setup_note),
    'lending_pool_handle_bankruptcy': 'Remaining accounts: the bank mint for Token-2022, then each active bank of the bankrupt account followed by its oracle and integration accounts.',
    'lending_pool_add_bank_drift': 'Remaining accounts: the oracle, then the spot market.',
    'lending_pool_add_bank_solend': 'Remaining accounts: the oracle, then the reserve.',
    'lending_pool_pulse_bank_price_cache': "Remaining accounts: the bank's oracle and integration accounts from `pdas::bank_observation_keys`, without the bank itself.",
    'propagate_staked_settings': 'When the settings change the oracle, the new oracle is the only remaining account.',
    'lending_pool_add_bank_permissionless': 'Remaining accounts: the oracle, the LST mint, the SOL pool, then the on-ramp.',
    'marginfi_account_start_rebalance': "Remaining accounts: one block per bank in the order's allowlist; `moves` refer to blocks by index. Each block is the bank, then JupLend's `TokenReserve` for JupLend banks, then the venue's reward accounts, then the bank's oracle and integration accounts.",
    'marginfi_account_end_rebalance': f'Remaining accounts: {rebalance_blocks}, then each active bank followed by its oracle and integration accounts, for the health check.',
    'marginfi_account_settle_rebalance_tip': f'Remaining accounts: {rebalance_blocks}, without the reward accounts.',
    'drift_harvest_reward': "Remaining accounts are forwarded to Drift's withdraw: oracle accounts, then the spot markets (writable), then the reward mint for Token-2022.",
}

lib=Path('programs/marginfi/src/lib.rs').read_text()
handlers={}
for path in Path('programs/marginfi/src/instructions').rglob('*.rs'):
 src=path.read_text()
 for m in re.finditer(r'pub fn (\w+)\b[^{;]*\{',src):
  depth,k=1,m.end()
  while depth and k<len(src):depth+={'{':1,'}':-1}.get(src[k],0);k+=1
  handlers[m.group(1)]=handlers.get(m.group(1),'')+src[m.start():k]
def reads_remaining(n):
 seg=lib[re.search(rf'pub fn {n}\b',lib).start():]
 seg=seg[:seg.find('pub fn ',7)]
 return any('remaining_accounts' in handlers.get(c,'') for c in re.findall(r'(\w+)\(\s*ctx\b',seg))
undocumented=[i['name'] for i in j['instructions'] if reads_remaining(i['name']) and i['name'] not in remaining_notes and 'remaining' not in ' '.join(i.get('docs',[])).lower()]
if undocumented:raise SystemExit('remaining_notes has no entry for these instructions, whose handlers read remaining accounts: '+', '.join(undocumented))

def pascal(n):return ''.join(w.capitalize() for w in n.split('_'))
def typ(t):
 if isinstance(t,str):return {'pubkey':'Pubkey','bytes':'Vec<u8>'}.get(t,t)
 if 'option' in t:return 'Option<'+typ(t['option'])+'>'
 if 'vec' in t:return 'Vec<'+typ(t['vec'])+'>'
 if 'array' in t:return '['+typ(t['array'][0])+'; '+str(t['array'][1])+']'
 if 'defined' in t:return t['defined']['name']
 raise ValueError(t)
by={}
for i in j['instructions']:by.setdefault(mod_for(i['name']),[]).append(i)
for mod,items in by.items():
 defined = sorted(set(re.findall(r"'name': '([A-Za-z][A-Za-z0-9_]*)'", str([a['type'] for i in items for a in i['args']]))))
 o=['//! Instruction builders generated from the marginfi IDL.', '#![allow(clippy::doc_lazy_continuation)]', '', 'use super::ToAccountMetas;', 'use crate::constants::ix_discriminators;']
 if defined: o.append('use crate::types::{'+', '.join(defined)+'};')
 if any(i['args'] for i in items): o.append('use borsh::BorshSerialize;')
 o += ['use solana_instruction::{AccountMeta, Instruction};', 'use solana_pubkey::Pubkey;','']
 for i in items:
  n=i['name'];name=pascal(n); ac=i['accounts'];args=i['args']
  o += [f'/// Fixed accounts for [`{n}`].', '#[derive(Clone, Debug, PartialEq, Eq)]',f'pub struct {name} {{']
  for a in ac:
   for line in a.get('docs',[]):o.append('    /// '+line)
   o.append('    pub '+a['name']+': '+('Option<Pubkey>' if a.get('optional') else 'Pubkey')+',')
  o += ['}',f'impl {name} {{','    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::{}::{};'.format(*ixd[tuple(i["discriminator"])]),'', '    /// Account metas for `program_id`. A missing optional account is passed as `program_id`.', '    pub fn to_account_metas_with_program_id(&self, '+('program_id' if any(a.get('optional') for a in ac) else '_program_id')+': Pubkey) -> Vec<AccountMeta> {','        vec![']
  for a in ac:
   is_optional=a.get('optional',False); writable=a.get('writable',False); signer=a.get('signer',False);key='self.'+a['name'];ctor='new' if writable else 'new_readonly';
   if is_optional:
    o.append(f'            match {key} {{ Some(key) => AccountMeta::{ctor}(key, {str(signer).lower()}), None => AccountMeta::new_readonly(program_id, false) }},')
   else:o.append(f'            AccountMeta::{ctor}({key}, {str(signer).lower()}),')
  o += ['        ]','    }','}',f'impl ToAccountMetas for {name} {{','    fn to_account_metas(&self) -> Vec<AccountMeta> {','        self.to_account_metas_with_program_id(crate::ID)','    }','}']
  for doc in i.get('docs',[]):o.append('/// '+doc)
  if n in remaining_notes: o += ['///', '/// '+remaining_notes[n]]
  o += ['///', '/// The returned instruction does not include remaining accounts.','#[allow(clippy::too_many_arguments)]', f'pub fn {n}(',f'    accounts: &{name},']
  for a in args:o.append('    '+a['name']+': '+typ(a['type'])+',')
  o += [') -> Instruction {',f'    {n}_with_program_id(crate::ID, accounts'+''.join(', '+a['name'] for a in args)+')','}','','/// Builds the instruction for a program ID chosen at runtime.','#[allow(clippy::too_many_arguments)]',f'pub fn {n}_with_program_id(', '    program_id: Pubkey,',f'    accounts: &{name},']
  for a in args:o.append('    '+a['name']+': '+typ(a['type'])+',')
  o += [') -> Instruction {',f'    let {"mut " if args else ""}data = {name}::DISCRIMINATOR.to_vec();']
  for a in args:o.append('    '+a['name']+'.serialize(&mut data).expect("serializing into Vec cannot fail");')
  o += ['    Instruction { program_id, accounts: accounts.to_account_metas_with_program_id(program_id), data }','}','']
 (dir/(mod+'.rs')).write_text('\n'.join(o))
mods=sorted(by)
o=['//! Anchor-independent marginfi instruction builders.','//!','//! Builders serialize fixed accounts and arguments for the current program interface.','//! Remaining accounts depend on the instruction and its oracle or integration setup, and are not included.','']+['pub mod '+m+';' for m in mods]+['','use solana_instruction::AccountMeta;','','/// Account metas in the order expected by the program.','pub trait ToAccountMetas {','    fn to_account_metas(&self) -> Vec<AccountMeta>;','}','','/// Names of all generated instruction builders.','pub const BUILDERS: &[&str] = &[']
for i in j['instructions']:o.append('    "'+i['name']+'",')
o += ['];',''];(dir/'mod.rs').write_text('\n'.join(o))
subprocess.run(['rustfmt', '--edition', '2021', *map(str, sorted(dir.glob('*.rs')))], check=True)
print('Generated',len(j['instructions']),'builders across',len(mods),'modules')
