use crate::{CONFIG, CONFIGDIR, HOME, SYSCONFIG};
use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::{BufReader, Write},
    path::{Path, PathBuf},
};

#[derive(Serialize, Deserialize, PartialEq, Eq, Clone, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub enum ConfigMode {
    #[default]
    Nix,
    Toml,
}

/// Struct containing locations of system configuration files and some user configuration.
#[derive(Serialize, Deserialize, PartialEq, Eq, Clone, Debug, Default)]
pub struct LibSnowConfig {
    /// Path to the NixOS configuration file. Typically `/etc/nixos/configuration.nix`.
    pub systemconfig: Option<String>,
    /// Path to home-manager configuration file. Typically `~/.config/nixpkgs/home.nix`.
    pub homeconfig: Option<String>,
    /// Path to the NixOS flake file. Typically `/etc/nixos/flake.nix`.
    pub flake: Option<String>,
    /// Specifies which configuration should be used from the `nixosConfigurations` attribute set in the flake file.
    /// If not set, NixOS defaults to the hostname of the system.
    pub host: Option<String>,
    /// Specifies how many NixOS generations to keep. If set to 0, all generations will be kept.
    /// If not set, the default is 5.
    pub generations: Option<u32>,
    /// Whether packages are managed via Nix files or a TOML packages file.
    #[serde(default)]
    pub mode: ConfigMode,
    /// Path to the system TOML config file. Only used when `mode` is `Toml`.
    pub system_config_file: Option<String>,
    /// Path to the home-manager TOML config file. Only used when `mode` is `Toml`.
    pub home_config_file: Option<String>,
    /// Whether home-manager is configured as part of the system config or seperately.
    #[serde(default)]
    pub system_for_home_manager: bool,
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
        Ok(fs::read_to_string(path)?)
    }

    pub fn read_home_config_file(&self) -> Result<String> {
        let path = self
            .homeconfig
            .clone()
            .context("No home config file found")?;
        Ok(fs::read_to_string(path)?)
    }

    pub fn read_flake_file(&self) -> Result<String> {
        let path = self.flake.clone().context("No flake file found")?;
        Ok(fs::read_to_string(path)?)
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
        self.generations
    }

    pub fn nixos_configured(&self) -> bool {
        match self.mode {
            ConfigMode::Nix => self.systemconfig.is_some(),
            ConfigMode::Toml => self.system_config_file.is_some(),
        }
    }

    pub fn home_manager_configured(&self) -> bool {
        match self.mode {
            ConfigMode::Nix => self.homeconfig.is_some(),
            ConfigMode::Toml => self.home_config_file.is_some(),
        }
    }

    pub fn merge(self, other: LibSnowConfig) -> LibSnowConfig {
        LibSnowConfig {
            systemconfig: other.systemconfig.or(self.systemconfig),
            homeconfig: other.homeconfig.or(self.homeconfig),
            flake: other.flake.or(self.flake),
            host: other.host.or(self.host),
            generations: other.generations.or(self.generations),
            mode: other.mode,
            system_config_file: other.system_config_file.or(self.system_config_file),
            home_config_file: other.home_config_file.or(self.home_config_file),
            system_for_home_manager: other.system_for_home_manager || self.system_for_home_manager,
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
/// If both system (`/etc/libsnow/config.json`) and user (`~/.config/libsnow/config.json`)
/// configs exist, they are merged.
pub fn get_config() -> Result<LibSnowConfig> {
    let sys_exists = Path::new(SYSCONFIG).exists();
    let home_exists = Path::new(&*CONFIG).exists();

    match (sys_exists, home_exists) {
        (true, true) => {
            let sys: LibSnowConfig =
                serde_json::from_reader(BufReader::new(File::open(SYSCONFIG)?))?;
            let home: LibSnowConfig =
                serde_json::from_reader(BufReader::new(File::open(&*CONFIG)?))?;
            Ok(sys.merge(home))
        }
        (false, true) => {
            let config: LibSnowConfig =
                serde_json::from_reader(BufReader::new(File::open(&*CONFIG)?))?;
            Ok(config)
        }
        (true, false) => {
            let config: LibSnowConfig =
                serde_json::from_reader(BufReader::new(File::open(SYSCONFIG)?))?;
            Ok(config)
        }
        (false, false) => Err(anyhow!("No config file found")),
    }
}

/// Get the use package type
pub fn get_user_pkg_type() -> UserPkgType {
    if Path::new(&format!("{}/.nix-profile/manifest.json", &*HOME)).exists()
        || !Path::new("/nix/var/nix/profiles/per-user/root/channels/nixos").exists()
        || !Path::new(&format!("{}/.nix-profile/manifest.nix", &*HOME)).exists()
        || if let Ok(m) = fs::read_to_string(format!("{}/.nix-profile/manifest.nix", &*HOME)) {
            m == "[ ]"
        } else {
            false
        }
    {
        UserPkgType::Profile
    } else {
        UserPkgType::Env
    }
}
