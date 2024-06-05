use crate::{
    profile::list::{list, name_from_attr},
    PackageAttr,
};
use anyhow::{anyhow, Result};
use log::debug;
use tokio::process::Command;

pub async fn remove(pkgs: &[&str]) -> Result<()> {
    let list = list()?
        .into_iter()
        .map(|x| match x.attr {
            PackageAttr::NixPkgs { attr } => attr,
            PackageAttr::External { url, attr } => format!("{}#{}", url, attr),
        })
        .collect::<Vec<_>>();
    let mut pkgs_to_remove = Vec::new();
    for pkg in pkgs {
        if list.contains(&pkg.to_string()) {
            if let Ok(name) = name_from_attr(pkg) {
                pkgs_to_remove.push(name);
            }
        } else {
            println!("Package {} is not installed", pkg);
        }
    }

    if pkgs_to_remove.is_empty() {
        return Err(anyhow!("No packages to remove"));
    }

    let output = Command::new("nix")
        .arg("profile")
        .arg("remove")
        .args(pkgs_to_remove)
        .status()
        .await?;
    debug!("{}", output);
    Ok(())
}
