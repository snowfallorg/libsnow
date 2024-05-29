use crate::{CONFIG, CONFIGDIR, HOME, SYSCONFIG};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::{BufReader, Write},
    path::Path,
};

/// Struct containing locations of system configuration files and some user configuration.
#[derive(Serialize, Deserialize, PartialEq, Eq, Clone, Debug, Default)]
pub struct LibSnowConfig {
    /// Path to the NixOS configuration file. Typically `/etc/nixos/configuration.nix`.
    pub systemconfig: Option<String>,
    /// Path to home-manager configuration file. Typically `~/.config/nixpkgs/home.nix`.
    pub homeconfig: Option<String>,
    /// Path to the NixOS flake file. Typically `/etc/nixos/flake.nix`.
    pub flake: Option<String>,
    /// Specifies which configuration should be user from the `nixosConfigurations` attribute set in the flake file.
    /// If not set, NixOS defaults to the hostname of the system.
    pub host: Option<String>,
    /// Specifies how many NixOS generations to keep. If set to 0, all generations will be kept.
    /// If not set, the default is 5.
    pub generations: Option<u32>,
}

/// Type of package management used by the user.
/// - [Profile](UserPkgType::Profile) refers to the `nix profile` command.
/// - [Env](UserPkgType::Env) refers to the `nix-env` command.
#[derive(Serialize, Deserialize, PartialEq, Eq, Clone, Debug)]
pub enum UserPkgType {
    Profile,
    Env,
}

/// Reads the config file and returns the config struct.
/// If the config file doesn't exist in both the user (`~/.config/nix-data`) and system (`/etc/nix-data`) config directories,
/// this function will return an error.
pub fn getconfig() -> Result<LibSnowConfig> {
    // Check if user config exists
    if Path::new(&*CONFIG).exists() {
        // Read user config
        let config: LibSnowConfig = serde_json::from_reader(BufReader::new(File::open(&*CONFIG)?))?;
        Ok(config)
    } else if Path::new(SYSCONFIG).exists() {
        // Read system config
        let config: LibSnowConfig =
            serde_json::from_reader(BufReader::new(File::open(SYSCONFIG)?))?;
        Ok(config)
    } else {
        Err(anyhow!("No config file found"))
    }
}

/// Writes the config struct to the config file in the user config directory (`~/.config/nix-data`).
pub fn setuserconfig(config: LibSnowConfig) -> Result<()> {
    // Check if config directory exists
    if !Path::new(&*CONFIGDIR).exists() {
        fs::create_dir_all(&*CONFIGDIR)?;
    }

    // Write user config
    let mut file = File::create(&*CONFIG)?;
    file.write_all(serde_json::to_string_pretty(&config)?.as_bytes())?;
    Ok(())
}

/// Get the use package type
pub fn get_user_pkg_type() -> UserPkgType {
    let userpkgtype = if Path::new(&format!("{}/.nix-profile/manifest.json", &*HOME)).exists()
        || !Path::new("/nix/var/nix/profiles/per-user/root/channels/nixos").exists()
        || !Path::new(&format!("{}/.nix-profile/manifest.nix", &*HOME)).exists()
        || if let Ok(m) = fs::read_to_string(&format!("{}/.nix-profile/manifest.nix", &*HOME)) {
            m == "[ ]"
        } else {
            false
        } {
        UserPkgType::Profile
    } else {
        UserPkgType::Env
    };
    return userpkgtype;
}

/// Get the configuration file
pub fn get_config_file() -> Result<String> {
    let config = getconfig()?;
    let path = if let Some(systemconfig) = config.systemconfig {
        systemconfig
    } else {
        String::from("/etc/nixos/configuration.nix")
    };
    return Ok(fs::read_to_string(path)?);
}

/// Get the home configuration file
pub fn get_home_file() -> Result<String> {
    let config = getconfig()?;
    let path = if let Some(homeconfig) = config.homeconfig {
        homeconfig
    } else {
        String::from(format!("{}/.config/nixpkgs/home.nix", &*HOME))
    };
    return Ok(fs::read_to_string(path)?);
}

/// Get the flake file
pub fn get_flake_file() -> Result<String> {
    let config = getconfig()?;
    let path = if let Some(flake) = config.flake {
        flake
    } else {
        String::from("/etc/nixos/flake.nix")
    };
    return Ok(fs::read_to_string(path)?);
}