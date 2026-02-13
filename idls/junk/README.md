TODO: Add a bash or python script to convert IDLs from original to workable with declare_program macro.
- Find all repeated instances of nested account names, e.g. farms_accounts, deposit_accounts
- After the second instance of each nested account name pre-append the original repeated account with the instruction name, e.g. deposit_collateral_v2_farms_accounts

It should be easy enough to write a python script to do this. We just did it manually last time and got chatgpt to list out all the instances of repeated names and manually edited them which was quick enough to do.