use crate::arbitrary_helpers::TokenType;
use anchor_lang::{
    prelude::{AccountInfo, Pubkey, Rent, SolanaSysvar},
    AnchorSerialize, Discriminator,
};
use anchor_spl::token_2022::spl_token_2022::{
    self,
    extension::{
        transfer_fee::{TransferFee, TransferFeeConfig},
        BaseStateWithExtensions, BaseStateWithExtensionsMut, ExtensionType, StateWithExtensions,
        StateWithExtensionsMut,
    },
    state::Mint,
};
use bumpalo::Bump;
use marginfi::state::bank::BankVaultType;
use marginfi_type_crate::constants::FEE_STATE_SEED;
use pyth_solana_receiver_sdk::price_update::{PriceUpdateV2, VerificationLevel};
use safe_transmute::transmute_to_bytes_mut;
use solana_program::{
    bpf_loader, program_pack::Pack, stake_history::Epoch, system_program, sysvar,
};
use solana_sdk::{signature::Keypair, signer::Signer};
use std::mem::size_of;

pub struct AccountsState {
    pub bump: Bump,
}

impl AccountsState {
    pub fn new() -> Self {
        Self { bump: Bump::new() }
    }

    fn random_pubkey<'bump>(&'bump self) -> &'bump Pubkey {
        #[allow(deprecated)]
        self.bump.alloc(Pubkey::new_unique())
    }

    pub fn new_sol_account<'bump>(
        &'bump self,
        lamports: u64,
        signer: bool,
        writeable: bool,
    ) -> AccountInfo<'bump> {
        self.new_sol_account_with_pubkey(self.random_pubkey(), lamports, signer, writeable)
    }

    pub fn new_sol_account_with_pubkey<'bump>(
        &'bump self,
        pubkey: &'bump Pubkey,
        lamports: u64,
        signer: bool,
        writeable: bool,
    ) -> AccountInfo<'bump> {
        AccountInfo::new(
            pubkey,
            signer,
            writeable,
            self.bump.alloc(lamports),
            &mut [],
            &system_program::ID,
            false,
            Epoch::default(),
        )
    }

    pub fn new_fee_state<'a>(&'a self, program_id: Pubkey) -> (AccountInfo<'a>, u8) {
        let (fee_state_key, fee_state_bump) =
            Pubkey::find_program_address(&[FEE_STATE_SEED.as_bytes()], &marginfi::ID);

        (
            AccountInfo::new(
                self.bump.alloc(fee_state_key),
                false,
                true,
                self.bump.alloc(9999999),
                self.allocate_dex_owned_account(256 + 8),
                self.bump.alloc(program_id),
                false,
                Epoch::default(),
            ),
            fee_state_bump,
        )
    }

    pub fn new_token_mint<'bump>(
        &'bump self,
        rent: Rent,
        decimals: u8,
        token_type: TokenType,
    ) -> AccountInfo<'bump> {
        match token_type {
            TokenType::Tokenkeg => {
                let data = self.bump.alloc_slice_fill_copy(Mint::LEN, 0u8);
                let mut mint = Mint::default();
                mint.is_initialized = true;
                mint.decimals = decimals;
                Mint::pack(mint, data).unwrap();
                AccountInfo::new(
                    self.random_pubkey(),
                    false,
                    true,
                    self.bump.alloc(rent.minimum_balance(data.len())),
                    data,
                    &spl_token::ID,
                    false,
                    Epoch::default(),
                )
            }
            TokenType::Token22 => {
                let data = self.bump.alloc_slice_fill_copy(Mint::LEN, 0u8);
                let mut mint = Mint::default();
                mint.is_initialized = true;
                mint.decimals = decimals;
                Mint::pack(mint, data).unwrap();
                AccountInfo::new(
                    self.random_pubkey(),
                    false,
                    true,
                    self.bump.alloc(rent.minimum_balance(data.len())),
                    data,
                    &anchor_spl::token_2022::ID,
                    false,
                    Epoch::default(),
                )
            }
            TokenType::Token22WithFee {
                transfer_fee_basis_points,
                maximum_fee,
            } => {
                // Calculate mint size for t22 mint with transfer fee
                let data_len = ExtensionType::try_calculate_account_len::<
                    spl_token_2022::state::Mint,
                >(&[ExtensionType::TransferFeeConfig])
                .unwrap();

                let mut data = self.bump.alloc_slice_fill_copy(data_len, 0u8);
                let mut mint_state =
                    StateWithExtensionsMut::<spl_token_2022::state::Mint>::unpack_uninitialized(
                        &mut data,
                    )
                    .unwrap();
                mint_state.init_account_type().unwrap();

                let transfer_fee_config = mint_state
                    .init_extension::<TransferFeeConfig>(false)
                    .unwrap();
                *transfer_fee_config = TransferFeeConfig {
                    transfer_fee_config_authority: Default::default(),
                    withdraw_withheld_authority: Default::default(),
                    withheld_amount: 0.into(),
                    older_transfer_fee: TransferFee {
                        epoch: 0.into(),
                        maximum_fee: maximum_fee.into(),
                        transfer_fee_basis_points: transfer_fee_basis_points.into(),
                    },
                    newer_transfer_fee: TransferFee {
                        epoch: 0.into(),
                        maximum_fee: maximum_fee.into(),
                        transfer_fee_basis_points: transfer_fee_basis_points.into(),
                    },
                };

                let mut mint = spl_token_2022::state::Mint::default();
                mint.decimals = decimals;
                mint.is_initialized = true;

                mint_state.base = mint;
                mint_state.pack_base();

                AccountInfo::new(
                    self.random_pubkey(),
                    false,
                    true,
                    self.bump.alloc(rent.minimum_balance(data.len())),
                    data,
                    &anchor_spl::token_2022::ID,
                    false,
                    Epoch::default(),
                )
            }
        }
    }

    pub fn new_token_account<'a>(
        &'a self,
        mint_ai: AccountInfo<'a>,
        owner_pubkey: &'a Pubkey,
        balance: u64,
        rent: Rent,
    ) -> AccountInfo<'a> {
        self.new_token_account_with_pubkey(
            Keypair::new().pubkey(),
            mint_ai,
            owner_pubkey,
            balance,
            rent,
        )
    }

    pub fn new_token_account_with_pubkey<'a>(
        &'a self,
        account_pubkey: Pubkey,
        mint_ai: AccountInfo<'a>,
        owner_pubkey: &'a Pubkey,
        balance: u64,
        rent: Rent,
    ) -> AccountInfo<'a> {
        // Load mint
        let mint_data = mint_ai.try_borrow_data().unwrap();
        let mint_state =
            StateWithExtensions::<spl_token_2022::state::Mint>::unpack(&mint_data).unwrap();
        let mint_extensions = mint_state.get_extension_types().unwrap();
        let required_extensions =
            ExtensionType::get_required_init_account_extensions(&mint_extensions);
        let space = ExtensionType::try_calculate_account_len::<spl_token_2022::state::Account>(
            &required_extensions,
        )
        .unwrap();

        let data = self.bump.alloc_slice_fill_copy(space, 0u8);

        let mut token_account_state =
            StateWithExtensionsMut::<spl_token_2022::state::Account>::unpack_uninitialized(data)
                .unwrap();

        if required_extensions.contains(&ExtensionType::TransferFeeAmount) {
            token_account_state
                .init_account_extension_from_type(ExtensionType::TransferFeeAmount)
                .unwrap();
        }

        token_account_state.base = spl_token_2022::state::Account::default();
        token_account_state.base.state = spl_token_2022::state::AccountState::Initialized;
        token_account_state.base.mint = *mint_ai.key;
        token_account_state.base.owner = *owner_pubkey;
        token_account_state.base.amount = balance;

        token_account_state.pack_base();
        token_account_state.init_account_type().unwrap();
        drop(token_account_state);
        assert!(StateWithExtensionsMut::<spl_token_2022::state::Account>::unpack(data).is_ok());

        AccountInfo::new(
            self.bump.alloc(account_pubkey),
            false,
            true,
            self.bump.alloc(rent.minimum_balance(data.len())),
            data,
            mint_ai.owner,
            false,
            Epoch::default(),
        )
    }

    pub fn new_owned_account(
        &self,
        unpadded_len: usize,
        owner_pubkey: Pubkey,
        rent: Rent,
    ) -> AccountInfo {
        let data_len = unpadded_len;
        self.new_dex_owned_account_with_lamports(
            unpadded_len,
            rent.minimum_balance(data_len),
            self.bump.alloc(owner_pubkey),
        )
    }

    pub fn new_blank_owned_account_with_key(
        &self,
        key: Pubkey,
        owner_pubkey: Pubkey,
    ) -> AccountInfo {
        self.new_dex_owned_blank_account_with_key(
            self.bump.alloc(key),
            self.bump.alloc(owner_pubkey),
        )
    }

    pub fn new_dex_owned_account_with_lamports<'bump>(
        &'bump self,
        unpadded_len: usize,
        lamports: u64,
        program_id: &'bump Pubkey,
    ) -> AccountInfo<'bump> {
        AccountInfo::new(
            self.random_pubkey(),
            false,
            true,
            self.bump.alloc(lamports),
            self.allocate_dex_owned_account(unpadded_len),
            program_id,
            false,
            Epoch::default(),
        )
    }

    pub fn new_dex_owned_blank_account_with_key<'bump>(
        &'bump self,
        key: &'bump Pubkey,
        program_id: &'bump Pubkey,
    ) -> AccountInfo<'bump> {
        AccountInfo::new(
            key,
            false,
            true,
            self.bump.alloc(0),
            &mut [],
            program_id,
            false,
            Epoch::default(),
        )
    }

    fn allocate_dex_owned_account<'bump>(&'bump self, unpadded_size: usize) -> &'bump mut [u8] {
        assert_eq!(unpadded_size % 8, 0);
        let padded_size = unpadded_size;
        let u64_data = self.bump.alloc_slice_fill_copy(padded_size / 8 + 1, 0u64);

        transmute_to_bytes_mut(u64_data) as _
    }

    pub fn new_spl_token_program(&self) -> AccountInfo {
        self.new_program(spl_token::id())
    }

    pub fn new_system_program(&self) -> AccountInfo {
        self.new_program(system_program::id())
    }

    pub fn new_marginfi_program(&self) -> AccountInfo {
        self.new_program(marginfi::ID)
    }

    pub fn new_program(&self, pubkey: Pubkey) -> AccountInfo {
        AccountInfo::new(
            self.bump.alloc(pubkey),
            false,
            false,
            self.bump.alloc(0),
            &mut [],
            &bpf_loader::ID,
            true,
            Epoch::default(),
        )
    }

    pub fn new_oracle_account(
        &self,
        rent: Rent,
        native_price: i64,
        mint_decimals: i32,
    ) -> AccountInfo {
        let feed_key = *self.random_pubkey();
        let feed_id = feed_key.to_bytes();
        let price_update = PriceUpdateV2 {
            write_authority: Pubkey::default(),
            verification_level: VerificationLevel::Full,
            price_message: pyth_solana_receiver_sdk::price_update::PriceFeedMessage {
                feed_id,
                price: native_price,
                conf: 0,
                exponent: -mint_decimals,
                publish_time: 0,
                prev_publish_time: 0,
                ema_price: native_price,
                ema_conf: 0,
            },
            posted_slot: 1,
        };

        let mut data = vec![];
        let mut account_data = vec![];

        data.extend_from_slice(PriceUpdateV2::DISCRIMINATOR);

        price_update.serialize(&mut account_data).unwrap();

        data.extend_from_slice(&account_data);

        let data_len = data.len();
        let lamports = self.bump.alloc(rent.minimum_balance(data_len));
        let data_ptr = self.bump.alloc_slice_fill_copy(data_len, 0u8);
        data_ptr.copy_from_slice(data.as_slice());

        AccountInfo::new(
            self.random_pubkey(),
            false,
            true,
            lamports,
            data_ptr,
            &pyth_solana_receiver_sdk::ID,
            false,
            Epoch::default(),
        )
    }

    pub fn new_rent_sysvar_account(&self, rent: Rent) -> AccountInfo {
        let data = self.bump.alloc_slice_fill_copy(size_of::<Rent>(), 0u8);
        let lamports = rent.minimum_balance(data.len());

        let mut account_info = AccountInfo::new(
            &sysvar::rent::ID,
            false,
            false,
            self.bump.alloc(lamports),
            data,
            &sysvar::ID,
            false,
            Epoch::default(),
        );

        rent.to_account_info(&mut account_info).unwrap();

        account_info
    }

    pub fn new_vault_account<'a>(
        &'a self,
        vault_type: BankVaultType,
        mint_ai: AccountInfo<'a>,
        owner: &'a Pubkey,
        bank: &'a Pubkey,
    ) -> (AccountInfo<'a>, u8) {
        let (vault_address, seed_bump) = get_vault_address(bank, vault_type);

        (
            self.new_token_account_with_pubkey(vault_address, mint_ai, owner, 0, Rent::free()),
            seed_bump,
        )
    }

    pub fn new_vault_authority<'a>(
        &'a self,
        vault_type: BankVaultType,
        bank: &'a Pubkey,
    ) -> (AccountInfo<'a>, u8) {
        let (vault_address, seed_bump) = get_vault_authority(bank, vault_type);

        (
            AccountInfo::new(
                self.bump.alloc(vault_address),
                false,
                false,
                self.bump.alloc(0),
                &mut [],
                &system_program::ID,
                false,
                Epoch::default(),
            ),
            seed_bump,
        )
    }

    pub fn reset(&mut self) {
        self.bump.reset();
    }
}

