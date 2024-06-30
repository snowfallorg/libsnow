use crate::{CONFIG, CONFIGDIR, HOME, SYSCONFIG};
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::{BufReader, Write},
    path::{Path, PathBuf},
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

impl LibSnowConfig {
    pub fn write(&self) -> Result<()> {
        if !Path::new(&*CONFIGDIR).exists() {
            fs::create_dir_all(&*CONFIGDIR)?;
        }
        let mut file = File::create(&*CONFIG)?;
        file.write_all(serde_json::to_string_pretty(&self)?.as_bytes())?;
        Ok(())
    }

    pub fn read_system_config_file(&self) -> Result<String> {
        let path = self
            .systemconfig
            .clone()
            .context("No system config file found")?;
        return Ok(fs::read_to_string(path)?);
    }

    pub fn read_home_config_file(&self) -> Result<String> {
        let path = self
            .homeconfig
            .clone()
            .context("No home config file found")?;
        return Ok(fs::read_to_string(path)?);
    }

    pub fn read_flake_file(&self) -> Result<String> {
        let path = self.flake.clone().context("No flake file found")?;
        return Ok(fs::read_to_string(path)?);
    }

    pub fn get_flake_dir(&self) -> Result<String> {
        let flake_file = PathBuf::from(self.flake.clone().context("No flake file found")?);
        if flake_file.is_dir() {
            Ok(flake_file.to_str().context("No path found")?.to_string())
        } else {
            let flake_dir = flake_file.parent().context("No parent found")?;
            Ok(flake_dir.to_str().context("No path found")?.to_string())
        }
    }

    pub fn get_generation_count(&self) -> Option<u32> {
        if let Some(generations) = self.generations {
            Some(generations)
        } else {
            None
        }
    }
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
pub fn get_config() -> Result<LibSnowConfig> {
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
