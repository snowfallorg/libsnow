use super::list::list;
use crate::PackageAttr;
use anyhow::{anyhow, Result};
use tokio::process::Command;

pub async fn remove(pkgs: &[&str], db: &rusqlite::Connection) -> Result<()> {
    let installed = list(db).await?;
    let mut pkgs_to_remove = Vec::new();
    for pkg in pkgs {
        if let Some(Some(pname)) = installed.iter().find(|x| match x.attr {
            PackageAttr::NixPkgs { ref attr } => attr == pkg,
            _ => false,
        }).map(|x| x.pname.clone()) {
            pkgs_to_remove.push(pname);
        } else {
            println!("Package {} is not installed", pkg);
        }
    }

    if pkgs_to_remove.is_empty() {
        return Err(anyhow!("No packages to remove"));
    }

    let status = Command::new("nix-env")
        .arg("--uninstall")
        .args(pkgs_to_remove.iter())
        .status()
        .await?;

    if !status.success() {
        Err(anyhow!("Failed to install packages"))
    } else {
        Ok(())
    }
}
