use {
    crate::config::{CliSigner, Config, GlobalOptions},
    anchor_client::{Client, Cluster},
    anyhow::{anyhow, bail, Context, Result},
    dirs::home_dir,
    serde::{Deserialize, Serialize},
    solana_sdk::{
        commitment_config::{CommitmentConfig, CommitmentLevel},
        pubkey,
        pubkey::Pubkey,
        signature::{read_keypair_file, Keypair},
    },
    std::{
        fs,
        panic::{catch_unwind, AssertUnwindSafe},
        path::PathBuf,
    },
};

#[derive(Serialize, Deserialize, Clone)]
pub struct Profile {
    pub name: String,
    pub cluster: Cluster,
    pub keypair_path: String,
    pub multisig: Option<Pubkey>,
    pub rpc_url: String,
    pub program_id: Option<Pubkey>,
    pub commitment: Option<CommitmentLevel>,
    pub marginfi_group: Option<Pubkey>,
    pub marginfi_account: Option<Pubkey>,
}

#[derive(Serialize, Deserialize)]
pub struct CliConfig {
    pub profile_name: String,
}

impl Profile {
    pub fn resolved_program_id(&self) -> Result<Pubkey> {
        match self.program_id {
            Some(pid) => Ok(pid),
            None => match self.cluster {
                Cluster::Localnet => Ok(pubkey!("2jGhuVUuy3umdzByFx8sNWUAaf5vaeuDm78RDPEnhrMr")),
                Cluster::Devnet => Ok(pubkey!("neetcne3Ctrrud7vLdt2ypMm21gZHGN2mCmqWaMVcBQ")),
                Cluster::Mainnet => Ok(pubkey!("MFv2hWf31Z9kbCa1snEPYctwafyhdvnV7FZnsebVacA")),
                _ => bail!(
                    "cluster {:?} does not have a default target program ID, please provide it through the --pid option",
                    self.cluster
                ),
            },
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        name: String,
        cluster: Cluster,
        keypair_path: String,
        multisig: Option<Pubkey>,
        rpc_url: String,
        program_id: Option<Pubkey>,
        commitment: Option<CommitmentLevel>,
        marginfi_group: Option<Pubkey>,
        marginfi_account: Option<Pubkey>,
    ) -> Self {
        Profile {
            name,
            cluster,
            keypair_path,
            multisig,
            rpc_url,
            program_id,
            commitment,
            marginfi_group,
            marginfi_account,
        }
    }

    pub fn get_config(&self, global_options: Option<&GlobalOptions>) -> Result<Config> {
        let fee_payer =
            read_keypair_file(&*shellexpand::tilde(&self.keypair_path)).map_err(|err| {
                anyhow!(
                    "unable to read keypair file at {}: {}",
                    self.keypair_path,
                    err
                )
            })?;

        let multisig = self.multisig;

        let send_tx = match global_options {
            Some(options) => !options.no_send_tx,
            None => true,
        };
        let cluster = self.cluster.clone();
        let program_id = self.resolved_program_id()?;
        let commitment = CommitmentConfig {
            commitment: self.commitment.unwrap_or(CommitmentLevel::Processed),
        };
        let client = Client::new_with_options(
            Cluster::Custom(self.rpc_url.clone(), "https://dontcare.com:123".to_string()),
            CliSigner::Keypair(Keypair::new()),
            commitment,
        );
        let prev_panic_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let program_result = catch_unwind(AssertUnwindSafe(|| client.program(program_id)));
        std::panic::set_hook(prev_panic_hook);
        let program = program_result
            .map_err(|_| {
                anyhow!(
                    "unable to initialize RPC client (proxy/system configuration lookup panicked)"
                )
            })?
            .with_context(|| {
                format!("unable to build marginfi program client for {}", program_id)
            })?;

        let (json_output, compute_unit_price, compute_unit_limit, lookup_tables) =
            match global_options {
                Some(opts) => (
                    opts.json_output,
                    opts.compute_unit_price,
                    opts.compute_unit_limit,
                    opts.lookup_tables.clone(),
                ),
                None => (false, None, None, vec![]),
            };

        Ok(Config {
            cluster,
            fee_payer,
            multisig,
            program_id,
            commitment,
            send_tx,
            json_output,
            compute_unit_price,
            compute_unit_limit,
            lookup_tables,
            client,
            mfi_program: program,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn config(
        &mut self,
        new_name: Option<String>,
        cluster: Option<Cluster>,
        keypair_path: Option<String>,
        multisig: Option<Pubkey>,
        rpc_url: Option<String>,
        program_id: Option<Pubkey>,
        commitment: Option<CommitmentLevel>,
        group: Option<Pubkey>,
        account: Option<Pubkey>,
    ) -> Result<()> {
        if let Some(name) = new_name {
            self.name = name;
        }

        if let Some(cluster) = cluster {
            self.cluster = cluster;
        }

        if let Some(keypair_path) = keypair_path {
            self.keypair_path = keypair_path;
        }

        if let Some(multisig) = multisig {
            self.multisig = Some(multisig);
        }

        if let Some(rpc_url) = rpc_url {
            self.rpc_url = rpc_url;
        }

        if let Some(program_id) = program_id {
            self.program_id = Some(program_id);
        }

        if let Some(commitment) = commitment {
            self.commitment = Some(commitment);
        }

        if let Some(group) = group {
            self.marginfi_group = Some(group);
        }

        if let Some(account) = account {
            self.marginfi_account = Some(account);
        }

        self.write_to_file()?;

        Ok(())
    }

    pub fn get_marginfi_account(&self) -> Result<Pubkey> {
        self.marginfi_account.ok_or_else(|| {
            anyhow!(
                "No default marginfi account set for profile \"{}\". Use `mfi account list`, `mfi account use <ACCOUNT>`, or `mfi account create`.",
                self.name
            )
        })
    }

    pub fn set_marginfi_group(&mut self, address: Pubkey) -> Result<()> {
        self.marginfi_group = Some(address);
        self.write_to_file()?;

        Ok(())
    }

    pub fn set_marginfi_account(&mut self, address: Option<Pubkey>) -> Result<()> {
        self.marginfi_account = address;
        self.write_to_file()?;

        Ok(())
    }

    fn write_to_file(&self) -> Result<()> {
        let cli_config_dir = get_cli_config_dir();
        let cli_profiles_dir = cli_config_dir.join("profiles");
        fs::create_dir_all(&cli_profiles_dir)?;
        let profile_file = cli_profiles_dir.join(self.name.clone() + ".json");

        fs::write(profile_file, serde_json::to_string(&self)?)?;

        Ok(())
    }
}

pub fn load_profile() -> Result<Profile> {
    let cli_config_dir = get_cli_config_dir();
    let cli_config_file = cli_config_dir.join("config.json");

    if !cli_config_file.exists() {
        return Err(anyhow!("Profiles not configured, run `mfi profile create`"));
    }

    let cli_config = fs::read_to_string(&cli_config_file)?;
    let cli_config: CliConfig = serde_json::from_str(&cli_config)?;

    let profile_file = cli_config_dir
        .join("profiles")
        .join(format!("{}.json", cli_config.profile_name));

    if !profile_file.exists() {
        return Err(anyhow!(
            "Profile {} does not exist",
            cli_config.profile_name
        ));
    }

    let profile = fs::read_to_string(&profile_file)?;
    let profile: Profile = serde_json::from_str(&profile)?;

    Ok(profile)
}

pub fn load_profile_by_name(name: &str) -> Result<Profile> {
    let cli_config_dir = get_cli_config_dir();
    let profile_file = cli_config_dir.join("profiles").join(format!("{name}.json"));

    if !profile_file.exists() {
        return Err(anyhow!("Profile {} does not exist", name));
    }

    let profile = fs::read_to_string(&profile_file)?;
    let profile: Profile = serde_json::from_str(&profile)?;

    Ok(profile)
}

pub fn delete_profile_by_name(name: &str) -> Result<()> {
    let cli_config_dir = get_cli_config_dir();
    let profile_file = cli_config_dir.join("profiles").join(format!("{name}.json"));

    if !profile_file.exists() {
        return Err(anyhow!("Profile {} does not exist", name));
    }

    match fs::remove_file(profile_file) {
        Ok(()) => {
            println!("successfully deleted profile {name}");
            Ok(())
        }
        Err(e) => {
            println!("failed to delete profile {name}: {e:?}");
            Err(e.into())
        }
    }
}

pub fn get_cli_config_dir() -> PathBuf {
    home_dir()
        .expect("$HOME not set")
        .as_path()
        .join(".config/mfi-cli")
}

impl std::fmt::Debug for Profile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (program, fee_payer, authority) = match self.get_config(None) {
            Ok(config) => (
                config.program_id.to_string(),
                config.explicit_fee_payer().to_string(),
                config.authority().to_string(),
            ),
            Err(err) => (
                self.program_id
                    .map(|pk| pk.to_string())
                    .unwrap_or_else(|| "Unknown".to_owned()),
                "Unknown".to_owned(),
                format!("Unknown ({err})"),
            ),
        };
        write!(
            f,
            r#"
Profile:
    Name: {}
    Program: {}
    Marginfi Group: {}
    Marginfi Account: {}
    Cluster: {}
    Rpc URL: {}
    Fee Payer: {}
    Authority: {}
    Keypair: {}
    Multisig: {}
        "#,
            self.name,
            program,
            self.marginfi_group
                .map(|x| x.to_string())
                .unwrap_or_else(|| "None".to_owned()),
            self.marginfi_account
                .map(|x| x.to_string())
                .unwrap_or_else(|| "None".to_owned()),
            self.cluster,
            self.rpc_url,
            fee_payer,
            authority,
            self.keypair_path.clone(),
            self.multisig
                .map(|x| x.to_string())
                .unwrap_or_else(|| "None".to_owned()),
        )?;

        Ok(())
    }
}
