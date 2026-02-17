use anchor_lang::{
    prelude::{AccountInfo, CpiContext},
    Result,
};

use anchor_spl::token_interface;

pub fn mint_with_signer<'info>(
    token_program: AccountInfo<'info>,
    token_mint: AccountInfo<'info>,
    token_mint_authority: AccountInfo<'info>,
    user_token_ata: AccountInfo<'info>,
    authority_signer_seeds: &[&[&[u8]]],
    mint_amount: u64,
) -> Result<()> {
    token_interface::mint_to(
        CpiContext::new_with_signer(
            token_program,
            token_interface::MintTo {
                mint: token_mint,
                to: user_token_ata,
                authority: token_mint_authority,
            },
            authority_signer_seeds,
        ),
        mint_amount,
    )?;

    Ok(())
}

pub fn burn<'info>(
    token_mint: AccountInfo<'info>,
    user_token_ata: AccountInfo<'info>,
    user: AccountInfo<'info>,
    token_program: AccountInfo<'info>,
    burn_amount: u64,
) -> Result<()> {
    token_interface::burn(
        CpiContext::new(
            token_program,
            token_interface::Burn {
                mint: token_mint,
                from: user_token_ata,
                authority: user,
            },
        ),
        burn_amount,
    )?;

    Ok(())
}
