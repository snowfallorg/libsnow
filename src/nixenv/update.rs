use super::get_channel;
use crate::{Error, PackageUpdate, Result, metadata::Metadata, nixenv::list::list, utils};

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
        return Err(Error::NothingToDo {
            reason: "no packages to update".into(),
        });
    }

    let status = tokio::process::Command::new("nix-env")
        .arg("-uA")
        .args(pkgs.iter())
        .status()
        .await?;

    if !status.success() {
        Err(Error::SubprocessFailed {
            reason: "failed to update packages".into(),
        })
    } else {
        Ok(())
    }
}
