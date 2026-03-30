use anyhow::Result;
use std::{fmt, fs, process::Command, sync::LazyLock};

pub mod config;
pub mod dbus;
pub mod homemanager;
pub mod metadata;
pub mod nixenv;
pub mod nixos;
pub mod profile;
pub mod toml;
pub mod utils;

#[derive(Debug, Clone, Default)]
pub struct Package {
    pub attr: PackageAttr,
    pub pname: Option<String>,
    pub version: Option<String>,

    pub profile_name: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct PackageUpdate {
    pub attr: PackageAttr,
    pub new_version: String,
    pub old_version: String,
}

#[derive(Debug, Clone)]
pub enum PackageAttr {
    NixPkgs { attr: String },
    External { url: String, attr: String },
}

impl fmt::Display for PackageAttr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PackageAttr::NixPkgs { attr } => write!(f, "{}", attr),
            PackageAttr::External { url, attr } => write!(f, "{}#{}", url, attr),
        }
    }
}

impl Default for PackageAttr {
    fn default() -> Self {
        PackageAttr::NixPkgs {
            attr: String::new(),
        }
    }
}

pub fn get_nix_arch() -> Result<String> {
    let output = Command::new("nix")
        .arg("--experimental-features")
        .arg("nix-command")
        .arg("show-config")
        .output()?;
    let stdout = String::from_utf8(output.stdout)?;
    let arch = stdout
        .split('\n')
        .collect::<Vec<_>>()
        .iter()
        .find(|x| x.contains("system ="))
        .unwrap()
        .split('=')
        .collect::<Vec<_>>()[1]
        .trim()
        .to_string();
    Ok(arch)
}

pub fn get_nixos_arch() -> Result<String> {
    let output = fs::read_to_string("/run/current-system/system")?;
    Ok(output)
}

pub fn get_eval_arch() -> Result<String> {
    let output = Command::new("nix")
        .arg("--experimental-features")
        .arg("nix-command flakes")
        .arg("eval")
        .arg("nixpkgs#system")
        .arg("--raw")
        .output()?;
    let stdout = String::from_utf8(output.stdout)?;
    Ok(stdout)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NixBackend {
    Nix,
    Lix,
}

impl fmt::Display for NixBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NixBackend::Nix => write!(f, "Nix"),
            NixBackend::Lix => write!(f, "Lix"),
        }
    }
}

pub fn detect_nix_backend() -> Result<NixBackend> {
    let output = Command::new("nix").arg("--version").output()?;
    let version_str = String::from_utf8(output.stdout)?;
    if version_str.contains("Lix") {
        Ok(NixBackend::Lix)
    } else {
        Ok(NixBackend::Nix)
    }
}

fn get_arch() -> String {
    if let Ok(arch) = get_nixos_arch() {
        return arch;
    }
    if let Ok(arch) = get_nix_arch() {
        return arch;
    }
    if let Ok(arch) = get_eval_arch() {
        return arch;
    }
    panic!("Could not determine architecture");
}

pub static NIXARCH: LazyLock<String> = LazyLock::new(get_arch);
pub static CACHEDIR: LazyLock<String> = LazyLock::new(|| {
    dirs::cache_dir()
        .expect("Could not determine cache directory")
        .join("libsnow")
        .to_string_lossy()
        .to_string()
});
pub static CONFIGDIR: LazyLock<String> = LazyLock::new(|| {
    dirs::config_dir()
        .expect("Could not determine config directory")
        .join("libsnow")
        .to_string_lossy()
        .to_string()
});
pub static CONFIG: LazyLock<String> = LazyLock::new(|| {
    dirs::config_dir()
        .expect("Could not determine config directory")
        .join("libsnow/config.json")
        .to_string_lossy()
        .to_string()
});
pub static HOME: LazyLock<String> = LazyLock::new(|| {
    dirs::home_dir()
        .expect("Could not determine home directory")
        .to_string_lossy()
        .to_string()
});
pub static IS_NIXOS: LazyLock<bool> = LazyLock::new(|| std::path::Path::new("/etc/NIXOS").exists());
pub static NIX_BACKEND: LazyLock<NixBackend> =
    LazyLock::new(|| detect_nix_backend().unwrap_or(NixBackend::Nix));
static SYSCONFIG: &str = "/etc/libsnow/config.json";
static HELPER_EXEC: &str = "libsnow-helper";
static ICON_UPDATER_EXEC: &str = "update-icons.trigger";
