#!/usr/bin/env python3
"""Generate SDK builders after `anchor build -p marginfi` refreshes target/idl/marginfi.json."""
import json, re, subprocess
from pathlib import Path
j=json.loads(Path('target/idl/marginfi.json').read_text())
dir=Path('type-crate/src/ix_builders')
oldmap = {}
for path in dir.glob('*.rs'):
 if path.name == 'mod.rs': continue
 for name in re.findall(r'^pub fn (\w+)\(', path.read_text(), re.M):
  if not name.endswith('_with_program_id'): oldmap[name] = path.stem

def mod_for(n):
 if n in oldmap:return oldmap[n]
 if 'rebalance' in n:return 'rebalance'
 if n.startswith('monitor_archive'):return 'archive'
 if n.startswith('propagate_kamino'):return 'kamino'
 if n.startswith('marginfi_account_'):return 'account'
 if n.startswith('marginfi_group_'):return 'pool'
 if n.startswith('lending_pool_'):return 'pool'
 return 'admin'

# Remaining-account layouts are not represented by the IDL. Keep these notes in sync with
# instruction implementations when the program changes.
remaining_notes = {
    'lending_account_deposit': 'For Token-2022, start remaining accounts with the bank mint; append transfer-hook accounts if required.',
    'lending_account_repay': 'For Token-2022, start remaining accounts with the bank mint; append transfer-hook accounts if required.',
    'lending_account_borrow': 'For Token-2022, start with the bank mint. Include each active bank and its oracle accounts; group rate limits can require the borrowed bank oracle group.',
    'lending_account_withdraw': 'For Token-2022, start with the bank mint. Include each active bank and its oracle accounts; group rate limits can require the withdrawn bank oracle group.',
    'lending_account_end_flashloan': 'Pass each active bank followed by its oracle and integration accounts for the health check.',
    'lending_account_pulse_health': 'Pass each active bank followed by its oracle and integration accounts for the health checks.',
    'lending_account_liquidate': 'After the Token-2022 liability mint when required, pass the liquidatee and liquidator health-account partitions in that order.',
    'monitor_archive_upsert_batch': 'Pass one mint account per update in the same order as the updates.',
}

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
 o=['//! Instruction builders generated from the marginfi IDL.', '#![allow(clippy::doc_lazy_continuation)]', '', 'use super::ToAccountMetas;']
 if defined: o.append('use crate::types::{'+', '.join(defined)+'};')
 if any(i['args'] for i in items): o.append('use borsh::BorshSerialize;')
 o += ['use solana_instruction::{AccountMeta, Instruction};', 'use solana_pubkey::Pubkey;','']
 for i in items:
  n=i['name'];name=pascal(n); ac=i['accounts'];args=i['args']
  o += [f'/// Fixed accounts for [`{n}`].', '#[derive(Clone, Debug, PartialEq, Eq)]',f'pub struct {name} {{']
  for a in ac:
   for line in a.get('docs',[]):o.append('    /// '+line)
   o.append('    pub '+a['name']+': '+('Option<Pubkey>' if a.get('optional') else 'Pubkey')+',')
  o += ['}',f'impl {name} {{',f'    pub const DISCRIMINATOR: [u8; 8] = {i["discriminator"]};','', '    /// Construct account metas for a specific program ID, including optional-account sentinels.', '    pub fn to_account_metas_with_program_id(&self, '+('program_id' if any(a.get('optional') for a in ac) else '_program_id')+': Pubkey) -> Vec<AccountMeta> {','        vec![']
  for a in ac:
   is_optional=a.get('optional',False); writable=a.get('writable',False); signer=a.get('signer',False);key='self.'+a['name'];ctor='new' if writable else 'new_readonly';
   if is_optional:
    o.append(f'            match {key} {{ Some(key) => AccountMeta::{ctor}(key, {str(signer).lower()}), None => AccountMeta::new_readonly(program_id, false) }},')
   else:o.append(f'            AccountMeta::{ctor}({key}, {str(signer).lower()}),')
  o += ['        ]','    }','}',f'impl ToAccountMetas for {name} {{','    fn to_account_metas(&self) -> Vec<AccountMeta> {','        self.to_account_metas_with_program_id(crate::ID)','    }','}']
  for doc in i.get('docs',[]):o.append('/// '+doc)
  if n in remaining_notes: o += ['///', '/// '+remaining_notes[n]]
  o += ['///', '/// Append any required remaining accounts after the fixed account metas.','#[allow(clippy::too_many_arguments)]', f'pub fn {n}(',f'    accounts: &{name},']
  for a in args:o.append('    '+a['name']+': '+typ(a['type'])+',')
  o += [') -> Instruction {',f'    {n}_with_program_id(crate::ID, accounts'+''.join(', '+a['name'] for a in args)+')','}','','/// Build the instruction for a program ID selected at runtime.','#[allow(clippy::too_many_arguments)]',f'pub fn {n}_with_program_id(', '    program_id: Pubkey,',f'    accounts: &{name},']
  for a in args:o.append('    '+a['name']+': '+typ(a['type'])+',')
  o += [') -> Instruction {',f'    let {"mut " if args else ""}data = {name}::DISCRIMINATOR.to_vec();']
  for a in args:o.append('    '+a['name']+'.serialize(&mut data).expect("serializing into Vec cannot fail");')
  o += ['    Instruction { program_id, accounts: accounts.to_account_metas_with_program_id(program_id), data }','}','']
 (dir/(mod+'.rs')).write_text('\n'.join(o))
mods=sorted(by)
o=['//! Anchor-independent marginfi instruction builders.','//!','//! Builders serialize fixed accounts and arguments for the current program interface.','//! Append remaining accounts required by the instruction and its oracle or integration setup.','']+['pub mod '+m+';' for m in mods]+['','use solana_instruction::AccountMeta;','','/// Account metas in the order expected by the program.','pub trait ToAccountMetas {','    fn to_account_metas(&self) -> Vec<AccountMeta>;','}','','/// Names of all generated instruction builders.','pub const BUILDERS: &[&str] = &[']
for i in j['instructions']:o.append('    "'+i['name']+'",')
o += ['];',''];(dir/'mod.rs').write_text('\n'.join(o))
subprocess.run(['rustfmt', '--edition', '2021', *map(str, sorted(dir.glob('*.rs')))], check=True)
print('Generated',len(j['instructions']),'builders across',len(mods),'modules')
