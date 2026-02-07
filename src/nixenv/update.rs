use super::get_channel;
use crate::{PackageUpdate, metadata::Metadata, nixenv::list::list, utils};
use anyhow::{Result, anyhow};

pub async fn updatable(md: &Metadata) -> Result<Vec<PackageUpdate>> {
    utils::misc::updatable(list(md).await?).await
}

pub async fn update(pkgs: &[&str], md: &Metadata) -> Result<()> {
    let list = list(md)
        .await?
        .into_iter()
        .map(|x| x.attr.to_string())
        .collect::<Vec<_>>();
    let mut pkgs_to_update = Vec::new();
    let channel = get_channel()?;
    for pkg in pkgs {
        if list.contains(&pkg.to_string()) {
            pkgs_to_update.push(format!("{}.{}", channel, pkg));
        } else {
            println!("Package {} is not installed", pkg);
        }
    }

    if pkgs_to_update.is_empty() {
        return Err(anyhow!("No packages to update"));
    }

    let status = tokio::process::Command::new("nix-env")
        .arg("-uA")
        .args(pkgs.iter())
        .status()
        .await?;

    if !status.success() {
        Err(anyhow!("Failed to update packages"))
    } else {
        Ok(())
    }
}
