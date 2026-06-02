use {
    anchor_client::{Client, Cluster, Program},
    clap::Parser,
    serde::{Deserialize, Serialize},
    solana_sdk::{
        commitment_config::CommitmentConfig,
        pubkey::Pubkey,
        signature::{Keypair, Signer},
    },
    std::ops::Deref,
    std::str::FromStr,
};

#[derive(Default, Debug, Parser)]
pub struct GlobalOptions {
    #[clap(
        global = true,
        long = "profile",
        help = "Use a saved profile for this command only"
    )]
    pub profile_name: Option<String>,

    /// Do not sign and broadcast the transaction.
    /// By default, the CLI simulates, signs, and sends the transaction.
    #[clap(global = true, long = "no-send-tx", action, default_value_t = false)]
    pub no_send_tx: bool,

    #[clap(
        global = true,
        long = "skip-confirmation",
        short = 'y',
        action,
        default_value_t = false
    )]
    pub skip_confirmation: bool,

    #[clap(global = true, long)]
    pub compute_unit_price: Option<u64>,

    #[clap(global = true, long, help = "Compute unit limit for transactions")]
    pub compute_unit_limit: Option<u32>,

    #[clap(
        global = true,
        long = "lookup-table",
        short = 'l',
        help = "Address lookup table(s) for versioned transactions"
    )]
    pub lookup_tables: Vec<Pubkey>,

    #[clap(
        global = true,
        long = "json",
        action,
        default_value_t = false,
        help = "Output in JSON format"
    )]
    pub json_output: bool,
}

#[derive(Copy, Clone, Debug)]
pub enum TxMode {
    /// Default: simulate, sign, and broadcast
    SendTx,
    /// --no-send-tx: simulate and output unsigned base58 for external signing
    MultisigOutput,
}

pub enum CliSigner {
    Keypair(Keypair),
}

pub fn clone_keypair(keypair: &Keypair) -> Keypair {
    Keypair::from_bytes(&keypair.to_bytes()).unwrap()
}

impl Clone for CliSigner {
    fn clone(&self) -> Self {
        match self {
            CliSigner::Keypair(keypair) => CliSigner::Keypair(clone_keypair(keypair)),
        }
    }
}

impl Deref for CliSigner {
    type Target = Keypair;

    fn deref(&self) -> &Self::Target {
        match self {
            CliSigner::Keypair(keypair) => keypair,
        }
    }
}

#[allow(dead_code)]
pub struct Config {
    #[allow(dead_code)]
    pub cluster: Cluster,
    pub fee_payer: Keypair,
    pub multisig: Option<Pubkey>,
    pub program_id: Pubkey,
    #[allow(dead_code)]
    pub commitment: CommitmentConfig,
    pub send_tx: bool,
    pub json_output: bool,
    pub compute_unit_price: Option<u64>,
    pub compute_unit_limit: Option<u32>,
    pub lookup_tables: Vec<Pubkey>,
    #[allow(dead_code)]
    pub client: Client<CliSigner>,
    pub mfi_program: Program<CliSigner>,
}

impl Config {
    /// Payer encoded into instructions and output transactions.
    ///
    /// In `--no-send-tx` multisig mode this switches to the multisig so the emitted
    /// payload can actually be signed and executed externally.
    pub fn explicit_fee_payer(&self) -> Pubkey {
        if !self.send_tx {
            self.multisig.unwrap_or(self.fee_payer.pubkey())
        } else {
            self.fee_payer.pubkey()
        }
    }

    /// Either the fee payer or the multisig authority.
    pub fn authority(&self) -> Pubkey {
        if let Some(multisig) = &self.multisig {
            *multisig
        } else {
            self.fee_payer.pubkey()
        }
    }

    pub fn get_tx_mode(&self) -> TxMode {
        if self.send_tx {
            TxMode::SendTx
        } else {
            TxMode::MultisigOutput
        }
    }

    pub fn get_signers(&self, explicit_fee_payer: bool) -> Vec<&Keypair> {
        if explicit_fee_payer || self.multisig.is_none() {
            vec![&self.fee_payer]
        } else {
            vec![]
        }
    }

    /// Get the authority keypair for signing transactions.
    /// This errors if the authority is a multisig.
    pub fn get_non_ms_authority_keypair(&self) -> anyhow::Result<&Keypair> {
        if self.multisig.is_none() {
            Ok(&self.fee_payer)
        } else {
            Err(anyhow::anyhow!("Cannot get authority keypair for multisig"))
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountEntry {
    // Base58 pubkey string.
    pub address: String,
    // Name of JSON file containing the account data.
    pub filename: String,
}

crate::home_path!(WalletPath, ".config/solana/id.json");
