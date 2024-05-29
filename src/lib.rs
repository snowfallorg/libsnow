use std::{process::Command, fs};
use anyhow::Result;

pub mod homemanager;
pub mod nixenv;
pub mod nixos;
pub mod profile;
pub mod utils;
pub mod config;
pub mod metadata;

#[derive(Debug, Clone)]
pub struct Package {
    pub attr: PackageAttr,
    pub pname: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Clone)]
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

pub fn get_nix_arch() -> Result<String> {
    let output = Command::new("nix")
        .arg("--experimental-features")
        .arg("nix-command")
        .arg("show-config")
        .output()?;
    let stdout = String::from_utf8(output.stdout)?;
    let arch = stdout.split("\n").collect::<Vec<_>>().iter().find(|x| x.contains("system =")).unwrap().split("=").collect::<Vec<_>>()[1].trim().to_string();
    Ok(arch.to_string())
}

pub fn get_nixos_arch() -> Result<String> {
    let output = fs::read_to_string("/run/current-system/system")?;
    return Ok(output);
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
    Ok(stdout.to_string())
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

lazy_static::lazy_static! {
    pub static ref NIXARCH: String = get_arch();
    static ref CACHEDIR: String = format!("{}/.cache/libsnow", std::env::var("HOME").unwrap());
    static ref CONFIGDIR: String = format!("{}/.config/libsnow", std::env::var("HOME").unwrap());
    static ref CONFIG: String = format!("{}/config.json", &*CONFIGDIR);
    static ref HOME: String = std::env::var("HOME").unwrap();
    static ref IS_NIXOS: bool = std::path::Path::new("/etc/NIXOS").exists(); 
}
static SYSCONFIG: &str = "/etc/libsnow/config.json";

