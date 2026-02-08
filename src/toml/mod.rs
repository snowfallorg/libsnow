use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};

use crate::config::configfile::get_config;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PackagesFile {
    #[serde(default)]
    pub system: Section,
    #[serde(default)]
    pub home: BTreeMap<String, Section>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Section {
    #[serde(default)]
    pub packages: Vec<String>,
}

pub fn read(path: &Path) -> Result<PackagesFile> {
    let content =
        fs::read_to_string(path).with_context(|| format!("Failed to read {}", path.display()))?;
    Ok(toml::from_str(&content)?)
}

pub fn packages_file_path() -> Result<String> {
    let config = get_config()?;
    if let Some(ref path) = config.packages_file {
        return Ok(path.clone());
    }
    if let Some(ref sys) = config.systemconfig
        && let Some(parent) = Path::new(sys).parent()
    {
        return Ok(parent.join("packages.toml").to_string_lossy().to_string());
    }
    if let Some(ref home) = config.homeconfig
        && let Some(parent) = Path::new(home).parent()
    {
        return Ok(parent.join("packages.toml").to_string_lossy().to_string());
    }
    Err(anyhow!("Cannot determine packages.toml location"))
}

pub fn current_user() -> Result<String> {
    std::env::var("USER").context("$USER not set")
}
