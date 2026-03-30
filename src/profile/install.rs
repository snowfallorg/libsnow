use crate::{Error, NIX_BACKEND, NixBackend, PackageAttr, Result, profile::list::list};
use tokio::process::Command;

pub async fn install(pkgs: &[&str]) -> Result<()> {
    let mut child = install_spawn(pkgs)?;
    let status = child.wait().await?;
    if !status.success() {
        Err(Error::SubprocessFailed {
            reason: "failed to install packages".into(),
        })
    } else {
        Ok(())
    }
}

pub fn install_spawn(pkgs: &[&str]) -> Result<tokio::process::Child> {
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
        return Err(Error::NothingToDo {
            reason: "no new packages to install".into(),
        });
    }

    let subcmd = match *NIX_BACKEND {
        NixBackend::Nix => "add",
        NixBackend::Lix => "install",
    };

    let child = Command::new("nix")
        .arg("--extra-experimental-features")
        .arg("nix-command flakes")
        .arg("profile")
        .arg(subcmd)
        .args(pkgs_to_install.iter().map(|x| {
            if x.contains('#') || x.contains(':') {
                x.to_string()
            } else {
                format!("nixpkgs#{}", x)
            }
        }))
        .arg("--impure")
        .spawn()?;

    Ok(child)
}
