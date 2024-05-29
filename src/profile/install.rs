use anyhow::{anyhow, Result};
// use serde::Deserialize;
use tokio::process::Command;

use crate::{profile::list::list, PackageAttr};

// #[derive(Debug, Deserialize)]
// struct FlakeMetadata {
//     revision: String,
// }

pub async fn install(package: &str) -> Result<()> {
    let installed = list()?;
    if installed.iter().any(|x| match x.attr {
        PackageAttr::NixPkgs { ref attr } => attr == package,
        PackageAttr::External { ref attr, ref url } => {
            (format!("{}#{}", url, attr) == package)
                || (attr.ends_with(".default") && url == package)
        }
    }) {
        println!("Package {} is already installed", package);
        return Ok(());
    }

    let status = Command::new("nix")
        .arg("--extra-experimental-features")
        .arg("nix-command flakes")
        .arg("profile")
        .arg("install")
        .arg(&format!("nixpkgs#{}", package))
        .arg("--impure")
        .status()
        .await?;

    if status.success() {
        Err(anyhow!("Failed to install {}", package))
    } else {
        Ok(())
    }
}

pub async fn install_external(url: &str) -> Result<()> {
    let installed = list()?;
    if installed.iter().any(|x| match x.attr {
        PackageAttr::External {
            ref attr,
            url: ref u,
        } => (format!("{}#{}", u, attr) == url) || (attr.ends_with(".default") && u == url),
        _ => false,
    }) {
        println!("Package {} is already installed", url);
        return Ok(());
    }

    let status = Command::new("nix")
        .arg("--extra-experimental-features")
        .arg("nix-command flakes")
        .arg("profile")
        .arg("install")
        .arg(url)
        .arg("--impure")
        .status()
        .await?;

    if status.success() {
        Err(anyhow!("Failed to install {}", url))
    } else {
        Ok(())
    }
}

// pub async fn available_version(pkg: &str) -> Result<String> {
//     // Instead get version from revision
//     // nix flake metadata nixpkgs
//     let output = Command::new("nix")
//         .arg("--extra-experimental-features")
//         .arg("nix-command flakes")
//         .arg("eval")
//         .arg("--raw")
//         .arg(format!("nixpkgs#{}.version", pkg))
//         .output()
//         .await?;
//     let version = String::from_utf8(output.stdout)?;
//     Ok(version.trim().to_string())
// }

// pub async fn available_version2(pkg: &str) -> Result<String> {
//     // Instead get version from revision
//     // nix flake metadata nixpkgs
//     let output = Command::new("nix")
//         .arg("--extra-experimental-features")
//         .arg("flake")
//         .arg("metadata")
//         .arg("--json")
//         .arg("nixpkgs")
//         .output()
//         .await?;
//     let meta: FlakeMetadata = serde_json::from_slice(&output.stdout)?;
//     let revision = meta.revision;

//     // Now get db for revision

//     unimplemented!()
// }
