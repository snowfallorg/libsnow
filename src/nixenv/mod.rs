pub mod list;
pub mod install;
pub mod update;
pub mod remove;

use anyhow::{Context, Result};
use std::process::Command;

pub fn get_channel() -> Result<String> {
    let output = Command::new("nix-channel")
        .arg("--list")
        .output()?;
    let output = String::from_utf8(output.stdout)?;
    let channel = output
        .split("\n")
        .collect::<Vec<_>>()
        .iter()
        .find(|x| x.starts_with("nixos") || x.starts_with("nixpkgs"))
        .context("Failed to get channel")?
        .split_whitespace()
        .collect::<Vec<_>>()[0]
        .to_string();
    Ok(channel)
}
