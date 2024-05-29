use crate::{metadata::revision::get_latest_nixpkgs_revision, Package, PackageAttr, PackageUpdate};
use anyhow::{Context, Result};
use log::debug;
// use serde::Deserialize;
// use std::process::Command;

// #[derive(Debug, Deserialize, Clone)]
// pub struct Derivation {
//     pub env: DerivationEnv,
//     pub name: String,
//     pub system: String,
// }

// #[derive(Debug, Deserialize, Clone)]
// pub struct DerivationEnv {
//     pub name: String,
//     pub pname: Option<String>,
//     pub out: String,
//     pub system: String,
//     pub version: Option<String>,
// }

// pub fn get_drv(path: &str) -> Result<Derivation> {
//     let derivation: HashMap<String, Derivation> = serde_json::from_slice(
//         &Command::new("nix")
//             .arg("--experimental-features")
//             .arg("nix-command")
//             .arg("derivation")
//             .arg("show")
//             .arg(path)
//             .output()?
//             .stdout,
//     )?;
//     let output = *derivation.values().collect::<Vec<_>>().get(0).context("No derivation found")?;
//     return Ok(output.clone());
// }

pub fn get_name_from_storepath(path: &str) -> Result<String> {
    let name = path.split("/").last().context("No name found")?;
    let name = name.split("-").skip(1).collect::<Vec<_>>().join("-");
    Ok(name)
}

pub fn get_pname_version_from_storepath(path: &str) -> Result<(String, String)> {
    let name = get_name_from_storepath(path)?;
    // Split hello-1.2.3 -> hello, 1.2.3
    let parts = name.split("-");
    // Find index where next element starts with a number or "unstable"/"nightly"
    let index = parts
        .clone()
        .enumerate()
        .find(|(_, x)| {
            x.chars().next().unwrap().is_numeric() || x == &"unstable" || x == &"nightly"
        })
        .map(|(i, _)| i)
        .context("No version found")?;
    let pname = parts.clone().take(index).collect::<Vec<_>>().join("-");
    let mut version = parts.skip(index).collect::<Vec<_>>().join("-");
    for s in ["bin", "dev", "out", "debug"] {
        version = version
            .strip_suffix(&format!("-{}", s))
            .unwrap_or(&version)
            .to_string();
    }
    Ok((pname, version))
}

pub fn get_pname_from_storepath(path: &str, version: Option<String>) -> Result<String> {
    let name = get_name_from_storepath(path)?;
    // hello-1.2.3 -> hello: where version="1.2.3"
    let name = if let Some(version) = version {
        name.strip_suffix(format!("-{}", version).as_str())
            .context("No name found")?
    } else {
        &name
    };
    Ok(name.to_string())
}

pub fn get_version_from_storepath(path: &str, pname: &str) -> Result<String> {
    let name = get_name_from_storepath(path)?;
    // hello-1.2.3 -> 1.2.3: where pname="hello"
    let version = name
        .strip_prefix(&format!("{}-", pname))
        .context("No version found")?;
    Ok(version.to_string())
}

// // Usually not cached, so may take a while
// // Online access potentially required
// pub fn nixpkgs_source() -> Result<String> {
//     // If flakes, check flake
//     // nix eval .\#pkgs.x86_64-linux.nixpkgs.path
//     let path = if let Some(flake) = getconfig()?.flake {
//         let output = Command::new("nix")
//             .arg("eval")
//             .arg("--extra-experimental-features")
//             .arg("nix-command flakes")
//             .arg(format!("{}/#pkgs.x86_64-linux.nixpkgs.path", flake))
//             .output()?;
//         let stdout = String::from_utf8(output.stdout)?;
//         stdout.trim().to_string()
//     } else {
//         // nix-instantiate --eval -E 'with import <nixpkgs> {}; pkgs.path'
//         let output = Command::new("nix-instantiate")
//             .arg("--eval")
//             .arg("-E")
//             .arg("with import <nixpkgs> {}; pkgs.path")
//             .output()?;
//         let stdout = String::from_utf8(output.stdout)?;
//         stdout.trim().to_string()
//     };
//     Ok(path)
// }

pub async fn updatable(installed: Vec<Package>) -> Result<Vec<PackageUpdate>> {
    let mut updatable = vec![];

    let newrev = get_latest_nixpkgs_revision().await?;

    let newdb = rusqlite::Connection::open(
        crate::metadata::database::fetch_database(&newrev, false).await?,
    )?;
    let mut stmt = newdb.prepare("SELECT version FROM pkgs WHERE attribute = ?")?;

    for pkg in installed {
        match &pkg.attr {
            PackageAttr::NixPkgs { attr } => {
                let version: String = stmt.query_row(&[attr], |row| Ok(row.get(0)?))?;

                debug!(
                    "{}: {} -> {}",
                    attr,
                    pkg.version.clone().unwrap_or_default(),
                    version
                );

                if version.is_empty() {
                    continue;
                } else if pkg.version.is_none() || version != *pkg.version.as_ref().unwrap() {
                    updatable.push(PackageUpdate {
                        attr: pkg.attr.clone(),
                        new_version: version.to_string(),
                        old_version: pkg.version.unwrap_or_default(),
                    });
                }
            }
            PackageAttr::External { url: _, attr: _ } => {}
        }
    }
    Ok(updatable)
}
