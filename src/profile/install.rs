use crate::{profile::list::list, PackageAttr};
use anyhow::{anyhow, Result};
use tokio::process::Command;

pub async fn install(pkgs: &[&str]) -> Result<()> {
    let installed = list()?;
    let mut pkgs_to_install = Vec::new();
    for pkg in pkgs {
        if installed.iter().any(|x| match x.attr {
            PackageAttr::NixPkgs { ref attr } => attr == pkg,
            PackageAttr::External { ref attr, ref url } => {
                (format!("{}#{}", url, attr) == *pkg) || (attr.ends_with(".default") && url == pkg)
            }
        }) {
            println!("Package {} is already installed", pkg);
        } else {
            pkgs_to_install.push(pkg);
        }
    }

    if pkgs_to_install.is_empty() {
        return Err(anyhow!("No new packages to install"));
    }

    let status = Command::new("nix")
        .arg("--extra-experimental-features")
        .arg("nix-command flakes")
        .arg("profile")
        .arg("install")
        .args(pkgs_to_install.iter().map(|x| {
            if x.contains('#') || x.contains(':') {
                x.to_string()
            } else {
                format!("nixpkgs#{}", x)
            }
        }))
        .arg("--impure")
        .status()
        .await?;

    if !status.success() {
        Err(anyhow!("Failed to install packages"))
    } else {
        Ok(())
    }
}