pub struct AccountInfoCache<'bump> {
    account_data: Vec<Vec<u8>>,
    account_info: Vec<AccountInfo<'bump>>,
}

impl<'info> AccountInfoCache<'info> {
    pub fn new(ais: &[AccountInfo<'info>]) -> Self {
        let account_data = ais.iter().map(|ai| ai.data.borrow().to_owned()).collect();
        Self {
            account_data,
            account_info: ais.to_vec(),
        }
    }

    pub fn revert(&self) {
        for (ai, data) in self.account_info.iter().zip(self.account_data.iter()) {
            ai.data.borrow_mut().copy_from_slice(data);
        }
    }
}

pub fn get_vault_address(bank: &Pubkey, vault_type: BankVaultType) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[vault_type.get_seed(), &bank.to_bytes()], &marginfi::ID)
}

pub fn get_vault_authority(bank: &Pubkey, vault_type: BankVaultType) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[vault_type.get_authority_seed(), &bank.to_bytes()],
        &marginfi::ID,
    )
}

pub fn set_discriminator<T: Discriminator>(ai: AccountInfo) {
    let mut data = ai.try_borrow_mut_data().unwrap();

    if data[..8].ne(&[0u8; 8]) {
        panic!("Account discriminator is already set");
    }

    data[..8].copy_from_slice(&T::DISCRIMINATOR);
}
