use crate::profile::{self, get_cli_config_dir, load_profile, CliConfig, Profile};
use anchor_client::Cluster;
use anyhow::{anyhow, Result};
use solana_sdk::{commitment_config::CommitmentLevel, pubkey::Pubkey};
use std::fs;

fn load_cli_config_file() -> Result<Option<CliConfig>> {
    let cli_config_file = get_cli_config_dir().join("config.json");
    if !cli_config_file.exists() {
        return Ok(None);
    }

    Ok(Some(serde_json::from_str(&fs::read_to_string(
        cli_config_file,
    )?)?))
}

fn write_cli_config_file(config: &CliConfig) -> Result<()> {
    let cli_config_dir = get_cli_config_dir();
    fs::create_dir_all(&cli_config_dir)?;
    fs::write(
        cli_config_dir.join("config.json"),
        serde_json::to_string(config)?,
    )?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn create_profile(
    name: String,
    cluster: Cluster,
    keypair_path: String,
    multisig: Option<Pubkey>,
    rpc_url: String,
    program_id: Option<Pubkey>,
    commitment: Option<CommitmentLevel>,
    marginfi_group: Option<Pubkey>,
    marginfi_account: Option<Pubkey>,
) -> Result<()> {
    let cli_config_dir = get_cli_config_dir();
    let profile = Profile::new(
        name,
        cluster,
        keypair_path,
        multisig,
        rpc_url,
        program_id,
        commitment,
        marginfi_group,
        marginfi_account,
    );
    if !cli_config_dir.exists() {
        fs::create_dir_all(&cli_config_dir)?;
    }

    if load_cli_config_file()?.is_none() {
        write_cli_config_file(&CliConfig {
            profile_name: profile.name.clone(),
        })?;
    }

    let cli_profiles_dir = cli_config_dir.join("profiles");

    if !cli_profiles_dir.exists() {
        fs::create_dir_all(&cli_profiles_dir)?;
    }

    let profile_file = cli_profiles_dir.join(profile.name.clone() + ".json");
    if profile_file.exists() {
        return Err(anyhow!("Profile {} already exists", profile.name));
    }

    println!(
        "Creating profile '{}' (cluster={}, rpc={})",
        profile.name, profile.cluster, profile.rpc_url
    );

    fs::write(&profile_file, serde_json::to_string(&profile)?)?;

    Ok(())
}

fn print_profile(profile: &Profile, is_active: bool) {
    println!("Profile: {}", profile.name);
    println!("Active: {}", if is_active { "yes" } else { "no" });
    println!("Cluster: {}", profile.cluster);
    println!("RPC URL: {}", profile.rpc_url);
    println!("Keypair Path: {}", profile.keypair_path);
    println!(
        "Multisig: {}",
        profile
            .multisig
            .map(|value| value.to_string())
            .unwrap_or_else(|| "-".to_string())
    );
    println!(
        "Program ID: {}",
        profile
            .program_id
            .map(|value| value.to_string())
            .unwrap_or_else(|| "default for cluster".to_string())
    );
    println!(
        "Commitment: {}",
        profile
            .commitment
            .map(|value| value.to_string())
            .unwrap_or_else(|| "processed".to_string())
    );
    println!(
        "Group: {}",
        profile
            .marginfi_group
            .map(|value| value.to_string())
            .unwrap_or_else(|| "-".to_string())
    );
    println!(
        "Account: {}",
        profile
            .marginfi_account
            .map(|value| value.to_string())
            .unwrap_or_else(|| "-".to_string())
    );
}

pub fn show_profile(name: Option<String>) -> Result<()> {
    let active_profile = load_profile()?;
    let requested_name = name.as_deref();
    let profile = match requested_name {
        Some(name) => profile::load_profile_by_name(name)?,
        None => active_profile.clone(),
    };
    let is_active = profile.name == active_profile.name;

    print_profile(&profile, is_active);
    Ok(())
}

pub fn set_profile(name: String) -> Result<()> {
    let cli_config_dir = get_cli_config_dir();
    let profile_file = cli_config_dir.join("profiles").join(format!("{name}.json"));

    if !profile_file.exists() {
        return Err(anyhow!("Profile {} does not exist", name));
    }

    let mut cli_config = load_cli_config_file()?
        .ok_or_else(|| anyhow!("Profiles not configured, run `mfi profile create`"))?;

    cli_config.profile_name = name;
    write_cli_config_file(&cli_config)?;

    Ok(())
}

pub fn list_profiles() -> Result<()> {
    let cli_config_dir = get_cli_config_dir();
    let cli_profiles_dir = cli_config_dir.join("profiles");

    if !cli_profiles_dir.exists() {
        return Err(anyhow!("Profiles not configured, run `mfi profile create`"));
    }

    let mut profiles = fs::read_dir(&cli_profiles_dir)?
        .map(|entry| {
            let entry =
                entry.map_err(|e| anyhow!("failed to read profile directory entry: {}", e))?;
            entry
                .file_name()
                .into_string()
                .map_err(|name| anyhow!("profile filename is not valid UTF-8: {:?}", name))
        })
        .collect::<Result<Vec<String>>>()?;

    if profiles.is_empty() {
        println!("No profiles exist");
    }

    let cli_config = serde_json::from_str::<CliConfig>(&fs::read_to_string(
        cli_config_dir.join("config.json"),
    )?)?;

    println!("Current profile: {}", cli_config.profile_name);

    profiles.sort();

    println!("Found {} profiles", profiles.len());
    for profile in profiles {
        println!("{profile}");
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn configure_profile(
    name: String,
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
    let mut profile = profile::load_profile_by_name(&name)?;
    let old_name = profile.name.clone();
    let renamed_to = new_name.clone();
    profile.config(
        new_name,
        cluster,
        keypair_path,
        multisig,
        rpc_url,
        program_id,
        commitment,
        group,
        account,
    )?;

    if let Some(new_name) = renamed_to {
        if let Some(mut cli_config) = load_cli_config_file()? {
            if cli_config.profile_name == old_name {
                cli_config.profile_name = new_name;
                write_cli_config_file(&cli_config)?;
            }
        }

        if let Err(e) = profile::delete_profile_by_name(&old_name) {
            println!("failed to delete old profile {old_name}: {e:?}");
            return Err(e);
        }
    }

    Ok(())
}

pub fn delete_profile(name: String) -> Result<()> {
    if let Some(cli_config) = load_cli_config_file()? {
        if cli_config.profile_name == name {
            return Err(anyhow!(
                "Cannot delete the active profile {}; set another active profile first",
                name
            ));
        }
    }

    profile::delete_profile_by_name(&name)
}
