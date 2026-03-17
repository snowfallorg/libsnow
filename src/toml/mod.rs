use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::config::configfile::get_config;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SystemConfigFile {
    #[serde(default)]
    pub packages: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub options: BTreeMap<String, toml::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HomeConfigFile {
    #[serde(flatten)]
    pub users: BTreeMap<String, Section>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Section {
    #[serde(default)]
    pub packages: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub options: BTreeMap<String, toml::Value>,
}

pub fn read_system(path: &Path) -> Result<SystemConfigFile> {
    let content =
        fs::read_to_string(path).with_context(|| format!("Failed to read {}", path.display()))?;
    Ok(toml::from_str(&content)?)
}

pub fn read_home(path: &Path) -> Result<HomeConfigFile> {
    let content =
        fs::read_to_string(path).with_context(|| format!("Failed to read {}", path.display()))?;
    Ok(toml::from_str(&content)?)
}

pub fn system_config_file_path() -> Result<String> {
    let config = get_config()?;
    config
        .system_config_file
        .context("No system config file path configured")
}

pub fn home_config_file_path() -> Result<String> {
    let config = get_config()?;
    config
        .home_config_file
        .context("No home config file path configured")
}

pub fn current_user() -> Result<String> {
    std::env::var("USER").context("$USER not set")
}
