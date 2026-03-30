pub mod install;
pub mod list;
pub mod remove;
pub mod update;

use crate::{Error, Result};
use std::process::Command;

pub fn get_channel() -> Result<String> {
    let output = Command::new("nix-channel").arg("--list").output()?;
    let output = String::from_utf8(output.stdout)?;
    let channel = output
        .split('\n')
        .collect::<Vec<_>>()
        .iter()
        .find(|x| x.starts_with("nixos") || x.starts_with("nixpkgs"))
        .ok_or_else(|| Error::Config {
            reason: "failed to get channel".into(),
        })?
        .split_whitespace()
        .collect::<Vec<_>>()[0]
        .to_string();
    Ok(channel)
}
