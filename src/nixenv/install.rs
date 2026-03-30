use super::{get_channel, list::list};
use crate::{Error, PackageAttr, Result, metadata::Metadata};
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
        return Err(Error::NothingToDo {
            reason: "no new packages to install".into(),
        });
    }

    let channel = get_channel()?;
    let status = Command::new("nix-env")
        .arg("-iA")
        .args(pkgs_to_install.iter().map(|x| format!("{}.{}", channel, x)))
        .status()
        .await?;

    if !status.success() {
        Err(Error::SubprocessFailed {
            reason: "failed to install packages".into(),
        })
    } else {
        Ok(())
    }
}
