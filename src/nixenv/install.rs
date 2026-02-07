use super::{get_channel, list::list};
use crate::{PackageAttr, metadata::Metadata};
use anyhow::{Result, anyhow};
use tokio::process::Command;

pub async fn install(pkgs: &[&str], md: &Metadata) -> Result<()> {
    let installed = list(md).await?;
    let mut pkgs_to_install = Vec::new();
    for pkg in pkgs {
        if installed.iter().any(|x| match x.attr {
            PackageAttr::NixPkgs { ref attr } => attr == pkg,
            _ => false,
        }) {
            println!("Package {} is already installed", pkg);
        } else {
            pkgs_to_install.push(pkg);
        }
    }

    if pkgs_to_install.is_empty() {
        return Err(anyhow!("No new packages to install"));
    }

    let channel = get_channel()?;
    let status = Command::new("nix-env")
        .arg("-iA")
        .args(pkgs_to_install.iter().map(|x| format!("{}.{}", channel, x)))
        .status()
        .await?;

    if !status.success() {
        Err(anyhow!("Failed to install packages"))
    } else {
        Ok(())
    }
}
