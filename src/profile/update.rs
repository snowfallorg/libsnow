use crate::{
    Error, NIX_BACKEND, NixBackend, PackageAttr, PackageUpdate, Result,
    profile::list::{list, name_from_attr},
    utils,
};
use tokio::process::Command;
use tracing::debug;

/// Check for available updates against the latest nixpkgs revision
pub async fn updatable() -> Result<Vec<PackageUpdate>> {
    utils::misc::updatable(list()?).await
}

/// Check for available updates against the user's current nixpkgs revision
pub async fn updatable_user() -> Result<Vec<PackageUpdate>> {
    utils::misc::updatable_user(list()?).await
}

/// Check for available updates by running `nix eval` per package
/// Support non-nixpkgs packages
pub async fn updatable_all() -> Result<Vec<PackageUpdate>> {
    let installed = list()?;
    let mut updatable = vec![];

    for pkg in installed {
        debug!("Checking for updates: {:?}", pkg);
        match &pkg.attr {
            PackageAttr::NixPkgs { attr } => {
                let output = Command::new("nix")
                    .arg("eval")
                    .arg(format!("nixpkgs#{}.version", attr))
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
                    .arg(format!("{}#{}.version", url, attr))
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
    let mut child = update_spawn(pkgs)?;
    let status = child.wait().await?;
    debug!("{}", status);
    if !status.success() {
        return Err(Error::SubprocessFailed {
            reason: "failed to update packages".into(),
        });
    }
    Ok(())
}

pub fn update_spawn(pkgs: &[&str]) -> Result<tokio::process::Child> {
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
            debug!("Package {} is not installed", pkg);
        }
    }

    if pkgs_to_update.is_empty() {
        return Err(Error::NothingToDo {
            reason: "no packages to update".into(),
        });
    }

    let child = Command::new("nix")
        .arg("--extra-experimental-features")
        .arg("nix-command flakes")
        .arg("profile")
        .arg("upgrade")
        .args(pkgs_to_update)
        .arg("--impure")
        .spawn()?;

    Ok(child)
}

pub async fn update_all() -> Result<()> {
    let mut child = update_all_spawn()?;
    let status = child.wait().await?;
    debug!("{}", status);
    if !status.success() {
        return Err(Error::SubprocessFailed {
            reason: "failed to update packages".into(),
        });
    }
    Ok(())
}

pub fn update_all_spawn() -> Result<tokio::process::Child> {
    let upgrade_all_arg = match *NIX_BACKEND {
        NixBackend::Nix => "--all",
        NixBackend::Lix => ".*",
    };

    let child = Command::new("nix")
        .arg("--extra-experimental-features")
        .arg("nix-command flakes")
        .arg("profile")
        .arg("upgrade")
        .arg(upgrade_all_arg)
        .arg("--impure")
        .spawn()?;

    Ok(child)
}
