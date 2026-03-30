use crate::{
    Error, PackageAttr, Result,
    profile::list::{list, name_from_attr},
};
use tokio::process::Command;
use tracing::debug;

pub async fn remove(pkgs: &[&str]) -> Result<()> {
    let mut child = remove_spawn(pkgs)?;
    let status = child.wait().await?;
    debug!("{}", status);
    if !status.success() {
        Err(Error::SubprocessFailed {
            reason: "failed to remove packages".into(),
        })
    } else {
        Ok(())
    }
}

pub fn remove_spawn(pkgs: &[&str]) -> Result<tokio::process::Child> {
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
        return Err(Error::NothingToDo {
            reason: "no packages to remove".into(),
        });
    }

    let child = Command::new("nix")
        .arg("profile")
        .arg("remove")
        .args(pkgs_to_remove)
        .spawn()?;

    Ok(child)
}
