use crate::{profile::list::{list, name_from_attr}, utils, PackageAttr, PackageUpdate};
use anyhow::{anyhow, Result};
use log::debug;
use tokio::process::Command;


pub async fn updatable() -> Result<Vec<PackageUpdate>> {
    utils::misc::updatable(list()?).await
}

pub async fn updatable_all() -> Result<Vec<PackageUpdate>> {
    let installed = list()?;
    let mut updatable = vec![];

    for pkg in installed {
        debug!("Checking for updates: {:?}", pkg);
        match &pkg.attr {
            PackageAttr::NixPkgs { attr } => {
                let output = Command::new("nix")
                    .arg("eval")
                    .arg(&format!("nixpkgs#{}.version", attr))
                    .arg("--raw")
                    .output()
                    .await;
                if let Ok(output) = output {
                    let output = String::from_utf8(output.stdout)?;
                    let version = output.trim();

                    if version.is_empty() {
                        continue;
                    } else if pkg.version.is_none() || version != pkg.version.as_ref().unwrap() {
                        updatable.push(PackageUpdate {
                            attr: pkg.attr.clone(),
                            new_version: version.to_string(),
                            old_version: pkg.version.unwrap_or_default(),
                        });
                    }
                }
            }
            PackageAttr::External { url, attr } => {
                let output = Command::new("nix")
                    .arg("eval")
                    .arg(&format!("{}#{}.version", url, attr))
                    .arg("--raw")
                    .output()
                    .await;
                if let Ok(output) = output {
                    let output = String::from_utf8(output.stdout)?;
                    let version = output.trim();

                    if version.is_empty() {
                        continue;
                    } else if pkg.version.is_none() || version != pkg.version.as_ref().unwrap() {
                        updatable.push(PackageUpdate {
                            attr: pkg.attr.clone(),
                            new_version: version.to_string(),
                            old_version: pkg.version.unwrap_or_default(),
                        });
                    }
                }
            }
        }
    }
    Ok(updatable)
}

pub async fn update(pkgs: &[&str]) -> Result<()> {
    let list = list()?
        .into_iter()
        .map(|x| x.attr.to_string())
        .collect::<Vec<_>>();
    let mut pkgs_to_update = Vec::new();
    for pkg in pkgs {
        if list.contains(&pkg.to_string()) {
            if let Ok(name) = name_from_attr(pkg) {
                pkgs_to_update.push(name);
            }
        } else {
            println!("Package {} is not installed", pkg);
        }
    }

    if pkgs_to_update.is_empty() {
        return Err(anyhow!("No packages to update"));
    }

    let output = Command::new("nix")
        .arg("profile")
        .arg("upgrade")
        .args(pkgs_to_update)
        .status()
        .await?;
    debug!("{}", output);
    Ok(())
}

pub async fn update_all() -> Result<()> {
    let output = Command::new("nix")
        .arg("profile")
        .arg("upgrade")
        .arg("--all")
        .status()
        .await?;
    debug!("{}", output);
    Ok(())
}
